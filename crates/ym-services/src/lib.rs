//! Сервисы приложения (без Qt). Каждый сервис — Arc, состояние в watch-канале (снимок) + broadcast (события).
//! Мосты QML подписываются в cxx_qt::Initialize и переносят изменения в GUI-поток через CxxQtThread.
//! Всё, что в Python работало независимо от QML (учёт прослушиваний, обратная связь волне, MPRIS,
//! громкость/качество/переход из настроек), запускается здесь, в [`Services::start`].
pub mod auth;
pub mod catalog;
pub mod glue;
pub mod library;
pub mod lyrics;
pub mod reporter;
pub mod resolver;
pub mod trackfilter;
pub mod wave;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use ym_platform::{SettingKey, SettingsStore, TokenStore};
use ym_player::{PlayerConfig, PlayerHandle, Quality};

pub use auth::{Account, AuthEvent, AuthService, AuthSnapshot, AuthState};
pub use catalog::{AlbumPage, ArtistPage, CatalogService, SearchBest, SearchKind, SearchResults, SearchTotals};
pub use library::{FeedbackKind, LibraryEvent, LibraryService, PlaylistKey};
pub use lyrics::{LyricsService, LyricsSnapshot, LyricsStatus};
pub use wave::{WaveEvent, WaveGroup, WaveItem, WaveService, WaveSnapshot};

/// Изменение списка — мост применяет его к DictListModel (begin/endInsertRows и т. д.).
#[derive(Clone, Debug, PartialEq)]
pub enum ListChange<T> {
    Reset(Vec<T>),
    Append(Vec<T>),
    Insert { at: usize, items: Vec<T> },
    Remove { at: usize, count: usize },
    Loading(bool),
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Войдите в аккаунт")]
    NotSignedIn,
    #[error(transparent)]
    Api(#[from] ym_api::ApiError),
    #[error("{0}")]
    Other(String),
}

/// Просьбы к оболочке Qt (MPRIS Raise/Quit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppRequest {
    RaiseWindow,
    Quit,
}

#[derive(Clone, Debug, Default)]
pub struct ServicesOptions {
    /// очередь и позиция прошлого запуска (false в режиме снимков без YAMUSIC_SESSION_FILE)
    pub persist_session: bool,
    /// MPRIS (только Linux; false в режиме снимков)
    pub enable_mpris: bool,
    /// $YAMUSIC_NO_REPORT: не писать play-audio и не слать обратную связь волне (только лог)
    pub no_report: bool,
}

/// Все сервисы. Clone — дешёвый (Arc).
#[derive(Clone)]
pub struct Services {
    pub settings: Arc<SettingsStore>,
    pub auth: Arc<AuthService>,
    pub library: Arc<LibraryService>,
    pub catalog: Arc<CatalogService>,
    pub wave: Arc<WaveService>,
    pub lyrics: Arc<LyricsService>,
    pub player: ym_player::PlayerHandle,
    app_requests: broadcast::Sender<AppRequest>,
    /// остановка MPRIS при выходе (Some — сервер запускался)
    mpris: Arc<Mutex<Option<glue::MprisStop>>>,
}

impl Services {
    /// Создать сервисы и фоновые задачи на `rt`: плеер (libmpv; громкость/переход/качество — из настроек),
    /// restore_session (если persist_session), учёт прослушиваний, связки настроек с плеером, волну, тексты, MPRIS.
    /// Вход НЕ начинается — оболочка вызывает auth.start() после загрузки QML (если не --offline).
    pub fn start(rt: &Handle, settings: Arc<SettingsStore>, options: ServicesOptions) -> anyhow::Result<Services> {
        let auth = AuthService::new(rt.clone(), TokenStore::new());
        let library = LibraryService::new(rt.clone(), auth.clone());
        let catalog = CatalogService::new(auth.clone());

        let config = PlayerConfig {
            cover_dir: ym_platform::dirs::cache_subdir("covers"),
            session_file: options.persist_session.then(ym_platform::dirs::session_file),
            audio_output: env_nonempty("YAMUSIC_AO"),
            libmpv_path: env_nonempty("YAMUSIC_LIBMPV").map(Into::into),
            volume: settings.get_f64(SettingKey::Volume).clamp(0.0, 1.0),
            crossfade_s: settings.get_int(SettingKey::Crossfade).clamp(0, i64::from(u32::MAX)) as u32,
            quality: Quality::from_setting(&settings.get_str(SettingKey::Quality)),
        };
        let player = PlayerHandle::start(rt, config, resolver::ApiResolver::new(auth.clone()))
            .context("не удалось запустить плеер")?;
        if options.persist_session {
            player.restore_session();
        }

        let wave = WaveService::new(
            rt.clone(),
            auth.clone(),
            player.clone(),
            library.clone(),
            settings.clone(),
            options.no_report,
        );
        let fetcher = ym_lyrics::LyricsFetcher::new(ym_platform::dirs::cache_subdir("lyrics"));
        let lyrics = LyricsService::new(rt.clone(), auth.clone(), player.clone(), fetcher);
        reporter::spawn(rt, auth.clone(), player.clone(), options.no_report);
        glue::spawn_settings_glue(rt, settings.clone(), player.clone());

        let (app_requests, _) = broadcast::channel(16);
        let mpris = if options.enable_mpris && cfg!(target_os = "linux") {
            Some(glue::start_mpris(rt, settings.clone(), player.clone(), app_requests.clone()))
        } else {
            None
        };

        Ok(Services {
            settings,
            auth,
            library,
            catalog,
            wave,
            lyrics,
            player,
            app_requests,
            mpris: Arc::new(Mutex::new(mpris)),
        })
    }

    pub fn app_requests(&self) -> broadcast::Receiver<AppRequest> {
        self.app_requests.subscribe()
    }

    /// При выходе: сохранить сессию плеера, остановить mpv, закрыть MPRIS. Блокирует ≤ 2 с.
    pub fn shutdown(&self) {
        self.player.shutdown();
        if let Some(stop) = lock(&self.mpris).take() {
            stop.stop(Duration::from_millis(500));
        }
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Mutex без паники на «отравлении»: состояние сервисов всегда целостно между шагами.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
