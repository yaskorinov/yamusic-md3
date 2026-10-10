// Library (library.py) + меню трека: плейлисты, «Мне нравится», треки плейлистов, добавление в плейлист,
// создание плейлиста, закреплённые волны. Модели повторяют LibraryEvent; Lagged — перечитать всё у сервиса.
use std::collections::HashMap;
use std::pin::Pin;

use cxx::UniquePtr;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use ym_api::items::{PlayContext, contexts};
use ym_services::{LibraryEvent, PlaylistKey};

use super::{Delivery, forward_events, model_ptr, track_from_map, with_model};
use crate::context::ctx;
use crate::models::{DictListModel, ModelKind, apply_list_change, to_rows};
use crate::qt::QVariantMap;

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
        #[qproperty(*mut QObject, playlists, READ = playlists_ptr, CONSTANT)]
        #[qproperty(*mut QObject, liked, READ = liked_ptr, CONSTANT)]
        #[qproperty(QString, error_text, cxx_name = "errorText", READ, NOTIFY = error_changed)]
        #[qproperty(i32, likes_revision, cxx_name = "likesRevision", READ, NOTIFY = likes_changed)]
        #[qproperty(i32, pins_revision, cxx_name = "pinsRevision", READ, NOTIFY = pins_changed)]
        type Library = super::LibraryRust;

        fn playlists_ptr(self: &Library) -> *mut QObject;
        fn liked_ptr(self: &Library) -> *mut QObject;

        #[qsignal]
        #[cxx_name = "errorChanged"]
        fn error_changed(self: Pin<&mut Library>);
        #[qsignal]
        #[cxx_name = "likesChanged"]
        fn likes_changed(self: Pin<&mut Library>);
        #[qsignal]
        #[cxx_name = "pinsChanged"]
        fn pins_changed(self: Pin<&mut Library>);
        /// Короткое сообщение для пользователя (итог действия из меню трека).
        #[qsignal]
        fn notice(self: Pin<&mut Library>, text: QString);

        #[qinvokable]
        fn refresh(self: &Library);
        /// Модель треков плейлиста (кэшируется по uid:kind); каждый вызов перезагружает треки.
        #[qinvokable]
        #[cxx_name = "playlistTracks"]
        fn playlist_tracks(self: Pin<&mut Library>, uid: i64, kind: i64) -> *mut QObject;
        #[qinvokable]
        #[cxx_name = "isLiked"]
        fn is_liked(self: &Library, track_id: &QString) -> bool;
        #[qinvokable]
        #[cxx_name = "toggleLike"]
        fn toggle_like(self: &Library, track: &QMap_QString_QVariant);
        #[qinvokable]
        fn dislike(self: &Library, track: &QMap_QString_QVariant);

        #[qinvokable]
        #[cxx_name = "addToPlaylist"]
        fn add_to_playlist(self: &Library, track: &QMap_QString_QVariant, kind: i64);
        #[qinvokable]
        #[cxx_name = "createPlaylist"]
        fn create_playlist(self: &Library, title: &QString);
        #[qinvokable]
        #[cxx_name = "createPlaylist"]
        fn create_playlist_with(self: &Library, title: &QString, track: &QMap_QString_QVariant);
        #[qinvokable]
        #[cxx_name = "pinWave"]
        fn pin_wave(self: &Library, seed: &QString);
        #[qinvokable]
        #[cxx_name = "unpinWave"]
        fn unpin_wave(self: &Library, seed: &QString);
        #[qinvokable]
        #[cxx_name = "isWavePinned"]
        fn is_wave_pinned(self: &Library, seed: &QString) -> bool;
    }

    impl cxx_qt::Threading for Library {}
    impl cxx_qt::Initialize for Library {}
}

pub struct LibraryRust {
    playlists: UniquePtr<DictListModel>,
    liked: UniquePtr<DictListModel>,
    playlist_models: HashMap<PlaylistKey, UniquePtr<DictListModel>>,
    error_text: QString,
    likes_revision: i32,
    pins_revision: i32,
}

impl Default for LibraryRust {
    fn default() -> Self {
        let mut liked = DictListModel::create(ModelKind::Track);
        with_model(&mut liked, |m| m.set_context(PlayContext::new(contexts::LIKED.0, contexts::LIKED.1)));
        Self {
            playlists: DictListModel::create(ModelKind::Playlist),
            liked,
            playlist_models: HashMap::new(),
            error_text: QString::default(),
            likes_revision: 0,
            pins_revision: 0,
        }
    }
}

fn playlist_context(key: PlaylistKey) -> PlayContext {
    let (kind, from) = contexts::PLAYLIST;
    PlayContext { kind: kind.to_owned(), from: from.to_owned(), playlist_id: Some(key.id()) }
}

impl qobject::Library {
    fn playlists_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.playlists)
    }

    fn liked_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.liked)
    }

    fn refresh(&self) {
        ctx().services.library.refresh();
    }

    fn playlist_tracks(mut self: Pin<&mut Self>, uid: i64, kind: i64) -> *mut cxx_qt::QObject {
        let key = PlaylistKey { uid, kind };
        let this = self.as_mut().rust_mut().get_mut();
        let model = this.playlist_models.entry(key).or_insert_with(|| {
            let mut model = DictListModel::create(ModelKind::Track);
            with_model(&mut model, |m| m.set_context(playlist_context(key)));
            model
        });
        let ptr = model_ptr(model);
        ctx().services.library.load_playlist(key);
        ptr
    }

    fn is_liked(&self, track_id: &QString) -> bool {
        ctx().services.library.is_liked(&track_id.to_string())
    }

    fn toggle_like(&self, track: &QVariantMap) {
        if let Some(track) = track_from_map(track) {
            ctx().services.library.toggle_like(track);
        }
    }

    fn dislike(&self, track: &QVariantMap) {
        if let Some(track) = track_from_map(track) {
            ctx().services.library.dislike(track);
        }
    }

    fn add_to_playlist(&self, track: &QVariantMap, kind: i64) {
        if let Some(track) = track_from_map(track) {
            ctx().services.library.add_to_playlist(track, kind);
        }
    }

    fn create_playlist(&self, title: &QString) {
        let title = title.to_string().trim().to_owned();
        if !title.is_empty() {
            ctx().services.library.create_playlist(title, None);
        }
    }

    fn create_playlist_with(&self, title: &QString, track: &QVariantMap) {
        let title = title.to_string().trim().to_owned();
        if !title.is_empty() {
            ctx().services.library.create_playlist(title, track_from_map(track));
        }
    }

    fn pin_wave(&self, seed: &QString) {
        let seed = seed.to_string();
        if !seed.is_empty() {
            ctx().services.library.pin_wave(seed);
        }
    }

    fn unpin_wave(&self, seed: &QString) {
        let seed = seed.to_string();
        if !seed.is_empty() {
            ctx().services.library.unpin_wave(seed);
        }
    }

    fn is_wave_pinned(&self, seed: &QString) -> bool {
        ctx().services.library.is_wave_pinned(&seed.to_string())
    }

    fn sync_error(mut self: Pin<&mut Self>) {
        let text = QString::from(ctx().services.library.error_text().as_str());
        if self.error_text != text {
            self.as_mut().rust_mut().error_text = text;
            self.error_changed();
        }
    }

    fn sync_likes(mut self: Pin<&mut Self>) {
        let revision = ctx().services.library.likes_revision() as i32;
        if self.likes_revision != revision {
            self.as_mut().rust_mut().likes_revision = revision;
            self.likes_changed();
        }
    }

    fn sync_pins(mut self: Pin<&mut Self>) {
        let revision = ctx().services.library.pins_revision() as i32;
        if self.pins_revision != revision {
            self.as_mut().rust_mut().pins_revision = revision;
            self.pins_changed();
        }
    }

    /// Всё содержимое заново (старт и пропуск событий).
    fn resync(mut self: Pin<&mut Self>) {
        let library = &ctx().services.library;
        {
            let this = self.as_mut().rust_mut().get_mut();
            with_model(&mut this.playlists, |mut m| {
                m.as_mut().reset_rows(to_rows(&library.playlists()));
                m.set_loading(library.playlists_loading());
            });
            with_model(&mut this.liked, |mut m| {
                m.as_mut().reset_rows(to_rows(&library.liked()));
                m.set_loading(library.liked_loading());
            });
        }
        let keys: Vec<PlaylistKey> = self.playlist_models.keys().copied().collect();
        for key in keys {
            library.load_playlist(key);
        }
        self.as_mut().sync_error();
        self.as_mut().sync_likes();
        self.sync_pins();
    }

    fn on_event(mut self: Pin<&mut Self>, delivery: Delivery<LibraryEvent>) {
        let event = match delivery {
            Delivery::Event(event) => event,
            Delivery::Lagged => return self.resync(),
        };
        match event {
            LibraryEvent::Playlists(change) => {
                with_model(&mut self.as_mut().rust_mut().get_mut().playlists, |m| apply_list_change(m, &change));
            }
            LibraryEvent::Liked(change) => {
                with_model(&mut self.as_mut().rust_mut().get_mut().liked, |m| apply_list_change(m, &change));
            }
            LibraryEvent::PlaylistTracks { key, change } => {
                if let Some(model) = self.as_mut().rust_mut().get_mut().playlist_models.get_mut(&key) {
                    with_model(model, |m| apply_list_change(m, &change));
                }
            }
            LibraryEvent::LikesChanged => self.sync_likes(),
            LibraryEvent::ErrorChanged => self.sync_error(),
            LibraryEvent::PinsChanged => self.sync_pins(),
            LibraryEvent::Notice(text) => self.notice(QString::from(text.as_str())),
            LibraryEvent::Feedback { .. } => {}
        }
    }
}

impl cxx_qt::Initialize for qobject::Library {
    fn initialize(mut self: Pin<&mut Self>) {
        let events = ctx().services.library.subscribe();
        self.as_mut().resync();
        forward_events(self.qt_thread(), events, Self::on_event);
    }
}
