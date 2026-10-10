// Catalog (catalog.py): переходы (сигнал openRequested слушает главное окно), страницы альбома/исполнителя
// (кэш AlbumData/ArtistData, повтор при ошибке), поиск с поколениями запросов, меню трека.
use std::collections::HashMap;
use std::pin::Pin;

use cxx::UniquePtr;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Map, Value};
use ym_api::items::{PlayContext, contexts};
use ym_services::{AuthEvent, SearchKind, SearchResults, ServiceError};

use super::data::qobject::{AlbumData, ArtistData};
use super::{Delivery, forward_events, json_str, map_to_json, model_ptr, with_model};
use crate::context::ctx;
use crate::models::{DictListModel, ModelKind, to_rows};
use crate::qt::{QVariantList, QVariantMap, json_to_qvariantlist, json_to_qvariantmap, qvariant_to_json};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, search_state, cxx_name = "searchState", READ, NOTIFY = search_changed)]
        #[qproperty(QString, search_query, cxx_name = "searchQuery", READ, NOTIFY = search_changed)]
        #[qproperty(QMap_QString_QVariant, best, READ, NOTIFY = search_changed)]
        #[qproperty(QMap_QString_QVariant, totals, READ, NOTIFY = search_changed)]
        #[qproperty(*mut QObject, search_tracks, cxx_name = "searchTracks", READ = search_tracks_ptr, CONSTANT)]
        #[qproperty(*mut QObject, search_albums, cxx_name = "searchAlbums", READ = search_albums_ptr, CONSTANT)]
        #[qproperty(*mut QObject, search_artists, cxx_name = "searchArtists", READ = search_artists_ptr, CONSTANT)]
        #[qproperty(*mut QObject, search_playlists, cxx_name = "searchPlaylists", READ = search_playlists_ptr, CONSTANT)]
        type Catalog = super::CatalogRust;

        fn search_tracks_ptr(self: &Catalog) -> *mut QObject;
        fn search_albums_ptr(self: &Catalog) -> *mut QObject;
        fn search_artists_ptr(self: &Catalog) -> *mut QObject;
        fn search_playlists_ptr(self: &Catalog) -> *mut QObject;

        /// страница ("artist" | "album" | "playlist"), параметры
        #[qsignal]
        #[cxx_name = "openRequested"]
        fn open_requested(self: Pin<&mut Catalog>, page: QString, props: QMap_QString_QVariant);
        /// у трека несколько исполнителей — пусть выберет
        #[qsignal]
        #[cxx_name = "chooseArtist"]
        fn choose_artist(self: Pin<&mut Catalog>, refs: QList_QVariant);
        /// карточка исполнителя: id, x, y (низ ссылки), ширина
        #[qsignal]
        #[cxx_name = "previewRequested"]
        fn preview_requested(self: Pin<&mut Catalog>, artist_id: QString, x: f64, y: f64, width: f64);
        #[qsignal]
        #[cxx_name = "previewCancelled"]
        fn preview_cancelled(self: Pin<&mut Catalog>);
        #[qsignal]
        #[cxx_name = "searchChanged"]
        fn search_changed(self: Pin<&mut Catalog>);
        /// меню трека у точки (x, y) — его показывает главное окно
        #[qsignal]
        #[cxx_name = "trackMenuRequested"]
        fn track_menu_requested(self: Pin<&mut Catalog>, track: QMap_QString_QVariant, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "openArtist"]
        fn open_artist(self: Pin<&mut Catalog>, artist_id: &QString);
        #[qinvokable]
        #[cxx_name = "openAlbum"]
        fn open_album(self: Pin<&mut Catalog>, album_id: &QString);
        /// Страница альбома прокручивает к треку и подсвечивает его.
        #[qinvokable]
        #[cxx_name = "openAlbum"]
        fn open_album_at(self: Pin<&mut Catalog>, album_id: &QString, track_id: &QString);
        #[qinvokable]
        #[cxx_name = "openArtists"]
        fn open_artists(self: Pin<&mut Catalog>, refs: &QList_QVariant);
        #[qinvokable]
        #[cxx_name = "previewArtist"]
        fn preview_artist(self: Pin<&mut Catalog>, artist_id: &QString, x: f64, y: f64, width: f64);
        #[qinvokable]
        #[cxx_name = "cancelPreview"]
        fn cancel_preview(self: Pin<&mut Catalog>);
        #[qinvokable]
        #[cxx_name = "openPlaylist"]
        fn open_playlist(self: Pin<&mut Catalog>, playlist: &QMap_QString_QVariant);

        #[qinvokable]
        fn album(self: Pin<&mut Catalog>, album_id: &QString) -> *mut QObject;
        #[qinvokable]
        fn artist(self: Pin<&mut Catalog>, artist_id: &QString) -> *mut QObject;

        #[qinvokable]
        fn search(self: Pin<&mut Catalog>, query: &QString, kind: &QString);
        #[qinvokable]
        #[cxx_name = "search"]
        fn search_all(self: Pin<&mut Catalog>, query: &QString);

        #[qinvokable]
        #[cxx_name = "copyLink"]
        fn copy_link(self: &Catalog, url: &QString);
        /// https://music.yandex.ru/album/<albumId>/track/<trackId> (без альбома — /track/<trackId>)
        #[qinvokable]
        #[cxx_name = "trackUrl"]
        fn track_url(self: &Catalog, track: &QMap_QString_QVariant) -> QString;
        #[qinvokable]
        #[cxx_name = "requestTrackMenu"]
        fn request_track_menu(self: Pin<&mut Catalog>, track: &QMap_QString_QVariant, x: f64, y: f64);
    }

    impl cxx_qt::Threading for Catalog {}
    impl cxx_qt::Initialize for Catalog {}
}

const NOT_SIGNED_IN: &str = "Войдите в аккаунт";

/// Объект страницы и номер его текущей загрузки (ответ старой загрузки отбрасывается).
struct Entry<T: cxx::memory::UniquePtrTarget> {
    load: u64,
    data: UniquePtr<T>,
}

pub struct CatalogRust {
    albums: HashMap<String, Entry<AlbumData>>,
    artists: HashMap<String, Entry<ArtistData>>,
    loads: u64,
    // поиск
    search_state: QString,
    search_query: QString,
    best: QVariantMap,
    totals: QVariantMap,
    query: String,
    kind: SearchKind,
    state: &'static str,
    generation: u64,
    search_tracks: UniquePtr<DictListModel>,
    search_albums: UniquePtr<DictListModel>,
    search_artists: UniquePtr<DictListModel>,
    search_playlists: UniquePtr<DictListModel>,
}

impl Default for CatalogRust {
    fn default() -> Self {
        let mut search_tracks = DictListModel::create(ModelKind::Track);
        with_model(&mut search_tracks, |m| m.set_context(PlayContext::new(contexts::SEARCH.0, contexts::SEARCH.1)));
        Self {
            albums: HashMap::new(),
            artists: HashMap::new(),
            loads: 0,
            search_state: QString::from("idle"),
            search_query: QString::default(),
            best: QVariantMap::default(),
            totals: QVariantMap::default(),
            query: String::new(),
            kind: SearchKind::All,
            state: "idle",
            generation: 0,
            search_tracks,
            search_albums: DictListModel::create(ModelKind::Album),
            search_artists: DictListModel::create(ModelKind::Artist),
            search_playlists: DictListModel::create(ModelKind::Playlist),
        }
    }
}

fn props(pairs: &[(&str, Value)]) -> QVariantMap {
    let map: Map<String, Value> = pairs.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
    json_to_qvariantmap(&map)
}

fn as_int(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        Some(Value::Bool(b)) => *b as i64,
        _ => 0,
    }
}

fn to_map<T: serde::Serialize>(value: &T) -> QVariantMap {
    match serde_json::to_value(value) {
        Ok(Value::Object(map)) => json_to_qvariantmap(&map),
        _ => QVariantMap::default(),
    }
}

fn signed_in() -> bool {
    ctx().services.auth.client().is_some()
}

impl qobject::Catalog {
    fn search_tracks_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.search_tracks)
    }

    fn search_albums_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.search_albums)
    }

    fn search_artists_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.search_artists)
    }

    fn search_playlists_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.search_playlists)
    }

    // --- переходы ------------------------------------------------------------

    fn open_artist(self: Pin<&mut Self>, artist_id: &QString) {
        if !artist_id.is_empty() {
            self.open_requested(QString::from("artist"), props(&[("artistId", Value::from(artist_id.to_string()))]));
        }
    }

    fn open_album(self: Pin<&mut Self>, album_id: &QString) {
        self.open_album_at(album_id, &QString::default());
    }

    fn open_album_at(self: Pin<&mut Self>, album_id: &QString, track_id: &QString) {
        if album_id.is_empty() {
            return;
        }
        let mut pairs = vec![("albumId", Value::from(album_id.to_string()))];
        if !track_id.is_empty() {
            pairs.push(("trackId", Value::from(track_id.to_string())));
        }
        self.open_requested(QString::from("album"), props(&pairs));
    }

    fn open_artists(self: Pin<&mut Self>, refs: &QVariantList) {
        let refs: Vec<Value> = refs
            .iter()
            .map(qvariant_to_json)
            .filter(|r| r.as_object().is_some_and(|o| !json_str(o, "id").is_empty()))
            .collect();
        match refs.as_slice() {
            [] => {}
            [single] => {
                let id = single.as_object().map(|o| json_str(o, "id")).unwrap_or_default();
                self.open_artist(&QString::from(id.as_str()));
            }
            _ => self.choose_artist(json_to_qvariantlist(&refs)),
        }
    }

    fn preview_artist(self: Pin<&mut Self>, artist_id: &QString, x: f64, y: f64, width: f64) {
        if !artist_id.is_empty() {
            self.preview_requested(artist_id.clone(), x, y, width);
        }
    }

    fn cancel_preview(self: Pin<&mut Self>) {
        self.preview_cancelled();
    }

    fn open_playlist(self: Pin<&mut Self>, playlist: &QVariantMap) {
        let playlist = map_to_json(playlist);
        let pairs = [
            ("uid", Value::from(as_int(playlist.get("uid")))),
            ("kind", Value::from(as_int(playlist.get("kind")))),
            ("playlistTitle", Value::from(json_str(&playlist, "title"))),
            ("cover", Value::from(json_str(&playlist, "cover"))),
        ];
        self.open_requested(QString::from("playlist"), props(&pairs));
    }

    // --- альбом и исполнитель ------------------------------------------------

    fn album(mut self: Pin<&mut Self>, album_id: &QString) -> *mut cxx_qt::QObject {
        let id = album_id.to_string();
        let thread = self.qt_thread();
        let mut this = self.as_mut().rust_mut();
        this.loads += 1;
        let load = this.loads;
        let entry = this.albums.entry(id.clone()).or_insert_with(|| Entry { load: 0, data: AlbumData::create() });
        let Some(mut data) = entry.data.as_mut() else { return std::ptr::null_mut() };
        let ptr = data.as_mut().as_qobject_ptr();
        // уже загружен или грузится
        if entry.load != 0 && !data.has_error() {
            return ptr;
        }
        entry.load = load;
        if !signed_in() {
            data.set_error(NOT_SIGNED_IN);
            return ptr;
        }
        if data.has_error() {
            data.begin_loading();
        }
        let catalog = ctx().services.catalog.clone();
        ctx().rt.spawn(async move {
            let result = catalog.album(&id).await;
            let _ = thread.queue(move |catalog| catalog.album_loaded(id, load, result.map_err(|e| e.to_string())));
        });
        ptr
    }

    fn album_loaded(mut self: Pin<&mut Self>, id: String, load: u64, result: Result<ym_services::AlbumPage, String>) {
        let Some(entry) = self.as_mut().rust_mut().get_mut().albums.get_mut(&id) else { return };
        if entry.load != load {
            return;
        }
        let Some(data) = entry.data.as_mut() else { return };
        match result {
            Ok(page) => data.set_page(page),
            Err(error) => data.set_error(&format!("Не удалось загрузить: {error}")),
        }
    }

    fn artist(mut self: Pin<&mut Self>, artist_id: &QString) -> *mut cxx_qt::QObject {
        let id = artist_id.to_string();
        let thread = self.qt_thread();
        let mut this = self.as_mut().rust_mut();
        this.loads += 1;
        let load = this.loads;
        let entry = this.artists.entry(id.clone()).or_insert_with(|| Entry { load: 0, data: ArtistData::create() });
        let Some(mut data) = entry.data.as_mut() else { return std::ptr::null_mut() };
        let ptr = data.as_mut().as_qobject_ptr();
        if entry.load != 0 && !data.has_error() {
            return ptr;
        }
        entry.load = load;
        if !signed_in() {
            data.set_error(NOT_SIGNED_IN);
            return ptr;
        }
        if data.has_error() {
            data.begin_loading();
        }
        let catalog = ctx().services.catalog.clone();
        ctx().rt.spawn(async move {
            let result = catalog.artist(&id).await;
            let _ = thread.queue(move |catalog| catalog.artist_loaded(id, load, result.map_err(|e| e.to_string())));
        });
        ptr
    }

    fn artist_loaded(mut self: Pin<&mut Self>, id: String, load: u64, result: Result<ym_services::ArtistPage, String>) {
        let Some(entry) = self.as_mut().rust_mut().get_mut().artists.get_mut(&id) else { return };
        if entry.load != load {
            return;
        }
        let Some(data) = entry.data.as_mut() else { return };
        match result {
            Ok(page) => data.set_page(page),
            Err(error) => data.set_error(&format!("Не удалось загрузить: {error}")),
        }
    }

    // --- поиск ---------------------------------------------------------------

    fn set_state(mut self: Pin<&mut Self>, state: &'static str) {
        let mut this = self.as_mut().rust_mut();
        this.state = state;
        this.search_state = QString::from(state);
    }

    fn search(mut self: Pin<&mut Self>, query: &QString, kind: &QString) {
        let query = query.to_string().trim().to_owned();
        let kind = SearchKind::from_name(&kind.to_string());
        if query == self.query && kind == self.kind && matches!(self.state, "loading" | "ok" | "empty") {
            return;
        }
        let thread = self.qt_thread();
        let mut this = self.as_mut().rust_mut();
        this.query = query.clone();
        this.search_query = QString::from(query.as_str());
        this.kind = kind;
        this.generation += 1;
        let generation = this.generation;
        if query.is_empty() {
            return self.clear();
        }
        if !signed_in() {
            self.as_mut().set_state("error");
            return self.search_changed();
        }
        self.as_mut().set_state("loading");
        self.as_mut().search_changed();
        let catalog = ctx().services.catalog.clone();
        ctx().rt.spawn(async move {
            let result = catalog.search(&query, kind).await;
            let _ = thread.queue(move |catalog| catalog.search_done(generation, result));
        });
    }

    fn search_all(self: Pin<&mut Self>, query: &QString) {
        self.search(query, &QString::from("all"));
    }

    fn search_done(mut self: Pin<&mut Self>, generation: u64, result: Result<SearchResults, ServiceError>) {
        if generation != self.generation {
            return;
        }
        let results = match result {
            Ok(results) => results,
            Err(error) => {
                tracing::warn!("поиск: {error}");
                self.as_mut().set_state("error");
                return self.search_changed();
            }
        };
        {
            let this = self.as_mut().rust_mut().get_mut();
            with_model(&mut this.search_tracks, |m| m.reset_rows(to_rows(&results.tracks)));
            with_model(&mut this.search_albums, |m| m.reset_rows(to_rows(&results.albums)));
            with_model(&mut this.search_artists, |m| m.reset_rows(to_rows(&results.artists)));
            with_model(&mut this.search_playlists, |m| m.reset_rows(to_rows(&results.playlists)));
            this.totals = to_map(&results.totals);
            this.best = results.best.as_ref().map(to_map).unwrap_or_default();
        }
        self.as_mut().set_state(if results.is_empty() { "empty" } else { "ok" });
        self.search_changed();
    }

    fn clear(mut self: Pin<&mut Self>) {
        {
            let this = self.as_mut().rust_mut().get_mut();
            this.best = QVariantMap::default();
            this.totals = QVariantMap::default();
            for model in [&mut this.search_tracks, &mut this.search_albums, &mut this.search_artists, &mut this.search_playlists]
            {
                with_model(model, |m| m.reset_rows(Vec::new()));
            }
        }
        self.as_mut().set_state("idle");
        self.search_changed();
    }

    fn on_auth(mut self: Pin<&mut Self>, delivery: Delivery<AuthEvent>) {
        let signed_out = match delivery {
            Delivery::Event(event) => event == AuthEvent::SignedOut,
            Delivery::Lagged => !signed_in(),
        };
        if signed_out {
            // ответы запросов прошлого аккаунта отбрасываются
            self.as_mut().rust_mut().generation += 1;
            self.clear();
        }
    }

    // --- меню трека ----------------------------------------------------------

    fn copy_link(&self, url: &QString) {
        crate::qt::copy_to_clipboard(&url.to_string());
    }

    fn track_url(&self, track: &QVariantMap) -> QString {
        let track = map_to_json(track);
        let track_id = json_str(&track, "trackId");
        let album_id = json_str(&track, "albumId");
        let url = match (track_id.is_empty(), album_id.is_empty()) {
            (true, _) => String::new(),
            (false, true) => format!("https://music.yandex.ru/track/{track_id}"),
            (false, false) => format!("https://music.yandex.ru/album/{album_id}/track/{track_id}"),
        };
        QString::from(url.as_str())
    }

    fn request_track_menu(self: Pin<&mut Self>, track: &QVariantMap, x: f64, y: f64) {
        self.track_menu_requested(track.clone(), x, y);
    }
}

impl cxx_qt::Initialize for qobject::Catalog {
    fn initialize(self: Pin<&mut Self>) {
        forward_events(self.qt_thread(), ctx().services.auth.subscribe(), Self::on_auth);
    }
}
