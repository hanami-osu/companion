mod client;
mod loopback;
mod pkce;
mod token_store;

use std::{sync::atomic::Ordering, time::Duration};

use tauri::{AppHandle, Manager};
use thiserror::Error;

pub use client::AuthConfig;
use client::{AccessToken, AuthApi, AuthClientError, HanamiClient};
use loopback::{CallbackError, LoopbackListener};
use token_store::{KeyringTokenStore, TokenStore, TokenStoreError};

use crate::app_state::{AppState, AuthState, update_snapshot};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("another Hanami sign-in is already in progress")]
    AlreadyInProgress,
    #[error("the system browser could not be opened")]
    BrowserOpen,
    #[error(transparent)]
    Callback(#[from] CallbackError),
    #[error(transparent)]
    Client(#[from] AuthClientError),
    #[error(transparent)]
    CredentialStore(#[from] TokenStoreError),
}

impl AuthError {
    pub fn user_message(&self) -> String {
        match self {
            Self::AlreadyInProgress => self.to_string(),
            Self::BrowserOpen => "Your system browser could not be opened.".into(),
            Self::Callback(CallbackError::TimedOut) => {
                "Hanami approval timed out. Start the connection again.".into()
            }
            Self::Callback(CallbackError::AccessDenied) => {
                "Hanami connection was cancelled in the browser.".into()
            }
            Self::Callback(CallbackError::StateMismatch | CallbackError::InvalidPath) => {
                "The Hanami callback could not be verified.".into()
            }
            Self::Callback(_) => "The Hanami callback was incomplete.".into(),
            Self::Client(AuthClientError::Network) => {
                "Hanami could not be reached. The stored session will be retried.".into()
            }
            Self::Client(AuthClientError::Rejected) => {
                "The stored Hanami session is no longer valid.".into()
            }
            Self::Client(_) => "Hanami could not complete authentication.".into(),
            Self::CredentialStore(_) => {
                "Secure credential storage is unavailable. No token was saved.".into()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefreshOutcome {
    Restored,
    NoSession,
    TemporaryFailure,
    Rejected,
    CredentialStoreUnavailable,
}

struct RestoreBackoff {
    next: Duration,
}

impl Default for RestoreBackoff {
    fn default() -> Self {
        Self {
            next: Duration::from_secs(2),
        }
    }
}

impl RestoreBackoff {
    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(Duration::from_secs(60));
        delay
    }

    fn reset(&mut self) {
        self.next = Duration::from_secs(2);
    }
}

pub struct AuthRuntime {
    client: Box<dyn AuthApi>,
    store: Box<dyn TokenStore>,
    access: Option<AccessToken>,
    refresh_available: bool,
}

impl AuthRuntime {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            client: Box::new(HanamiClient::new(config)),
            store: Box::<KeyringTokenStore>::default(),
            access: None,
            refresh_available: false,
        }
    }

    async fn exchange(
        &mut self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<(), AuthError> {
        let response = self
            .client
            .exchange_code(code, redirect_uri, verifier)
            .await?;
        let refresh = response
            .refresh_token
            .as_deref()
            .ok_or(AuthClientError::MissingRefreshToken)?;
        self.store.save(refresh)?;
        self.refresh_available = true;
        self.access = Some(response.into_access_token());
        Ok(())
    }

    fn inspect_stored_session(&mut self) -> Result<bool, AuthError> {
        self.refresh_available = self.store.load()?.is_some();
        Ok(self.refresh_available)
    }

    fn should_refresh(&self) -> bool {
        self.refresh_available
            && self.access.as_ref().is_none_or(|token| {
                token.expires_at <= std::time::Instant::now() + Duration::from_secs(90)
            })
    }

    fn needs_session_probe(&self) -> bool {
        !self.refresh_available && self.access.is_none()
    }

    async fn refresh(&mut self) -> Result<RefreshOutcome, AuthError> {
        let Some(refresh) = self.store.load()? else {
            self.refresh_available = false;
            self.access = None;
            return Ok(RefreshOutcome::NoSession);
        };
        self.refresh_available = true;
        let response = match self.client.refresh(&refresh).await {
            Ok(response) => response,
            Err(AuthClientError::Network) => return Ok(RefreshOutcome::TemporaryFailure),
            Err(AuthClientError::Rejected) => {
                self.access = None;
                self.refresh_available = false;
                self.store.delete()?;
                return Ok(RefreshOutcome::Rejected);
            }
            Err(error) => return Err(error.into()),
        };
        let rotated = response
            .refresh_token
            .as_deref()
            .ok_or(AuthClientError::MissingRefreshToken)?;
        self.store.save(rotated)?;
        self.refresh_available = true;
        self.access = Some(response.into_access_token());
        Ok(RefreshOutcome::Restored)
    }

    async fn logout(&mut self) -> Result<Option<String>, AuthError> {
        let stored = self.store.load();
        let token = stored
            .as_ref()
            .ok()
            .and_then(|value| value.as_deref())
            .or_else(|| self.access.as_ref().map(|access| access.value.as_str()))
            .map(str::to_owned);
        let revoke_failed = if let Some(token) = token.as_deref() {
            self.client.revoke(token).await.is_err()
        } else {
            stored.is_err()
        };

        let deletion_failed = self.store.delete().is_err();
        self.access = None;
        self.refresh_available = false;

        Ok(if deletion_failed {
            Some(
                "Signed out in memory, but the secure credential store could not confirm deletion."
                    .into(),
            )
        } else if revoke_failed {
            Some("Signed out locally. Hanami could not confirm remote token revocation.".into())
        } else {
            None
        })
    }
}

pub async fn connect(app: AppHandle) -> Result<(), AuthError> {
    let state = app.state::<AppState>();
    if state
        .auth_flow_active
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(AuthError::AlreadyInProgress);
    }

    let result = connect_inner(&app).await;
    state.auth_flow_active.store(false, Ordering::SeqCst);
    if let Err(error) = &result {
        let message = error.user_message();
        update_snapshot(&app, |snapshot| {
            snapshot.auth.state = AuthState::Error;
            snapshot.auth.message = Some(message);
        })
        .await;
    }
    result
}

async fn connect_inner(app: &AppHandle) -> Result<(), AuthError> {
    let listener = LoopbackListener::bind().await?;
    let redirect_uri = listener.redirect_uri().to_owned();
    let pkce = pkce::generate_pkce();
    let state_value = pkce::generate_state();
    let device_name = safe_device_name();
    let platform = platform_name();
    let authorization_url = {
        let app_state = app.state::<AppState>();
        let runtime = app_state.auth.lock().await;
        runtime.client.authorization_url(
            &redirect_uri,
            &state_value,
            &pkce.challenge,
            &device_name,
            platform,
        )?
    };

    set_auth_state(app, AuthState::OpeningBrowser, None).await;
    tauri_plugin_opener::open_url(authorization_url.as_str(), None::<&str>)
        .map_err(|_| AuthError::BrowserOpen)?;
    set_auth_state(app, AuthState::WaitingForApproval, None).await;
    let code = listener
        .wait_for_code(&state_value, Duration::from_secs(180))
        .await?;
    set_auth_state(app, AuthState::ExchangingCode, None).await;
    app.state::<AppState>()
        .auth
        .lock()
        .await
        .exchange(&code, &redirect_uri, &pkce.verifier)
        .await?;
    set_auth_state(app, AuthState::SignedIn, Some("Connected to Hanami".into())).await;
    Ok(())
}

pub async fn logout(app: AppHandle) -> Result<(), AuthError> {
    let result = app.state::<AppState>().auth.lock().await.logout().await;
    match result {
        Ok(warning) => {
            set_auth_state(&app, AuthState::SignedOut, warning).await;
            Ok(())
        }
        Err(error) => {
            let message = error.user_message();
            update_snapshot(&app, |snapshot| {
                snapshot.auth.state = AuthState::Error;
                snapshot.auth.message = Some(message);
            })
            .await;
            Err(error)
        }
    }
}

pub fn start_supervisor(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.auth_supervisor_started.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut backoff = RestoreBackoff::default();
        let mut delay = match inspect_stored_session(&app).await {
            Ok(true) => Duration::ZERO,
            Ok(false) => Duration::from_secs(30),
            Err(error) => {
                set_auth_state(&app, AuthState::Error, Some(error.user_message())).await;
                Duration::from_secs(30)
            }
        };

        loop {
            tokio::time::sleep(delay).await;
            if app.state::<AppState>().shutting_down.load(Ordering::SeqCst) {
                break;
            }
            if app
                .state::<AppState>()
                .auth_flow_active
                .load(Ordering::SeqCst)
            {
                delay = Duration::from_secs(2);
                continue;
            }

            let needs_refresh = app.state::<AppState>().auth.lock().await.should_refresh();
            if !needs_refresh {
                let needs_probe = app
                    .state::<AppState>()
                    .auth
                    .lock()
                    .await
                    .needs_session_probe();
                if needs_probe {
                    match inspect_stored_session(&app).await {
                        Ok(true) => {
                            delay = Duration::ZERO;
                            continue;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            set_auth_state(&app, AuthState::Error, Some(error.user_message()))
                                .await;
                        }
                    }
                }
                delay = Duration::from_secs(30);
                continue;
            }
            match refresh_session(&app).await {
                RefreshOutcome::Restored => {
                    backoff.reset();
                    delay = Duration::from_secs(30);
                }
                RefreshOutcome::TemporaryFailure => delay = backoff.take(),
                RefreshOutcome::NoSession
                | RefreshOutcome::Rejected
                | RefreshOutcome::CredentialStoreUnavailable => {
                    backoff.reset();
                    delay = Duration::from_secs(30);
                }
            }
        }
    });
}

async fn inspect_stored_session(app: &AppHandle) -> Result<bool, AuthError> {
    app.state::<AppState>()
        .auth
        .lock()
        .await
        .inspect_stored_session()
}

async fn refresh_session(app: &AppHandle) -> RefreshOutcome {
    set_auth_state(app, AuthState::Refreshing, None).await;
    let result = app.state::<AppState>().auth.lock().await.refresh().await;
    match result {
        Ok(RefreshOutcome::Restored) => {
            set_auth_state(app, AuthState::SignedIn, Some("Connected to Hanami".into())).await;
            RefreshOutcome::Restored
        }
        Ok(RefreshOutcome::TemporaryFailure) => {
            set_auth_state(
                app,
                AuthState::Error,
                Some("Hanami is temporarily unreachable. Session restoration will retry.".into()),
            )
            .await;
            RefreshOutcome::TemporaryFailure
        }
        Ok(outcome @ (RefreshOutcome::Rejected | RefreshOutcome::NoSession)) => {
            set_auth_state(app, AuthState::SignedOut, None).await;
            outcome
        }
        Ok(RefreshOutcome::CredentialStoreUnavailable) => {
            RefreshOutcome::CredentialStoreUnavailable
        }
        Err(AuthError::CredentialStore(error)) => {
            set_auth_state(
                app,
                AuthState::Error,
                Some(AuthError::CredentialStore(error).user_message()),
            )
            .await;
            RefreshOutcome::CredentialStoreUnavailable
        }
        Err(error) => {
            set_auth_state(app, AuthState::Error, Some(error.user_message())).await;
            RefreshOutcome::TemporaryFailure
        }
    }
}

async fn set_auth_state(app: &AppHandle, state: AuthState, message: Option<String>) {
    update_snapshot(app, |snapshot| {
        snapshot.auth.state = state;
        snapshot.auth.message = message;
    })
    .await;
}

fn safe_device_name() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Hanami Companion".into());
    let value: String = raw
        .chars()
        .filter(|character| character.is_alphanumeric() || " ._-".contains(*character))
        .take(64)
        .collect();
    if value.trim().is_empty() {
        "Hanami Companion".into()
    } else {
        value
    }
}

fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use url::Url;

    use super::*;
    use crate::auth::client::TokenResponse;

    struct FakeApi {
        refreshes: Mutex<VecDeque<Result<TokenResponse, AuthClientError>>>,
        revoke_result: Mutex<Option<Result<(), AuthClientError>>>,
    }

    #[async_trait]
    impl AuthApi for FakeApi {
        fn authorization_url(
            &self,
            _redirect_uri: &str,
            _state: &str,
            _challenge: &str,
            _device_name: &str,
            _platform: &str,
        ) -> Result<Url, AuthClientError> {
            Url::parse("http://localhost/oauth").map_err(|_| AuthClientError::InvalidUrl)
        }

        async fn exchange_code(
            &self,
            _code: &str,
            _redirect_uri: &str,
            _verifier: &str,
        ) -> Result<TokenResponse, AuthClientError> {
            unreachable!()
        }

        async fn refresh(&self, _refresh_token: &str) -> Result<TokenResponse, AuthClientError> {
            self.refreshes
                .lock()
                .expect("refresh queue")
                .pop_front()
                .expect("queued refresh")
        }

        async fn revoke(&self, _token: &str) -> Result<(), AuthClientError> {
            self.revoke_result
                .lock()
                .expect("revoke result")
                .take()
                .unwrap_or(Ok(()))
        }
    }

    #[derive(Clone)]
    struct FakeStore(Arc<Mutex<Option<String>>>);

    impl TokenStore for FakeStore {
        fn load(&self) -> Result<Option<String>, TokenStoreError> {
            Ok(self.0.lock().expect("store").clone())
        }

        fn save(&self, refresh_token: &str) -> Result<(), TokenStoreError> {
            *self.0.lock().expect("store") = Some(refresh_token.into());
            Ok(())
        }

        fn delete(&self) -> Result<(), TokenStoreError> {
            *self.0.lock().expect("store") = None;
            Ok(())
        }
    }

    fn token(refresh: &str) -> TokenResponse {
        TokenResponse {
            access_token: "access".into(),
            refresh_token: Some(refresh.into()),
            expires_in: 3600,
            token_type: Some("Bearer".into()),
        }
    }

    fn runtime(
        refreshes: Vec<Result<TokenResponse, AuthClientError>>,
        revoke: Result<(), AuthClientError>,
    ) -> (AuthRuntime, Arc<Mutex<Option<String>>>) {
        let stored = Arc::new(Mutex::new(Some("stored-refresh".into())));
        (
            AuthRuntime {
                client: Box::new(FakeApi {
                    refreshes: Mutex::new(refreshes.into()),
                    revoke_result: Mutex::new(Some(revoke)),
                }),
                store: Box::new(FakeStore(Arc::clone(&stored))),
                access: None,
                refresh_available: true,
            },
            stored,
        )
    }

    #[tokio::test]
    async fn restoration_retries_after_a_temporary_network_failure() {
        let (mut runtime, _) = runtime(
            vec![Err(AuthClientError::Network), Ok(token("rotated"))],
            Ok(()),
        );
        assert_eq!(
            runtime.refresh().await.expect("temporary"),
            RefreshOutcome::TemporaryFailure
        );
        assert!(runtime.should_refresh());
        assert_eq!(
            runtime.refresh().await.expect("restored"),
            RefreshOutcome::Restored
        );
        assert!(!runtime.should_refresh());
    }

    #[tokio::test]
    async fn rejected_refresh_clears_the_stored_session() {
        let (mut runtime, stored) = runtime(vec![Err(AuthClientError::Rejected)], Ok(()));
        assert_eq!(
            runtime.refresh().await.expect("rejected"),
            RefreshOutcome::Rejected
        );
        assert!(stored.lock().expect("store").is_none());
        assert!(!runtime.refresh_available);
    }

    #[tokio::test]
    async fn offline_logout_still_deletes_the_local_credential() {
        let (mut runtime, stored) = runtime(Vec::new(), Err(AuthClientError::Network));
        let warning = runtime.logout().await.expect("local logout");
        assert!(warning.is_some());
        assert!(stored.lock().expect("store").is_none());
        assert!(!runtime.refresh_available);
    }

    #[test]
    fn restoration_backoff_is_bounded() {
        let mut backoff = RestoreBackoff::default();
        assert_eq!(backoff.take(), Duration::from_secs(2));
        for _ in 0..10 {
            backoff.take();
        }
        assert_eq!(backoff.take(), Duration::from_secs(60));
    }
}
