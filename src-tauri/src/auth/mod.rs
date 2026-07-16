mod client;
mod loopback;
mod pkce;
mod token_store;

use std::{sync::atomic::Ordering, time::Duration};

use tauri::{AppHandle, Manager};
use thiserror::Error;

pub use client::AuthConfig;
use client::{AccessToken, AuthClientError, HanamiClient};
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
                "Hanami could not be reached. Check your connection and try again.".into()
            }
            Self::Client(_) => "Hanami could not complete authentication.".into(),
            Self::CredentialStore(_) => {
                "Secure credential storage is unavailable. No token was saved.".into()
            }
        }
    }
}

pub struct AuthRuntime {
    client: HanamiClient,
    store: Box<dyn TokenStore>,
    access: Option<AccessToken>,
}

impl AuthRuntime {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            client: HanamiClient::new(config),
            store: Box::<KeyringTokenStore>::default(),
            access: None,
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
        self.access = Some(response.into_access_token());
        Ok(())
    }

    fn has_stored_session(&self) -> Result<bool, AuthError> {
        Ok(self.store.load()?.is_some())
    }

    fn should_refresh(&self) -> bool {
        self.access.as_ref().is_some_and(|token| {
            token.expires_at <= std::time::Instant::now() + Duration::from_secs(90)
        })
    }

    async fn refresh(&mut self) -> Result<(), AuthError> {
        let refresh = self.store.load()?.ok_or(AuthClientError::Rejected)?;
        let response = self.client.refresh(&refresh).await?;
        let rotated = response
            .refresh_token
            .as_deref()
            .ok_or(AuthClientError::MissingRefreshToken)?;
        self.store.save(rotated)?;
        self.access = Some(response.into_access_token());
        Ok(())
    }

    async fn logout(&mut self) -> Result<(), AuthError> {
        let stored = self.store.load()?;
        let token = stored
            .as_deref()
            .or_else(|| self.access.as_ref().map(|access| access.value.as_str()));
        if let Some(token) = token {
            self.client.revoke(token).await?;
        }
        self.store.delete()?;
        self.access = None;
        Ok(())
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
        Ok(()) => {
            set_auth_state(&app, AuthState::SignedOut, None).await;
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
        restore_session(&app).await;
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if app.state::<AppState>().shutting_down.load(Ordering::SeqCst) {
                break;
            }
            if app
                .state::<AppState>()
                .auth_flow_active
                .load(Ordering::SeqCst)
            {
                continue;
            }
            let needs_refresh = app.state::<AppState>().auth.lock().await.should_refresh();
            if needs_refresh {
                refresh_session(&app).await;
            }
        }
    });
}

async fn restore_session(app: &AppHandle) {
    let stored = app
        .state::<AppState>()
        .auth
        .lock()
        .await
        .has_stored_session();
    match stored {
        Ok(true) => refresh_session(app).await,
        Ok(false) => {}
        Err(error) => {
            set_auth_state(app, AuthState::Error, Some(error.user_message())).await;
        }
    }
}

async fn refresh_session(app: &AppHandle) {
    set_auth_state(app, AuthState::Refreshing, None).await;
    let result = app.state::<AppState>().auth.lock().await.refresh().await;
    match result {
        Ok(()) => {
            set_auth_state(app, AuthState::SignedIn, Some("Connected to Hanami".into())).await;
        }
        Err(error) => {
            set_auth_state(app, AuthState::Error, Some(error.user_message())).await;
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
