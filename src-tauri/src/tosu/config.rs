use std::{net::SocketAddr, str::FromStr};

use serde::{Deserialize, Serialize};
use url::Url;

pub const DEFAULT_TOSU_PORT: u16 = 24_050;
const LOOPBACK_HOST: &str = "127.0.0.1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TosuEndpoint {
    port: u16,
}

impl Default for TosuEndpoint {
    fn default() -> Self {
        Self {
            port: DEFAULT_TOSU_PORT,
        }
    }
}

impl TosuEndpoint {
    pub fn new(port: u16) -> Self {
        Self {
            port: if port == 0 { DEFAULT_TOSU_PORT } else { port },
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from_str(&format!("{LOOPBACK_HOST}:{}", self.port))
            .expect("the fixed loopback tosu endpoint is valid")
    }

    pub fn websocket_url(&self) -> Url {
        Url::parse(&format!("ws://{LOOPBACK_HOST}:{}/websocket/v2", self.port))
            .expect("the fixed loopback tosu WebSocket URL is valid")
    }

    pub fn dashboard_url(&self) -> Url {
        Url::parse(&format!("http://{LOOPBACK_HOST}:{}", self.port))
            .expect("the fixed loopback tosu dashboard URL is valid")
    }

    pub fn background_url(&self, cache_key: &str) -> Url {
        let mut url = self
            .dashboard_url()
            .join("/files/beatmap/background")
            .expect("the fixed tosu background path is valid");
        if !cache_key.is_empty() {
            url.query_pairs_mut().append_pair("v", cache_key);
        }
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_all_endpoint_urls_from_one_port() {
        let endpoint = TosuEndpoint::new(24_051);

        assert_eq!(endpoint.socket_addr().to_string(), "127.0.0.1:24051");
        assert_eq!(
            endpoint.websocket_url().as_str(),
            "ws://127.0.0.1:24051/websocket/v2"
        );
        assert_eq!(endpoint.dashboard_url().as_str(), "http://127.0.0.1:24051/");
        assert_eq!(
            endpoint.background_url("abc 123").as_str(),
            "http://127.0.0.1:24051/files/beatmap/background?v=abc+123"
        );
    }

    #[test]
    fn rejects_zero_as_a_persisted_port() {
        assert_eq!(TosuEndpoint::new(0).port(), DEFAULT_TOSU_PORT);
    }
}
