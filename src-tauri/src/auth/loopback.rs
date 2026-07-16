use std::time::Duration;

use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::Instant,
};
use url::Url;

const MAX_REQUEST_BYTES: usize = 8 * 1024;
const CONNECTION_READ_TIMEOUT: Duration = Duration::from_secs(3);

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
    #[error("OAuth callback request was too large")]
    RequestTooLarge,
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
        self.wait_for_code_with_limits(
            expected_state,
            timeout,
            CONNECTION_READ_TIMEOUT,
            MAX_REQUEST_BYTES,
        )
        .await
    }

    async fn wait_for_code_with_limits(
        self,
        expected_state: &str,
        overall_timeout: Duration,
        read_timeout: Duration,
        max_request_bytes: usize,
    ) -> Result<String, CallbackError> {
        let deadline = Instant::now() + overall_timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(CallbackError::TimedOut)?;
            let (mut stream, _) = tokio::time::timeout(remaining, self.listener.accept())
                .await
                .map_err(|_| CallbackError::TimedOut)?
                .map_err(|error| CallbackError::Io(error.to_string()))?;

            let request =
                read_request_headers(&mut stream, deadline, read_timeout, max_request_bytes).await;
            let parsed =
                request.and_then(|request| parse_callback_request(&request, expected_state));
            match parsed {
                Ok(code) => {
                    write_browser_response(&mut stream, true).await;
                    return Ok(code);
                }
                Err(CallbackError::AccessDenied) => {
                    write_browser_response(&mut stream, false).await;
                    return Err(CallbackError::AccessDenied);
                }
                Err(CallbackError::TimedOut) if Instant::now() >= deadline => {
                    return Err(CallbackError::TimedOut);
                }
                Err(_) => {
                    write_browser_response(&mut stream, false).await;
                }
            }
        }
    }
}

async fn read_request_headers(
    stream: &mut TcpStream,
    deadline: Instant,
    read_timeout: Duration,
    max_request_bytes: usize,
) -> Result<String, CallbackError> {
    let mut request = Vec::with_capacity(2048);
    let mut chunk = [0_u8; 1024];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(CallbackError::TimedOut)?;
        let read = tokio::time::timeout(read_timeout.min(remaining), stream.read(&mut chunk))
            .await
            .map_err(|_| CallbackError::TimedOut)?
            .map_err(|error| CallbackError::Io(error.to_string()))?;
        if read == 0 {
            return Err(CallbackError::Malformed);
        }
        if request.len() + read > max_request_bytes {
            return Err(CallbackError::RequestTooLarge);
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return String::from_utf8(request).map_err(|_| CallbackError::Malformed);
        }
    }
}

async fn write_browser_response(stream: &mut TcpStream, success: bool) {
    let (status, html) = if success {
        (
            "200 OK",
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>Authorization received</title></head><body style=\"font-family:system-ui;background:#0b0b0d;color:#f4f4f5;padding:48px\"><h1>Authorization received</h1><p>Return to Hanami Companion while it completes sign-in.</p></body></html>",
        )
    } else {
        (
            "400 Bad Request",
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>Connection was not completed</title></head><body style=\"font-family:system-ui;background:#0b0b0d;color:#f4f4f5;padding:48px\"><h1>Connection was not completed</h1><p>Return to Hanami Companion for details.</p></body></html>",
        )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        stream.write_all(response.as_bytes()),
    )
    .await;
    let _ = stream.shutdown().await;
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
    if !query
        .get("state")
        .is_some_and(|received| constant_time_eq(received.as_bytes(), expected_state.as_bytes()))
    {
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

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let compared = left.len().max(right.len());
    for index in 0..compared {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn request(port: u16, bytes: &[u8]) {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        stream.write_all(bytes).await.expect("write request");
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response).await;
    }

    async fn listener_port(listener: &LoopbackListener) -> u16 {
        Url::parse(listener.redirect_uri())
            .expect("redirect URL")
            .port()
            .expect("port")
    }

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

    #[tokio::test]
    async fn ignores_favicon_and_malformed_requests_before_a_valid_callback() {
        let listener = LoopbackListener::bind().await.expect("listener");
        let port = listener_port(&listener).await;
        let waiting = tokio::spawn(listener.wait_for_code_with_limits(
            "expected",
            Duration::from_secs(1),
            Duration::from_millis(50),
            MAX_REQUEST_BYTES,
        ));
        request(
            port,
            b"GET /favicon.ico HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await;
        request(port, b"not http\r\n\r\n").await;
        request(
            port,
            b"GET /callback?code=accepted&state=expected HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await;
        assert_eq!(waiting.await.expect("task"), Ok("accepted".into()));
    }

    #[tokio::test]
    async fn stalled_and_oversized_connections_do_not_end_the_listener() {
        let listener = LoopbackListener::bind().await.expect("listener");
        let port = listener_port(&listener).await;
        let waiting = tokio::spawn(listener.wait_for_code_with_limits(
            "expected",
            Duration::from_secs(1),
            Duration::from_millis(30),
            128,
        ));
        let stalled = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("stalled connect");
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(stalled);
        request(port, &[b'x'; 256]).await;
        request(
            port,
            b"GET /callback?code=accepted&state=expected HTTP/1.1\r\n\r\n",
        )
        .await;
        assert_eq!(waiting.await.expect("task"), Ok("accepted".into()));
    }

    #[tokio::test]
    async fn invalid_path_and_state_do_not_end_the_listener() {
        let listener = LoopbackListener::bind().await.expect("listener");
        let port = listener_port(&listener).await;
        let waiting = tokio::spawn(listener.wait_for_code("expected", Duration::from_secs(1)));
        request(port, b"GET /wrong?code=x&state=expected HTTP/1.1\r\n\r\n").await;
        request(port, b"GET /callback?code=x&state=wrong HTTP/1.1\r\n\r\n").await;
        request(
            port,
            b"GET /callback?code=accepted&state=expected HTTP/1.1\r\n\r\n",
        )
        .await;
        assert_eq!(waiting.await.expect("task"), Ok("accepted".into()));
    }

    #[tokio::test]
    async fn access_denial_and_overall_timeout_finish_the_flow() {
        let listener = LoopbackListener::bind().await.expect("listener");
        let port = listener_port(&listener).await;
        let denied = tokio::spawn(listener.wait_for_code("expected", Duration::from_secs(1)));
        request(
            port,
            b"GET /callback?error=access_denied&state=expected HTTP/1.1\r\n\r\n",
        )
        .await;
        assert_eq!(
            denied.await.expect("task"),
            Err(CallbackError::AccessDenied)
        );

        let listener = LoopbackListener::bind().await.expect("listener");
        assert_eq!(
            listener
                .wait_for_code("expected", Duration::from_millis(20))
                .await,
            Err(CallbackError::TimedOut)
        );
    }
}
