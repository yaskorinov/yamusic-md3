//! Вход (src/yamusic/auth.py). Состояния: checking → signedOut | signedIn;
//! signedOut → requestingCode → awaitingUser → signingIn → signedIn; error — с текстом в error_text.
//! Переход в requestingCode/signingIn/signedIn очищает error_text. Тексты ошибок — дословно как в Python:
//! - код истёк: «Время действия кода истекло»; отказ: «Вход отклонён: <e>» (оба → signedOut);
//! - 401/403 при входе: токен удаляется, «Токен недействителен или отозван — войдите заново» → signedOut;
//! - сеть: «Нет связи с Яндексом. Проверьте подключение к интернету» → error;
//! - прочие ошибки API: «Ошибка Яндекс Музыки: <e>»; остальное: «Непредвиденная ошибка: <e>» → error.
//!
//! Device flow: request_device_code(DEVICE_NAME), показать код (awaitingUser), опрос раз в max(interval, 2) с до expires_in.
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use ym_api::{ApiError, YmClient};
use ym_platform::TokenStore;

use crate::lock;

/// "YaMusic (Windows)" | "YaMusic (Linux)" | "YaMusic (macOS)" | "YaMusic (Desktop)"
pub fn device_name() -> String {
    let platform = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Desktop"
    };
    format!("YaMusic ({platform})")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthState {
    #[default]
    Checking,
    SignedOut,
    RequestingCode,
    AwaitingUser,
    SigningIn,
    SignedIn,
    Error,
}

impl AuthState {
    /// "checking" | "signedOut" | "requestingCode" | "awaitingUser" | "signingIn" | "signedIn" | "error"
    pub fn as_str(self) -> &'static str {
        match self {
            AuthState::Checking => "checking",
            AuthState::SignedOut => "signedOut",
            AuthState::RequestingCode => "requestingCode",
            AuthState::AwaitingUser => "awaitingUser",
            AuthState::SigningIn => "signingIn",
            AuthState::SignedIn => "signedIn",
            AuthState::Error => "error",
        }
    }
}

/// Auth.account: {uid, login, displayName, hasPlus}; displayName = display_name || full_name || login.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uid: i64,
    pub login: String,
    pub display_name: String,
    pub has_plus: bool,
}

impl Account {
    pub fn from_status(status: &ym_api::models::AccountStatus) -> Self {
        let has_plus = status.plus.as_ref().and_then(|p| p.has_plus).unwrap_or(false);
        let Some(account) = status.account.as_ref() else {
            return Account { has_plus, ..Account::default() };
        };
        let login = account.login.clone().unwrap_or_default();
        let display_name = [&account.display_name, &account.full_name]
            .into_iter()
            .flatten()
            .find(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| login.clone());
        Account { uid: account.uid.unwrap_or(0), login, display_name, has_plus }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthSnapshot {
    pub state: AuthState,
    pub error_text: String,
    pub user_code: String,
    pub verification_url: String,
    /// Unix-время в мс, когда код перестанет действовать (0 — кода нет)
    pub code_expires_at_ms: f64,
    /// "keyring" | "file" | ""
    pub token_storage: String,
    /// None — Auth.account = {}
    pub account: Option<Account>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthEvent {
    /// клиент готов (Library обновляется, Wave грузит настройки)
    SignedIn(Account),
    SignedOut,
}

/// Ошибка шагов входа (аналог исключений Python).
#[derive(Debug)]
enum LoginError {
    /// DeviceAuthError: истёк код или пользователь отказал
    Device(String),
    Api(ApiError),
    Other(String),
}

impl From<ApiError> for LoginError {
    fn from(e: ApiError) -> Self {
        LoginError::Api(e)
    }
}

/// Текст и итоговое состояние для ошибки входа (_on_error в auth.py).
fn describe_error(error: &LoginError) -> (String, AuthState) {
    match error {
        LoginError::Device(text) => {
            let text = if text.contains("expired") {
                "Время действия кода истекло".to_owned()
            } else {
                format!("Вход отклонён: {text}")
            };
            (text, AuthState::SignedOut)
        }
        LoginError::Api(e) if e.is_network() => {
            ("Нет связи с Яндексом. Проверьте подключение к интернету".to_owned(), AuthState::Error)
        }
        LoginError::Api(e) => (format!("Ошибка Яндекс Музыки: {e}"), AuthState::Error),
        LoginError::Other(e) => (format!("Непредвиденная ошибка: {e}"), AuthState::Error),
    }
}

/// Токен из поля ввода: trim, без префикса "OAuth " (без учёта регистра).
pub fn normalize_token(token: &str) -> String {
    let token = token.trim();
    match token.get(..6) {
        Some(prefix) if prefix.eq_ignore_ascii_case("oauth ") => token[6..].trim().to_owned(),
        _ => token.to_owned(),
    }
}

fn unix_now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[derive(Default)]
struct Inner {
    client: Option<YmClient>,
    /// опрос device flow
    poll: Option<JoinHandle<()>>,
    /// номер попытки входа по коду: отменённый опрос не должен ничего менять
    login_gen: u64,
    /// номер входа по токену: выход отбрасывает незавершённый вход
    signin_gen: u64,
}

pub struct AuthService {
    rt: Handle,
    tokens: TokenStore,
    snapshot: watch::Sender<AuthSnapshot>,
    events: broadcast::Sender<AuthEvent>,
    inner: Mutex<Inner>,
    me: Weak<AuthService>,
}

impl AuthService {
    pub fn new(rt: Handle, tokens: TokenStore) -> Arc<Self> {
        Arc::new_cyclic(|me| AuthService {
            rt,
            tokens,
            snapshot: watch::Sender::new(AuthSnapshot::default()),
            events: broadcast::channel(16).0,
            inner: Mutex::new(Inner::default()),
            me: me.clone(),
        })
    }

    /// Восстановить сессию из keyring: checking → (токен есть) signingIn без сохранения | signedOut.
    pub fn start(&self) {
        self.set_state(AuthState::Checking);
        let Some(me) = self.me.upgrade() else { return };
        self.rt.spawn(async move {
            let loaded = me.tokens.load().await;
            let backend = me.tokens.backend().await;
            match loaded {
                Ok(Some(token)) if !token.is_empty() => {
                    me.set_storage(backend.as_str());
                    me.sign_in(token, false);
                }
                Ok(_) => me.set_state(AuthState::SignedOut),
                Err(e) => me.fail(&LoginError::Other(e.to_string())),
            }
        });
    }

    pub fn start_device_login(&self) {
        self.cancel_login();
        self.set_state(AuthState::RequestingCode);
        let Some(me) = self.me.upgrade() else { return };
        let mut inner = lock(&self.inner);
        let epoch = inner.login_gen;
        inner.poll = Some(self.rt.spawn(async move {
            let result = me.device_flow(epoch).await;
            if lock(&me.inner).login_gen != epoch {
                return;
            }
            match result {
                Ok(token) => me.on_token_received(token),
                Err(e) => me.fail(&e),
            }
        }));
    }

    /// Отменить опрос; из requestingCode/awaitingUser — в signedOut, код очищается.
    pub fn cancel_login(&self) {
        {
            let mut inner = lock(&self.inner);
            inner.login_gen += 1;
            if let Some(poll) = inner.poll.take() {
                poll.abort();
            }
        }
        let state = self.snapshot.borrow().state;
        if matches!(state, AuthState::RequestingCode | AuthState::AwaitingUser) {
            self.set_code("", "", 0.0);
            self.set_state(AuthState::SignedOut);
        }
    }

    /// Токен вручную: trim, срезать префикс "OAuth " (без учёта регистра); пустой — игнор.
    pub fn sign_in_with_token(&self, token: &str) {
        let token = normalize_token(token);
        if token.is_empty() {
            return;
        }
        self.cancel_login();
        self.on_token_received(token);
    }

    /// Забыть клиента и аккаунт, удалить токен, tokenStorage = "", signedOut, событие SignedOut.
    pub fn sign_out(&self) {
        self.cancel_login();
        {
            let mut inner = lock(&self.inner);
            inner.client = None;
            inner.signin_gen += 1;
        }
        self.snapshot.send_if_modified(|s| {
            let changed = s.account.is_some();
            s.account = None;
            changed
        });
        let tokens = self.tokens.clone();
        self.rt.spawn(async move {
            if let Err(e) = tokens.clear().await {
                tracing::warn!("не удалось удалить токен: {e}");
            }
        });
        self.set_storage("");
        self.set_state(AuthState::SignedOut);
        let _ = self.events.send(AuthEvent::SignedOut);
    }

    /// = start()
    pub fn retry(&self) {
        self.start();
    }

    pub fn client(&self) -> Option<YmClient> {
        lock(&self.inner).client.clone()
    }

    pub fn account(&self) -> Option<Account> {
        self.snapshot.borrow().account.clone()
    }

    /// Клиент и аккаунт вместе (для запросов с uid).
    pub fn session(&self) -> Option<(YmClient, Account)> {
        let client = self.client()?;
        let account = self.account()?;
        Some((client, account))
    }

    pub fn snapshot(&self) -> AuthSnapshot {
        self.snapshot.borrow().clone()
    }

    pub fn watch(&self) -> watch::Receiver<AuthSnapshot> {
        self.snapshot.subscribe()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AuthEvent> {
        self.events.subscribe()
    }

    // --- поток входа ---------------------------------------------------------

    async fn device_flow(&self, epoch: u64) -> Result<String, LoginError> {
        let client = YmClient::new(None)?;
        let code = client.request_device_code(&device_name()).await?;
        let expires_at = unix_now() + code.expires_in as f64;
        if lock(&self.inner).login_gen != epoch {
            return Err(LoginError::Other("отменено".to_owned()));
        }
        self.set_code(&code.user_code, &code.verification_url, expires_at);
        self.set_state(AuthState::AwaitingUser);
        let interval = Duration::from_secs(code.interval.max(2));
        while unix_now() < expires_at {
            tokio::time::sleep(interval).await;
            match client.poll_device_token(&code.device_code).await {
                Ok(Some(token)) => return Ok(token.access_token),
                Ok(None) => {}
                Err(ApiError::BadRequest { code, message }) => {
                    let text = if message.is_empty() { code } else { message };
                    return Err(LoginError::Device(text));
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(LoginError::Device("expired_token".to_owned()))
    }

    fn on_token_received(&self, token: String) {
        lock(&self.inner).poll = None;
        self.set_code("", "", 0.0);
        self.sign_in(token, true);
    }

    fn sign_in(&self, token: String, save: bool) {
        self.set_state(AuthState::SigningIn);
        let epoch = {
            let mut inner = lock(&self.inner);
            inner.signin_gen += 1;
            inner.signin_gen
        };
        let Some(me) = self.me.upgrade() else { return };
        self.rt.spawn(async move {
            let result = me.init_client(&token, save).await;
            if lock(&me.inner).signin_gen != epoch {
                return;
            }
            match result {
                Ok((client, account, storage)) => {
                    lock(&me.inner).client = Some(client);
                    me.set_storage(&storage);
                    me.snapshot.send_modify(|s| s.account = Some(account.clone()));
                    me.set_state(AuthState::SignedIn);
                    let _ = me.events.send(AuthEvent::SignedIn(account));
                }
                Err(LoginError::Api(ApiError::Unauthorized(_))) => {
                    let tokens = me.tokens.clone();
                    me.rt.spawn(async move {
                        let _ = tokens.clear().await;
                    });
                    me.set_error("Токен недействителен или отозван — войдите заново");
                    me.set_state(AuthState::SignedOut);
                }
                Err(e) => me.fail(&e),
            }
        });
    }

    async fn init_client(&self, token: &str, save: bool) -> Result<(YmClient, Account, String), LoginError> {
        let client = YmClient::new(Some(token.to_owned()))?;
        let status = client.account_status().await?;
        let account = Account::from_status(&status);
        let storage = if save {
            match self.tokens.save(token).await {
                Ok(storage) => storage.as_str().to_owned(),
                Err(e) => return Err(LoginError::Other(e.to_string())),
            }
        } else {
            self.snapshot.borrow().token_storage.clone()
        };
        Ok((client, account, storage))
    }

    /// _on_error
    fn fail(&self, error: &LoginError) {
        lock(&self.inner).poll = None;
        self.set_code("", "", 0.0);
        let (text, state) = describe_error(error);
        self.set_error(&text);
        self.set_state(state);
    }

    // --- служебное -----------------------------------------------------------

    fn set_state(&self, state: AuthState) {
        self.snapshot.send_if_modified(|s| {
            if s.state == state {
                return false;
            }
            if matches!(state, AuthState::RequestingCode | AuthState::SigningIn | AuthState::SignedIn) {
                s.error_text.clear();
            }
            s.state = state;
            true
        });
    }

    fn set_storage(&self, storage: &str) {
        self.snapshot.send_if_modified(|s| {
            if s.token_storage == storage {
                return false;
            }
            s.token_storage = storage.to_owned();
            true
        });
    }

    fn set_error(&self, text: &str) {
        self.snapshot.send_if_modified(|s| {
            if s.error_text == text {
                return false;
            }
            s.error_text = text.to_owned();
            true
        });
    }

    fn set_code(&self, code: &str, url: &str, expires_at: f64) {
        let expires_ms = expires_at * 1000.0;
        self.snapshot.send_if_modified(|s| {
            if s.user_code == code && s.verification_url == url && s.code_expires_at_ms == expires_ms {
                return false;
            }
            s.user_code = code.to_owned();
            s.verification_url = url.to_owned();
            s.code_expires_at_ms = expires_ms;
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_names() {
        assert_eq!(AuthState::SignedOut.as_str(), "signedOut");
        assert_eq!(AuthState::AwaitingUser.as_str(), "awaitingUser");
        assert_eq!(AuthState::default(), AuthState::Checking);
    }

    #[test]
    fn device_name_has_platform() {
        let name = device_name();
        assert!(name.starts_with("YaMusic (") && name.ends_with(')'));
        #[cfg(target_os = "windows")]
        assert_eq!(name, "YaMusic (Windows)");
    }

    #[test]
    fn token_prefix() {
        assert_eq!(normalize_token("  OAuth  abc "), "abc");
        assert_eq!(normalize_token("oauth xyz"), "xyz");
        assert_eq!(normalize_token("token"), "token");
        assert_eq!(normalize_token("   "), "");
        assert_eq!(normalize_token("OAuth"), "OAuth");
    }

    #[test]
    fn error_texts() {
        let (text, state) = describe_error(&LoginError::Device("expired_token".into()));
        assert_eq!((text.as_str(), state), ("Время действия кода истекло", AuthState::SignedOut));
        let (text, state) = describe_error(&LoginError::Device("access_denied".into()));
        assert_eq!((text.as_str(), state), ("Вход отклонён: access_denied", AuthState::SignedOut));
        let (text, state) = describe_error(&LoginError::Other("boom".into()));
        assert_eq!((text.as_str(), state), ("Непредвиденная ошибка: boom", AuthState::Error));
    }

    #[test]
    fn account_display_name() {
        use ym_api::models::AccountStatus;
        let status: AccountStatus = serde_json::from_value(serde_json::json!({
            "account": {"uid": 7, "login": "login", "displayName": "", "fullName": "Full"},
            "plus": {"hasPlus": true}
        }))
        .unwrap();
        let account = Account::from_status(&status);
        assert_eq!(account, Account { uid: 7, login: "login".into(), display_name: "Full".into(), has_plus: true });
        assert_eq!(Account::from_status(&AccountStatus::default()), Account::default());
    }
}
