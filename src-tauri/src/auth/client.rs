use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::{Client, StatusCode, header::HeaderValue};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

const CLIENT_ID: &str = "hanami-companion";
const PRODUCTION_BASE_URL: &str = "https://hanami.yorunoken.com";
const DEVELOPMENT_BASE_URL: &str = "http://localhost:3000";

#[derive(Clone, Debug)]
pub struct AuthConfig {
    base_url: String,
}

impl AuthConfig {
    pub fn from_environment() -> Self {
        let default_url = if cfg!(debug_assertions) {
            DEVELOPMENT_BASE_URL
        } else {
            PRODUCTION_BASE_URL
        };
        let base_url = std::env::var("HANAMI_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_url.into())
            .trim_end_matches('/')
            .to_owned();
        Self { base_url }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn is_production(&self) -> bool {
        self.base_url == PRODUCTION_BASE_URL
    }
}

#[derive(Debug, Error)]
pub enum AuthClientError {
    #[error("Hanami URL is invalid")]
    InvalidUrl,
    #[error("could not reach Hanami")]
    Network,
    #[error("the Hanami authorization grant is invalid or revoked")]
    InvalidGrant,
    #[error("Hanami rate limited the request")]
    RateLimited { retry_after: Option<Duration> },
    #[error("Hanami is temporarily unavailable (HTTP {status})")]
    ServerUnavailable { status: u16 },
    #[error("Hanami returned an unexpected HTTP status ({status})")]
    UnexpectedStatus { status: u16 },
    #[error("Hanami returned an invalid token response")]
    InvalidResponse,
    #[error("Hanami did not rotate the refresh token")]
    MissingRefreshToken,
}

impl AuthClientError {
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            Self::Network
                | Self::RateLimited { .. }
                | Self::ServerUnavailable { .. }
                | Self::UnexpectedStatus { .. }
                | Self::InvalidResponse
                | Self::MissingRefreshToken
        )
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AccessToken {
    pub value: String,
    pub expires_at: Instant,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    #[serde(default)]
    pub token_type: Option<String>,
}

#[async_trait]
pub trait AuthApi: Send + Sync {
    fn authorization_url(
        &self,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<Url, AuthClientError>;

    async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<TokenResponse, AuthClientError>;

    async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, AuthClientError>;
    async fn revoke(&self, token: &str) -> Result<(), AuthClientError>;
}

pub struct HanamiClient {
    http: Client,
    config: AuthConfig,
}

impl HanamiClient {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent(concat!("Hanami-Companion/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("valid HTTP client configuration"),
            config,
        }
    }

    fn build_authorization_url(
        &self,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<Url, AuthClientError> {
        let mut url = Url::parse(&format!("{}/oauth/authorize", self.config.base_url()))
            .map_err(|_| AuthClientError::InvalidUrl)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", CLIENT_ID)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("device_name", device_name)
            .append_pair("platform", platform);
        Ok(url)
    }

    async fn exchange_code_request(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<TokenResponse, AuthClientError> {
        self.token_request(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .await
    }

    async fn refresh_request(&self, refresh_token: &str) -> Result<TokenResponse, AuthClientError> {
        self.token_request(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .await
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<TokenResponse, AuthClientError> {
        let response = self
            .http
            .post(format!("{}/oauth/token", self.config.base_url()))
            .form(form)
            .send()
            .await
            .map_err(|_| AuthClientError::Network)?;
        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .cloned();
            if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                return Err(classify_oauth_failure(status, retry_after.as_ref(), &[]));
            }
            let body = if response
                .content_length()
                .is_some_and(|length| length > 16 * 1024)
            {
                Vec::new()
            } else {
                response
                    .bytes()
                    .await
                    .map_or_else(|_| Vec::new(), |bytes| bytes.to_vec())
            };
            return Err(classify_oauth_failure(status, retry_after.as_ref(), &body));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| AuthClientError::InvalidResponse)?;
        parse_token_response(&bytes)
    }

    async fn revoke_request(&self, token: &str) -> Result<(), AuthClientError> {
        let response = self
            .http
            .post(format!("{}/oauth/revoke", self.config.base_url()))
            .form(&[("token", token), ("client_id", CLIENT_ID)])
            .send()
            .await
            .map_err(|_| AuthClientError::Network)?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let retry_after = response.headers().get(reqwest::header::RETRY_AFTER);
            Err(classify_oauth_failure(status, retry_after, &[]))
        }
    }
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
    #[serde(default)]
    error: Option<String>,
}

fn classify_oauth_failure(
    status: StatusCode,
    retry_after: Option<&HeaderValue>,
    body: &[u8],
) -> AuthClientError {
    if status == StatusCode::TOO_MANY_REQUESTS {
        let retry_after = retry_after
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(|seconds| Duration::from_secs(seconds.min(300)));
        return AuthClientError::RateLimited { retry_after };
    }
    if status.is_server_error() {
        return AuthClientError::ServerUnavailable {
            status: status.as_u16(),
        };
    }
    let invalid_grant = serde_json::from_slice::<OAuthErrorResponse>(body)
        .ok()
        .and_then(|response| response.error)
        .is_some_and(|error| error.eq_ignore_ascii_case("invalid_grant"));
    if invalid_grant {
        AuthClientError::InvalidGrant
    } else {
        AuthClientError::UnexpectedStatus {
            status: status.as_u16(),
        }
    }
}

#[async_trait]
impl AuthApi for HanamiClient {
    fn authorization_url(
        &self,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<Url, AuthClientError> {
        self.build_authorization_url(redirect_uri, state, challenge, device_name, platform)
    }

    async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<TokenResponse, AuthClientError> {
        self.exchange_code_request(code, redirect_uri, verifier)
            .await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, AuthClientError> {
        self.refresh_request(refresh_token).await
    }

    async fn revoke(&self, token: &str) -> Result<(), AuthClientError> {
        self.revoke_request(token).await
    }
}

pub fn parse_token_response(bytes: &[u8]) -> Result<TokenResponse, AuthClientError> {
    let response: TokenResponse =
        serde_json::from_slice(bytes).map_err(|_| AuthClientError::InvalidResponse)?;
    if response.access_token.is_empty()
        || response.expires_in == 0
        || response
            .token_type
            .as_deref()
            .is_some_and(|kind| !kind.eq_ignore_ascii_case("bearer"))
    {
        return Err(AuthClientError::InvalidResponse);
    }
    Ok(response)
}

impl TokenResponse {
    pub fn into_access_token(self) -> AccessToken {
        AccessToken {
            value: self.access_token,
            expires_at: Instant::now() + Duration::from_secs(self.expires_in),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_token_response_without_profile_fields() {
        let response = parse_token_response(
            br#"{"access_token":"access","refresh_token":"refresh","expires_in":3600,"token_type":"Bearer"}"#,
        )
        .expect("valid response");
        assert_eq!(response.refresh_token.as_deref(), Some("refresh"));
    }

    #[test]
    fn rejects_empty_or_malformed_token_responses() {
        assert!(parse_token_response(br#"{"access_token":"","expires_in":3600}"#).is_err());
        assert!(parse_token_response(br#"{"access_token":"access"}"#).is_err());
    }

    #[test]
    fn classifies_only_invalid_grant_as_permanent() {
        assert!(matches!(
            classify_oauth_failure(
                StatusCode::BAD_REQUEST,
                None,
                br#"{"error":"invalid_grant"}"#,
            ),
            AuthClientError::InvalidGrant
        ));
        assert!(matches!(
            classify_oauth_failure(
                StatusCode::BAD_REQUEST,
                None,
                br#"{"error":"invalid_request"}"#,
            ),
            AuthClientError::UnexpectedStatus { status: 400 }
        ));
    }

    #[test]
    fn classifies_rate_limits_and_server_failures_as_temporary() {
        let retry_after = HeaderValue::from_static("12");
        let rate_limited =
            classify_oauth_failure(StatusCode::TOO_MANY_REQUESTS, Some(&retry_after), &[]);
        assert!(rate_limited.is_temporary());
        assert_eq!(rate_limited.retry_after(), Some(Duration::from_secs(12)));

        for status in [
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let error = classify_oauth_failure(status, None, &[]);
            assert!(matches!(error, AuthClientError::ServerUnavailable { .. }));
            assert!(error.is_temporary());
        }
    }

    #[test]
    fn malformed_success_is_not_an_invalid_grant() {
        let error = parse_token_response(br#"{"access_token":"access","expires_in":"soon"}"#)
            .expect_err("malformed response");
        assert!(matches!(error, AuthClientError::InvalidResponse));
        assert!(error.is_temporary());
    }
}
