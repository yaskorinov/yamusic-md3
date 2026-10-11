//! Синхронные тексты — порт src/yamusic/lyrics_sources.py (+ кэш из lyrics.py).
//!
//! Цепочка источников (fetch): NetEase YRC (по словам) → Musixmatch richsync (по словам) → LRCLIB (строки)
//! → Musixmatch subtitles → NetEase LRC → Яндекс LRC (последним). У построчных текстов время слов интерполируется.
//! Формат результата и кэша (JSON) — байт-в-байт по ключам как в Python, поэтому старый кэш читается.
pub mod lrc;
pub mod sources;
pub mod text;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::future::BoxFuture;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use sha1::{Digest, Sha1};

use crate::lrc::{add_gaps, build_from_timed_lines, lines_from_lrc, parse_yrc};
use crate::text::is_placeholder;

/// 4 — Яндекс в конце цепочки; 5 — заглушки NetEase «текста нет» пропускаются;
/// 6 — LRCLIB: нечёткое название и разметка, с которой согласно большинство записей
pub const CACHE_VERSION: u32 = 6;
/// «Не найдено» перепроверяется через 6 часов
pub const NOT_FOUND_TTL_SECS: u64 = 6 * 3600;
/// Таймаут HTTP источников, с (2 повтора на 5xx/сеть с паузой 0,7·(n+1) с)
pub const TIMEOUT_SECS: u64 = 8;
/// Тишина ≥ 4,5 с — вставка «• • •» (gap)
pub const GAP_MIN: f64 = 4.5;
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

/// Что ищем. title уже с версией: "Title (Version)", если версия есть.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LyricsQuery {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// длительность трека, секунды
    pub duration: f64,
}

/// Слово: {"x": текст (с хвостовым пробелом), "s": начало, "e": конец, "p": [[символов, s, e], …]?}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub x: String,
    pub s: f64,
    pub e: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub p: Option<Vec<(u32, f64, f64)>>,
}

/// Строка текста {"t","e","w","i"} или пауза {"gap": true, "t", "e"} — порядок вариантов важен для untagged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Line {
    Gap { gap: bool, t: f64, e: f64 },
    Words { t: f64, e: f64, w: Vec<Word>, i: bool },
}

impl Line {
    /// "t" — начало
    pub fn start(&self) -> f64 {
        match self {
            Self::Gap { t, .. } | Self::Words { t, .. } => *t,
        }
    }

    /// "e" — конец
    pub fn end(&self) -> f64 {
        match self {
            Self::Gap { e, .. } | Self::Words { e, .. } => *e,
        }
    }

    /// Текст строки (слова подряд); у паузы — пусто
    pub fn text(&self) -> String {
        match self {
            Self::Gap { .. } => String::new(),
            Self::Words { w, .. } => w.iter().map(|w| w.x.as_str()).collect(),
        }
    }
}

/// Результат fetch и содержимое файла кэша:
/// {"ok": true, "source": "NetEase"|"Musixmatch"|"LRCLIB"|"Яндекс", "kind": "word"|"line", "lines": [...]}
/// или {"ok": false, "error": "not found", "details": ["источник: ошибка", …]}.
/// Сериализация — вручную, ровно этими двумя формами (как dict в Python).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LyricsResult {
    pub ok: bool,
    pub source: String,
    pub kind: String,
    pub lines: Vec<Line>,
    pub error: Option<String>,
    pub details: Vec<String>,
}

impl LyricsResult {
    pub fn found(source: &str, kind: &str, lines: Vec<Line>) -> Self {
        Self { ok: true, source: source.into(), kind: kind.into(), lines, ..Default::default() }
    }

    pub fn not_found(details: Vec<String>) -> Self {
        Self { ok: false, error: Some("not found".into()), details, ..Default::default() }
    }

    /// JSON как json.dumps(result, ensure_ascii=False) в Python (разделители ", " и ": ")
    pub fn to_python_json(&self) -> String {
        let mut out = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut out, PyFormatter);
        // запись в Vec не падает
        let _ = self.serialize(&mut ser);
        String::from_utf8(out).unwrap_or_default()
    }
}

impl Serialize for LyricsResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("ok", &self.ok)?;
        if self.ok {
            map.serialize_entry("source", &self.source)?;
            map.serialize_entry("kind", &self.kind)?;
            map.serialize_entry("lines", &self.lines)?;
        } else {
            map.serialize_entry("error", self.error.as_deref().unwrap_or("not found"))?;
            map.serialize_entry("details", &self.details)?;
        }
        map.end()
    }
}

/// Разделители json.dumps по умолчанию
struct PyFormatter;

impl serde_json::ser::Formatter for PyFormatter {
    fn begin_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W, first: bool) -> io::Result<()> {
        if first { Ok(()) } else { writer.write_all(b", ") }
    }

    fn begin_object_key<W: ?Sized + io::Write>(&mut self, writer: &mut W, first: bool) -> io::Result<()> {
        if first { Ok(()) } else { writer.write_all(b", ") }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }
}

/// LRC от Яндекса (замыкание из ym-services поверх YmClient::tracks_lyrics + download_text). Ok(None) — нет текста.
pub type YandexLrc = Arc<dyn Fn() -> BoxFuture<'static, Result<Option<String>, String>> + Send + Sync>;

/// Имя файла кэша: sha1(f"{CACHE_VERSION}|{track_id}") hex + ".json"
pub fn cache_file_name(track_id: &str) -> String {
    cache_file_name_v(CACHE_VERSION, track_id)
}

/// То же для явной версии (эталоны Python посчитаны для версии 5)
pub fn cache_file_name_v(version: u32, track_id: &str) -> String {
    let key = Sha1::digest(format!("{version}|{track_id}").as_bytes());
    format!("{}.json", hex::encode(key))
}

/// Ошибка источника → в details (как attempt в Python)
fn attempt<T>(errors: &mut Vec<String>, name: &str, r: Result<T, String>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::debug!("lyrics: {name} failed: {e}");
            errors.push(format!("{name}: {e}"));
            None
        }
    }
}

/// Источник текстов с дисковым кэшем.
#[derive(Clone)]
pub struct LyricsFetcher {
    cache_dir: PathBuf,
    http: reqwest::Client,
}

impl LyricsFetcher {
    /// `cache_dir` — dirs::cache_subdir("lyrics"); в нём же mxm_token.json.
    pub fn new(cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .unwrap_or_default();
        Self::with_client(cache_dir, http)
    }

    /// Со своим HTTP-клиентом (заголовки и таймаут источники всё равно ставят на каждый запрос).
    pub fn with_client(cache_dir: PathBuf, http: reqwest::Client) -> Self {
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!("lyrics: каталог кэша {} не создан: {e}", cache_dir.display());
        }
        Self { cache_dir, http }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// <cache_dir>/<sha1>.json
    pub fn cache_path(&self, track_id: &str) -> PathBuf {
        self.cache_dir.join(cache_file_name(track_id))
    }

    /// Свежая запись кэша: ok — навсегда, not found — пока младше NOT_FOUND_TTL_SECS.
    pub async fn read_cache(&self, track_id: &str) -> Option<LyricsResult> {
        let path = self.cache_path(track_id);
        let raw = tokio::fs::read_to_string(&path).await.ok()?;
        let cached: LyricsResult = serde_json::from_str(&raw).ok()?;
        if cached.ok {
            return Some(cached);
        }
        let mtime = tokio::fs::metadata(&path).await.ok()?.modified().ok()?;
        // mtime из будущего — тоже свежий, как в Python
        let age = SystemTime::now().duration_since(mtime).unwrap_or_default();
        (age < Duration::from_secs(NOT_FOUND_TTL_SECS)).then_some(cached)
    }

    /// Запись через .part + rename
    pub async fn write_cache(&self, track_id: &str, result: &LyricsResult) -> io::Result<()> {
        let path = self.cache_path(track_id);
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        let tmp = path.with_extension("part");
        tokio::fs::write(&tmp, result.to_python_json()).await?;
        tokio::fs::rename(&tmp, &path).await
    }

    /// Как _fetch_cached в lyrics.py: файл <cache_dir>/<sha1(f"{CACHE_VERSION}|{track_id}")>.json;
    /// ok — навсегда, not found — NOT_FOUND_TTL_SECS по mtime. Найденное проходит add_gaps(lines, duration).
    /// Результат с непустым details (источник ответил ошибкой) не кэшируется. Запись — через .part + rename.
    pub async fn fetch_cached(&self, query: &LyricsQuery, yandex: Option<YandexLrc>) -> LyricsResult {
        if let Some(cached) = self.read_cache(&query.track_id).await {
            return cached;
        }
        let mut result = self.fetch(query, "", yandex).await;
        if result.ok {
            result.lines = add_gaps(std::mem::take(&mut result.lines), query.duration);
        }
        // Источник с ошибкой (503, таймаут) мог знать текст — такое «не найдено» не кэшируем
        if (result.ok || result.details.is_empty())
            && let Err(e) = self.write_cache(&query.track_id, &result).await
        {
            tracing::warn!("lyrics: кэш {} не записан: {e}", query.track_id);
        }
        result
    }

    /// Цепочка источников без кэша (fetch в lyrics_sources.py); mxm_user_token — пусто = анонимный токен.
    pub async fn fetch(&self, query: &LyricsQuery, mxm_user_token: &str, yandex: Option<YandexLrc>) -> LyricsResult {
        let http = &self.http;
        let duration = query.duration;
        let mut errors = Vec::new();

        // 1. NetEase YRC (по словам)
        let mut ne_data = None;
        if let Some(song) = attempt(&mut errors, "netease-search", sources::ne_search(http, query).await).flatten() {
            tracing::debug!("lyrics: netease match {} {}", song["id"], song["name"]);
            let lyric = match song.get("id") {
                Some(id) => sources::ne_lyric_by_id(http, &py_id(id)).await,
                None => Err("'id'".to_owned()),
            };
            ne_data = attempt(&mut errors, "netease-lyric", lyric);
            let yrc = ne_data.as_ref().and_then(|d| d.get("yrc")?.get("lyric")?.as_str()).unwrap_or("");
            if !yrc.is_empty() {
                let lines = parse_yrc(yrc);
                let texts: Vec<String> = lines.iter().map(Line::text).collect();
                if !lines.is_empty() && is_placeholder(texts.iter().map(String::as_str)) {
                    tracing::debug!("lyrics: netease yrc: placeholder, skip");
                } else if !lines.is_empty() {
                    return LyricsResult::found("NetEase", "word", lines);
                }
            }
        }

        // 2. Musixmatch richsync (по словам) — заодно даёт построчный запасной вариант
        let mut mxm_line = Vec::new();
        let token = sources::mxm_token(http, &self.cache_dir, mxm_user_token).await;
        let token = attempt(&mut errors, "musixmatch-token", token).flatten().filter(|t| !t.is_empty());
        if let Some(token) = token {
            match attempt(&mut errors, "musixmatch", sources::mxm_fetch(http, query, &token).await).flatten() {
                Some(("word", lines)) => return LyricsResult::found("Musixmatch", "word", lines),
                Some((_, lines)) => mxm_line = lines,
                None => {}
            }
        }

        // 3. LRCLIB (по строкам, интерполяция)
        let lrc = attempt(&mut errors, "lrclib", sources::lrclib_fetch(http, query).await).flatten();
        if let Some(lrc) = lrc.filter(|l| !l.is_empty()) {
            let lines = build_from_timed_lines(&lines_from_lrc(&lrc), duration);
            if !lines.is_empty() {
                return LyricsResult::found("LRCLIB", "line", lines);
            }
        }

        // 4. Musixmatch subtitles
        if !mxm_line.is_empty() {
            return LyricsResult::found("Musixmatch", "line", mxm_line);
        }

        // 5. NetEase LRC
        let ne_lrc = ne_data.as_ref().and_then(|d| d.get("lrc")?.get("lyric")?.as_str()).unwrap_or("");
        if !ne_lrc.is_empty() {
            let pairs = lines_from_lrc(ne_lrc);
            if !is_placeholder(pairs.iter().map(|(_, t)| t.as_str())) {
                let lines = build_from_timed_lines(&pairs, duration);
                if !lines.is_empty() {
                    return LyricsResult::found("NetEase", "line", lines);
                }
            }
        }

        // 6. Яндекс — последним: синхронизация у него грубее, но русскую музыку он знает почти всю
        if let Some(yandex) = yandex {
            let lrc = attempt(&mut errors, "yandex", yandex().await).flatten();
            if let Some(lrc) = lrc.filter(|l| !l.is_empty()) {
                let lines = build_from_timed_lines(&lines_from_lrc(&lrc), duration);
                if !lines.is_empty() {
                    return LyricsResult::found("Яндекс", "line", lines);
                }
            }
        }

        LyricsResult::not_found(errors)
    }
}

/// str(song["id"]) Python
fn py_id(id: &serde_json::Value) -> String {
    match id {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LyricsResult {
        LyricsResult::found(
            "Яндекс",
            "line",
            vec![
                Line::Gap { gap: true, t: 0.0, e: 4.85 },
                Line::Words {
                    t: 5.0,
                    e: 6.5,
                    w: vec![Word { x: "раз ".into(), s: 5.0, e: 5.5, p: Some(vec![(1, 5.0, 5.25), (2, 5.25, 5.5)]) }],
                    i: true,
                },
            ],
        )
    }

    #[test]
    fn result_json_is_python_shaped() {
        assert_eq!(
            sample().to_python_json(),
            r#"{"ok": true, "source": "Яндекс", "kind": "line", "lines": [{"gap": true, "t": 0.0, "e": 4.85}, {"t": 5.0, "e": 6.5, "w": [{"x": "раз ", "s": 5.0, "e": 5.5, "p": [[1, 5.0, 5.25], [2, 5.25, 5.5]]}], "i": true}]}"#
        );
        assert_eq!(
            LyricsResult::not_found(vec![]).to_python_json(),
            r#"{"ok": false, "error": "not found", "details": []}"#
        );
    }

    #[test]
    fn result_json_roundtrips() {
        let r = sample();
        assert_eq!(serde_json::from_str::<LyricsResult>(&r.to_python_json()).unwrap(), r);
        let nf = LyricsResult::not_found(vec!["lrclib: HTTP Error 503: Service Unavailable".into()]);
        assert_eq!(serde_json::from_str::<LyricsResult>(&serde_json::to_string(&nf).unwrap()).unwrap(), nf);
    }

    #[test]
    fn cache_key_matches_python() {
        // hashlib.sha1(b"5|12345").hexdigest()
        assert_eq!(cache_file_name_v(5, "12345"), "e13466f72878a42f8fe435c7ee2b441ee93e2d57.json");
        assert_ne!(cache_file_name("12345"), cache_file_name_v(5, "12345"));
    }

    #[tokio::test]
    async fn cache_is_read_back_and_not_found_expires() {
        let dir = std::env::temp_dir().join(format!("ym-lyrics-test-{}", std::process::id()));
        let fetcher = LyricsFetcher::new(dir.clone());
        fetcher.write_cache("1", &sample()).await.unwrap();
        assert_eq!(fetcher.read_cache("1").await, Some(sample()));
        assert!(!dir.join(cache_file_name("1")).with_extension("part").exists());

        let nf = LyricsResult::not_found(vec![]);
        fetcher.write_cache("2", &nf).await.unwrap();
        assert_eq!(fetcher.read_cache("2").await, Some(nf));
        // устаревшее «не найдено» перепроверяется
        let old = SystemTime::now() - Duration::from_secs(NOT_FOUND_TTL_SECS + 60);
        std::fs::File::options().write(true).open(fetcher.cache_path("2")).unwrap().set_modified(old).unwrap();
        assert_eq!(fetcher.read_cache("2").await, None);
        assert_eq!(fetcher.read_cache("missing").await, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn cached_result_skips_sources() {
        let dir = std::env::temp_dir().join(format!("ym-lyrics-test-cached-{}", std::process::id()));
        let fetcher = LyricsFetcher::new(dir.clone());
        fetcher.write_cache("42", &sample()).await.unwrap();
        let query = LyricsQuery { track_id: "42".into(), title: "x".into(), ..Default::default() };
        assert_eq!(fetcher.fetch_cached(&query, None).await, sample());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Живая проверка источников: `YAMUSIC_LYRICS_LIVE=1 cargo test -p ym-lyrics live -- --nocapture`.
    #[tokio::test]
    async fn live_sources_find_a_known_song() {
        if std::env::var("YAMUSIC_LYRICS_LIVE").as_deref() != Ok("1") {
            return;
        }
        let dir = std::env::temp_dir().join("ym-lyrics-live");
        let fetcher = LyricsFetcher::new(dir);
        for (title, artist, duration) in [("Creep", "Radiohead", 238.0), ("Группа крови", "Кино", 286.0)] {
            let query = LyricsQuery { title: title.into(), artist: artist.into(), duration, ..Default::default() };
            if let Ok(Some(song)) = sources::ne_search(&fetcher.http, &query).await {
                let data = sources::ne_lyric_by_id(&fetcher.http, &py_id(&song["id"])).await;
                let has = |k: &str| data.as_ref().ok().and_then(|d| d[k]["lyric"].as_str()).is_some_and(|s| !s.is_empty());
                println!("netease {} {}: yrc {} lrc {}", song["id"], song["name"], has("yrc"), has("lrc"));
            }
            let r = fetcher.fetch(&query, "", None).await;
            println!("{artist} — {title}: {} {} строк {} ошибки {:?}", r.source, r.kind, r.lines.len(), r.details);
            let first: Vec<String> = r.lines.iter().take(3).map(Line::text).collect();
            println!("   {}", first.join(" | "));
            assert!(r.ok);
        }
    }
}
