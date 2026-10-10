//! Настройки приложения: схема из src/yamusic/settings.py (имена, типы, значения по умолчанию).
//!
//! Хранилище совместимо с QSettings() Python-версии:
//! - Linux/macOS, а также при --config-dir на любой ОС: INI-файл (`[General]`, `ключ=значение`;
//!   bool — "true"/"false", числа — текстом, строки с запятой/спецсимволами — в кавычках, как пишет QSettings);
//! - Windows без --config-dir: реестр HKCU\Software\yamusic\YaMusic (строки, bool и float — REG_SZ,
//!   int — REG_DWORD; при чтении REG_SZ числа тоже принимаются).
//!
//! Чтение как `_read` в Python: нет значения или не приводится к типу — значение по умолчанию.
//! Запись как `_write`: если новое значение равно текущему — ничего не происходит и событие не шлётся.
//! Все методы потокобезопасны; запись на диск — сразу (синхронно, файл маленький).
//!
//! Windows: `get(WindowButtons)` всегда true (кнопки окна нужны без системной рамки); запись сохраняется,
//! сравнение при записи идёт с сохранённым значением (`get_stored`).
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

/// Все ключи. Порядок и имена — как SCHEMA в settings.py.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SettingKey {
    // Внешний вид
    ThemeMode,
    SchemeVariant,
    AccentFromCover,
    CustomSeed,
    WindowButtons,
    TrackTransition,
    BackdropMode,
    NowPlayingBlur,
    NowPlayingDrift,
    AmbientBackground,
    CoverSpin,
    CoverPulse,
    CalmWhenInactive,
    // Раскладка
    RightPanelOpen,
    RightPanelTab,
    SidebarCollapsed,
    WindowWidth,
    WindowHeight,
    // Воспроизведение
    Quality,
    Volume,
    Crossfade,
    WaveSeeds,
    // Поведение
    CloseToTray,
    WheelStep,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingValue {
    Str(String),
    Bool(bool),
    Int(i64),
    Float(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingType {
    Str,
    Bool,
    Int,
    Float,
}

impl SettingKey {
    pub const ALL: [SettingKey; 24] = [
        SettingKey::ThemeMode,
        SettingKey::SchemeVariant,
        SettingKey::AccentFromCover,
        SettingKey::CustomSeed,
        SettingKey::WindowButtons,
        SettingKey::TrackTransition,
        SettingKey::BackdropMode,
        SettingKey::NowPlayingBlur,
        SettingKey::NowPlayingDrift,
        SettingKey::AmbientBackground,
        SettingKey::CoverSpin,
        SettingKey::CoverPulse,
        SettingKey::CalmWhenInactive,
        SettingKey::RightPanelOpen,
        SettingKey::RightPanelTab,
        SettingKey::SidebarCollapsed,
        SettingKey::WindowWidth,
        SettingKey::WindowHeight,
        SettingKey::Quality,
        SettingKey::Volume,
        SettingKey::Crossfade,
        SettingKey::WaveSeeds,
        SettingKey::CloseToTray,
        SettingKey::WheelStep,
    ];

    /// Имя ключа в хранилище и в QML ("themeMode", …).
    pub fn name(self) -> &'static str {
        use SettingKey::*;
        match self {
            ThemeMode => "themeMode",
            SchemeVariant => "schemeVariant",
            AccentFromCover => "accentFromCover",
            CustomSeed => "customSeed",
            WindowButtons => "windowButtons",
            TrackTransition => "trackTransition",
            BackdropMode => "backdropMode",
            NowPlayingBlur => "nowPlayingBlur",
            NowPlayingDrift => "nowPlayingDrift",
            AmbientBackground => "ambientBackground",
            CoverSpin => "coverSpin",
            CoverPulse => "coverPulse",
            CalmWhenInactive => "calmWhenInactive",
            RightPanelOpen => "rightPanelOpen",
            RightPanelTab => "rightPanelTab",
            SidebarCollapsed => "sidebarCollapsed",
            WindowWidth => "windowWidth",
            WindowHeight => "windowHeight",
            Quality => "quality",
            Volume => "volume",
            Crossfade => "crossfade",
            WaveSeeds => "waveSeeds",
            CloseToTray => "closeToTray",
            WheelStep => "wheelStep",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }

    pub fn value_type(self) -> SettingType {
        match self.default_value() {
            SettingValue::Str(_) => SettingType::Str,
            SettingValue::Bool(_) => SettingType::Bool,
            SettingValue::Int(_) => SettingType::Int,
            SettingValue::Float(_) => SettingType::Float,
        }
    }

    /// Значение по умолчанию (таблица в docs/RUST_CONTRACT.md §6).
    pub fn default_value(self) -> SettingValue {
        use SettingKey::*;
        use SettingValue::{Bool, Float, Int, Str};
        match self {
            ThemeMode => Str("dark".into()),
            SchemeVariant => Str("content".into()),
            AccentFromCover => Bool(true),
            CustomSeed => Str("#FFCC00".into()),
            WindowButtons => Bool(false),
            TrackTransition => Str("wipe".into()),
            BackdropMode => Str("gauss".into()),
            NowPlayingBlur => Float(1.0),
            NowPlayingDrift => Float(0.5),
            AmbientBackground => Bool(true),
            CoverSpin => Bool(true),
            CoverPulse => Bool(true),
            CalmWhenInactive => Bool(true),
            RightPanelOpen => Bool(false),
            RightPanelTab => Str("queue".into()),
            SidebarCollapsed => Bool(false),
            WindowWidth => Int(1280),
            WindowHeight => Int(800),
            Quality => Str("lossless".into()),
            Volume => Float(0.7),
            Crossfade => Int(4),
            WaveSeeds => Str(String::new()),
            CloseToTray => Bool(true),
            WheelStep => Int(150),
        }
    }
}

impl SettingValue {
    pub fn value_type(&self) -> SettingType {
        match self {
            SettingValue::Str(_) => SettingType::Str,
            SettingValue::Bool(_) => SettingType::Bool,
            SettingValue::Int(_) => SettingType::Int,
            SettingValue::Float(_) => SettingType::Float,
        }
    }

    /// Привести к типу, как `typ(value)` в Python (строковый bool из INI — `value.lower() == "true"`).
    /// None — не приводится.
    pub fn coerce(&self, to: SettingType) -> Option<SettingValue> {
        use SettingValue::*;
        Some(match (to, self) {
            (SettingType::Str, Str(s)) => Str(s.clone()),
            (SettingType::Str, Bool(b)) => Str(if *b { "True" } else { "False" }.into()),
            (SettingType::Str, Int(i)) => Str(i.to_string()),
            (SettingType::Str, Float(f)) => Str(py_float_str(*f)),
            (SettingType::Bool, Str(s)) => Bool(s.eq_ignore_ascii_case("true")),
            (SettingType::Bool, Bool(b)) => Bool(*b),
            (SettingType::Bool, Int(i)) => Bool(*i != 0),
            (SettingType::Bool, Float(f)) => Bool(*f != 0.0),
            (SettingType::Int, Str(s)) => Int(s.trim().parse().ok()?),
            (SettingType::Int, Bool(b)) => Int(i64::from(*b)),
            (SettingType::Int, Int(i)) => Int(*i),
            (SettingType::Int, Float(f)) => {
                if !f.is_finite() {
                    return None;
                }
                Int(f.trunc() as i64)
            }
            (SettingType::Float, Str(s)) => Float(s.trim().parse().ok()?),
            (SettingType::Float, Bool(b)) => Float(if *b { 1.0 } else { 0.0 }),
            (SettingType::Float, Int(i)) => Float(*i as f64),
            (SettingType::Float, Float(f)) => Float(*f),
        })
    }

    /// Текст для INI/REG_SZ — как QVariant::toString у QSettings.
    fn to_qt_string(&self) -> String {
        match self {
            SettingValue::Str(s) => s.clone(),
            SettingValue::Bool(b) => b.to_string(),
            SettingValue::Int(i) => i.to_string(),
            SettingValue::Float(f) => qt_float_str(*f),
        }
    }
}

/// str(float) в Python: 1.0 → "1.0".
fn py_float_str(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

/// QString::number(d, 'g', FloatingPointShortest): 1.0 → "1", 0.7 → "0.7".
fn qt_float_str(f: f64) -> String {
    if f.is_nan() {
        "nan".into()
    } else if f.is_infinite() {
        if f > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        f.to_string()
    }
}

/// Хранилище настроек. Клонируйте Arc<SettingsStore>.
pub struct SettingsStore {
    backend: Box<dyn Backend>,
    changes: broadcast::Sender<SettingKey>,
    write_lock: Mutex<()>,
}

/// Низкоуровневое хранилище строк/чисел (INI или реестр).
pub trait Backend: Send + Sync {
    fn read(&self, name: &str) -> Option<SettingValue>;
    fn write(&self, name: &str, value: &SettingValue) -> std::io::Result<()>;
}

impl SettingsStore {
    /// Хранилище по умолчанию для ОС; `config_override` — каталог из --config-dir.
    pub fn open(config_override: Option<PathBuf>) -> Arc<Self> {
        #[cfg(target_os = "windows")]
        if config_override.is_none() {
            return Self::with_backend(Box::new(registry::RegistryBackend::new()));
        }
        let path = crate::dirs::settings_ini_file(config_override.as_deref());
        Self::with_backend(Box::new(IniBackend::open(path)))
    }

    /// Хранилище в памяти (тесты).
    pub fn in_memory() -> Arc<Self> {
        Self::with_backend(Box::new(MemoryBackend::default()))
    }

    /// Своё хранилище (тесты, особые случаи).
    pub fn with_backend(backend: Box<dyn Backend>) -> Arc<Self> {
        let (changes, _) = broadcast::channel(64);
        Arc::new(Self { backend, changes, write_lock: Mutex::new(()) })
    }

    pub fn get(&self, key: SettingKey) -> SettingValue {
        if cfg!(target_os = "windows") && key == SettingKey::WindowButtons {
            return SettingValue::Bool(true);
        }
        self.get_stored(key)
    }

    /// Сохранённое значение без платформенных подмен (windowButtons на Windows).
    pub fn get_stored(&self, key: SettingKey) -> SettingValue {
        self.backend
            .read(key.name())
            .and_then(|value| value.coerce(key.value_type()))
            .unwrap_or_else(|| key.default_value())
    }

    pub fn get_str(&self, key: SettingKey) -> String {
        match self.get(key).coerce(SettingType::Str) {
            Some(SettingValue::Str(s)) => s,
            _ => String::new(),
        }
    }

    pub fn get_bool(&self, key: SettingKey) -> bool {
        matches!(self.get(key).coerce(SettingType::Bool), Some(SettingValue::Bool(true)))
    }

    pub fn get_int(&self, key: SettingKey) -> i64 {
        match self.get(key).coerce(SettingType::Int) {
            Some(SettingValue::Int(i)) => i,
            _ => 0,
        }
    }

    pub fn get_f64(&self, key: SettingKey) -> f64 {
        match self.get(key).coerce(SettingType::Float) {
            Some(SettingValue::Float(f)) => f,
            _ => 0.0,
        }
    }

    /// Записать; true — значение изменилось (и разослано подписчикам). Значение приводится к типу ключа.
    pub fn set(&self, key: SettingKey, value: SettingValue) -> bool {
        let Some(value) = value.coerce(key.value_type()) else {
            tracing::warn!("настройка {}: значение {value:?} не приводится к типу", key.name());
            return false;
        };
        {
            let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
            if self.get_stored(key) == value {
                return false;
            }
            if let Err(err) = self.backend.write(key.name(), &value) {
                tracing::warn!("настройка {} не сохранена: {err}", key.name());
                return false;
            }
        }
        let _ = self.changes.send(key);
        true
    }

    /// Изменения от любого источника (мост Settings, Player.volume, Wave.select…).
    pub fn subscribe(&self) -> broadcast::Receiver<SettingKey> {
        self.changes.subscribe()
    }
}

/// В памяти.
#[derive(Default)]
pub struct MemoryBackend {
    values: Mutex<HashMap<String, SettingValue>>,
}

impl Backend for MemoryBackend {
    fn read(&self, name: &str) -> Option<SettingValue> {
        self.values.lock().unwrap_or_else(|e| e.into_inner()).get(name).cloned()
    }

    fn write(&self, name: &str, value: &SettingValue) -> std::io::Result<()> {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_owned(), value.clone());
        Ok(())
    }
}

/// INI-файл в формате QSettings. Чужие секции и ключи сохраняются при перезаписи.
pub struct IniBackend {
    path: PathBuf,
    doc: Mutex<ini::Document>,
}

impl IniBackend {
    pub fn open(path: PathBuf) -> Self {
        let text = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("настройки {} не прочитаны: {err}", path.display());
                }
                String::new()
            }
        };
        Self { path, doc: Mutex::new(ini::Document::parse(&text)) }
    }
}

impl Backend for IniBackend {
    fn read(&self, name: &str) -> Option<SettingValue> {
        let doc = self.doc.lock().unwrap_or_else(|e| e.into_inner());
        let raw = doc.get(name)?;
        ini::decode_value(raw).map(SettingValue::Str)
    }

    fn write(&self, name: &str, value: &SettingValue) -> std::io::Result<()> {
        let mut doc = self.doc.lock().unwrap_or_else(|e| e.into_inner());
        let encoded = match value {
            SettingValue::Str(s) => ini::encode_string(&escape_at(s)),
            other => other.to_qt_string(),
        };
        doc.set(name, &encoded);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // атомарная замена: сначала временный файл рядом
        let tmp = self.path.with_extension("conf.tmp");
        std::fs::write(&tmp, doc.render())?;
        std::fs::rename(&tmp, &self.path)
    }
}

/// QSettings: строка, начинающаяся с '@', пишется с удвоенным '@'.
fn escape_at(s: &str) -> String {
    if s.starts_with('@') { format!("@{s}") } else { s.to_owned() }
}

/// Обратное к escape_at и разбор служебных форм QSettings ("@Invalid()", "@ByteArray(...)").
fn unescape_at(s: String) -> Option<String> {
    if let Some(rest) = s.strip_prefix("@@") {
        return Some(format!("@{rest}"));
    }
    if let Some(inner) = s.strip_prefix("@ByteArray(").and_then(|r| r.strip_suffix(')')) {
        return Some(inner.to_owned());
    }
    if s.starts_with('@') && s.ends_with(')') && s[1..].chars().next().is_some_and(char::is_alphabetic) {
        // @Invalid(), @Variant(...) и прочие типы Qt — не наши значения
        return None;
    }
    Some(s)
}

mod ini {
    //! Разбор и запись INI так, как это делает QSettings (iniEscapedString / iniUnescapedStringList).

    #[derive(Debug, Default)]
    pub struct Document {
        /// Строки до первой секции (QSettings считает их General).
        preamble: Vec<String>,
        sections: Vec<Section>,
    }

    #[derive(Debug)]
    struct Section {
        name: String,
        lines: Vec<String>,
    }

    fn header_name(line: &str) -> Option<&str> {
        let t = line.trim();
        t.strip_prefix('[')?.strip_suffix(']').map(str::trim)
    }

    fn split_entry(line: &str) -> Option<(&str, &str)> {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with(';') || t.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        Some((key.trim(), value))
    }

    impl Document {
        pub fn parse(text: &str) -> Self {
            let mut doc = Document::default();
            // продолжение строки обратным слешем QSettings не пишет — не поддерживаем
            for line in text.lines() {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if let Some(name) = header_name(line) {
                    doc.sections.push(Section { name: name.to_owned(), lines: Vec::new() });
                } else if let Some(section) = doc.sections.last_mut() {
                    section.lines.push(line.to_owned());
                } else {
                    doc.preamble.push(line.to_owned());
                }
            }
            doc
        }

        fn general_lines(&self) -> impl Iterator<Item = &String> {
            self.preamble.iter().chain(
                self.sections
                    .iter()
                    .filter(|s| s.name.eq_ignore_ascii_case("General"))
                    .flat_map(|s| s.lines.iter()),
            )
        }

        /// Сырое значение (после '=') ключа секции General; последнее вхождение побеждает.
        pub fn get(&self, key: &str) -> Option<&str> {
            self.general_lines()
                .filter_map(|line| split_entry(line))
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v)
                .last()
        }

        pub fn set(&mut self, key: &str, encoded: &str) {
            let entry = format!("{key}={encoded}");
            let replace = |lines: &mut Vec<String>| -> bool {
                let mut found = false;
                lines.retain_mut(|line| {
                    if split_entry(line).is_some_and(|(k, _)| k == key) {
                        if found {
                            return false;
                        }
                        found = true;
                        *line = entry.clone();
                    }
                    true
                });
                found
            };
            if replace(&mut self.preamble) {
                return;
            }
            for section in self.sections.iter_mut().filter(|s| s.name.eq_ignore_ascii_case("General")) {
                if replace(&mut section.lines) {
                    return;
                }
            }
            match self.sections.iter_mut().find(|s| s.name.eq_ignore_ascii_case("General")) {
                Some(section) => {
                    // перед хвостовыми пустыми строками секции
                    let at = section
                        .lines
                        .iter()
                        .rposition(|l| !l.trim().is_empty())
                        .map_or(0, |i| i + 1);
                    section.lines.insert(at, entry);
                }
                None => {
                    let mut lines = vec![entry];
                    if !self.sections.is_empty() {
                        lines.push(String::new());
                    }
                    self.sections.insert(0, Section { name: "General".into(), lines });
                }
            }
        }

        pub fn render(&self) -> String {
            let mut out = String::new();
            for line in &self.preamble {
                out.push_str(line);
                out.push('\n');
            }
            for section in &self.sections {
                out.push('[');
                out.push_str(&section.name);
                out.push_str("]\n");
                for line in &section.lines {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out
        }
    }

    /// iniEscapedString из qsettings.cpp.
    pub fn encode_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        let mut needs_quotes = false;
        let mut escape_next_if_digit = false;
        for ch in s.chars() {
            if matches!(ch, ';' | ',' | '=') {
                needs_quotes = true;
            }
            if escape_next_if_digit && ch.is_ascii_hexdigit() {
                out.push_str(&format!("\\x{:x}", ch as u32));
                continue;
            }
            escape_next_if_digit = false;
            match ch {
                '\0' => {
                    out.push_str("\\0");
                    escape_next_if_digit = true;
                }
                '\u{7}' => out.push_str("\\a"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{b}' => out.push_str("\\v"),
                '"' | '\\' => {
                    out.push('\\');
                    out.push(ch);
                }
                c if (c as u32) <= 0x1f => {
                    out.push_str(&format!("\\x{:x}", c as u32));
                    escape_next_if_digit = true;
                }
                c => out.push(c),
            }
        }
        if needs_quotes || out.starts_with(' ') || out.ends_with(' ') {
            format!("\"{out}\"")
        } else {
            out
        }
    }

    /// iniUnescapedStringList: кавычки, escape-последовательности, запятые вне кавычек — список.
    /// Список склеивается обратно через ',' (так его записала бы рука человека).
    pub fn decode_parts(raw: &str) -> Vec<String> {
        // (символ, защищён ли от обрезки пробелов)
        let mut parts: Vec<Vec<(char, bool)>> = vec![Vec::new()];
        let mut in_quotes = false;
        let mut chars = raw.chars().peekable();
        while let Some(ch) = chars.next() {
            let cur = parts.last_mut().expect("непустой");
            match ch {
                '"' => in_quotes = !in_quotes,
                '\\' => {
                    let Some(next) = chars.next() else { break };
                    let c = match next {
                        'a' => '\u{7}',
                        'b' => '\u{8}',
                        'f' => '\u{c}',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'v' => '\u{b}',
                        'x' => {
                            let mut code = 0u32;
                            while let Some(d) = chars.peek().and_then(|c| c.to_digit(16)) {
                                code = code.saturating_mul(16).saturating_add(d);
                                chars.next();
                            }
                            char::from_u32(code).unwrap_or('\u{fffd}')
                        }
                        '0'..='7' => {
                            let mut code = next.to_digit(8).unwrap_or(0);
                            while let Some(d) = chars.peek().and_then(|c| c.to_digit(8)) {
                                code = code.saturating_mul(8).saturating_add(d);
                                chars.next();
                            }
                            char::from_u32(code).unwrap_or('\u{fffd}')
                        }
                        other => other,
                    };
                    cur.push((c, true));
                }
                ',' if !in_quotes => parts.push(Vec::new()),
                ';' if !in_quotes => break,
                c => cur.push((c, in_quotes)),
            }
        }
        parts
            .into_iter()
            .map(|part| {
                let start = part.iter().position(|&(c, p)| p || !c.is_whitespace()).unwrap_or(part.len());
                let end = part.iter().rposition(|&(c, p)| p || !c.is_whitespace()).map_or(start, |i| i + 1);
                part[start..end.max(start)].iter().map(|&(c, _)| c).collect()
            })
            .collect()
    }

    /// Значение строкой; None — служебное значение Qt (@Invalid() и т. п.).
    pub fn decode_value(raw: &str) -> Option<String> {
        super::unescape_at(decode_parts(raw).join(","))
    }
}

#[cfg(target_os = "windows")]
mod registry {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, RegType};

    use super::{Backend, SettingValue};

    const KEY_PATH: &str = r"Software\yamusic\YaMusic";

    /// HKCU\<path>; по умолчанию — ключ QSettings Python-версии.
    pub struct RegistryBackend {
        path: String,
    }

    impl RegistryBackend {
        pub fn new() -> Self {
            Self::with_path(KEY_PATH)
        }

        pub fn with_path(path: &str) -> Self {
            Self { path: path.to_owned() }
        }
    }

    impl Backend for RegistryBackend {
        fn read(&self, name: &str) -> Option<SettingValue> {
            let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(&self.path).ok()?;
            let raw = key.get_raw_value(name).ok()?;
            match raw.vtype {
                RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
                    let s: String = key.get_value(name).ok()?;
                    super::unescape_at(s).map(SettingValue::Str)
                }
                RegType::REG_MULTI_SZ => {
                    let list: Vec<String> = key.get_value(name).ok()?;
                    Some(SettingValue::Str(list.join(",")))
                }
                // QSettings читает REG_DWORD как int (со знаком)
                RegType::REG_DWORD => {
                    let v: u32 = key.get_value(name).ok()?;
                    Some(SettingValue::Int(i64::from(v as i32)))
                }
                RegType::REG_QWORD => {
                    let v: u64 = key.get_value(name).ok()?;
                    Some(SettingValue::Int(v as i64))
                }
                _ => None,
            }
        }

        fn write(&self, name: &str, value: &SettingValue) -> std::io::Result<()> {
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(&self.path)?;
            match value {
                SettingValue::Int(i) => match i32::try_from(*i) {
                    Ok(v) => key.set_value(name, &(v as u32)),
                    Err(_) => key.set_value(name, &(*i as u64)),
                },
                SettingValue::Str(s) => key.set_value(name, &super::escape_at(s)),
                other => key.set_value(name, &other.to_qt_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema() {
        assert_eq!(SettingKey::ALL.len(), 24);
        for key in SettingKey::ALL {
            assert_eq!(SettingKey::from_name(key.name()), Some(key));
            assert_eq!(key.default_value().value_type(), key.value_type());
        }
        assert_eq!(SettingKey::from_name("nope"), None);
        assert_eq!(SettingKey::Volume.default_value(), SettingValue::Float(0.7));
        assert_eq!(SettingKey::WheelStep.value_type(), SettingType::Int);
    }

    #[test]
    fn coerce_like_python() {
        use SettingValue::*;
        assert_eq!(Str("TRUE".into()).coerce(SettingType::Bool), Some(Bool(true)));
        assert_eq!(Str("1".into()).coerce(SettingType::Bool), Some(Bool(false)));
        assert_eq!(Int(1).coerce(SettingType::Bool), Some(Bool(true)));
        assert_eq!(Str(" 12 ".into()).coerce(SettingType::Int), Some(Int(12)));
        assert_eq!(Str("1.5".into()).coerce(SettingType::Int), None);
        assert_eq!(Float(1.9).coerce(SettingType::Int), Some(Int(1)));
        assert_eq!(Str("0.7".into()).coerce(SettingType::Float), Some(Float(0.7)));
        assert_eq!(Str("x".into()).coerce(SettingType::Float), None);
        assert_eq!(Int(4).coerce(SettingType::Str), Some(Str("4".into())));
        assert_eq!(Float(1.0).coerce(SettingType::Str), Some(Str("1.0".into())));
    }

    #[test]
    fn memory_store() {
        let store = SettingsStore::in_memory();
        let mut rx = store.subscribe();
        assert_eq!(store.get_str(SettingKey::ThemeMode), "dark");
        assert_eq!(store.get_int(SettingKey::WindowWidth), 1280);
        assert!((store.get_f64(SettingKey::Volume) - 0.7).abs() < 1e-12);
        assert!(!store.set(SettingKey::ThemeMode, SettingValue::Str("dark".into())));
        assert!(rx.try_recv().is_err());
        assert!(store.set(SettingKey::ThemeMode, SettingValue::Str("light".into())));
        assert_eq!(rx.try_recv().unwrap(), SettingKey::ThemeMode);
        assert_eq!(store.get_str(SettingKey::ThemeMode), "light");
        // приведение к типу ключа
        assert!(store.set(SettingKey::Crossfade, SettingValue::Float(2.0)));
        assert_eq!(store.get(SettingKey::Crossfade), SettingValue::Int(2));
        assert!(!store.set(SettingKey::Crossfade, SettingValue::Str("2".into())));
        assert!(!store.set(SettingKey::Volume, SettingValue::Str("громко".into())));
    }

    #[test]
    fn window_buttons() {
        let store = SettingsStore::in_memory();
        assert!(!store.get_stored(SettingKey::WindowButtons).eq(&SettingValue::Bool(true)));
        assert_eq!(store.get_bool(SettingKey::WindowButtons), cfg!(target_os = "windows"));
        assert!(store.set(SettingKey::WindowButtons, SettingValue::Bool(true)));
        assert!(store.get_bool(SettingKey::WindowButtons));
        assert!(store.set(SettingKey::WindowButtons, SettingValue::Bool(false)));
        assert_eq!(store.get_stored(SettingKey::WindowButtons), SettingValue::Bool(false));
    }

    #[test]
    fn ini_escape() {
        assert_eq!(ini::encode_string("plain"), "plain");
        assert_eq!(ini::encode_string("a,b"), "\"a,b\"");
        assert_eq!(ini::encode_string(" pad"), "\" pad\"");
        assert_eq!(ini::encode_string("q\"\\"), "q\\\"\\\\");
        assert_eq!(ini::encode_string("x\ty"), "x\\ty");
        assert_eq!(ini::encode_string("\u{1}a"), "\\x1\\x61");
        assert_eq!(ini::encode_string("тест"), "тест");
        for s in ["plain", "a,b", " pad ", "q\"\\", "x\ty\n", "\u{1}abc", "тест; ок", "", "k=v"] {
            assert_eq!(ini::decode_value(&ini::encode_string(s)).as_deref(), Some(s), "{s:?}");
        }
        // как читает QSettings
        assert_eq!(ini::decode_value("  spaced  ").as_deref(), Some("spaced"));
        assert_eq!(ini::decode_value("a, b").as_deref(), Some("a,b"));
        assert_eq!(ini::decode_value("v ; comment").as_deref(), Some("v"));
        assert_eq!(ini::decode_value("@Invalid()"), None);
        assert_eq!(ini::decode_value("@@at").as_deref(), Some("@at"));
    }

    fn temp_ini(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("ym-platform-test-{}-{name}", std::process::id()))
            .join("yamusic")
            .join("YaMusic.conf")
    }

    #[test]
    fn ini_python_compat() {
        let path = temp_ini("compat");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // так файл пишет QSettings из Python-версии
        let python = "[General]\naccentFromCover=false\ncrossfade=0\ncustomSeed=#FF0000\nvolume=0.35\n\
                      waveSeeds=\"genre:rock,mood:sad\"\nwindowWidth=oops\n\n[Other]\nkeep=me\n";
        std::fs::write(&path, python).unwrap();
        let store = SettingsStore::with_backend(Box::new(IniBackend::open(path.clone())));
        assert!(!store.get_bool(SettingKey::AccentFromCover));
        assert_eq!(store.get_int(SettingKey::Crossfade), 0);
        assert_eq!(store.get_str(SettingKey::CustomSeed), "#FF0000");
        assert!((store.get_f64(SettingKey::Volume) - 0.35).abs() < 1e-12);
        assert_eq!(store.get_str(SettingKey::WaveSeeds), "genre:rock,mood:sad");
        assert_eq!(store.get_int(SettingKey::WindowWidth), 1280);

        assert!(store.set(SettingKey::Volume, SettingValue::Float(0.5)));
        assert!(store.set(SettingKey::WaveSeeds, SettingValue::Str("a,b".into())));
        assert!(store.set(SettingKey::ThemeMode, SettingValue::Str("light".into())));
        assert!(store.set(SettingKey::NowPlayingBlur, SettingValue::Float(0.0)));
        assert!(store.set(SettingKey::CoverSpin, SettingValue::Bool(false)));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("volume=0.5\n"), "{text}");
        assert!(text.contains("waveSeeds=\"a,b\"\n"), "{text}");
        assert!(text.contains("themeMode=light\n"), "{text}");
        assert!(text.contains("nowPlayingBlur=0\n"), "{text}");
        assert!(text.contains("coverSpin=false\n"), "{text}");
        assert!(text.contains("[Other]\nkeep=me\n"), "{text}");
        let general_end = text.find("[Other]").unwrap();
        assert!(text[..general_end].contains("themeMode=light"));

        // перечитать с диска
        let again = SettingsStore::with_backend(Box::new(IniBackend::open(path.clone())));
        assert_eq!(again.get_str(SettingKey::WaveSeeds), "a,b");
        assert!((again.get_f64(SettingKey::Volume) - 0.5).abs() < 1e-12);
        assert_eq!(again.get_str(SettingKey::ThemeMode), "light");
        assert!(!again.get_bool(SettingKey::CoverSpin));
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn registry_python_compat() {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let path = format!(r"Software\yamusic\YaMusic-ym-platform-test-{}", std::process::id());
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        {
            // так значения пишет QSettings из Python-версии
            let (key, _) = hkcu.create_subkey(&path).unwrap();
            key.set_value("nowPlayingBlur", &"1".to_string()).unwrap();
            key.set_value("accentFromCover", &"false".to_string()).unwrap();
            key.set_value("windowWidth", &1600u32).unwrap();
            key.set_value("crossfade", &"0".to_string()).unwrap();
        }
        let store = SettingsStore::with_backend(Box::new(registry::RegistryBackend::with_path(&path)));
        assert!((store.get_f64(SettingKey::NowPlayingBlur) - 1.0).abs() < 1e-12);
        assert!(!store.get_bool(SettingKey::AccentFromCover));
        assert_eq!(store.get_int(SettingKey::WindowWidth), 1600);
        assert_eq!(store.get_int(SettingKey::Crossfade), 0);
        assert!(store.set(SettingKey::WindowHeight, SettingValue::Int(700)));
        assert!(store.set(SettingKey::Volume, SettingValue::Float(0.25)));
        assert!(store.set(SettingKey::CoverPulse, SettingValue::Bool(false)));
        assert!(store.set(SettingKey::CustomSeed, SettingValue::Str("@x".into())));
        let key = hkcu.open_subkey(&path).unwrap();
        assert_eq!(key.get_value::<u32, _>("windowHeight").unwrap(), 700);
        assert_eq!(key.get_value::<String, _>("volume").unwrap(), "0.25");
        assert_eq!(key.get_value::<String, _>("coverPulse").unwrap(), "false");
        assert_eq!(key.get_value::<String, _>("customSeed").unwrap(), "@@x");
        assert_eq!(store.get_str(SettingKey::CustomSeed), "@x");
        assert_eq!(store.get_int(SettingKey::WindowHeight), 700);
        hkcu.delete_subkey_all(&path).unwrap();
    }

    #[test]
    fn ini_new_file() {
        let path = temp_ini("new");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let store = SettingsStore::with_backend(Box::new(IniBackend::open(path.clone())));
        assert!(store.set(SettingKey::WindowHeight, SettingValue::Int(900)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[General]\nwindowHeight=900\n");
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }
}
