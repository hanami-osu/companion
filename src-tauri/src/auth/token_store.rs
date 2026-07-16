use keyring::{Entry, Error as KeyringError};
use thiserror::Error;

const SERVICE: &str = "com.yorunoken.hanami.companion";
const ACCOUNT: &str = "hanami-refresh-token";

#[derive(Debug, Error)]
pub enum TokenStoreError {
    #[error("secure credential storage is unavailable: {0}")]
    Unavailable(String),
}

pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, TokenStoreError>;
    fn save(&self, refresh_token: &str) -> Result<(), TokenStoreError>;
    fn delete(&self) -> Result<(), TokenStoreError>;
}

#[derive(Default)]
pub struct KeyringTokenStore;

impl KeyringTokenStore {
    fn entry() -> Result<Entry, TokenStoreError> {
        Entry::new(SERVICE, ACCOUNT)
            .map_err(|error| TokenStoreError::Unavailable(error.to_string()))
    }
}

impl TokenStore for KeyringTokenStore {
    fn load(&self) -> Result<Option<String>, TokenStoreError> {
        match Self::entry()?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(TokenStoreError::Unavailable(error.to_string())),
        }
    }

    fn save(&self, refresh_token: &str) -> Result<(), TokenStoreError> {
        Self::entry()?
            .set_password(refresh_token)
            .map_err(|error| TokenStoreError::Unavailable(error.to_string()))
    }

    fn delete(&self) -> Result<(), TokenStoreError> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(TokenStoreError::Unavailable(error.to_string())),
        }
    }
}
