//! Платформенный слой: каталоги, настройки (совместимо с QSettings Python-версии), токен,
//! MPRIS (Linux) и «единственный экземпляр» (Linux — D-Bus, Windows — именованный канал, macOS — unix-сокет).
pub mod dirs;
pub mod mpris;
pub mod settings;
pub mod single_instance;
pub mod tokens;

pub use settings::{SettingKey, SettingType, SettingValue, SettingsStore};
pub use single_instance::InstanceListener;
pub use tokens::{TokenStorage, TokenStore};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("ввод-вывод: {0}")]
    Io(#[from] std::io::Error),
    #[error("хранилище секретов: {0}")]
    Secret(String),
    #[error("D-Bus: {0}")]
    DBus(String),
    #[error("не поддерживается на этой ОС")]
    Unsupported,
}
