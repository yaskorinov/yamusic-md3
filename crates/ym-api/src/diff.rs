//! Изменения плейлиста (yandex_music/utils/difference.py).
//! diff — JSON-массив операций: [{"op":"insert","at":0,"tracks":[{"id":"123","albumId":"456"}]}]
//! или [{"op":"delete","from":0,"to":1}]. Внимание: ключ альбома в JSON — "albumId".
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DiffTrack {
    pub id: String,
    #[serde(rename = "albumId")]
    pub album_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum DiffOp {
    Insert { at: i64, tracks: Vec<DiffTrack> },
    Delete { from: i64, to: i64 },
}

impl DiffOp {
    /// Вставка одного трека в позицию `at`.
    pub fn insert_one(at: i64, track_id: &str, album_id: &str) -> Self {
        Self::Insert { at, tracks: vec![DiffTrack { id: track_id.to_owned(), album_id: album_id.to_owned() }] }
    }
}

/// JSON-строка для поля формы diff.
pub fn to_json(ops: &[DiffOp]) -> String {
    serde_json::to_string(ops).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_delete() {
        let ops = [DiffOp::insert_one(0, "123", "456"), DiffOp::Delete { from: 2, to: 3 }];
        assert_eq!(
            to_json(&ops),
            r#"[{"op":"insert","at":0,"tracks":[{"id":"123","albumId":"456"}]},{"op":"delete","from":2,"to":3}]"#
        );
    }
}
