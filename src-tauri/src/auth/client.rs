use std::time::{Duration, Instant};

use reqwest::Client;
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
    #[error("Hanami rejected the OAuth request")]
    Rejected,
    #[error("Hanami returned an invalid token response")]
    InvalidResponse,
    #[error("Hanami did not rotate the refresh token")]
    MissingRefreshToken,
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

    pub fn authorization_url(
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

    pub async fn exchange_code(
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

    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, AuthClientError> {
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
        if !response.status().is_success() {
            return Err(AuthClientError::Rejected);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| AuthClientError::InvalidResponse)?;
        parse_token_response(&bytes)
    }

    pub async fn revoke(&self, token: &str) -> Result<(), AuthClientError> {
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
            Err(AuthClientError::Rejected)
        }
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
}
