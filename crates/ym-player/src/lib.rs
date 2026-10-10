//! Плеер (src/yamusic/player.py) без Qt: очередь, libmpv (загружается динамически), gapless,
//! плавный переход вторым экземпляром mpv, уровень звука, сохранение сессии.
//!
//! Модель потоков:
//! - актор — tokio-задача, владеет [`state::PlayerState`]; команды приходят через mpsc из [`PlayerHandle`];
//! - команды libmpv выполняются по порядку в отдельном потоке «mpv-cmd» (libmpv может блокировать);
//! - у каждого экземпляра mpv свой поток mpv_wait_event → события в актор;
//! - наружу: watch<[`PlayerSnapshot`]> (свойства) + broadcast<[`PlayerEvent`]> (что изменилось, изменения очереди,
//!   trackStarted/trackEnded, запрос догрузки волны).
pub mod engine;
pub mod mpv;
pub mod session;
pub mod state;

use std::path::PathBuf;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::{broadcast, watch};
use ym_api::items::{PlayContext, TrackItem};

pub use engine::PlayerHandle;

/// Опрос позиции, мс (QML сглаживает прогресс между обновлениями)
pub const POSITION_POLL_MS: u64 = 250;
/// Задержка перед запросом ссылки на следующий трек, мс
pub const PREFETCH_DELAY_MS: u64 = 2500;
/// Шаг громкости плавного перехода, мс
pub const FADE_STEP_MS: u64 = 40;
/// Волна: сколько треков держать впереди текущего
pub const FEED_AHEAD: usize = 3;
/// Фильтр уровня для «дыхания» обложки (метаданные af-metadata/lvl)
pub const LEVEL_FILTER: &str = "@lvl:lavfi=[astats=metadata=1:reset=1]";
/// Сохранение сессии: через 2 с после изменения очереди/трека/режима и каждые 15 с, пока играет
pub const SAVE_DEBOUNCE_MS: u64 = 2000;
pub const AUTOSAVE_MS: u64 = 15000;

/// Качество (Settings.quality → параметр quality у get-file-info).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    /// "lossless" → lossless
    #[default]
    Lossless,
    /// "high" → hq
    High,
    /// "low" → nq
    Low,
}

impl Quality {
    /// Значение настройки: "lossless" | "high" | "low"; неизвестное → Lossless.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "high" => Self::High,
            "low" => Self::Low,
            _ => Self::Lossless,
        }
    }

    /// Параметр API: "lossless" | "hq" | "nq"
    pub fn api_value(self) -> &'static str {
        match self {
            Self::Lossless => "lossless",
            Self::High => "hq",
            Self::Low => "nq",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    /// "off" | "all" | "one"
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "all" => Some(Self::All),
            "one" => Some(Self::One),
            _ => None,
        }
    }
}

/// Прямая ссылка на трек.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamInfo {
    pub url: String,
    /// flac | flac-mp4 | aac-mp4 | he-aac-mp4 | aac | he-aac | mp3
    pub codec: String,
    pub bitrate: Option<i64>,
}

/// Подпись кодека для Player.codec: CODEC_LABEL[codec] или codec.upper(), + " <bitrate>" если bitrate > 0
/// (flac, flac-mp4 → FLAC; aac-mp4, aac → AAC; he-aac-mp4, he-aac → HE-AAC; mp3 → MP3).
pub fn codec_label(codec: &str, bitrate: Option<i64>) -> String {
    let mut label = match codec {
        "flac" | "flac-mp4" => "FLAC".to_owned(),
        "aac-mp4" | "aac" => "AAC".to_owned(),
        "he-aac-mp4" | "he-aac" => "HE-AAC".to_owned(),
        "mp3" => "MP3".to_owned(),
        other => other.to_uppercase(),
    };
    if let Some(bitrate) = bitrate.filter(|b| *b > 0) {
        label.push_str(&format!(" {bitrate}"));
    }
    label
}

/// Громкость ползунка 0..1 → громкость mpv: 100 · clamp(ui)^(2/3).
pub fn mpv_volume(ui: f64) -> f64 {
    100.0 * ui.clamp(0.0, 1.0).powf(2.0 / 3.0)
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("libmpv не найдена: {0}")]
    LibraryNotFound(String),
    #[error("libmpv: {0}")]
    Mpv(String),
    #[error("{0}")]
    Resolve(String),
}

/// Откуда брать ссылки: реализует ym-services поверх YmClient::tracks_file_info
/// (качество из настроек, при ошибке не-hq — повтор с "hq"; нет входа → Err("Войдите в аккаунт, чтобы слушать музыку")).
pub trait StreamResolver: Send + Sync + 'static {
    fn resolve(&self, track_id: String, quality: Quality) -> BoxFuture<'static, Result<StreamInfo, String>>;
}

/// Идентификатор «подкормки» очереди (волна): плеер шлёт PlayerEvent::NeedMore(id), когда впереди < FEED_AHEAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FeederId(pub u64);

#[derive(Clone, Debug)]
pub struct PlayerConfig {
    /// dirs::cache_subdir("covers"): обложка текущего трека → <sha1(url)>.jpg (для темы и MPRIS)
    pub cover_dir: PathBuf,
    /// None — не сохранять/не восстанавливать сессию (режим снимков без YAMUSIC_SESSION_FILE)
    pub session_file: Option<PathBuf>,
    /// $YAMUSIC_AO (например "null" в тестах)
    pub audio_output: Option<String>,
    /// путь к libmpv, если задан явно ($YAMUSIC_LIBMPV); иначе поиск см. mpv::load_library
    pub libmpv_path: Option<PathBuf>,
    pub volume: f64,
    pub crossfade_s: u32,
    pub quality: Quality,
}

/// Снимок всех свойств Player для QML (обновляется актором до рассылки событий).
#[derive(Clone, Debug, Default)]
pub struct PlayerSnapshot {
    /// текущий трек (None — нет трека); hasTrack/trackId/title/artist/album/cover/track — из него
    pub track: Option<TrackItem>,
    pub codec: String,
    pub cover_file: String,
    pub playing: bool,
    pub buffering: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// индекс текущего трека в очереди (не в порядке воспроизведения), -1 — нет
    pub current_index: i32,
    pub error_text: String,
    /// +1 — переход вперёд (следующий, новый список), -1 — назад
    pub direction: i32,
    pub level: f64,
    pub levels_wanted: bool,
    pub context: PlayContext,
    /// вся очередь (Player.queue); изменения по месту — PlayerEvent::Queue
    pub queue: Arc<Vec<TrackItem>>,
    /// сколько секунд текущего трека реально проиграно
    pub played_seconds: f64,
    pub feeder: Option<FeederId>,
}

impl PlayerSnapshot {
    pub fn has_track(&self) -> bool {
        self.track.is_some()
    }

    /// Player.source: context.type ("wave" | "liked" | "playlist" | …)
    pub fn source(&self) -> &str {
        &self.context.kind
    }

    /// Player.position: position_ms / duration_ms (0 при неизвестной длительности)
    pub fn position(&self) -> f64 {
        if self.duration_ms > 0 { self.position_ms as f64 / self.duration_ms as f64 } else { 0.0 }
    }
}

/// Изменение очереди — мост повторяет его в DictListModel, чтобы список не терял прокрутку.
#[derive(Clone, Debug)]
pub enum QueueChange {
    Reset(Arc<Vec<TrackItem>>),
    Append(Vec<TrackItem>),
    Remove { at: usize, count: usize },
}

/// {"track", "context"} — trackStarted в Python.
#[derive(Clone, Debug)]
pub struct TrackStarted {
    pub track: TrackItem,
    pub context: PlayContext,
}

/// {"track", "context", "played", "end", "natural"} — trackEnded (секунды, округление до 0,1).
#[derive(Clone, Debug)]
pub struct TrackEnded {
    pub track: TrackItem,
    pub context: PlayContext,
    pub played: f64,
    pub end: f64,
    /// доигран сам (или переход начался ≤ 3 с до конца), а не пропущен
    pub natural: bool,
}

/// События — по одному на каждый Qt-сигнал Player + служебные.
#[derive(Clone, Debug)]
pub enum PlayerEvent {
    TrackChanged,
    StateChanged,
    PositionChanged,
    VolumeChanged,
    ModeChanged,
    QueueChanged,
    CoverFileChanged,
    ErrorChanged,
    LevelChanged,
    Queue(QueueChange),
    TrackStarted(TrackStarted),
    TrackEnded(TrackEnded),
    NeedMore(FeederId),
}

/// Подписка: снимок + события. Отставание broadcast (Lagged) → перечитать snapshot и сбросить модель очереди.
pub struct PlayerSubscription {
    pub snapshot: watch::Receiver<PlayerSnapshot>,
    pub events: broadcast::Receiver<PlayerEvent>,
}

/// RMS-уровень astats (дБ) → 0..1 для «дыхания» обложки: clamp((db + 42) / 36), бесконечность → 0.
pub fn level_from_db(db: f64) -> f64 {
    if db.is_finite() { ((db + 42.0) / 36.0).clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_labels() {
        assert_eq!(codec_label("flac", None), "FLAC");
        assert_eq!(codec_label("flac-mp4", Some(0)), "FLAC");
        assert_eq!(codec_label("aac-mp4", Some(256)), "AAC 256");
        assert_eq!(codec_label("aac", Some(192)), "AAC 192");
        assert_eq!(codec_label("he-aac-mp4", Some(64)), "HE-AAC 64");
        assert_eq!(codec_label("he-aac", None), "HE-AAC");
        assert_eq!(codec_label("mp3", Some(320)), "MP3 320");
        assert_eq!(codec_label("opus", Some(-1)), "OPUS");
        assert_eq!(codec_label("", None), "");
    }

    #[test]
    fn volume_curve() {
        assert_eq!(mpv_volume(0.0), 0.0);
        assert_eq!(mpv_volume(1.0), 100.0);
        assert_eq!(mpv_volume(2.0), 100.0);
        assert_eq!(mpv_volume(-1.0), 0.0);
        // амплитуда = (vol/100)³ = ui²
        let ui: f64 = 0.5;
        assert!(((mpv_volume(ui) / 100.0).powi(3) - ui * ui).abs() < 1e-12);
        assert!((mpv_volume(0.7) - 100.0 * 0.7f64.powf(2.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn quality_and_repeat() {
        assert_eq!(Quality::from_setting("high").api_value(), "hq");
        assert_eq!(Quality::from_setting("low").api_value(), "nq");
        assert_eq!(Quality::from_setting("lossless").api_value(), "lossless");
        assert_eq!(Quality::from_setting("?"), Quality::Lossless);
        for r in [Repeat::Off, Repeat::All, Repeat::One] {
            assert_eq!(Repeat::from_str(r.as_str()), Some(r));
        }
        assert_eq!(Repeat::from_str("ALL"), None);
    }

    #[test]
    fn levels() {
        assert_eq!(level_from_db(f64::NEG_INFINITY), 0.0);
        assert_eq!(level_from_db(f64::INFINITY), 0.0);
        assert_eq!(level_from_db(-42.0), 0.0);
        assert_eq!(level_from_db(-6.0), 1.0);
        assert_eq!(level_from_db(0.0), 1.0);
        assert!((level_from_db(-24.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn snapshot_position() {
        let mut s = PlayerSnapshot::default();
        assert_eq!(s.position(), 0.0);
        s.duration_ms = 200;
        s.position_ms = 50;
        assert_eq!(s.position(), 0.25);
        assert!(!s.has_track());
    }
}
