//! Каталоги — те же, что QStandardPaths в Python-версии (organization "yamusic", application "YaMusic"),
//! чтобы токен-файл, session.json и кэши подхватились после перехода на Rust.
//!
//! | что                    | Linux                         | Windows                                   | macOS                                         |
//! |------------------------|-------------------------------|-------------------------------------------|-----------------------------------------------|
//! | app_data_dir (AppData) | ~/.local/share/yamusic/YaMusic| %APPDATA%\yamusic\YaMusic                 | ~/Library/Application Support/yamusic/YaMusic |
//! | cache_dir (Cache)      | ~/.cache/yamusic/YaMusic      | %LOCALAPPDATA%\yamusic\YaMusic\cache      | ~/Library/Caches/yamusic/YaMusic              |
//! | settings INI           | ~/.config/yamusic/YaMusic.conf| (реестр HKCU\Software\yamusic\YaMusic)    | ~/Library/Preferences/yamusic/YaMusic.conf    |
use std::path::{Path, PathBuf};

pub const ORGANIZATION: &str = "yamusic";
pub const APPLICATION: &str = "YaMusic";

fn home() -> PathBuf {
    ::dirs::home_dir().unwrap_or_else(std::env::temp_dir)
}

fn app_subdir(base: PathBuf) -> PathBuf {
    base.join(ORGANIZATION).join(APPLICATION)
}

/// Каталог данных приложения (token, session.json). Не создаётся автоматически.
pub fn app_data_dir() -> PathBuf {
    // Linux: $XDG_DATA_HOME или ~/.local/share; Windows: Roaming; macOS: Application Support
    let base = ::dirs::data_dir().unwrap_or_else(|| home().join(".local").join("share"));
    app_subdir(base)
}

/// Корень кэша приложения.
pub fn cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = ::dirs::data_local_dir().unwrap_or_else(|| home().join("AppData").join("Local"));
        app_subdir(base).join("cache")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let base = ::dirs::cache_dir().unwrap_or_else(|| home().join(".cache"));
        app_subdir(base)
    }
}

/// Подкаталог кэша ("images", "covers", "lyrics"), создаётся при первом вызове.
pub fn cache_subdir(sub: &str) -> PathBuf {
    let dir = cache_dir().join(sub);
    if let Err(err) = std::fs::create_dir_all(&dir) {
        tracing::warn!("не удалось создать {}: {err}", dir.display());
    }
    dir
}

/// Файл сессии плеера: $YAMUSIC_SESSION_FILE или app_data_dir()/session.json.
pub fn session_file() -> PathBuf {
    match std::env::var_os("YAMUSIC_SESSION_FILE") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => app_data_dir().join("session.json"),
    }
}

/// Файл токена (запасной вариант без keyring): app_data_dir()/token.
pub fn token_file() -> PathBuf {
    app_data_dir().join("token")
}

/// Файл настроек INI. `config_override` — каталог из --config-dir: тогда <dir>/yamusic/YaMusic.conf на любой ОС
/// (так QSettings::setPath раскладывает INI). Без него: Linux/macOS — путь из таблицы; на Windows INI не используется.
pub fn settings_ini_file(config_override: Option<&Path>) -> PathBuf {
    let base = match config_override {
        Some(dir) => dir.to_path_buf(),
        None => default_config_base(),
    };
    base.join(ORGANIZATION).join(format!("{APPLICATION}.conf"))
}

fn default_config_base() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home().join("Library").join("Preferences")
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux: $XDG_CONFIG_HOME или ~/.config; на Windows путь только для полноты (там реестр)
        ::dirs::config_dir().unwrap_or_else(|| home().join(".config"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        assert!(app_data_dir().ends_with(Path::new("yamusic").join("YaMusic")));
        assert_eq!(token_file().file_name().unwrap(), "token");
        let ini = settings_ini_file(Some(Path::new("cfg")));
        assert_eq!(ini, Path::new("cfg").join("yamusic").join("YaMusic.conf"));
        #[cfg(target_os = "windows")]
        assert!(cache_dir().ends_with(Path::new("yamusic").join("YaMusic").join("cache")));
    }
}
