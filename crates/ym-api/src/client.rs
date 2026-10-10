//! HTTP-клиент. Повторяет поведение yandex_music.ClientAsync + RetryingRequest (src/yamusic/api.py):
//! - базовый URL https://api.music.yandex.net, OAuth: https://oauth.yandex.ru;
//! - заголовки: User-Agent: Yandex-Music-API, X-Yandex-Music-Client: YandexMusicAndroid/24023621,
//!   Accept-Language: ru, Authorization: OAuth <token> (если есть); для /get-file-info клиент —
//!   YandexMusicDesktopAppWindows/5.95.0 (к нему привязан ключ подписи);
//! - тело ответа — конверт {"result": …, "error": …}; если result нет — весь объект и есть результат;
//! - не больше MAX_CONCURRENT запросов разом (Semaphore); 429 и 500–504 — до RETRIES повторов
//!   с паузой 0,8 с × 2^n × (1 + 0,25·rand); таймаут запроса — 10 с (у Python 5 с; скачивание текста — 15 с);
//! - POST с формой — application/x-www-form-urlencoded; списки id — через запятую.
use std::sync::Arc;
use std::time::Duration;

use reqwest::Method;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::diff::DiffOp;
use crate::error::{ApiError, Result};
use crate::models::{
    AccountStatus, Album, ArtistAlbums, ArtistBriefInfo, DeviceCode, DownloadInfo, FileInfo, LegacyDownloadInfo,
    LikesLibrary, OAuthToken, Pin, PinsList, Playlist, RotorSession, Search, Track, TrackLyrics, TrackShort, TracksList,
};
use crate::sign;

pub const BASE_URL: &str = "https://api.music.yandex.net";
pub const OAUTH_URL: &str = "https://oauth.yandex.ru";
pub const USER_AGENT: &str = "Yandex-Music-API";
pub const CLIENT_HEADER: &str = "YandexMusicAndroid/24023621";
pub const DESKTOP_CLIENT_HEADER: &str = "YandexMusicDesktopAppWindows/5.95.0";
pub const CLIENT_ID: &str = "23cabbbdc6cd418abb4b39c32c41195d";
pub const CLIENT_SECRET: &str = "53bc75238f0c4d08a118e51fe9203300";
pub const MAX_CONCURRENT: usize = 4;
pub const RETRIES: u32 = 5;
pub const FILE_INFO_CODECS: [&str; 7] = ["flac", "flac-mp4", "aac-mp4", "he-aac-mp4", "aac", "he-aac", "mp3"];
/// Таймаут скачивания текста по прямой ссылке
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15);
/// Имя устройства по умолчанию для Device Flow (как в yandex_music)
pub const DEFAULT_DEVICE_NAME: &str = "YandexMusicAPI";
/// eitherUserId у /landing3 (константа yandex_music)
const LANDING_USER_ID: &str = "10254713668400548221";
const RETRY_BASE_SECS: f64 = 0.8;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub base_url: String,
    pub oauth_url: String,
    pub language: String,
    pub timeout: std::time::Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: BASE_URL.to_owned(),
            oauth_url: OAUTH_URL.to_owned(),
            language: "ru".to_owned(),
            timeout: std::time::Duration::from_secs(10),
        }
    }
}

/// Тип поиска: параметр type у /search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchType {
    All,
    Track,
    Album,
    Artist,
    Playlist,
}

impl SearchType {
    /// "all" | "track" | "album" | "artist" | "playlist"
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Track => "track",
            Self::Album => "album",
            Self::Artist => "artist",
            Self::Playlist => "playlist",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsFormat {
    Text,
    Lrc,
}

impl LyricsFormat {
    /// "TEXT" | "LRC"
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Lrc => "LRC",
        }
    }
}

/// POST /play-audio (форма). Пустые album_id/playlist_id не отправляются (как в Android-клиенте).
#[derive(Clone, Debug, Default)]
pub struct PlayAudio {
    pub track_id: String,
    /// контекст: PlayContext.from || DEFAULT_FROM
    pub from: String,
    pub album_id: String,
    pub playlist_id: Option<String>,
    pub play_id: String,
    pub uid: i64,
    pub track_length_seconds: i64,
    pub total_played_seconds: f64,
    pub end_position_seconds: f64,
}

/// Тело POST /rotor/session/new
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotorSessionNew {
    pub seeds: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<Vec<String>>,
    pub include_tracks_in_response: bool,
}

/// event у /rotor/session/{id}/feedback
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotorEvent {
    /// radioStarted | trackStarted | trackFinished | skip | like | unlike | dislike
    #[serde(rename = "type")]
    pub kind: String,
    /// UTC ISO-8601 с миллисекундами и Z: 2026-10-11T12:00:00.000Z
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_played_seconds: Option<f64>,
}

impl RotorEvent {
    /// Событие с текущим временем.
    pub fn now(kind: &str, track_id: Option<String>, total_played_seconds: Option<f64>) -> Self {
        Self { kind: kind.to_owned(), timestamp: utc_now_iso(), track_id, total_played_seconds }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotorFeedback {
    pub event: RotorEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

/// Клиент дёшево клонируется (Arc внутри) и безопасен для параллельных вызовов.
#[derive(Clone)]
pub struct YmClient {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for YmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YmClient")
            .field("base_url", &self.inner.config.base_url)
            .field("token", &self.inner.token.as_ref().map(|_| "***"))
            .finish()
    }
}

struct Inner {
    http: reqwest::Client,
    token: Option<String>,
    config: ClientConfig,
    gate: tokio::sync::Semaphore,
}

enum Body {
    Empty,
    Form(Vec<(&'static str, String)>),
    Json(Value),
}

/// Описание запроса; собирается заново на каждый повтор.
struct Call {
    method: Method,
    url: String,
    query: Vec<(&'static str, String)>,
    body: Body,
    /// заголовки API музыки (иначе — только User-Agent: OAuth и прямые ссылки)
    api: bool,
    client_header: &'static str,
    timeout: Option<Duration>,
}

impl Call {
    fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            query: Vec::new(),
            body: Body::Empty,
            api: true,
            client_header: CLIENT_HEADER,
            timeout: None,
        }
    }

    fn query(mut self, query: Vec<(&'static str, String)>) -> Self {
        self.query = query;
        self
    }

    fn form(mut self, form: Vec<(&'static str, String)>) -> Self {
        self.body = Body::Form(form);
        self
    }

    fn json(mut self, body: Value) -> Self {
        self.body = Body::Json(body);
        self
    }

    fn bare(mut self) -> Self {
        self.api = false;
        self
    }
}

/// bool в форме/запросе — как str(bool) в Python.
fn py_bool(value: bool) -> String {
    if value { "True" } else { "False" }.to_owned()
}

/// float в форме — как str(float) в Python: 0.0, 12.5.
fn py_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// Результат из конверта: result, а если его нет (или null) — весь объект (_parse в request_base.py).
pub(crate) fn unwrap_envelope(body: &str) -> Result<Value> {
    let data: Value = serde_json::from_str(body)?;
    Ok(match data {
        Value::Object(mut map) => match map.remove("result") {
            Some(result) if !result.is_null() => result,
            _ => Value::Object(map),
        },
        other => other,
    })
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| ApiError::Decode(e.to_string()))
}

/// Текст первого элемента <tag>…</tag> в XML ответа download-info.
fn xml_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))? + start;
    Some(xml[start..end].trim())
}

impl YmClient {
    pub fn new(token: Option<String>) -> Result<Self> {
        Self::with_config(token, ClientConfig::default())
    }

    pub fn with_config(token: Option<String>, config: ClientConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(config.timeout)
            .connect_timeout(config.timeout)
            .gzip(true)
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let token = token.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        Ok(Self { inner: Arc::new(Inner { http, token, config, gate: tokio::sync::Semaphore::new(MAX_CONCURRENT) }) })
    }

    pub fn token(&self) -> Option<&str> {
        self.inner.token.as_deref()
    }

    pub fn config(&self) -> &ClientConfig {
        &self.inner.config
    }

    /// Тот же HTTP-клиент и настройки, другой токен.
    pub fn with_token(&self, token: Option<String>) -> Self {
        let token = token.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        Self {
            inner: Arc::new(Inner {
                http: self.inner.http.clone(),
                token,
                config: self.inner.config.clone(),
                gate: tokio::sync::Semaphore::new(MAX_CONCURRENT),
            }),
        }
    }

    // --- вход: OAuth Device Flow ------------------------------------------------

    /// POST {oauth}/device/code: client_id, device_id (10 случайных [A-Za-z0-9]), device_name.
    pub async fn request_device_code(&self, device_name: &str) -> Result<DeviceCode> {
        let device_name = if device_name.is_empty() { DEFAULT_DEVICE_NAME } else { device_name };
        let device_id: String = std::iter::repeat_with(fastrand::alphanumeric).take(10).collect();
        let call = Call::new(Method::POST, self.oauth("device/code")).bare().form(vec![
            ("client_id", CLIENT_ID.to_owned()),
            ("device_id", device_id),
            ("device_name", device_name.to_owned()),
        ]);
        self.call(call).await
    }

    /// POST {oauth}/token: grant_type=device_code, code, client_id, client_secret.
    /// Ok(None) — 400 с error=authorization_pending (пользователь ещё не подтвердил).
    pub async fn poll_device_token(&self, device_code: &str) -> Result<Option<OAuthToken>> {
        let call = Call::new(Method::POST, self.oauth("token")).bare().form(vec![
            ("grant_type", "device_code".to_owned()),
            ("code", device_code.to_owned()),
            ("client_id", CLIENT_ID.to_owned()),
            ("client_secret", CLIENT_SECRET.to_owned()),
        ]);
        match self.call::<OAuthToken>(call).await {
            Ok(token) => Ok(Some(token)),
            Err(ApiError::BadRequest { code, message })
                if code == "authorization_pending" || message.contains("authorization_pending") =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    // --- аккаунт и библиотека ---------------------------------------------------

    /// GET /account/status
    pub async fn account_status(&self) -> Result<AccountStatus> {
        self.need_token()?;
        self.call(self.get("/account/status")).await
    }

    /// GET /users/{uid}/playlists/list
    pub async fn users_playlists_list(&self, uid: i64) -> Result<Vec<Playlist>> {
        self.need_token()?;
        self.call_list(self.get(&format!("/users/{uid}/playlists/list"))).await
    }

    /// GET /users/{uid}/playlists/{kind} (треки внутри, ревизия)
    pub async fn users_playlist(&self, uid: i64, kind: i64) -> Result<Playlist> {
        let value = self.call_value(self.get(&format!("/users/{uid}/playlists/{kind}"))).await?;
        // при нескольких kinds API отвечает списком
        match value {
            Value::Array(mut items) if !items.is_empty() => decode(items.swap_remove(0)),
            other => decode(other),
        }
    }

    /// GET /users/{uid}/likes/tracks → result.library.tracks (новые сверху)
    pub async fn users_likes_tracks(&self, uid: i64) -> Result<Vec<TrackShort>> {
        Ok(self.users_likes_tracks_library(uid).await?.tracks.unwrap_or_default())
    }

    /// GET /users/{uid}/likes/tracks?if-modified-since-revision=0 → library целиком (с ревизией).
    pub async fn users_likes_tracks_library(&self, uid: i64) -> Result<TracksList> {
        self.need_token()?;
        let call = self
            .get(&format!("/users/{uid}/likes/tracks"))
            .query(vec![("if-modified-since-revision", "0".to_owned())]);
        let likes: LikesLibrary = self.call(call).await?;
        Ok(likes.library.unwrap_or_default())
    }

    /// GET /users/{uid}/dislikes/tracks → library.tracks
    pub async fn users_dislikes_tracks(&self, uid: i64) -> Result<Vec<TrackShort>> {
        self.need_token()?;
        let call = self
            .get(&format!("/users/{uid}/dislikes/tracks"))
            .query(vec![("if_modified_since_revision", "0".to_owned())]);
        let dislikes: LikesLibrary = self.call(call).await?;
        Ok(dislikes.library.and_then(|l| l.tracks).unwrap_or_default())
    }

    /// POST /tracks: track-ids=<id,id,…>, with-positions=True. Порядок ответа = порядок запроса.
    pub async fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let call = self.post("/tracks").form(vec![("track-ids", ids.join(",")), ("with-positions", py_bool(true))]);
        self.call_list(call).await
    }

    /// POST /albums: album-ids=<id,id,…>
    pub async fn albums(&self, ids: &[String]) -> Result<Vec<Album>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.call_list(self.post("/albums").form(vec![("album-ids", ids.join(","))])).await
    }

    /// POST /users/{uid}/likes/tracks/add-multiple: track-ids=<full_id>
    pub async fn users_likes_tracks_add(&self, uid: i64, full_id: &str) -> Result<()> {
        self.track_action(&format!("/users/{uid}/likes/tracks/add-multiple"), full_id).await
    }

    /// POST /users/{uid}/likes/tracks/remove: track-ids=<full_id>
    pub async fn users_likes_tracks_remove(&self, uid: i64, full_id: &str) -> Result<()> {
        self.track_action(&format!("/users/{uid}/likes/tracks/remove"), full_id).await
    }

    /// POST /users/{uid}/dislikes/tracks/add-multiple: track-ids=<full_id>
    pub async fn users_dislikes_tracks_add(&self, uid: i64, full_id: &str) -> Result<()> {
        self.track_action(&format!("/users/{uid}/dislikes/tracks/add-multiple"), full_id).await
    }

    /// POST /users/{uid}/dislikes/tracks/remove: track-ids=<full_id>
    pub async fn users_dislikes_tracks_remove(&self, uid: i64, full_id: &str) -> Result<()> {
        self.track_action(&format!("/users/{uid}/dislikes/tracks/remove"), full_id).await
    }

    /// POST /users/{uid}/playlists/create: title, visibility ("public" | "private")
    pub async fn users_playlists_create(&self, uid: i64, title: &str, visibility: &str) -> Result<Playlist> {
        self.need_token()?;
        let call = self
            .post(&format!("/users/{uid}/playlists/create"))
            .form(vec![("title", title.to_owned()), ("visibility", visibility.to_owned())]);
        self.call(call).await
    }

    /// POST /users/{uid}/playlists/{kind}/delete
    pub async fn users_playlists_delete(&self, uid: i64, kind: i64) -> Result<()> {
        self.need_token()?;
        self.call_unit(self.post(&format!("/users/{uid}/playlists/{kind}/delete"))).await
    }

    /// POST /users/{uid}/playlists/{kind}/name: value
    pub async fn users_playlists_name(&self, uid: i64, kind: i64, name: &str) -> Result<Playlist> {
        self.need_token()?;
        let call = self.post(&format!("/users/{uid}/playlists/{kind}/name")).form(vec![("value", name.to_owned())]);
        self.call(call).await
    }

    /// POST /users/{uid}/playlists/{kind}/change: kind, revision, diff=<JSON массива DiffOp>.
    /// Устаревшая ревизия — ApiError::Http { status: 412 } (иногда 409/400 wrong-revision): вызывающий перечитывает плейлист.
    pub async fn users_playlists_change(&self, uid: i64, kind: i64, revision: i64, diff: &[DiffOp]) -> Result<Playlist> {
        self.need_token()?;
        let call = self.post(&format!("/users/{uid}/playlists/{kind}/change")).form(vec![
            ("kind", kind.to_string()),
            ("revision", revision.to_string()),
            ("diff", crate::diff::to_json(diff)),
        ]);
        self.call(call).await
    }

    /// Удобная обёртка: вставить один трек в позицию `at`.
    pub async fn users_playlists_insert_track(
        &self,
        uid: i64,
        kind: i64,
        revision: i64,
        at: i64,
        track_id: &str,
        album_id: &str,
    ) -> Result<Playlist> {
        self.users_playlists_change(uid, kind, revision, &[DiffOp::insert_one(at, track_id, album_id)]).await
    }

    /// Удалить треки с позиции `from` по `to` (не включая).
    pub async fn users_playlists_delete_tracks(
        &self,
        uid: i64,
        kind: i64,
        revision: i64,
        from: i64,
        to: i64,
    ) -> Result<Playlist> {
        self.users_playlists_change(uid, kind, revision, &[DiffOp::Delete { from, to }]).await
    }

    // --- закреплённое (pins) ------------------------------------------------------

    /// GET /pins
    pub async fn pins(&self) -> Result<Vec<Pin>> {
        self.need_token()?;
        let value = self.call_value(self.get("/pins")).await?;
        Ok(match value {
            Value::Array(_) => decode::<PinsList>(json!({ "pins": value }))?.pins.unwrap_or_default(),
            other => decode::<PinsList>(other)?.pins.unwrap_or_default(),
        })
    }

    /// PUT /pin/wave, JSON {"seeds": "<seed>"} (например "track:123")
    pub async fn pin_wave(&self, seeds: &str) -> Result<Pin> {
        self.pin(Method::PUT, "/pin/wave", json!({ "seeds": seeds }), "wave_item").await
    }

    /// DELETE /pin/wave, JSON {"seeds": "<seed>"}; результат "ok"
    pub async fn unpin_wave(&self, seeds: &str) -> Result<()> {
        self.need_token()?;
        self.call_unit(Call::new(Method::DELETE, self.api("/pin/wave")).json(json!({ "seeds": seeds }))).await
    }

    /// PUT /pin/album, JSON {"id": …}
    pub async fn pin_album(&self, album_id: &str) -> Result<Pin> {
        self.pin(Method::PUT, "/pin/album", json!({ "id": album_id }), "album_item").await
    }

    /// DELETE /pin/album
    pub async fn unpin_album(&self, album_id: &str) -> Result<()> {
        self.need_token()?;
        self.call_unit(Call::new(Method::DELETE, self.api("/pin/album")).json(json!({ "id": album_id }))).await
    }

    /// PUT /pin/artist, JSON {"id": …}
    pub async fn pin_artist(&self, artist_id: &str) -> Result<Pin> {
        self.pin(Method::PUT, "/pin/artist", json!({ "id": artist_id }), "artist_item").await
    }

    /// DELETE /pin/artist
    pub async fn unpin_artist(&self, artist_id: &str) -> Result<()> {
        self.need_token()?;
        self.call_unit(Call::new(Method::DELETE, self.api("/pin/artist")).json(json!({ "id": artist_id }))).await
    }

    /// PUT /pin/playlist, JSON {"uid": …, "kind": …}
    pub async fn pin_playlist(&self, uid: i64, kind: i64) -> Result<Pin> {
        self.pin(Method::PUT, "/pin/playlist", json!({ "uid": uid, "kind": kind }), "playlist_item").await
    }

    /// DELETE /pin/playlist
    pub async fn unpin_playlist(&self, uid: i64, kind: i64) -> Result<()> {
        self.need_token()?;
        let call = Call::new(Method::DELETE, self.api("/pin/playlist")).json(json!({ "uid": uid, "kind": kind }));
        self.call_unit(call).await
    }

    // --- каталог -----------------------------------------------------------------

    /// GET /albums/{id}/with-tracks
    pub async fn albums_with_tracks(&self, album_id: &str) -> Result<Album> {
        self.call(self.get(&format!("/albums/{album_id}/with-tracks"))).await
    }

    /// GET /artists/{id}/brief-info
    pub async fn artists_brief_info(&self, artist_id: &str) -> Result<ArtistBriefInfo> {
        self.call(self.get(&format!("/artists/{artist_id}/brief-info"))).await
    }

    /// GET /artists/{id}/direct-albums?page=<page>&page-size=<size>&sort-by=year
    pub async fn artists_direct_albums(&self, artist_id: &str, page: u32, page_size: u32) -> Result<ArtistAlbums> {
        let call = self.get(&format!("/artists/{artist_id}/direct-albums")).query(vec![
            ("sort-by", "year".to_owned()),
            ("page", page.to_string()),
            ("page-size", page_size.to_string()),
        ]);
        self.call(call).await
    }

    /// GET /artists/{id}/tracks?page=&page-size= → {tracks, pager} (сырой JSON)
    pub async fn artists_tracks(&self, artist_id: &str, page: u32, page_size: u32) -> Result<Value> {
        let call = self
            .get(&format!("/artists/{artist_id}/tracks"))
            .query(vec![("page", page.to_string()), ("page-size", page_size.to_string())]);
        self.call_value(call).await
    }

    /// GET /search?text=…&nocorrect=False&type=…&page=0&playlist-in-best=True
    pub async fn search(&self, text: &str, kind: SearchType, nocorrect: bool, page: u32) -> Result<Search> {
        let call = self.get("/search").query(vec![
            ("text", text.to_owned()),
            ("nocorrect", py_bool(nocorrect)),
            ("type", kind.as_str().to_owned()),
            ("page", page.to_string()),
            ("playlist-in-best", py_bool(true)),
        ]);
        match self.call_value(call).await? {
            // строка вместо объекта — ошибка запроса (как в search.py)
            Value::String(message) => Err(ApiError::BadRequest { code: String::new(), message }),
            value => decode(value),
        }
    }

    /// GET /search/suggest?part=… (сырой JSON)
    pub async fn search_suggest(&self, part: &str) -> Result<Value> {
        self.call_value(self.get("/search/suggest").query(vec![("part", part.to_owned())])).await
    }

    /// GET /landing3?blocks=a,b,… — блоки главной (сырой JSON: {blocks: [{type, entities…}]})
    pub async fn landing(&self, blocks: &[&str]) -> Result<Value> {
        let call = self
            .get("/landing3")
            .query(vec![("blocks", blocks.join(",")), ("eitherUserId", LANDING_USER_ID.to_owned())]);
        self.call_value(call).await
    }

    /// GET /feed — лента (сырой JSON)
    pub async fn feed(&self) -> Result<Value> {
        self.call_value(self.get("/feed")).await
    }

    // --- воспроизведение -------------------------------------------------------------

    /// GET /get-file-info: ts, trackId, quality (lossless|hq|nq), codecs (через запятую), transports=raw,
    /// sign = sign::file_info_sign(…); заголовок X-Yandex-Music-Client: DESKTOP_CLIENT_HEADER. → downloadInfo
    pub async fn tracks_file_info(&self, track_id: &str, quality: &str) -> Result<DownloadInfo> {
        self.tracks_file_info_with(track_id, quality, &FILE_INFO_CODECS, "raw").await
    }

    /// То же с явными кодеками и транспортом (raw | encraw; encraw расшифровывает [`crate::crypto`]).
    pub async fn tracks_file_info_with(
        &self,
        track_id: &str,
        quality: &str,
        codecs: &[&str],
        transport: &str,
    ) -> Result<DownloadInfo> {
        self.need_token()?;
        let sign = sign::file_info_sign(track_id, quality, codecs, transport, sign::now_ts());
        let mut call = self.get("/get-file-info").query(vec![
            ("ts", sign.timestamp.to_string()),
            ("trackId", track_id.to_owned()),
            ("quality", quality.to_owned()),
            ("codecs", codecs.join(",")),
            ("transports", transport.to_owned()),
            ("sign", sign.value),
        ]);
        call.client_header = DESKTOP_CLIENT_HEADER;
        let info: FileInfo = self.call(call).await?;
        info.download_info.ok_or_else(|| ApiError::Decode("в ответе нет downloadInfo".to_owned()))
    }

    /// GET /tracks/{id}/download-info — старый способ (mp3/aac, ссылка живёт минуту).
    pub async fn tracks_download_info(&self, track_id: &str) -> Result<Vec<LegacyDownloadInfo>> {
        self.call_list(self.get(&format!("/tracks/{track_id}/download-info"))).await
    }

    /// Прямая ссылка по варианту из /download-info: XML {host, path, ts, s} → подпись md5.
    pub async fn direct_link(&self, info: &LegacyDownloadInfo) -> Result<String> {
        let url = info
            .download_info_url
            .as_deref()
            .filter(|u| !u.is_empty())
            .ok_or_else(|| ApiError::Decode("нет downloadInfoUrl".to_owned()))?;
        let xml = self.download_text(url).await?;
        let field = |tag: &str| xml_text(&xml, tag).ok_or_else(|| ApiError::Decode(format!("в XML нет <{tag}>")));
        let (host, path, ts, s) = (field("host")?, field("path")?, field("ts")?, field("s")?);
        Ok(sign::direct_link(host, path, ts, s))
    }

    /// Скачать файл по прямой ссылке целиком; encraw расшифровывается ключом из `info`.
    pub async fn download_track(&self, info: &DownloadInfo) -> Result<Vec<u8>> {
        let url = info.best_url().ok_or_else(|| ApiError::Decode("Яндекс не дал ссылку на трек".to_owned()))?;
        let data = self.download_bytes(url).await?;
        match info.key.as_deref() {
            Some(key) if info.is_encrypted() => crate::crypto::decrypt_track(&data, key),
            _ => Ok(data),
        }
    }

    /// GET /tracks/{id}/lyrics?format=LRC|TEXT&timeStamp=<ts>&sign=<sign::lyrics_sign>. 404 → Ok(None).
    pub async fn tracks_lyrics(&self, track_id: &str, format: LyricsFormat) -> Result<Option<TrackLyrics>> {
        let sign = sign::lyrics_sign(track_id, sign::now_ts());
        let call = self.get(&format!("/tracks/{track_id}/lyrics")).query(vec![
            ("format", format.as_str().to_owned()),
            ("timeStamp", sign.timestamp.to_string()),
            ("sign", sign.value),
        ]);
        match self.call(call).await {
            Ok(lyrics) => Ok(Some(lyrics)),
            Err(ApiError::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Скачать текст по прямой ссылке (без заголовков API, с повторами 5xx).
    pub async fn download_text(&self, url: &str) -> Result<String> {
        let mut call = Call::new(Method::GET, url.to_owned()).bare();
        call.timeout = Some(DOWNLOAD_TIMEOUT);
        let bytes = self.send(&call).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Скачать байты по прямой ссылке (без заголовков API и общего таймаута, с повторами 5xx).
    pub async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let mut call = Call::new(Method::GET, url.to_owned()).bare();
        call.timeout = Some(Duration::from_secs(600));
        self.send(&call).await
    }

    /// POST /play-audio (форма: track-id, from-cache=False, from, play-id, uid, timestamp, track-length-seconds,
    /// total-played-seconds, end-position-seconds, album-id?, playlist-id?, client-now). timestamp = client-now = utc_now_iso().
    pub async fn play_audio(&self, report: &PlayAudio) -> Result<()> {
        self.need_token()?;
        let now = utc_now_iso();
        let from = if report.from.is_empty() { crate::items::contexts::DEFAULT_FROM } else { &report.from };
        let mut form = vec![
            ("track-id", report.track_id.clone()),
            ("from-cache", py_bool(false)),
            ("from", from.to_owned()),
            ("play-id", report.play_id.clone()),
            ("uid", report.uid.to_string()),
            ("timestamp", now.clone()),
            ("track-length-seconds", report.track_length_seconds.to_string()),
            ("total-played-seconds", py_float(report.total_played_seconds)),
            ("end-position-seconds", py_float(report.end_position_seconds)),
        ];
        if !report.album_id.is_empty() {
            form.push(("album-id", report.album_id.clone()));
        }
        if let Some(playlist) = report.playlist_id.as_ref().filter(|p| !p.is_empty()) {
            form.push(("playlist-id", playlist.clone()));
        }
        form.push(("client-now", now));
        self.call_unit(self.post("/play-audio").form(form)).await
    }

    // --- «Моя волна» -------------------------------------------------------------------

    /// GET /rotor/wave/settings?seeds=user:onyourwave — сырой JSON (разбор групп — в ym-services::wave).
    pub async fn rotor_wave_settings(&self, seeds: &str) -> Result<serde_json::Value> {
        let query = if seeds.is_empty() { Vec::new() } else { vec![("seeds", seeds.to_owned())] };
        self.call_value(self.get("/rotor/wave/settings").query(query)).await
    }

    /// GET /rotor/stations/dashboard — рекомендованные станции (сырой JSON)
    pub async fn rotor_stations_dashboard(&self) -> Result<Value> {
        self.call_value(self.get("/rotor/stations/dashboard")).await
    }

    /// GET /rotor/stations/list?language=… — все станции (сырой JSON)
    pub async fn rotor_stations_list(&self) -> Result<Value> {
        let call = self.get("/rotor/stations/list").query(vec![("language", self.inner.config.language.clone())]);
        self.call_value(call).await
    }

    /// POST /rotor/session/new (JSON)
    pub async fn rotor_session_new(&self, body: &RotorSessionNew) -> Result<RotorSession> {
        self.need_token()?;
        let body = serde_json::to_value(body)?;
        self.call(self.post("/rotor/session/new").json(body)).await
    }

    /// POST /rotor/session/{id}/tracks (JSON {"queue": [...]})
    pub async fn rotor_session_tracks(&self, session_id: &str, queue: &[String]) -> Result<RotorSession> {
        self.need_token()?;
        let call = self.post(&format!("/rotor/session/{session_id}/tracks")).json(json!({ "queue": queue }));
        self.call(call).await
    }

    /// POST /rotor/session/{id}/feedback (JSON RotorFeedback)
    pub async fn rotor_session_feedback(&self, session_id: &str, feedback: &RotorFeedback) -> Result<()> {
        self.need_token()?;
        let body = serde_json::to_value(feedback)?;
        self.call_unit(self.post(&format!("/rotor/session/{session_id}/feedback")).json(body)).await
    }

    // --- внутреннее ----------------------------------------------------------------------

    fn need_token(&self) -> Result<()> {
        if self.inner.token.is_some() { Ok(()) } else { Err(ApiError::NoToken) }
    }

    fn api(&self, path: &str) -> String {
        format!("{}{path}", self.inner.config.base_url.trim_end_matches('/'))
    }

    fn oauth(&self, path: &str) -> String {
        format!("{}/{path}", self.inner.config.oauth_url.trim_end_matches('/'))
    }

    fn get(&self, path: &str) -> Call {
        Call::new(Method::GET, self.api(path))
    }

    fn post(&self, path: &str) -> Call {
        Call::new(Method::POST, self.api(path))
    }

    async fn track_action(&self, path: &str, full_id: &str) -> Result<()> {
        self.need_token()?;
        self.call_unit(self.post(path).form(vec![("track-ids", full_id.to_owned())])).await
    }

    async fn pin(&self, method: Method, path: &str, body: Value, kind: &str) -> Result<Pin> {
        self.need_token()?;
        let value = self.call_value(Call::new(method, self.api(path)).json(body)).await?;
        Ok(match value {
            v @ Value::Object(_) => decode(v)?,
            _ => Pin { kind: kind.to_owned(), data: None },
        })
    }

    fn build(&self, call: &Call) -> reqwest::RequestBuilder {
        let mut request = self.inner.http.request(call.method.clone(), &call.url);
        if !call.query.is_empty() {
            request = request.query(&call.query);
        }
        if call.api {
            request = request
                .header("X-Yandex-Music-Client", call.client_header)
                .header(reqwest::header::ACCEPT_LANGUAGE, &self.inner.config.language);
            if let Some(token) = &self.inner.token {
                request = request.header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"));
            }
        }
        if let Some(timeout) = call.timeout {
            request = request.timeout(timeout);
        }
        match &call.body {
            Body::Empty => request,
            Body::Form(form) => request.form(form),
            Body::Json(body) => request.json(body),
        }
    }

    /// Запрос с семафором и повторами 429/5xx; тело успешного ответа.
    async fn send(&self, call: &Call) -> Result<Vec<u8>> {
        let mut delay = RETRY_BASE_SECS;
        let mut attempt = 0;
        loop {
            let (status, body) = {
                let _permit = self.inner.gate.acquire().await.map_err(|_| ApiError::Network("клиент закрыт".to_owned()))?;
                let response = self.build(call).send().await?;
                let status = response.status().as_u16();
                (status, response.bytes().await?.to_vec())
            };
            if (200..300).contains(&status) {
                return Ok(body);
            }
            let retryable = status == 429 || (500..=504).contains(&status);
            if !retryable || attempt >= RETRIES {
                return Err(ApiError::from_response(status, &String::from_utf8_lossy(&body)));
            }
            let pause = delay * (1.0 + 0.25 * fastrand::f64());
            tracing::info!("API занят ({status}), повтор через {pause:.1} с");
            tokio::time::sleep(Duration::from_secs_f64(pause)).await;
            delay *= 2.0;
            attempt += 1;
        }
    }

    async fn call_value(&self, call: Call) -> Result<Value> {
        let body = self.send(&call).await?;
        unwrap_envelope(&String::from_utf8_lossy(&body))
    }

    async fn call<T: DeserializeOwned>(&self, call: Call) -> Result<T> {
        decode(self.call_value(call).await?)
    }

    /// Список с пропуском null и битых элементов.
    async fn call_list<T: DeserializeOwned>(&self, call: Call) -> Result<Vec<T>> {
        match self.call_value(call).await? {
            Value::Array(items) => {
                Ok(items.into_iter().filter(|v| !v.is_null()).filter_map(|v| serde_json::from_value(v).ok()).collect())
            }
            Value::Null => Ok(Vec::new()),
            other => Err(ApiError::Decode(format!("ожидался список, пришло: {}", short(&other)))),
        }
    }

    /// Ответ не нужен ("ok" или объект).
    async fn call_unit(&self, call: Call) -> Result<()> {
        self.send(&call).await.map(drop)
    }
}

fn short(value: &Value) -> String {
    let text = value.to_string();
    match text.char_indices().nth(80) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

/// UTC ISO-8601 с миллисекундами и суффиксом Z (как utc_now_iso в yandex_music).
pub fn utc_now_iso() -> String {
    format_iso(chrono::Utc::now())
}

fn format_iso(time: chrono::DateTime<chrono::Utc>) -> String {
    time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope() {
        assert_eq!(unwrap_envelope(r#"{"invocationInfo":{},"result":{"a":1}}"#).unwrap(), json!({"a": 1}));
        assert_eq!(unwrap_envelope(r#"{"result":"ok"}"#).unwrap(), json!("ok"));
        assert_eq!(unwrap_envelope(r#"{"result":[1,2]}"#).unwrap(), json!([1, 2]));
        // OAuth: без result — весь объект
        assert_eq!(
            unwrap_envelope(r#"{"device_code":"d","interval":5}"#).unwrap(),
            json!({"device_code": "d", "interval": 5})
        );
        assert_eq!(unwrap_envelope(r#"{"result":null,"x":1}"#).unwrap(), json!({"x": 1}));
        assert!(matches!(unwrap_envelope("<html>"), Err(ApiError::Decode(_))));
    }

    #[test]
    fn python_formatting() {
        assert_eq!(py_bool(true), "True");
        assert_eq!(py_bool(false), "False");
        assert_eq!(py_float(0.0), "0.0");
        assert_eq!(py_float(12.5), "12.5");
        assert_eq!(py_float(123.456), "123.456");
        assert_eq!(py_float(180.0), "180.0");
    }

    #[test]
    fn iso_time() {
        let time = chrono::DateTime::from_timestamp_millis(1_760_000_000_500).unwrap();
        assert_eq!(format_iso(time), "2025-10-09T08:53:20.500Z");
        let now = utc_now_iso();
        assert_eq!(now.len(), 24);
        assert!(now.ends_with('Z'));
    }

    #[test]
    fn search_types() {
        let all = [SearchType::All, SearchType::Track, SearchType::Album, SearchType::Artist, SearchType::Playlist];
        let names: Vec<_> = all.iter().map(|t| t.as_str()).collect();
        assert_eq!(names, ["all", "track", "album", "artist", "playlist"]);
        assert_eq!(LyricsFormat::Lrc.as_str(), "LRC");
    }

    #[test]
    fn rotor_bodies() {
        let body = RotorSessionNew { seeds: vec!["user:onyourwave".into()], queue: None, include_tracks_in_response: true };
        assert_eq!(serde_json::to_value(&body).unwrap(), json!({"seeds": ["user:onyourwave"], "includeTracksInResponse": true}));
        let feedback = RotorFeedback {
            event: RotorEvent {
                kind: "trackFinished".into(),
                timestamp: "2026-10-11T12:00:00.000Z".into(),
                track_id: Some("1:2".into()),
                total_played_seconds: Some(30.5),
            },
            batch_id: Some("b".into()),
            from: None,
        };
        assert_eq!(serde_json::to_value(&feedback).unwrap(), json!({
            "event": {"type": "trackFinished", "timestamp": "2026-10-11T12:00:00.000Z", "trackId": "1:2", "totalPlayedSeconds": 30.5},
            "batchId": "b"
        }));
    }

    #[test]
    fn xml_fields() {
        let xml = "<?xml version=\"1.0\"?><download-info><host>h.net</host><path>/p/q</path><ts>00ab</ts><region>-1</region><s>sig</s></download-info>";
        assert_eq!(xml_text(xml, "host"), Some("h.net"));
        assert_eq!(xml_text(xml, "s"), Some("sig"));
        assert_eq!(xml_text(xml, "nope"), None);
    }

    #[tokio::test]
    async fn needs_token() {
        let client = YmClient::new(None).unwrap();
        assert!(matches!(client.account_status().await, Err(ApiError::NoToken)));
        assert!(matches!(client.tracks_file_info("1", "lossless").await, Err(ApiError::NoToken)));
        assert!(client.tracks(&[]).await.unwrap().is_empty());
        let client = YmClient::new(Some("  ".into())).unwrap();
        assert!(client.token().is_none());
    }

    /// Мини-сервер: принимает один запрос, отвечает заданным статусом и телом, возвращает текст запроса.
    async fn serve_once(status: u16, body: &'static str) -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let length = text[..head_end]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if request.len() >= head_end + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        (format!("http://{addr}"), handle)
    }

    fn local_client(base: &str, token: Option<&str>) -> YmClient {
        let config = ClientConfig { base_url: base.to_owned(), oauth_url: base.to_owned(), ..ClientConfig::default() };
        YmClient::with_config(token.map(str::to_owned), config).unwrap()
    }

    #[tokio::test]
    async fn tracks_request_and_parse() {
        let (base, server) = serve_once(200, r#"{"invocationInfo":{},"result":[{"id":1,"title":"A"},null,{"id":"2"}]}"#).await;
        let client = local_client(&base, Some("tok"));
        let tracks = client.tracks(&["1:10".into(), "2".into()]).await.unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[1].id.as_ref().unwrap().0, "2");
        let request = server.await.unwrap();
        let lower = request.to_ascii_lowercase();
        assert!(request.starts_with("POST /tracks HTTP/1.1"));
        assert!(lower.contains("authorization: oauth tok"));
        assert!(lower.contains("x-yandex-music-client: yandexmusicandroid/24023621"));
        assert!(lower.contains("user-agent: yandex-music-api"));
        assert!(lower.contains("content-type: application/x-www-form-urlencoded"));
        assert!(request.ends_with("track-ids=1%3A10%2C2&with-positions=True"));
    }

    #[tokio::test]
    async fn oauth_pending_is_none() {
        let (base, server) = serve_once(400, r#"{"error":"authorization_pending","error_description":"wait"}"#).await;
        let client = local_client(&base, None);
        assert!(client.poll_device_token("dc").await.unwrap().is_none());
        let request = server.await.unwrap();
        assert!(request.starts_with("POST /token HTTP/1.1"));
        assert!(!request.to_ascii_lowercase().contains("x-yandex-music-client"));
        assert!(request.contains("grant_type=device_code&code=dc&client_id=23cabbbdc6cd418abb4b39c32c41195d"));
    }

    #[tokio::test]
    async fn lyrics_not_found_and_unauthorized() {
        let (base, _server) = serve_once(404, r#"{"error":{"name":"not-found","message":"no lyrics"}}"#).await;
        assert!(local_client(&base, Some("t")).tracks_lyrics("5:6", LyricsFormat::Lrc).await.unwrap().is_none());

        let (base, _server) = serve_once(401, r#"{"error":{"name":"session-expired","message":"expired"}}"#).await;
        let error = local_client(&base, Some("t")).account_status().await.unwrap_err();
        assert!(matches!(error, ApiError::Unauthorized(ref m) if m == "expired"));
    }

    #[tokio::test]
    async fn file_info_request() {
        let (base, server) = serve_once(
            200,
            r#"{"result":{"downloadInfo":{"trackId":"7","quality":"lossless","codec":"flac","bitrate":0,"transport":"raw","url":"https://x","urls":["https://y"]}}}"#,
        )
        .await;
        let info = local_client(&base, Some("t")).tracks_file_info("7", "lossless").await.unwrap();
        assert_eq!(info.best_url(), Some("https://y"));
        let request = server.await.unwrap();
        assert!(request.to_ascii_lowercase().contains("x-yandex-music-client: yandexmusicdesktopappwindows/5.95.0"));
        assert!(request.contains("trackId=7&quality=lossless&codecs=flac%2Cflac-mp4%2Caac-mp4%2Che-aac-mp4%2Caac%2Che-aac%2Cmp3&transports=raw&sign="));
    }

    #[tokio::test]
    async fn playlist_change_form() {
        let (base, server) = serve_once(200, r#"{"result":{"uid":1,"kind":3,"revision":8}}"#).await;
        let playlist = local_client(&base, Some("t")).users_playlists_insert_track(1, 3, 7, 0, "11", "22").await.unwrap();
        assert_eq!(playlist.revision, Some(8));
        let request = server.await.unwrap();
        assert!(request.starts_with("POST /users/1/playlists/3/change HTTP/1.1"));
        let body = request.rsplit("\r\n\r\n").next().unwrap();
        let expected = format!(
            "kind=3&revision=7&diff={}",
            "%5B%7B%22op%22%3A%22insert%22%2C%22at%22%3A0%2C%22tracks%22%3A%5B%7B%22id%22%3A%2211%22%2C%22albumId%22%3A%2222%22%7D%5D%7D%5D"
        );
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn unpin_wave_json() {
        let (base, server) = serve_once(200, r#"{"result":"ok"}"#).await;
        local_client(&base, Some("t")).unpin_wave("track:1").await.unwrap();
        let request = server.await.unwrap();
        assert!(request.starts_with("DELETE /pin/wave HTTP/1.1"));
        assert!(request.ends_with(r#"{"seeds":"track:1"}"#));
    }

    #[test]
    #[ignore = "сеть: настоящий API"]
    fn live_search() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let found = rt.block_on(YmClient::new(None).unwrap().search("Кино", SearchType::All, false, 0)).unwrap();
        assert!(found.tracks.is_some());
    }
}
