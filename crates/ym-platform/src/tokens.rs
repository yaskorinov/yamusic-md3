//! OAuth-токен (src/yamusic/tokens.py), совместимо с уже сохранённым Python-версией:
//! - Windows: Credential Manager, TargetName "YaMusic", UserName "yandex-oauth-token"
//!   (keyring::Entry::new_with_target("YaMusic", "YaMusic", "yandex-oauth-token") — как питоновский WinVaultKeyring);
//! - Linux: Secret Service, коллекция по умолчанию (разблокировать, не создавать), атрибуты
//!   {"application": "yamusic", "account": "yandex-oauth-token"}, метка "YaMusic: токен Яндекса" (крейт secret-service);
//! - macOS: Keychain через keyring (service "YaMusic", user "yandex-oauth-token");
//! - запасной вариант везде: файл dirs::token_file() (на unix — права 0600). Успешная запись в keyring удаляет файл.
//!
//! Методы асинхронные (Secret Service — D-Bus); синхронные бэкенды выполняются в spawn_blocking.
use std::path::{Path, PathBuf};

use crate::PlatformError;

pub const KEYRING_SERVICE: &str = "YaMusic";
pub const KEYRING_USER: &str = "yandex-oauth-token";
pub const SECRET_LABEL: &str = "YaMusic: токен Яндекса";

/// Где лежит токен — Auth.tokenStorage: "keyring" | "file" (пусто — нигде).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenStorage {
    Keyring,
    File,
}

impl TokenStorage {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenStorage::Keyring => "keyring",
            TokenStorage::File => "file",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TokenStore {
    file: PathBuf,
    use_keyring: bool,
}

impl Default for TokenStore {
    fn default() -> Self {
        Self { file: crate::dirs::token_file(), use_keyring: true }
    }
}

impl TokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Только файл, без keyring (тесты, отладка).
    pub fn file_only(file: impl Into<PathBuf>) -> Self {
        Self { file: file.into(), use_keyring: false }
    }

    /// Какой бэкенд доступен сейчас: Windows/macOS — Keyring; Linux — Keyring, если коллекция Secret Service открылась, иначе File.
    pub async fn backend(&self) -> TokenStorage {
        if self.use_keyring && keyring_backend::available().await {
            TokenStorage::Keyring
        } else {
            TokenStorage::File
        }
    }

    /// Токен из keyring, иначе из файла (пустой файл — None).
    pub async fn load(&self) -> Result<Option<String>, PlatformError> {
        if self.use_keyring {
            match keyring_backend::load().await {
                Ok(Some(token)) if !token.is_empty() => return Ok(Some(token)),
                Ok(_) => {}
                Err(err) => tracing::debug!("токен из keyring не прочитан: {err}"),
            }
        }
        read_file(&self.file).await
    }

    /// Сохранить; возвращает, куда легло.
    pub async fn save(&self, token: &str) -> Result<TokenStorage, PlatformError> {
        if self.use_keyring {
            match keyring_backend::save(token).await {
                Ok(true) => {
                    remove_file(&self.file).await?;
                    return Ok(TokenStorage::Keyring);
                }
                Ok(false) => {}
                Err(err) => tracing::warn!("keyring недоступен, токен уйдёт в файл: {err}"),
            }
        }
        write_file(&self.file, token).await?;
        Ok(TokenStorage::File)
    }

    /// Удалить отовсюду (ошибки keyring игнорируются).
    pub async fn clear(&self) -> Result<(), PlatformError> {
        if self.use_keyring
            && let Err(err) = keyring_backend::clear().await
        {
            tracing::debug!("токен из keyring не удалён: {err}");
        }
        remove_file(&self.file).await
    }
}

async fn read_file(path: &Path) -> Result<Option<String>, PlatformError> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => {
            let token = text.trim();
            Ok((!token.is_empty()).then(|| token.to_owned()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

async fn remove_file(path: &Path) -> Result<(), PlatformError> {
    match tokio::fs::remove_file(path).await {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
        _ => Ok(()),
    }
}

async fn write_file(path: &Path, token: &str) -> Result<(), PlatformError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let path = path.to_path_buf();
    let token = token.to_owned();
    tokio::task::spawn_blocking(move || write_private(&path, &token))
        .await
        .map_err(|err| PlatformError::Io(std::io::Error::other(err)))??;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, token: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn write_private(path: &Path, token: &str) -> std::io::Result<()> {
    std::fs::write(path, token)
}

/// Windows/macOS: крейт keyring (синхронный — в spawn_blocking).
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod keyring_backend {
    use super::{KEYRING_SERVICE, KEYRING_USER};
    use crate::PlatformError;

    fn entry() -> keyring::Result<keyring::Entry> {
        #[cfg(target_os = "windows")]
        {
            keyring::Entry::new_with_target(KEYRING_SERVICE, KEYRING_SERVICE, KEYRING_USER)
        }
        #[cfg(not(target_os = "windows"))]
        {
            keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        }
    }

    async fn blocking<T: Send + 'static>(
        f: impl FnOnce() -> keyring::Result<T> + Send + 'static,
    ) -> Result<T, PlatformError> {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|err| PlatformError::Secret(err.to_string()))?
            .map_err(|err| PlatformError::Secret(err.to_string()))
    }

    pub async fn available() -> bool {
        true
    }

    pub async fn load() -> Result<Option<String>, PlatformError> {
        blocking(|| match entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err),
        })
        .await
    }

    pub async fn save(token: &str) -> Result<bool, PlatformError> {
        let token = token.to_owned();
        blocking(move || entry()?.set_password(&token).map(|()| true)).await
    }

    pub async fn clear() -> Result<(), PlatformError> {
        blocking(|| match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err),
        })
        .await
    }
}

/// Linux: Secret Service, коллекция по умолчанию (как secretstorage в Python).
#[cfg(target_os = "linux")]
mod keyring_backend {
    use std::collections::HashMap;

    use secret_service::{Collection, EncryptionType, SecretService};

    use super::{KEYRING_USER, SECRET_LABEL};
    use crate::PlatformError;

    fn attrs() -> HashMap<&'static str, &'static str> {
        HashMap::from([("application", "yamusic"), ("account", KEYRING_USER)])
    }

    fn err(e: secret_service::Error) -> PlatformError {
        PlatformError::Secret(e.to_string())
    }

    async fn connect() -> Option<SecretService<'static>> {
        match SecretService::connect(EncryptionType::Dh).await {
            Ok(ss) => Some(ss),
            Err(e) => {
                tracing::debug!("Secret Service недоступен: {e}");
                None
            }
        }
    }

    /// Коллекция по умолчанию или None (без попыток её создать).
    async fn collection<'a>(ss: &'a SecretService<'_>) -> Option<Collection<'a>> {
        let collection = ss.get_collection_by_alias("default").await.ok()?;
        if collection.is_locked().await.ok()? {
            let _ = collection.unlock().await;
        }
        match collection.is_locked().await {
            Ok(false) => Some(collection),
            _ => None,
        }
    }

    pub async fn available() -> bool {
        match connect().await {
            Some(ss) => collection(&ss).await.is_some(),
            None => false,
        }
    }

    pub async fn load() -> Result<Option<String>, PlatformError> {
        let Some(ss) = connect().await else { return Ok(None) };
        let Some(collection) = collection(&ss).await else { return Ok(None) };
        let items = collection.search_items(attrs()).await.map_err(err)?;
        match items.first() {
            Some(item) => {
                let secret = item.get_secret().await.map_err(err)?;
                Ok(Some(String::from_utf8_lossy(&secret).into_owned()))
            }
            None => Ok(None),
        }
    }

    /// false — коллекции нет (запишем в файл).
    pub async fn save(token: &str) -> Result<bool, PlatformError> {
        let Some(ss) = connect().await else { return Ok(false) };
        let Some(collection) = collection(&ss).await else { return Ok(false) };
        collection
            .create_item(SECRET_LABEL, attrs(), token.as_bytes(), true, "text/plain")
            .await
            .map_err(err)?;
        Ok(true)
    }

    pub async fn clear() -> Result<(), PlatformError> {
        let Some(ss) = connect().await else { return Ok(()) };
        let Some(collection) = collection(&ss).await else { return Ok(()) };
        for item in collection.search_items(attrs()).await.map_err(err)? {
            item.delete().await.map_err(err)?;
        }
        Ok(())
    }
}

/// Прочие ОС: только файл.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod keyring_backend {
    use crate::PlatformError;

    pub async fn available() -> bool {
        false
    }

    pub async fn load() -> Result<Option<String>, PlatformError> {
        Ok(None)
    }

    pub async fn save(_token: &str) -> Result<bool, PlatformError> {
        Ok(false)
    }

    pub async fn clear() -> Result<(), PlatformError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("ym-platform-test-{}-{name}", std::process::id()))
            .join("token")
    }

    #[tokio::test]
    async fn file_roundtrip() {
        let path = temp_file("roundtrip");
        let store = TokenStore::file_only(&path);
        assert_eq!(store.load().await.unwrap(), None);
        assert_eq!(store.backend().await, TokenStorage::File);
        assert_eq!(store.save("abc").await.unwrap(), TokenStorage::File);
        assert_eq!(store.load().await.unwrap().as_deref(), Some("abc"));
        // как Python: пробелы и перевод строки обрезаются, пустой файл — None
        std::fs::write(&path, "  tok\n").unwrap();
        assert_eq!(store.load().await.unwrap().as_deref(), Some("tok"));
        std::fs::write(&path, "\n").unwrap();
        assert_eq!(store.load().await.unwrap(), None);
        store.clear().await.unwrap();
        assert!(!path.exists());
        store.clear().await.unwrap();
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_file("private");
        let store = TokenStore::file_only(&path);
        store.save("secret").await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    #[ignore = "пишет в настоящий keyring пользователя"]
    async fn keyring_roundtrip() {
        let store = TokenStore::new();
        let previous = store.load().await.unwrap();
        assert_eq!(store.save("ym-platform-test").await.unwrap(), store.backend().await);
        assert_eq!(store.load().await.unwrap().as_deref(), Some("ym-platform-test"));
        match previous {
            Some(token) => {
                store.save(&token).await.unwrap();
            }
            None => store.clear().await.unwrap(),
        }
    }
}
