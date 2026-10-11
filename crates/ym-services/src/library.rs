//! Библиотека (src/yamusic/library.py) + меню трека: добавить в плейлист, создать плейлист, закрепить волну.
//!
//! - SignedIn → refresh(); SignedOut → всё очистить (generation += 1: ответы старых запросов отбрасываются);
//! - refresh: плейлисты (Loading(true) → Reset → Loading(false)); лайки: id из /likes/tracks (новые сверху),
//!   затем треки порциями по CHUNK=200 (Liked: Loading(true), Reset([]), Append(порция)…, Loading(false));
//! - load_playlist(uid, kind): треки плейлиста (недостающие дозапрашиваются порциями, порядок сохраняется);
//! - toggle_like / dislike: интерфейс меняется сразу (Liked Insert{at:0}/Remove, LikesChanged), при ошибке API — откат
//!   и error_text «Не удалось поставить лайк: <e>» / «Не удалось снять лайк: <e>» / «Не удалось отметить «Не рекомендовать»: <e>»;
//!   после действия — Feedback{like|unlike|dislike} (его слушает волна);
//! - ошибка загрузки: «Не удалось загрузить библиотеку: <e>».
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use tokio::runtime::Handle;
use tokio::sync::broadcast;
use ym_api::items::{PlaylistItem, TrackItem};
use ym_api::models::{Playlist, TrackShort};
use ym_api::{ApiError, YmClient};

use crate::auth::{AuthEvent, AuthService};
use crate::{ListChange, lock};

pub const CHUNK: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaylistKey {
    pub uid: i64,
    pub kind: i64,
}

impl PlaylistKey {
    /// "uid:kind" — PlayContext.playlistId
    pub fn id(&self) -> String {
        format!("{}:{}", self.uid, self.kind)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackKind {
    Like,
    Unlike,
    Dislike,
}

impl FeedbackKind {
    /// "like" | "unlike" | "dislike" — тип события rotor feedback
    pub fn as_str(self) -> &'static str {
        match self {
            FeedbackKind::Like => "like",
            FeedbackKind::Unlike => "unlike",
            FeedbackKind::Dislike => "dislike",
        }
    }
}

#[derive(Clone, Debug)]
pub enum LibraryEvent {
    Playlists(ListChange<PlaylistItem>),
    Liked(ListChange<TrackItem>),
    PlaylistTracks { key: PlaylistKey, change: ListChange<TrackItem> },
    /// likes_revision() изменился
    LikesChanged,
    /// error_text() изменился
    ErrorChanged,
    /// закреплённые волны изменились (pins_revision())
    PinsChanged,
    /// Короткое сообщение для пользователя (меню трека): «Добавлено в «X»», «Плейлист «X» создан», «Волна закреплена»…
    Notice(String),
    Feedback { kind: FeedbackKind, track: TrackItem },
}

/// Устаревшая ревизия плейлиста: 412, 409 или 400 wrong-revision.
pub fn is_stale_revision(error: &ApiError) -> bool {
    match error {
        ApiError::Http { status: 409 | 412, .. } => true,
        ApiError::BadRequest { code, message } => {
            let text = format!("{code} {message}").to_lowercase();
            text.contains("revision")
        }
        _ => false,
    }
}

/// id трека без альбома ("123:456" → "123")
fn bare_id(id: &str) -> &str {
    id.split(':').next().unwrap_or(id)
}

/// Треки плейлиста в исходном порядке: у элементов без трека — дозапрошенные (по id).
fn merge_tracks(shorts: &[TrackShort], fetched: Vec<ym_api::models::Track>) -> Vec<TrackItem> {
    let fetched: HashMap<String, TrackItem> = fetched
        .iter()
        .map(TrackItem::from_track)
        .map(|item| (item.track_id.clone(), item))
        .collect();
    shorts
        .iter()
        .filter_map(|s| match &s.track {
            Some(track) => Some(TrackItem::from_track(track)),
            None => fetched.get(bare_id(&s.full_id())).cloned(),
        })
        .collect()
}

#[derive(Default)]
struct State {
    generation: u64,
    playlists: Vec<PlaylistItem>,
    playlists_loading: bool,
    liked: Vec<TrackItem>,
    liked_loading: bool,
    liked_ids: HashSet<String>,
    likes_revision: u64,
    error: String,
    /// загруженные модели плейлистов (для вставки после «Добавить в плейлист»)
    playlist_tracks: HashMap<PlaylistKey, Vec<TrackItem>>,
    /// сиды закреплённых волн (несколько сидов — через запятую)
    pins: HashSet<String>,
    pins_revision: u64,
}

pub struct LibraryService {
    rt: Handle,
    auth: Arc<AuthService>,
    events: broadcast::Sender<LibraryEvent>,
    state: Mutex<State>,
    me: Weak<LibraryService>,
}

impl LibraryService {
    /// Подписывается на AuthEvent (SignedIn → refresh, SignedOut → clear).
    pub fn new(rt: Handle, auth: Arc<AuthService>) -> Arc<Self> {
        let mut auth_events = auth.subscribe();
        let service = Arc::new_cyclic(|me| LibraryService {
            rt: rt.clone(),
            auth,
            events: broadcast::channel(1024).0,
            state: Mutex::new(State::default()),
            me: me.clone(),
        });
        let weak = Arc::downgrade(&service);
        rt.spawn(async move {
            loop {
                let event = auth_events.recv().await;
                let Some(me) = weak.upgrade() else { return };
                match event {
                    Ok(AuthEvent::SignedIn(_)) => me.refresh(),
                    Ok(AuthEvent::SignedOut) => me.clear(),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if me.auth.client().is_some() {
                            me.refresh();
                        } else {
                            me.clear();
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        });
        service
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LibraryEvent> {
        self.events.subscribe()
    }

    pub fn refresh(&self) {
        let Some((client, account)) = self.auth.session() else { return };
        let Some(me) = self.me.upgrade() else { return };
        let uid = account.uid;
        let epoch = {
            let mut st = lock(&self.state);
            st.generation += 1;
            Self::set_error_locked(&mut st, &self.events, "");
            st.playlists_loading = true;
            self.emit(LibraryEvent::Playlists(ListChange::Loading(true)));
            st.liked_loading = true;
            st.liked.clear();
            self.emit(LibraryEvent::Liked(ListChange::Loading(true)));
            self.emit(LibraryEvent::Liked(ListChange::Reset(Vec::new())));
            st.generation
        };

        let (this, c) = (me.clone(), client.clone());
        self.rt.spawn(async move {
            match c.users_playlists_list(uid).await {
                Ok(list) => {
                    let items: Vec<PlaylistItem> = list.iter().map(PlaylistItem::from_playlist).collect();
                    this.with_current(epoch, |st, ev| {
                        st.playlists = items.clone();
                        st.playlists_loading = false;
                        ev(LibraryEvent::Playlists(ListChange::Reset(items)));
                        ev(LibraryEvent::Playlists(ListChange::Loading(false)));
                    });
                }
                Err(e) => this.failed(epoch, &e),
            }
        });

        let (this, c) = (me.clone(), client.clone());
        self.rt.spawn(async move {
            if let Err(e) = this.load_liked(&c, uid, epoch).await {
                this.failed(epoch, &e);
            }
        });

        self.rt.spawn(async move { me.load_pins(&client, epoch).await });
    }

    async fn load_liked(&self, client: &YmClient, uid: i64, epoch: u64) -> Result<(), ApiError> {
        let shorts = client.users_likes_tracks(uid).await?;
        let ids: Vec<String> = shorts.iter().map(TrackShort::full_id).collect();
        let current = self.with_current(epoch, |st, ev| {
            st.liked_ids = ids.iter().map(|i| bare_id(i).to_owned()).collect();
            st.likes_revision += 1;
            ev(LibraryEvent::LikesChanged);
        });
        if !current {
            return Ok(());
        }
        for chunk in ids.chunks(CHUNK) {
            let tracks = client.tracks(chunk).await?;
            let items: Vec<TrackItem> = tracks.iter().map(TrackItem::from_track).collect();
            let current = self.with_current(epoch, |st, ev| {
                st.liked.extend(items.iter().cloned());
                ev(LibraryEvent::Liked(ListChange::Append(items)));
            });
            if !current {
                return Ok(());
            }
        }
        self.with_current(epoch, |st, ev| {
            st.liked_loading = false;
            ev(LibraryEvent::Liked(ListChange::Loading(false)));
        });
        Ok(())
    }

    async fn load_pins(&self, client: &YmClient, epoch: u64) {
        match client.pins().await {
            Ok(pins) => {
                let seeds: HashSet<String> = pins
                    .iter()
                    .map(|p| p.wave_seeds().join(","))
                    .filter(|s| !s.is_empty())
                    .collect();
                self.with_current(epoch, |st, ev| {
                    if st.pins != seeds {
                        st.pins = seeds;
                        st.pins_revision += 1;
                        ev(LibraryEvent::PinsChanged);
                    }
                });
            }
            Err(e) => tracing::warn!("закреплённое не загрузилось: {e}"),
        }
    }

    /// Загрузить/перезагрузить треки плейлиста → PlaylistTracks{key, Loading(true), Reset, Loading(false)}.
    pub fn load_playlist(&self, key: PlaylistKey) {
        let Some(client) = self.auth.client() else { return };
        let Some(me) = self.me.upgrade() else { return };
        let epoch = {
            let st = lock(&self.state);
            self.emit(LibraryEvent::PlaylistTracks { key, change: ListChange::Loading(true) });
            st.generation
        };
        self.rt.spawn(async move {
            match Self::fetch_playlist_tracks(&client, key).await {
                Ok(items) => {
                    me.with_current(epoch, |st, ev| {
                        st.playlist_tracks.insert(key, items.clone());
                        ev(LibraryEvent::PlaylistTracks { key, change: ListChange::Reset(items) });
                        ev(LibraryEvent::PlaylistTracks { key, change: ListChange::Loading(false) });
                    });
                }
                Err(e) => {
                    me.emit(LibraryEvent::PlaylistTracks { key, change: ListChange::Loading(false) });
                    me.failed(epoch, &e);
                }
            }
        });
    }

    async fn fetch_playlist_tracks(client: &YmClient, key: PlaylistKey) -> Result<Vec<TrackItem>, ApiError> {
        let mut playlist = client.users_playlist(key.uid, key.kind).await?;
        let empty = playlist.tracks.as_ref().is_none_or(Vec::is_empty);
        if empty && playlist.track_count.unwrap_or(0) > 0 {
            // fetch_tracks_async: тот же плейлист от имени владельца
            let owner = playlist.owner.as_ref().and_then(|o| o.uid).unwrap_or(key.uid);
            playlist = client.users_playlist(owner, key.kind).await?;
        }
        let shorts = playlist.tracks.unwrap_or_default();
        let missing: Vec<String> = shorts.iter().filter(|s| s.track.is_none()).map(TrackShort::full_id).collect();
        let mut fetched = Vec::with_capacity(missing.len());
        for chunk in missing.chunks(CHUNK) {
            fetched.extend(client.tracks(chunk).await?);
        }
        Ok(merge_tracks(&shorts, fetched))
    }

    /// По trackId (часть до ':').
    pub fn is_liked(&self, track_id: &str) -> bool {
        lock(&self.state).liked_ids.contains(bare_id(track_id))
    }

    pub fn likes_revision(&self) -> u64 {
        lock(&self.state).likes_revision
    }

    pub fn error_text(&self) -> String {
        lock(&self.state).error.clone()
    }

    /// Текущее содержимое (для пересинхронизации моста после Lagged).
    pub fn liked(&self) -> Vec<TrackItem> {
        lock(&self.state).liked.clone()
    }

    pub fn playlists(&self) -> Vec<PlaylistItem> {
        lock(&self.state).playlists.clone()
    }

    pub fn liked_loading(&self) -> bool {
        lock(&self.state).liked_loading
    }

    pub fn playlists_loading(&self) -> bool {
        lock(&self.state).playlists_loading
    }

    /// Загруженные треки плейлиста (None — модель не загружалась); для пересинхронизации моста.
    pub fn playlist_tracks(&self, key: PlaylistKey) -> Option<Vec<TrackItem>> {
        lock(&self.state).playlist_tracks.get(&key).cloned()
    }

    pub fn toggle_like(&self, track: TrackItem) {
        let Some((client, account)) = self.auth.session() else { return };
        if track.track_id.is_empty() {
            return;
        }
        let Some(me) = self.me.upgrade() else { return };
        let liked = !self.is_liked(&track.track_id);
        self.apply_like(&track, liked);
        let full_id = track.full_id();
        let back = track.clone();
        self.rt.spawn(async move {
            let result = if liked {
                client.users_likes_tracks_add(account.uid, &full_id).await
            } else {
                client.users_likes_tracks_remove(account.uid, &full_id).await
            };
            if let Err(e) = result {
                me.apply_like(&back, !liked);
                let action = if liked { "поставить" } else { "снять" };
                me.set_error(&format!("Не удалось {action} лайк: {e}"));
                // иначе сердце молча откатывается и непонятно, что лайк не сохранился
                me.emit(LibraryEvent::Notice(format!("Не удалось {action} лайк: {}", short_reason(&e))));
            }
        });
        let kind = if liked { FeedbackKind::Like } else { FeedbackKind::Unlike };
        self.emit(LibraryEvent::Feedback { kind, track });
    }

    pub fn dislike(&self, track: TrackItem) {
        let Some((client, account)) = self.auth.session() else { return };
        if track.track_id.is_empty() {
            return;
        }
        let Some(me) = self.me.upgrade() else { return };
        if self.is_liked(&track.track_id) {
            self.apply_like(&track, false);
        }
        let full_id = track.full_id();
        self.rt.spawn(async move {
            if let Err(e) = client.users_dislikes_tracks_add(account.uid, &full_id).await {
                me.set_error(&format!("Не удалось отметить «Не рекомендовать»: {e}"));
                me.emit(LibraryEvent::Notice(format!("Не удалось отметить «Не рекомендовать»: {}", short_reason(&e))));
            }
        });
        self.emit(LibraryEvent::Feedback { kind: FeedbackKind::Dislike, track });
    }

    /// Вставить трек в начало своего плейлиста `kind` (uid — аккаунт): GET плейлиста за ревизией → change
    /// [insert at 0]; устаревшая ревизия (412, 409 или 400 wrong-revision) — перечитать и повторить один раз.
    /// Успех: Notice «Добавлено в «<title>»», trackCount плейлиста +1 (Playlists Reset с обновлённым элементом),
    /// если модель этого плейлиста загружена — PlaylistTracks Insert{at:0}. Ошибка: error_text + Notice «Не удалось добавить трек: <e>».
    pub fn add_to_playlist(&self, track: TrackItem, kind: i64) {
        let Some((client, account)) = self.auth.session() else { return };
        if track.track_id.is_empty() {
            return;
        }
        let Some(me) = self.me.upgrade() else { return };
        let epoch = lock(&self.state).generation;
        let key = PlaylistKey { uid: account.uid, kind };
        self.rt.spawn(async move {
            match Self::insert_track(&client, key, None, &track).await {
                Ok(playlist) => me.track_added(epoch, key, &playlist, track),
                Err(e) => {
                    let text = format!("Не удалось добавить трек: {e}");
                    me.set_error(&text);
                    me.emit(LibraryEvent::Notice(text));
                }
            }
        });
    }

    /// Создать плейлист (visibility "public") и, если передан трек, сразу добавить его (ревизия из ответа create).
    /// Затем перечитать список плейлистов. Notice «Плейлист «<title>» создан» (/«…» и трек добавлен).
    pub fn create_playlist(&self, title: String, track: Option<TrackItem>) {
        let title = title.trim().to_owned();
        if title.is_empty() {
            return;
        }
        let Some((client, account)) = self.auth.session() else { return };
        let Some(me) = self.me.upgrade() else { return };
        let epoch = lock(&self.state).generation;
        let track = track.filter(|t| !t.track_id.is_empty());
        self.rt.spawn(async move {
            let created = match client.users_playlists_create(account.uid, &title, "public").await {
                Ok(playlist) => playlist,
                Err(e) => {
                    let text = format!("Не удалось создать плейлист: {e}");
                    me.set_error(&text);
                    me.emit(LibraryEvent::Notice(text));
                    return;
                }
            };
            let name = created.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| title.clone());
            let mut notice = format!("Плейлист «{name}» создан");
            if let (Some(track), Some(kind)) = (track, created.kind) {
                let key = PlaylistKey { uid: account.uid, kind };
                match Self::insert_track(&client, key, created.revision, &track).await {
                    Ok(_) => notice.push_str(" и трек добавлен"),
                    Err(e) => {
                        let text = format!("Не удалось добавить трек: {e}");
                        me.set_error(&text);
                        notice = format!("{notice}, но трек не добавлен: {e}");
                    }
                }
            }
            match client.users_playlists_list(account.uid).await {
                Ok(list) => {
                    let items: Vec<PlaylistItem> = list.iter().map(PlaylistItem::from_playlist).collect();
                    me.with_current(epoch, |st, ev| {
                        st.playlists = items.clone();
                        ev(LibraryEvent::Playlists(ListChange::Reset(items)));
                    });
                }
                Err(e) => tracing::warn!("список плейлистов не обновился: {e}"),
            }
            me.emit(LibraryEvent::Notice(notice));
        });
    }

    /// Вставка в начало с одним повтором при устаревшей ревизии.
    async fn insert_track(
        client: &YmClient,
        key: PlaylistKey,
        revision: Option<i64>,
        track: &TrackItem,
    ) -> Result<Playlist, ApiError> {
        let mut revision = match revision {
            Some(r) => r,
            None => client.users_playlist(key.uid, key.kind).await?.revision.unwrap_or(0),
        };
        let mut retried = false;
        loop {
            let result = client
                .users_playlists_insert_track(key.uid, key.kind, revision, 0, &track.track_id, &track.album_id)
                .await;
            match result {
                Err(e) if !retried && is_stale_revision(&e) => {
                    retried = true;
                    revision = client.users_playlist(key.uid, key.kind).await?.revision.unwrap_or(0);
                }
                other => return other,
            }
        }
    }

    fn track_added(&self, epoch: u64, key: PlaylistKey, playlist: &Playlist, track: TrackItem) {
        let mut title = playlist.title.clone().unwrap_or_default();
        self.with_current(epoch, |st, ev| {
            if let Some(item) = st.playlists.iter_mut().find(|p| p.uid == key.uid && p.kind == key.kind) {
                item.track_count = playlist.track_count.unwrap_or(item.track_count + 1);
                if title.is_empty() {
                    title = item.title.clone();
                }
                ev(LibraryEvent::Playlists(ListChange::Reset(st.playlists.clone())));
            }
            if let Some(tracks) = st.playlist_tracks.get_mut(&key) {
                tracks.insert(0, track.clone());
                ev(LibraryEvent::PlaylistTracks { key, change: ListChange::Insert { at: 0, items: vec![track] } });
            }
        });
        self.emit(LibraryEvent::Notice(format!("Добавлено в «{title}»")));
    }

    /// Закреплённые волны: загрузка GET /pins после входа; seed вида "track:<id>", "artist:<id>".
    pub fn is_wave_pinned(&self, seed: &str) -> bool {
        lock(&self.state).pins.contains(seed)
    }

    pub fn pins_revision(&self) -> u64 {
        lock(&self.state).pins_revision
    }

    /// PUT /pin/wave; сразу меняет локальное состояние (PinsChanged), при ошибке — откат + Notice.
    pub fn pin_wave(&self, seed: String) {
        self.change_pin(seed, true);
    }

    /// DELETE /pin/wave; аналогично pin_wave.
    pub fn unpin_wave(&self, seed: String) {
        self.change_pin(seed, false);
    }

    fn change_pin(&self, seed: String, pin: bool) {
        if seed.is_empty() {
            return;
        }
        let Some(client) = self.auth.client() else { return };
        let Some(me) = self.me.upgrade() else { return };
        if !self.set_pinned(&seed, pin) {
            return;
        }
        self.rt.spawn(async move {
            let result = if pin { client.pin_wave(&seed).await.map(drop) } else { client.unpin_wave(&seed).await };
            match result {
                Ok(()) => {
                    let text = if pin { "Волна закреплена" } else { "Волна откреплена" };
                    me.emit(LibraryEvent::Notice(text.to_owned()));
                }
                Err(e) => {
                    me.set_pinned(&seed, !pin);
                    let action = if pin { "закрепить" } else { "открепить" };
                    me.emit(LibraryEvent::Notice(format!("Не удалось {action} волну: {e}")));
                }
            }
        });
    }

    /// true — состояние изменилось.
    fn set_pinned(&self, seed: &str, pinned: bool) -> bool {
        let mut st = lock(&self.state);
        let changed = if pinned { st.pins.insert(seed.to_owned()) } else { st.pins.remove(seed) };
        if changed {
            st.pins_revision += 1;
            self.emit(LibraryEvent::PinsChanged);
        }
        changed
    }

    // --- служебное -------------------------------------------------------

    fn emit(&self, event: LibraryEvent) {
        let _ = self.events.send(event);
    }

    /// Выполнить `f` над состоянием, если поколение не сменилось (вход/выход); false — ответ устарел.
    fn with_current(&self, epoch: u64, f: impl FnOnce(&mut State, &dyn Fn(LibraryEvent))) -> bool {
        let mut st = lock(&self.state);
        if st.generation != epoch {
            return false;
        }
        f(&mut st, &|e| self.emit(e));
        true
    }

    fn apply_like(&self, track: &TrackItem, liked: bool) {
        let mut st = lock(&self.state);
        let id = track.track_id.clone();
        let row = st.liked.iter().position(|t| t.track_id == id);
        if liked {
            st.liked_ids.insert(bare_id(&id).to_owned());
            if row.is_none() {
                st.liked.insert(0, track.clone());
                self.emit(LibraryEvent::Liked(ListChange::Insert { at: 0, items: vec![track.clone()] }));
            }
        } else {
            st.liked_ids.remove(bare_id(&id));
            if let Some(at) = row {
                st.liked.remove(at);
                self.emit(LibraryEvent::Liked(ListChange::Remove { at, count: 1 }));
            }
        }
        st.likes_revision += 1;
        self.emit(LibraryEvent::LikesChanged);
    }

    fn failed(&self, epoch: u64, error: &ApiError) {
        let mut st = lock(&self.state);
        if st.generation != epoch {
            return;
        }
        if st.playlists_loading {
            st.playlists_loading = false;
            self.emit(LibraryEvent::Playlists(ListChange::Loading(false)));
        }
        if st.liked_loading {
            st.liked_loading = false;
            self.emit(LibraryEvent::Liked(ListChange::Loading(false)));
        }
        Self::set_error_locked(&mut st, &self.events, &format!("Не удалось загрузить библиотеку: {error}"));
    }

    fn clear(&self) {
        let mut st = lock(&self.state);
        st.generation += 1;
        st.playlists.clear();
        self.emit(LibraryEvent::Playlists(ListChange::Reset(Vec::new())));
        if st.playlists_loading {
            st.playlists_loading = false;
            self.emit(LibraryEvent::Playlists(ListChange::Loading(false)));
        }
        st.liked.clear();
        self.emit(LibraryEvent::Liked(ListChange::Reset(Vec::new())));
        if st.liked_loading {
            st.liked_loading = false;
            self.emit(LibraryEvent::Liked(ListChange::Loading(false)));
        }
        st.liked_ids.clear();
        st.likes_revision += 1;
        self.emit(LibraryEvent::LikesChanged);
        for (key, tracks) in st.playlist_tracks.iter_mut() {
            tracks.clear();
            self.emit(LibraryEvent::PlaylistTracks { key: *key, change: ListChange::Reset(Vec::new()) });
        }
        if !st.pins.is_empty() {
            st.pins.clear();
            st.pins_revision += 1;
            self.emit(LibraryEvent::PinsChanged);
        }
    }

    fn set_error(&self, text: &str) {
        let mut st = lock(&self.state);
        Self::set_error_locked(&mut st, &self.events, text);
    }

    fn set_error_locked(st: &mut State, events: &broadcast::Sender<LibraryEvent>, text: &str) {
        if st.error != text {
            st.error = text.to_owned();
            let _ = events.send(LibraryEvent::ErrorChanged);
        }
    }
}

/// Причина для короткого уведомления: сетевые ошибки reqwest длинные и с URL.
fn short_reason(e: &ApiError) -> String {
    match e {
        ApiError::Network(_) => "сервер Яндекса не ответил".to_owned(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_names() {
        assert_eq!(FeedbackKind::Like.as_str(), "like");
        assert_eq!(FeedbackKind::Unlike.as_str(), "unlike");
        assert_eq!(FeedbackKind::Dislike.as_str(), "dislike");
    }

    #[test]
    fn playlist_key_id() {
        assert_eq!(PlaylistKey { uid: 12, kind: 1003 }.id(), "12:1003");
    }

    #[test]
    fn stale_revision() {
        let http = |status| ApiError::Http { status, code: String::new(), message: String::new() };
        assert!(is_stale_revision(&http(412)));
        assert!(is_stale_revision(&http(409)));
        assert!(!is_stale_revision(&http(500)));
        assert!(is_stale_revision(&ApiError::BadRequest {
            code: "wrong-revision".into(),
            message: "Wrong revision".into()
        }));
        assert!(!is_stale_revision(&ApiError::BadRequest { code: "validate".into(), message: "bad".into() }));
        assert!(!is_stale_revision(&ApiError::NotFound("x".into())));
    }

    #[test]
    fn bare_ids() {
        assert_eq!(bare_id("123:456"), "123");
        assert_eq!(bare_id("123"), "123");
        assert_eq!(bare_id(""), "");
    }
}
