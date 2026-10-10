//! Клиент API Яндекс Музыки (api.music.yandex.net) и модели данных.
//!
//! - [`client::YmClient`] — HTTP поверх reqwest: не больше 4 запросов разом, повтор 429/5xx;
//! - [`models`] — сырые ответы API (serde, все поля необязательные);
//! - [`items`] — плоские словари для QML (как в models.py) и контекст очереди;
//! - [`sign`] — подписи get-file-info, текстов и прямых ссылок; [`crypto`] — расшифровка encraw.
pub mod client;
pub mod crypto;
pub mod diff;
pub mod error;
pub mod items;
pub mod models;
pub mod sign;

pub use client::{
    ClientConfig, LyricsFormat, PlayAudio, RotorEvent, RotorFeedback, RotorSessionNew, SearchType, YmClient,
    utc_now_iso,
};
pub use diff::{DiffOp, DiffTrack};
pub use error::{ApiError, Result};
pub use items::{
    AlbumInfo, AlbumItem, ArtistInfo, ArtistItem, ArtistRef, PlayContext, PlaylistItem, TrackItem, cover_url,
};
