// AlbumData / ArtistData (catalog.py) — данные страниц, которые возвращают Catalog.album(id) / Catalog.artist(id).
// Создаются из Rust (make_unique) с CppOwnership и живут в кэше моста Catalog до конца процесса.
use std::pin::Pin;

use cxx::UniquePtr;
use cxx_qt::CxxQtType;
use cxx_qt::casting::Upcast;
use cxx_qt_lib::QString;
use serde::Serialize;
use ym_api::items::{PlayContext, contexts};
use ym_services::{AlbumPage, ArtistPage};

use super::{model_ptr, with_model};
use crate::models::{DictListModel, ModelKind, to_rows};
use crate::qt::{QVariantMap, json_to_qvariantmap};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QMap_QString_QVariant, info, READ, NOTIFY = changed)]
        #[qproperty(bool, loading, READ, NOTIFY = changed)]
        #[qproperty(QString, error_text, cxx_name = "errorText", READ, NOTIFY = changed)]
        #[qproperty(*mut QObject, tracks, READ = tracks_ptr, CONSTANT)]
        type AlbumData = super::AlbumDataRust;

        fn tracks_ptr(self: &AlbumData) -> *mut QObject;

        #[qsignal]
        fn changed(self: Pin<&mut AlbumData>);

        #[qobject]
        #[qproperty(QMap_QString_QVariant, info, READ, NOTIFY = changed)]
        #[qproperty(bool, loading, READ, NOTIFY = changed)]
        #[qproperty(QString, error_text, cxx_name = "errorText", READ, NOTIFY = changed)]
        #[qproperty(*mut QObject, popular, READ = popular_ptr, CONSTANT)]
        #[qproperty(*mut QObject, albums, READ = albums_ptr, CONSTANT)]
        #[qproperty(*mut QObject, also_albums, cxx_name = "alsoAlbums", READ = also_albums_ptr, CONSTANT)]
        #[qproperty(*mut QObject, similar, READ = similar_ptr, CONSTANT)]
        type ArtistData = super::ArtistDataRust;

        fn popular_ptr(self: &ArtistData) -> *mut QObject;
        fn albums_ptr(self: &ArtistData) -> *mut QObject;
        fn also_albums_ptr(self: &ArtistData) -> *mut QObject;
        fn similar_ptr(self: &ArtistData) -> *mut QObject;

        #[qsignal]
        fn changed(self: Pin<&mut ArtistData>);
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[doc(hidden)]
        #[namespace = "rust::cxxqtlib1"]
        #[cxx_name = "make_unique"]
        fn album_data_make_unique() -> UniquePtr<AlbumData>;
        #[doc(hidden)]
        #[namespace = "rust::cxxqtlib1"]
        #[cxx_name = "make_unique"]
        fn artist_data_make_unique() -> UniquePtr<ArtistData>;
    }
}

fn to_map<T: Serialize>(value: &T) -> QVariantMap {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::Object(map)) => json_to_qvariantmap(&map),
        _ => QVariantMap::default(),
    }
}

fn context(pair: (&str, &str)) -> PlayContext {
    PlayContext::new(pair.0, pair.1)
}

pub struct AlbumDataRust {
    info: QVariantMap,
    loading: bool,
    error_text: QString,
    tracks: UniquePtr<DictListModel>,
}

impl Default for AlbumDataRust {
    fn default() -> Self {
        let mut tracks = DictListModel::create(ModelKind::Track);
        with_model(&mut tracks, |m| m.set_loading(true));
        Self { info: QVariantMap::default(), loading: true, error_text: QString::default(), tracks }
    }
}

impl qobject::AlbumData {
    /// Новый объект во владении Rust (QML его не удалит).
    pub fn create() -> UniquePtr<Self> {
        let data = qobject::album_data_make_unique();
        if !data.is_null() {
            unsafe { crate::qt::set_cpp_ownership(<Self as Upcast<cxx_qt::QObject>>::upcast_ptr(data.as_mut_ptr()).cast_mut()) };
        }
        data
    }

    pub fn as_qobject_ptr(self: Pin<&mut Self>) -> *mut cxx_qt::QObject {
        let this: *const Self = &*self;
        unsafe { <Self as Upcast<cxx_qt::QObject>>::upcast_ptr(this).cast_mut() }
    }

    pub fn has_error(&self) -> bool {
        !self.error_text.is_empty()
    }

    fn tracks_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.tracks)
    }

    /// Повторная загрузка после ошибки.
    pub fn begin_loading(mut self: Pin<&mut Self>) {
        let mut this = self.as_mut().rust_mut();
        this.loading = true;
        this.error_text = QString::default();
        with_model(&mut this.tracks, |m| m.set_loading(true));
        self.changed();
    }

    pub fn set_page(mut self: Pin<&mut Self>, page: AlbumPage) {
        let mut this = self.as_mut().rust_mut();
        this.info = to_map(&page.info);
        with_model(&mut this.tracks, |mut m| {
            m.as_mut().set_context(context(contexts::ALBUM));
            m.as_mut().reset_rows(to_rows(&page.tracks));
            m.set_loading(false);
        });
        this.loading = false;
        this.error_text = QString::default();
        self.changed();
    }

    pub fn set_error(mut self: Pin<&mut Self>, text: &str) {
        let mut this = self.as_mut().rust_mut();
        with_model(&mut this.tracks, |m| m.set_loading(false));
        this.loading = false;
        this.error_text = QString::from(text);
        self.changed();
    }
}

pub struct ArtistDataRust {
    info: QVariantMap,
    loading: bool,
    error_text: QString,
    popular: UniquePtr<DictListModel>,
    albums: UniquePtr<DictListModel>,
    also_albums: UniquePtr<DictListModel>,
    similar: UniquePtr<DictListModel>,
}

impl Default for ArtistDataRust {
    fn default() -> Self {
        Self {
            info: QVariantMap::default(),
            loading: true,
            error_text: QString::default(),
            popular: DictListModel::create(ModelKind::Track),
            albums: DictListModel::create(ModelKind::Album),
            also_albums: DictListModel::create(ModelKind::Album),
            similar: DictListModel::create(ModelKind::Artist),
        }
    }
}

impl qobject::ArtistData {
    pub fn create() -> UniquePtr<Self> {
        let data = qobject::artist_data_make_unique();
        if !data.is_null() {
            unsafe { crate::qt::set_cpp_ownership(<Self as Upcast<cxx_qt::QObject>>::upcast_ptr(data.as_mut_ptr()).cast_mut()) };
        }
        data
    }

    pub fn as_qobject_ptr(self: Pin<&mut Self>) -> *mut cxx_qt::QObject {
        let this: *const Self = &*self;
        unsafe { <Self as Upcast<cxx_qt::QObject>>::upcast_ptr(this).cast_mut() }
    }

    pub fn has_error(&self) -> bool {
        !self.error_text.is_empty()
    }

    fn popular_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.popular)
    }

    fn albums_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.albums)
    }

    fn also_albums_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.also_albums)
    }

    fn similar_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.similar)
    }

    pub fn begin_loading(mut self: Pin<&mut Self>) {
        let mut this = self.as_mut().rust_mut();
        this.loading = true;
        this.error_text = QString::default();
        self.changed();
    }

    pub fn set_page(mut self: Pin<&mut Self>, page: ArtistPage) {
        let mut this = self.as_mut().rust_mut();
        this.info = to_map(&page.info);
        with_model(&mut this.popular, |mut m| {
            m.as_mut().set_context(context(contexts::ARTIST));
            m.reset_rows(to_rows(&page.popular));
        });
        with_model(&mut this.albums, |m| m.reset_rows(to_rows(&page.albums)));
        with_model(&mut this.also_albums, |m| m.reset_rows(to_rows(&page.also_albums)));
        with_model(&mut this.similar, |m| m.reset_rows(to_rows(&page.similar)));
        this.loading = false;
        this.error_text = QString::default();
        self.changed();
    }

    pub fn set_error(mut self: Pin<&mut Self>, text: &str) {
        let mut this = self.as_mut().rust_mut();
        this.loading = false;
        this.error_text = QString::from(text);
        self.changed();
    }
}
