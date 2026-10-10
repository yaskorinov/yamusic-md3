//! Сырые ответы API. Всё — `#[serde(default)]`: Яндекс часто не присылает поля.
//! Идентификаторы бывают числом или строкой — [`IdString`] принимает оба.
//! Поля сверены с yandex_music (Python) и android/…/api/Dto.kt; при необходимости добавляйте поля, не меняя существующие.
//! Разбор снисходительный: числа строкой, `null` и битые элементы в списках не роняют весь ответ.
use std::fmt;

use serde::de::{self, DeserializeOwned, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Идентификатор, пришедший числом или строкой, — всегда строка.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct IdString(pub String);

impl IdString {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for IdString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl<'de> Deserialize<'de> for IdString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdVisitor;

        impl Visitor<'_> for IdVisitor {
            type Value = IdString;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("идентификатор: строка или число")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<IdString, E> {
                Ok(IdString(v.to_owned()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<IdString, E> {
                Ok(IdString(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<IdString, E> {
                Ok(IdString(v.to_string()))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<IdString, E> {
                Ok(IdString(v.to_string()))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<IdString, E> {
                // 123.0 → "123", как str(int) в Python
                if v.fract() == 0.0 && v.abs() < 1e18 {
                    Ok(IdString(format!("{}", v as i64)))
                } else {
                    Ok(IdString(v.to_string()))
                }
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<IdString, E> {
                Ok(IdString(v.to_string()))
            }
        }

        deserializer.deserialize_any(IdVisitor)
    }
}

// --- снисходительные десериализаторы ----------------------------------------------

/// Число, строка с числом или что угодно (→ None).
fn lenient_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(i64::from(b)),
        _ => None,
    })
}

/// Строка или число (→ строка); прочее — None.
fn lenient_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

/// Bool, 0/1 или "true"/"false".
fn lenient_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Bool(b) => Some(b),
        Value::Number(n) => n.as_i64().map(|n| n != 0),
        Value::String(s) => match s.as_str() {
            "true" | "True" | "1" => Some(true),
            "false" | "False" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

/// Список, из которого выброшены null и элементы, не подошедшие по типу.
fn lenient_vec<'de, D: Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<Option<Vec<T>>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Array(items) => Some(
            items
                .into_iter()
                .filter(|v| !v.is_null())
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect(),
        ),
        _ => None,
    })
}

/// Диски альбома: список списков треков.
fn lenient_volumes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<Vec<Track>>>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Array(volumes) => Some(
            volumes
                .into_iter()
                .filter_map(|volume| match volume {
                    Value::Array(tracks) => Some(
                        tracks
                            .into_iter()
                            .filter(|v| !v.is_null())
                            .filter_map(|v| serde_json::from_value(v).ok())
                            .collect(),
                    ),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    })
}

/// Вложенный объект; не тот тип — None.
fn lenient_obj<'de, D: Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<Option<T>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        v @ Value::Object(_) => serde_json::from_value(v).ok(),
        _ => None,
    })
}

// --- модели --------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Cover {
    #[serde(rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub uri: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub items_uri: Option<Vec<String>>,
}

impl Cover {
    /// uri || itemsUri[0]
    pub fn best_uri(&self) -> Option<&str> {
        self.uri
            .as_deref()
            .filter(|u| !u.is_empty())
            .or_else(|| self.items_uri.as_ref().and_then(|items| items.first()).map(String::as_str))
            .filter(|u| !u.is_empty())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Artist {
    pub id: Option<IdString>,
    #[serde(deserialize_with = "lenient_string")]
    pub name: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub cover: Option<Cover>,
    #[serde(deserialize_with = "lenient_string")]
    pub og_image: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub genres: Option<Vec<String>>,
    #[serde(deserialize_with = "lenient_i64")]
    pub likes_count: Option<i64>,
    #[serde(deserialize_with = "lenient_bool")]
    pub various: Option<bool>,
    #[serde(deserialize_with = "lenient_bool")]
    pub composer: Option<bool>,
    #[serde(deserialize_with = "lenient_bool")]
    pub available: Option<bool>,
}

/// Лейбл приходит строкой или объектом {id, name}.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Label {
    Name(String),
    Object { id: Option<IdString>, name: Option<String> },
}

impl Label {
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Name(name) => Some(name),
            Self::Object { name, .. } => name.as_deref(),
        }
    }
}

/// Позиция трека в альбоме (track.albums[i].trackPosition)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackPosition {
    #[serde(deserialize_with = "lenient_i64")]
    pub volume: Option<i64>,
    #[serde(deserialize_with = "lenient_i64")]
    pub index: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Album {
    pub id: Option<IdString>,
    #[serde(deserialize_with = "lenient_string")]
    pub title: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub version: Option<String>,
    #[serde(rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub artists: Option<Vec<Artist>>,
    #[serde(deserialize_with = "lenient_i64")]
    pub year: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub release_date: Option<String>,
    #[serde(deserialize_with = "lenient_i64")]
    pub track_count: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub cover_uri: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub og_image: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub content_warning: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub genre: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub labels: Option<Vec<Label>>,
    /// только у /albums/{id}/with-tracks: диски → треки
    #[serde(deserialize_with = "lenient_volumes")]
    pub volumes: Option<Vec<Vec<Track>>>,
    #[serde(deserialize_with = "lenient_bool")]
    pub available: Option<bool>,
    #[serde(deserialize_with = "lenient_i64")]
    pub likes_count: Option<i64>,
    /// только у альбомов внутри трека
    #[serde(deserialize_with = "lenient_obj")]
    pub track_position: Option<TrackPosition>,
}

impl Album {
    /// Все треки всех дисков по порядку.
    pub fn all_tracks(&self) -> impl Iterator<Item = &Track> {
        self.volumes.iter().flatten().flatten()
    }
}

/// Наличие текста у трека (track.lyricsInfo)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsInfo {
    #[serde(deserialize_with = "lenient_bool")]
    pub has_available_sync_lyrics: Option<bool>,
    #[serde(deserialize_with = "lenient_bool")]
    pub has_available_text_lyrics: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Track {
    pub id: Option<IdString>,
    #[serde(deserialize_with = "lenient_string")]
    pub title: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub version: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub artists: Option<Vec<Artist>>,
    #[serde(deserialize_with = "lenient_vec")]
    pub albums: Option<Vec<Album>>,
    #[serde(deserialize_with = "lenient_string")]
    pub cover_uri: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub og_image: Option<String>,
    #[serde(deserialize_with = "lenient_i64")]
    pub duration_ms: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub content_warning: Option<String>,
    #[serde(deserialize_with = "lenient_bool")]
    pub available: Option<bool>,
    pub real_id: Option<IdString>,
    /// music | podcast-episode | audiobook…
    #[serde(rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub lyrics_info: Option<LyricsInfo>,
}

impl Track {
    /// 'id:albumId' первого альбома или 'id'
    pub fn full_id(&self) -> String {
        let id = self.id.as_ref().map(IdString::as_str).unwrap_or_default();
        match self.albums.as_ref().and_then(|a| a.first()).and_then(|a| a.id.as_ref()) {
            Some(album) if !album.0.is_empty() => format!("{id}:{album}"),
            _ => id.to_owned(),
        }
    }
}

/// Элемент плейлиста / лайков: id (+ albumId) и, если повезло, трек целиком.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackShort {
    pub id: Option<IdString>,
    pub album_id: Option<IdString>,
    #[serde(deserialize_with = "lenient_string")]
    pub timestamp: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub track: Option<Track>,
}

impl TrackShort {
    /// 'id:albumId' или 'id'
    pub fn full_id(&self) -> String {
        let id = self.id.as_ref().map(IdString::as_str).unwrap_or_default();
        match &self.album_id {
            Some(album) if !album.0.is_empty() => format!("{id}:{album}"),
            _ => id.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Owner {
    #[serde(deserialize_with = "lenient_i64")]
    pub uid: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub login: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Playlist {
    #[serde(deserialize_with = "lenient_i64")]
    pub uid: Option<i64>,
    #[serde(deserialize_with = "lenient_i64")]
    pub kind: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub title: Option<String>,
    #[serde(deserialize_with = "lenient_i64")]
    pub revision: Option<i64>,
    #[serde(deserialize_with = "lenient_i64")]
    pub track_count: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub visibility: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub cover: Option<Cover>,
    #[serde(deserialize_with = "lenient_string")]
    pub og_image: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub owner: Option<Owner>,
    #[serde(deserialize_with = "lenient_string")]
    pub playlist_uuid: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub tracks: Option<Vec<TrackShort>>,
    #[serde(deserialize_with = "lenient_i64")]
    pub duration_ms: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub description: Option<String>,
}

impl Playlist {
    /// uid плейлиста || owner.uid
    pub fn owner_uid(&self) -> Option<i64> {
        self.uid.filter(|&u| u != 0).or_else(|| self.owner.as_ref().and_then(|o| o.uid))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    #[serde(deserialize_with = "lenient_i64")]
    pub uid: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub login: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub display_name: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub full_name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Plus {
    #[serde(deserialize_with = "lenient_bool")]
    pub has_plus: Option<bool>,
}

/// GET /account/status
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountStatus {
    #[serde(deserialize_with = "lenient_obj")]
    pub account: Option<Account>,
    #[serde(deserialize_with = "lenient_obj")]
    pub plus: Option<Plus>,
    #[serde(deserialize_with = "lenient_string")]
    pub default_email: Option<String>,
}

impl AccountStatus {
    pub fn uid(&self) -> Option<i64> {
        self.account.as_ref().and_then(|a| a.uid).filter(|&u| u != 0)
    }

    /// displayName || fullName || login (как в auth.py)
    pub fn display_name(&self) -> String {
        let Some(account) = &self.account else { return String::new() };
        [&account.display_name, &account.full_name, &account.login]
            .into_iter()
            .flatten()
            .find(|s| !s.is_empty())
            .cloned()
            .unwrap_or_default()
    }

    pub fn has_plus(&self) -> bool {
        self.plus.as_ref().and_then(|p| p.has_plus).unwrap_or(false)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, bound(deserialize = "T: DeserializeOwned + Default"))]
pub struct SearchBlock<T> {
    #[serde(deserialize_with = "lenient_i64")]
    pub total: Option<i64>,
    #[serde(deserialize_with = "lenient_vec")]
    pub results: Option<Vec<T>>,
}

impl<T> SearchBlock<T> {
    pub fn items(&self) -> &[T] {
        self.results.as_deref().unwrap_or_default()
    }
}

/// best.result — объект нужного типа (сырое значение; тип в best.type)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchBest {
    #[serde(rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    pub result: Option<serde_json::Value>,
}

/// GET /search
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Search {
    #[serde(deserialize_with = "lenient_string")]
    pub text: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub best: Option<SearchBest>,
    #[serde(deserialize_with = "lenient_obj")]
    pub tracks: Option<SearchBlock<Track>>,
    #[serde(deserialize_with = "lenient_obj")]
    pub albums: Option<SearchBlock<Album>>,
    #[serde(deserialize_with = "lenient_obj")]
    pub artists: Option<SearchBlock<Artist>>,
    #[serde(deserialize_with = "lenient_obj")]
    pub playlists: Option<SearchBlock<Playlist>>,
    #[serde(deserialize_with = "lenient_string")]
    pub search_request_id: Option<String>,
    #[serde(deserialize_with = "lenient_bool")]
    pub misspell_corrected: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistStats {
    #[serde(deserialize_with = "lenient_i64")]
    pub last_month_listeners: Option<i64>,
}

/// GET /artists/{id}/brief-info
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistBriefInfo {
    #[serde(deserialize_with = "lenient_obj")]
    pub artist: Option<Artist>,
    #[serde(deserialize_with = "lenient_vec")]
    pub albums: Option<Vec<Album>>,
    #[serde(deserialize_with = "lenient_vec")]
    pub also_albums: Option<Vec<Album>>,
    #[serde(deserialize_with = "lenient_vec")]
    pub popular_tracks: Option<Vec<Track>>,
    #[serde(deserialize_with = "lenient_vec")]
    pub similar_artists: Option<Vec<Artist>>,
    #[serde(deserialize_with = "lenient_obj")]
    pub stats: Option<ArtistStats>,
}

/// GET /artists/{id}/direct-albums
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistAlbums {
    #[serde(deserialize_with = "lenient_vec")]
    pub albums: Option<Vec<Album>>,
}

/// GET /get-file-info → downloadInfo
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadInfo {
    pub track_id: Option<IdString>,
    #[serde(deserialize_with = "lenient_string")]
    pub quality: Option<String>,
    /// flac | flac-mp4 | aac-mp4 | he-aac-mp4 | aac | he-aac | mp3
    pub codec: String,
    #[serde(deserialize_with = "lenient_i64")]
    pub bitrate: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub transport: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub url: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub urls: Option<Vec<String>>,
    pub real_id: Option<IdString>,
    #[serde(deserialize_with = "lenient_bool")]
    pub gain: Option<bool>,
    /// hex-ключ AES-128-CTR для transport=encraw (см. [`crate::crypto`])
    #[serde(deserialize_with = "lenient_string")]
    pub key: Option<String>,
}

impl DownloadInfo {
    /// urls[0] || url
    pub fn best_url(&self) -> Option<&str> {
        self.urls
            .as_ref()
            .and_then(|urls| urls.first())
            .map(String::as_str)
            .filter(|u| !u.is_empty())
            .or_else(|| self.url.as_deref().filter(|u| !u.is_empty()))
    }

    /// Файл зашифрован (encraw с ключом).
    pub fn is_encrypted(&self) -> bool {
        self.transport.as_deref() == Some("encraw") && self.key.as_deref().is_some_and(|k| !k.is_empty())
    }
}

/// GET /tracks/{id}/download-info (старый способ; прямая ссылка — [`crate::sign::direct_link`])
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LegacyDownloadInfo {
    pub codec: String,
    #[serde(deserialize_with = "lenient_i64")]
    pub bitrate_in_kbps: Option<i64>,
    #[serde(deserialize_with = "lenient_bool")]
    pub gain: Option<bool>,
    #[serde(deserialize_with = "lenient_bool")]
    pub preview: Option<bool>,
    #[serde(deserialize_with = "lenient_string")]
    pub download_info_url: Option<String>,
    #[serde(deserialize_with = "lenient_bool")]
    pub direct: Option<bool>,
}

/// GET /tracks/{id}/lyrics
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackLyrics {
    #[serde(deserialize_with = "lenient_string")]
    pub download_url: Option<String>,
    #[serde(deserialize_with = "lenient_i64")]
    pub lyric_id: Option<i64>,
    #[serde(deserialize_with = "lenient_string")]
    pub external_lyric_id: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub writers: Option<Vec<String>>,
}

/// POST oauth.yandex.ru/device/code (snake_case!)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    /// секунды между опросами
    pub interval: u64,
    /// секунды жизни кода
    pub expires_in: u64,
}

/// POST oauth.yandex.ru/token
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct OAuthToken {
    pub access_token: String,
    pub expires_in: Option<u64>,
    pub token_type: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SequenceItem {
    #[serde(rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient_obj")]
    pub track: Option<Track>,
    #[serde(deserialize_with = "lenient_bool")]
    pub liked: Option<bool>,
    pub track_parameters: Option<serde_json::Value>,
}

/// POST /rotor/session/new и /rotor/session/{id}/tracks
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RotorSession {
    #[serde(deserialize_with = "lenient_string")]
    pub radio_session_id: Option<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub batch_id: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub sequence: Option<Vec<SequenceItem>>,
    /// сессия протухла (долгая пауза) — нужна новая
    #[serde(deserialize_with = "lenient_bool")]
    pub unknown_session: Option<bool>,
    #[serde(deserialize_with = "lenient_bool")]
    pub pumpkin: Option<bool>,
    #[serde(deserialize_with = "lenient_string")]
    pub description_seed: Option<String>,
    pub accepted_seeds: Option<serde_json::Value>,
    pub wave: Option<serde_json::Value>,
}

impl RotorSession {
    /// Треки партии по порядку.
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.sequence.iter().flatten().filter_map(|item| item.track.as_ref())
    }
}

/// GET /pins → pins[]. type: artist_item | album_item | playlist_item | wave_item;
/// data — сырой объект (у волны в нём сиды; формат не документирован).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pin {
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Option<serde_json::Value>,
}

impl Pin {
    /// Сиды закреплённой волны (data.seeds: строка или массив строк; иначе пусто).
    pub fn wave_seeds(&self) -> Vec<String> {
        let Some(data) = &self.data else { return Vec::new() };
        let seeds = data
            .get("seeds")
            .or_else(|| data.get("wave").and_then(|w| w.get("seeds")))
            .or_else(|| data.get("seed"));
        match seeds {
            Some(Value::String(s)) => s.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            Some(Value::Array(items)) => {
                items.iter().filter_map(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned).collect()
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PinsList {
    #[serde(deserialize_with = "lenient_vec")]
    pub pins: Option<Vec<Pin>>,
}

/// library у /users/{uid}/likes/tracks (и /dislikes/tracks)
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TracksList {
    #[serde(deserialize_with = "lenient_i64")]
    pub uid: Option<i64>,
    #[serde(deserialize_with = "lenient_i64")]
    pub revision: Option<i64>,
    #[serde(deserialize_with = "lenient_vec")]
    pub tracks: Option<Vec<TrackShort>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LikesLibrary {
    #[serde(deserialize_with = "lenient_obj")]
    pub library: Option<TracksList>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileInfo {
    #[serde(deserialize_with = "lenient_obj")]
    pub download_info: Option<DownloadInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn id_string_any() {
        let ids: Vec<IdString> = serde_json::from_value(json!([1, "2", 3.0, "4:5"])).unwrap();
        assert_eq!(ids.iter().map(IdString::as_str).collect::<Vec<_>>(), ["1", "2", "3", "4:5"]);
        let t: Track = serde_json::from_value(json!({"id": null})).unwrap();
        assert!(t.id.is_none());
    }

    #[test]
    fn lenient_track() {
        let t: Track = serde_json::from_value(json!({
            "id": 12345, "title": "Song", "durationMs": "215000", "available": 1,
            "artists": [null, {"id": 1, "name": "A", "cover": "oops"}, 42],
            "albums": [{"id": 777, "title": "Al", "year": "2020", "trackPosition": {"volume": 1, "index": 3}}],
            "lyricsInfo": {"hasAvailableSyncLyrics": true},
            "unknownField": {"x": 1}
        }))
        .unwrap();
        assert_eq!(t.duration_ms, Some(215000));
        assert_eq!(t.available, Some(true));
        let artists = t.artists.as_ref().unwrap();
        assert_eq!(artists.len(), 1);
        assert!(artists[0].cover.is_none());
        assert_eq!(t.albums.as_ref().unwrap()[0].year, Some(2020));
        assert_eq!(t.full_id(), "12345:777");
        assert_eq!(t.lyrics_info.unwrap().has_available_sync_lyrics, Some(true));
    }

    #[test]
    fn track_short_full_id() {
        let s: TrackShort = serde_json::from_value(json!({"id": "10", "albumId": 20, "timestamp": "2024"})).unwrap();
        assert_eq!(s.full_id(), "10:20");
        let s: TrackShort = serde_json::from_value(json!({"id": 10})).unwrap();
        assert_eq!(s.full_id(), "10");
        let s: TrackShort = serde_json::from_value(json!({"id": 10, "albumId": ""})).unwrap();
        assert_eq!(s.full_id(), "10");
    }

    #[test]
    fn album_with_volumes_and_labels() {
        let a: Album = serde_json::from_value(json!({
            "id": 5, "type": "single", "labels": ["Sony", {"id": 2, "name": "Warner"}],
            "volumes": [[{"id": 1, "durationMs": 1000}, null], [{"id": 2, "durationMs": 2000}]]
        }))
        .unwrap();
        assert_eq!(a.all_tracks().count(), 2);
        let labels: Vec<_> = a.labels.iter().flatten().filter_map(Label::name).collect();
        assert_eq!(labels, ["Sony", "Warner"]);
    }

    #[test]
    fn download_info_url() {
        let d: DownloadInfo = serde_json::from_value(json!({
            "trackId": "1", "quality": "lossless", "codec": "flac-mp4", "bitrate": 0, "transport": "raw",
            "url": "https://b", "urls": ["https://a", "https://b"]
        }))
        .unwrap();
        assert_eq!(d.best_url(), Some("https://a"));
        assert!(!d.is_encrypted());
        let d: DownloadInfo = serde_json::from_value(json!({"codec": "mp3", "url": "https://u", "urls": []})).unwrap();
        assert_eq!(d.best_url(), Some("https://u"));
        let d: DownloadInfo = serde_json::from_value(json!({"codec": "aac", "transport": "encraw", "key": "00"})).unwrap();
        assert!(d.is_encrypted());
        assert_eq!(d.best_url(), None);
    }

    #[test]
    fn pin_seeds() {
        let pins: PinsList = serde_json::from_value(json!({"pins": [
            {"type": "wave_item", "data": {"seeds": ["track:1", "artist:2"]}},
            {"type": "wave_item", "data": {"seeds": "track:3"}},
            {"type": "album_item", "data": {"id": 5}}
        ]}))
        .unwrap();
        let pins = pins.pins.unwrap();
        assert_eq!(pins[0].wave_seeds(), ["track:1", "artist:2"]);
        assert_eq!(pins[1].wave_seeds(), ["track:3"]);
        assert!(pins[2].wave_seeds().is_empty());
    }

    #[test]
    fn search_with_nulls() {
        let s: Search = serde_json::from_value(json!({
            "text": "q",
            "best": {"type": "artist", "result": {"id": 1, "name": "X"}},
            "tracks": {"total": 3, "results": [{"id": 1}, null, {"id": 2}]},
            "albums": {"total": "0"},
            "artists": null
        }))
        .unwrap();
        assert_eq!(s.tracks.as_ref().unwrap().items().len(), 2);
        assert_eq!(s.albums.as_ref().unwrap().total, Some(0));
        assert!(s.artists.is_none());
        assert_eq!(s.best.unwrap().kind.as_deref(), Some("artist"));
    }

    #[test]
    fn account_status() {
        let s: AccountStatus = serde_json::from_value(json!({
            "account": {"uid": 42, "login": "user", "displayName": "", "fullName": "Full Name"},
            "plus": {"hasPlus": true}
        }))
        .unwrap();
        assert_eq!(s.uid(), Some(42));
        assert_eq!(s.display_name(), "Full Name");
        assert!(s.has_plus());
    }

    #[test]
    fn rotor_session() {
        let r: RotorSession = serde_json::from_value(json!({
            "radioSessionId": "sid", "batchId": "b1",
            "sequence": [{"type": "track", "track": {"id": 1}, "liked": false}, {"type": "ad"}]
        }))
        .unwrap();
        assert_eq!(r.tracks().count(), 1);
        assert_eq!(r.radio_session_id.as_deref(), Some("sid"));
    }
}
