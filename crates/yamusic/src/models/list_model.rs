//! DictListModel — список словарей с фиксированным набором ключей; ключи = роли QML.
//! Замена TrackListModel/AlbumListModel/ArtistListModel/PlaylistListModel из models.py:
//! QML обращается к ним только через count, loading, totalDurationMs, get(), find() и роли.
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::CxxQtType;
use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};
use serde_json::{Map, Value};

use crate::qt::{QVariantMap, json_to_qvariant, json_to_qvariantmap, qvariant_to_json};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
    }

    extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qproperty(i32, count, READ = count, NOTIFY = count_changed)]
        #[qproperty(bool, loading, READ, NOTIFY = loading_changed)]
        #[qproperty(i64, total_duration_ms, cxx_name = "totalDurationMs", READ = total_duration_ms, NOTIFY = count_changed)]
        type DictListModel = super::DictListModelRust;

        fn count(self: &DictListModel) -> i32;
        #[cxx_name = "totalDurationMsValue"]
        fn total_duration_ms(self: &DictListModel) -> i64;

        #[qsignal]
        #[cxx_name = "countChanged"]
        fn count_changed(self: Pin<&mut DictListModel>);
        #[qsignal]
        #[cxx_name = "loadingChanged"]
        fn loading_changed(self: Pin<&mut DictListModel>);

        #[qinvokable]
        fn get(self: &DictListModel, row: i32) -> QMap_QString_QVariant;
        #[qinvokable]
        fn find(self: &DictListModel, key: &QString, value: &QVariant) -> i32;

        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &DictListModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        fn data(self: &DictListModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &DictListModel) -> QHash_i32_QByteArray;

        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut DictListModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut DictListModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(self: Pin<&mut DictListModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut DictListModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(self: Pin<&mut DictListModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut DictListModel>);
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[doc(hidden)]
        #[namespace = "rust::cxxqtlib1"]
        #[cxx_name = "make_unique"]
        fn dict_list_model_make_unique() -> UniquePtr<DictListModel>;
    }
}

cxx_qt::impl_transitive_cast!(qobject::DictListModel, qobject::QAbstractListModel, cxx_qt::QObject);

/// Строка модели: словарь с ключами модели. Arc — дешёвые копии и сравнение по указателю (TrackFilter).
pub type Row = Arc<Map<String, Value>>;

/// Набор ключей (= ролей QML) — ровно как в models.py.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelKind {
    #[default]
    Track,
    Album,
    Artist,
    Playlist,
}

pub const TRACK_KEYS: &[&str] = &[
    "trackId", "albumId", "title", "version", "artists", "artistRefs", "album", "cover", "durationMs", "explicit",
    "available",
];
pub const PLAYLIST_KEYS: &[&str] = &["uid", "kind", "title", "trackCount", "cover", "owner"];
pub const ALBUM_KEYS: &[&str] =
    &["albumId", "title", "version", "artists", "artistRefs", "year", "kind", "trackCount", "cover", "explicit"];
pub const ARTIST_KEYS: &[&str] = &["artistId", "name", "cover", "genres"];

impl ModelKind {
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            ModelKind::Track => TRACK_KEYS,
            ModelKind::Album => ALBUM_KEYS,
            ModelKind::Artist => ARTIST_KEYS,
            ModelKind::Playlist => PLAYLIST_KEYS,
        }
    }
}

pub(crate) const USER_ROLE: i32 = 0x0100;

#[derive(Default)]
pub struct DictListModelRust {
    kind: ModelKind,
    rows: Vec<Row>,
    loading: bool,
    /// откуда треки (для плеера и учёта прослушиваний); у моделей не-треков пустой
    context: ym_api::items::PlayContext,
}

impl qobject::DictListModel {
    /// Новая модель во владении Rust (CppOwnership уже выставлен — QML её не удалит).
    pub fn create(kind: ModelKind) -> cxx::UniquePtr<Self> {
        let mut model = qobject::dict_list_model_make_unique();
        if let Some(pinned) = model.as_mut() {
            pinned.rust_mut().kind = kind;
            let ptr = model.as_mut_ptr();
            unsafe { crate::qt::set_cpp_ownership(<Self as Upcast<cxx_qt::QObject>>::upcast_ptr(ptr).cast_mut()) };
        }
        model
    }

    pub fn kind(&self) -> ModelKind {
        self.rust().kind
    }

    pub fn rows(&self) -> &[Row] {
        &self.rust().rows
    }

    pub fn context(&self) -> &ym_api::items::PlayContext {
        &self.rust().context
    }

    pub fn set_context(self: Pin<&mut Self>, context: ym_api::items::PlayContext) {
        self.rust_mut().context = context;
    }

    /// Указатель для свойства/возврата в QML.
    pub fn as_qobject_ptr(self: Pin<&mut Self>) -> *mut cxx_qt::QObject {
        let this: *const Self = &*self;
        unsafe { <Self as Upcast<cxx_qt::QObject>>::upcast_ptr(this).cast_mut() }
    }

    pub fn reset_rows(mut self: Pin<&mut Self>, rows: Vec<Row>) {
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().rows = rows;
        self.as_mut().end_reset_model();
        self.count_changed();
    }

    pub fn append_rows(self: Pin<&mut Self>, rows: Vec<Row>) {
        let at = self.rust().rows.len();
        self.insert_rows(at, rows);
    }

    pub fn insert_rows(mut self: Pin<&mut Self>, at: usize, rows: Vec<Row>) {
        if rows.is_empty() {
            return;
        }
        let at = at.min(self.rust().rows.len());
        let last = at + rows.len() - 1;
        self.as_mut().begin_insert_rows(&QModelIndex::default(), at as i32, last as i32);
        self.as_mut().rust_mut().rows.splice(at..at, rows);
        self.as_mut().end_insert_rows();
        self.count_changed();
    }

    pub fn remove_rows(mut self: Pin<&mut Self>, at: usize, count: usize) {
        let len = self.rust().rows.len();
        if at >= len || count == 0 {
            return;
        }
        let count = count.min(len - at);
        self.as_mut().begin_remove_rows(&QModelIndex::default(), at as i32, (at + count - 1) as i32);
        self.as_mut().rust_mut().rows.drain(at..at + count);
        self.as_mut().end_remove_rows();
        self.count_changed();
    }

    pub fn set_loading(mut self: Pin<&mut Self>, value: bool) {
        if self.rust().loading != value {
            self.as_mut().rust_mut().loading = value;
            self.loading_changed();
        }
    }

    pub fn find_row(&self, key: &str, value: &Value) -> Option<usize> {
        find_in(&self.rust().rows, key, value)
    }

    fn count(&self) -> i32 {
        self.rust().rows.len() as i32
    }

    fn total_duration_ms(&self) -> i64 {
        total_duration(&self.rust().rows)
    }

    fn get(&self, row: i32) -> QVariantMap {
        row_map(&self.rust().rows, row)
    }

    fn find(&self, key: &QString, value: &QVariant) -> i32 {
        self.find_row(&key.to_string(), &qvariant_to_json(value)).map_or(-1, |i| i as i32)
    }

    fn row_count(&self, parent: &QModelIndex) -> i32 {
        if parent.is_valid() { 0 } else { self.count() }
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        row_data(&self.rust().rows, self.rust().kind.keys(), index, role)
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        role_names_for(self.rust().kind.keys())
    }
}

// Общие части DictListModel и TrackFilter

pub(crate) fn find_in(rows: &[Row], key: &str, value: &Value) -> Option<usize> {
    rows.iter().position(|row| row.get(key).is_some_and(|v| json_eq(v, value)))
}

/// sum(int(durationMs or 0)) из models.py
pub(crate) fn total_duration(rows: &[Row]) -> i64 {
    rows.iter()
        .filter_map(|r| r.get("durationMs"))
        .map(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(0))
        .sum()
}

pub(crate) fn row_map(rows: &[Row], row: i32) -> QVariantMap {
    usize::try_from(row).ok().and_then(|r| rows.get(r)).map(|r| json_to_qvariantmap(r)).unwrap_or_default()
}

pub(crate) fn row_data(rows: &[Row], keys: &[&str], index: &QModelIndex, role: i32) -> QVariant {
    if !index.is_valid() {
        return QVariant::default();
    }
    let row = usize::try_from(index.row()).ok().and_then(|r| rows.get(r));
    let key = usize::try_from(role - USER_ROLE).ok().and_then(|k| keys.get(k));
    match (row, key) {
        (Some(row), Some(key)) => row.get(*key).map(json_to_qvariant).unwrap_or_default(),
        _ => QVariant::default(),
    }
}

pub(crate) fn role_names_for(keys: &[&str]) -> QHash<QHashPair_i32_QByteArray> {
    let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
    for (i, key) in keys.iter().enumerate() {
        roles.insert(USER_ROLE + i as i32, QByteArray::from(*key));
    }
    roles
}

/// Равенство как в Python (`item.get(key) == value`): числа сравниваются по значению (5 == 5.0).
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}
