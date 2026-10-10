//! session.json — совместим с Python-версией:
//! {"items": [TrackItem…], "order": [int…], "pos": int, "positionMs": int, "context": {…}, "shuffle": bool, "repeat": "off"|"all"|"one"}.
//! Контекст волны сохраняется как {"type": "restored", "from": <from>}. Запись — через .part + rename.
//! Восстановление отбрасывает файл, если items пуст, pos вне order или sorted(order) != 0..len(items).
use std::path::Path;

use serde::{Deserialize, Serialize};
use ym_api::items::{PlayContext, TrackItem};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionData {
    pub items: Vec<TrackItem>,
    pub order: Vec<usize>,
    pub pos: usize,
    pub position_ms: i64,
    pub context: PlayContext,
    pub shuffle: bool,
    pub repeat: String,
}

impl SessionData {
    /// Контекст для записи: волна без своей сессии — просто список треков.
    pub fn saved_context(context: &PlayContext) -> PlayContext {
        if context.kind == "wave" { PlayContext::new("restored", &context.from) } else { context.clone() }
    }

    fn is_valid(&self) -> bool {
        if self.items.is_empty() || self.pos >= self.order.len() {
            return false;
        }
        let mut sorted = self.order.clone();
        sorted.sort_unstable();
        sorted.iter().copied().eq(0..self.items.len())
    }
}

pub fn load(path: &Path) -> Option<SessionData> {
    let text = std::fs::read_to_string(path).ok()?;
    parse(&text)
}

/// Разбор как restore_session в Python: items/order/pos обязательны, null в остальных — как отсутствие.
pub fn parse(text: &str) -> Option<SessionData> {
    let serde_json::Value::Object(mut map) = serde_json::from_str(text).ok()? else {
        return None;
    };
    if ["items", "order", "pos"].iter().any(|k| !map.contains_key(*k)) {
        return None;
    }
    map.retain(|_, v| !v.is_null());
    // «context» в Python — произвольный словарь; лишние/нестроковые ключи не должны ломать восстановление
    if let Some(serde_json::Value::Object(ctx)) = map.get_mut("context") {
        ctx.retain(|_, v| v.is_string());
    } else {
        map.remove("context");
    }
    if !map.get("repeat").is_some_and(|v| v.is_string()) {
        map.remove("repeat");
    }
    if let Some(v) = map.get("shuffle")
        && !v.is_boolean()
    {
        let truthy = match v {
            serde_json::Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(o) => !o.is_empty(),
            _ => false,
        };
        map.insert("shuffle".into(), truthy.into());
    }
    if let Some(v) = map.get("positionMs")
        && !v.is_i64()
    {
        let ms = v.as_f64().map_or(0, |f| f as i64);
        map.insert("positionMs".into(), ms.into());
    }
    let data: SessionData = serde_json::from_value(serde_json::Value::Object(map)).ok()?;
    data.is_valid().then_some(data)
}

pub fn save(path: &Path, data: &SessionData) -> std::io::Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string(data).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Так пишет Python: json.dumps(data, ensure_ascii=False)
    const PYTHON: &str = r#"{"items": [{"trackId": "1", "albumId": "10", "title": "Песня", "version": "", "artists": "Исполнитель, Другой", "artistRefs": [{"id": "5", "name": "Исполнитель"}], "album": "Альбом", "cover": "https://avatars.yandex.net/get-music-content/x/400x400", "durationMs": 215000, "explicit": false, "available": true}, {"trackId": "2", "albumId": "", "title": "B", "version": "remix", "artists": "", "artistRefs": [], "album": "", "cover": "", "durationMs": 1000, "explicit": true, "available": false}], "order": [1, 0], "pos": 1, "positionMs": 61234, "context": {"type": "playlist", "from": "desktop_win-playlist-track-default", "playlistId": "42:3"}, "shuffle": true, "repeat": "all"}"#;

    #[test]
    fn reads_python_session() {
        let data = parse(PYTHON).expect("valid session");
        assert_eq!(data.items.len(), 2);
        assert_eq!(data.items[0].title, "Песня");
        assert_eq!(data.items[0].artist_refs[0].name, "Исполнитель");
        assert_eq!(data.items[1].duration_ms, 1000);
        assert!(data.items[1].explicit && !data.items[1].available);
        assert_eq!(data.order, vec![1, 0]);
        assert_eq!(data.pos, 1);
        assert_eq!(data.position_ms, 61234);
        assert_eq!(data.context.playlist_id.as_deref(), Some("42:3"));
        assert!(data.shuffle);
        assert_eq!(data.repeat, "all");
    }

    #[test]
    fn writes_python_compatible_json() {
        let data = parse(PYTHON).unwrap();
        let ours: serde_json::Value = serde_json::to_value(&data).unwrap();
        let python: serde_json::Value = serde_json::from_str(PYTHON).unwrap();
        assert_eq!(ours, python);
    }

    #[test]
    fn rejects_invalid() {
        for text in [
            "",
            "[]",
            r#"{"items": [], "order": [], "pos": 0}"#,
            r#"{"items": [{"trackId": "1"}], "order": [0], "pos": 1}"#,
            r#"{"items": [{"trackId": "1"}], "order": [1], "pos": 0}"#,
            r#"{"items": [{"trackId": "1"}, {"trackId": "2"}], "order": [0, 0], "pos": 0}"#,
            r#"{"items": [{"trackId": "1"}], "order": [0]}"#,
            r#"{"items": [{"trackId": "1"}], "order": [0], "pos": -1}"#,
        ] {
            assert!(parse(text).is_none(), "{text}");
        }
    }

    #[test]
    fn tolerates_missing_optional() {
        let data = parse(r#"{"items": [{"trackId": "1"}], "order": [0], "pos": 0, "context": null, "repeat": 5, "positionMs": 12.7}"#)
            .unwrap();
        assert_eq!(data.context, PlayContext::default());
        assert_eq!(data.repeat, "");
        assert_eq!(data.position_ms, 12);
        assert!(!data.shuffle);
    }

    #[test]
    fn wave_context_saved_as_restored() {
        let mut wave = PlayContext::new("wave", "desktop_win-radio-user-onyourwave-default");
        wave.playlist_id = Some("x".into());
        let saved = SessionData::saved_context(&wave);
        assert_eq!(saved, PlayContext::new("restored", "desktop_win-radio-user-onyourwave-default"));
        let liked = PlayContext::new("liked", "f");
        assert_eq!(SessionData::saved_context(&liked), liked);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ym-player-session-{}", fastrand::u64(..)));
        let path = dir.join("sub").join("session.json");
        let data = parse(PYTHON).unwrap();
        save(&path, &data).unwrap();
        assert!(!path.with_extension("part").exists());
        assert_eq!(load(&path), Some(data));
        let _ = std::fs::remove_dir_all(dir);
    }
}
