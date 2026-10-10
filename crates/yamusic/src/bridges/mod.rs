//! QML-синглтоны YaMusic.Core (владелец — агент «bridges»). Каждый файл с #[cxx_qt::bridge]
//! подхватывается build.rs автоматически; здесь — объявления модулей и общие помощники мостов.
pub mod auth;
pub mod catalog;
pub mod data;
pub mod library;
pub mod lyrics;
pub mod player;
pub mod settings;
pub mod theme;
pub mod wave;

use std::pin::Pin;
use std::time::{Duration, Instant};

use cxx::UniquePtr;
use cxx_qt::casting::Upcast;
use cxx_qt::{CxxQtThread, Threading};
use serde_json::Value;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::watch;
use ym_api::items::TrackItem;

use crate::context::ctx;
use crate::models::DictListModel;
use crate::qt::{QVariantMap, qvariantmap_to_json};

/// Что пришло из broadcast-канала сервиса.
pub(crate) enum Delivery<E> {
    Event(E),
    /// пропущены события — мост перечитывает состояние сервиса целиком
    Lagged,
}

/// Слушатель событий сервиса на tokio: каждое событие переносится в GUI-поток.
/// Канал закрыт или объект уничтожен — задача завершается.
pub(crate) fn forward_events<T, E>(
    thread: CxxQtThread<T>,
    mut events: broadcast::Receiver<E>,
    apply: fn(Pin<&mut T>, Delivery<E>),
) where
    T: Threading + 'static,
    E: Clone + Send + 'static,
{
    ctx().rt.spawn(async move {
        loop {
            let delivery = match events.recv().await {
                Ok(event) => Delivery::Event(event),
                Err(RecvError::Lagged(_)) => Delivery::Lagged,
                Err(RecvError::Closed) => break,
            };
            if thread.queue(move |object| apply(object, delivery)).is_err() {
                break;
            }
        }
    });
}

/// Слушатель снимка (watch): каждое изменение — снимок в GUI-поток.
pub(crate) fn forward_watch<T, S>(thread: CxxQtThread<T>, mut snapshot: watch::Receiver<S>, apply: fn(Pin<&mut T>, S))
where
    T: Threading + 'static,
    S: Clone + Send + Sync + 'static,
{
    ctx().rt.spawn(async move {
        while snapshot.changed().await.is_ok() {
            let value = snapshot.borrow_and_update().clone();
            if thread.queue(move |object| apply(object, value)).is_err() {
                break;
            }
        }
    });
}

/// Модель во владении моста → указатель для QML (свойство CONSTANT или возврат из invokable).
pub(crate) fn model_ptr(model: &UniquePtr<DictListModel>) -> *mut cxx_qt::QObject {
    model.as_ref().map_or(std::ptr::null_mut(), |m| unsafe {
        <DictListModel as Upcast<cxx_qt::QObject>>::upcast_ptr(m).cast_mut()
    })
}

/// Применить изменение к модели, если она есть.
pub(crate) fn with_model(model: &mut UniquePtr<DictListModel>, f: impl FnOnce(Pin<&mut DictListModel>)) {
    if let Some(m) = model.as_mut() {
        f(m);
    }
}

/// Сколько держать локально записанное значение, не принимая «эхо» из сервиса (оно может быть старше записи).
pub(crate) const ECHO_HOLD: Duration = Duration::from_millis(500);

pub(crate) fn held(since: Option<Instant>) -> bool {
    since.is_some_and(|at| at.elapsed() < ECHO_HOLD)
}

/// Словарь из QML → JSON с числами, приведёнными как в Python (5.0 → 5; id числом → строка).
pub(crate) fn map_to_json(map: &QVariantMap) -> serde_json::Map<String, Value> {
    let mut object = qvariantmap_to_json(map);
    for (key, value) in object.iter_mut() {
        normalize(key, value);
    }
    object
}

fn normalize(key: &str, value: &mut Value) {
    match value {
        Value::Number(n) if matches!(key, "trackId" | "albumId" | "artistId" | "id") => {
            let text = match n.as_i64() {
                Some(i) => i.to_string(),
                None => n.as_f64().map(|f| if f.fract() == 0.0 { (f as i64).to_string() } else { f.to_string() })
                    .unwrap_or_default(),
            };
            *value = Value::String(text);
        }
        Value::Number(n) => {
            if n.as_i64().is_none() && n.as_u64().is_none()
                && let Some(f) = n.as_f64()
                && f.fract() == 0.0
                && f.abs() < 9.0e15
            {
                *value = Value::from(f as i64);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| normalize("", item)),
        Value::Object(map) => map.iter_mut().for_each(|(k, v)| normalize(k, v)),
        _ => {}
    }
}

/// Словарь трека из QML → TrackItem (неполный словарь — поля по умолчанию; без trackId — None).
pub(crate) fn track_from_map(map: &QVariantMap) -> Option<TrackItem> {
    let track: TrackItem = serde_json::from_value(Value::Object(map_to_json(map))).ok()?;
    (!track.track_id.is_empty()).then_some(track)
}

/// file:// URL → локальный путь, иначе строка как есть (QUrl.isLocalFile в Python).
pub(crate) fn local_path(source: &cxx_qt_lib::QString) -> String {
    let url = cxx_qt_lib::QUrl::from(source);
    if url.is_local_file() { url.to_local_file_or_default().to_string() } else { source.to_string() }
}

/// Строковое значение ключа словаря (число — текстом).
pub(crate) fn json_str(object: &serde_json::Map<String, Value>, key: &str) -> String {
    match object.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}
