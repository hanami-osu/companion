use std::time::Duration;

use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CallbackError {
    #[error("OAuth callback timed out")]
    TimedOut,
    #[error("OAuth callback used an invalid path")]
    InvalidPath,
    #[error("OAuth state did not match")]
    StateMismatch,
    #[error("OAuth approval was cancelled")]
    AccessDenied,
    #[error("OAuth callback did not contain an authorization code")]
    MissingCode,
    #[error("OAuth callback was malformed")]
    Malformed,
    #[error("loopback listener failed: {0}")]
    Io(String),
}

pub struct LoopbackListener {
    listener: TcpListener,
    redirect_uri: String,
}

impl LoopbackListener {
    pub async fn bind() -> Result<Self, CallbackError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| CallbackError::Io(error.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|error| CallbackError::Io(error.to_string()))?
            .port();
        Ok(Self {
            listener,
            redirect_uri: format!("http://127.0.0.1:{port}/callback"),
        })
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub async fn wait_for_code(
        self,
        expected_state: &str,
        timeout: Duration,
    ) -> Result<String, CallbackError> {
        let accepted = tokio::time::timeout(timeout, self.listener.accept())
            .await
            .map_err(|_| CallbackError::TimedOut)?
            .map_err(|error| CallbackError::Io(error.to_string()))?;
        let (mut stream, _) = accepted;
        let mut request = Vec::with_capacity(2048);
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| CallbackError::Io(error.to_string()))?;
            if read == 0 || request.len() + read > 8192 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let parsed = parse_callback_request(&String::from_utf8_lossy(&request), expected_state);
        let (status, heading, body) = if parsed.is_ok() {
            (
                "200 OK",
                "Authorization received",
                "Return to Hanami Companion while it completes sign-in.",
            )
        } else {
            (
                "400 Bad Request",
                "Connection was not completed",
                "Return to Hanami Companion for details.",
            )
        };
        let html = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{heading}</title></head><body style=\"font-family:system-ui;background:#0b0b0d;color:#f4f4f5;padding:48px\"><h1>{heading}</h1><p>{body}</p></body></html>"
        );
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
            html.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        parsed
    }
}

pub fn parse_callback_request(
    request: &str,
    expected_state: &str,
) -> Result<String, CallbackError> {
    let request_line = request.lines().next().ok_or(CallbackError::Malformed)?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err(CallbackError::Malformed);
    }
    let target = parts.next().ok_or(CallbackError::Malformed)?;
    let url =
        Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| CallbackError::Malformed)?;
    if url.path() != "/callback" {
        return Err(CallbackError::InvalidPath);
    }

    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    if query.get("state").map(String::as_str) != Some(expected_state) {
        return Err(CallbackError::StateMismatch);
    }
    if query
        .get("error")
        .is_some_and(|error| error == "access_denied")
    {
        return Err(CallbackError::AccessDenied);
    }
    query
        .get("code")
        .filter(|code| !code.is_empty())
        .cloned()
        .ok_or(CallbackError::MissingCode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_callback_and_validates_state() {
        let request =
            "GET /callback?code=secret-code&state=expected HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(
            parse_callback_request(request, "expected"),
            Ok("secret-code".into())
        );
        assert_eq!(
            parse_callback_request(request, "different"),
            Err(CallbackError::StateMismatch)
        );
    }

    #[test]
    fn rejects_wrong_path_and_access_denied() {
        assert_eq!(
            parse_callback_request("GET /wrong?code=x&state=s HTTP/1.1\r\n\r\n", "s"),
            Err(CallbackError::InvalidPath)
        );
        assert_eq!(
            parse_callback_request(
                "GET /callback?error=access_denied&state=s HTTP/1.1\r\n\r\n",
                "s"
            ),
            Err(CallbackError::AccessDenied)
        );
    }
}
