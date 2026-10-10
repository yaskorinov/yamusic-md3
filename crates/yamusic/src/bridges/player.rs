// Player (player.py) — зеркало PlayerHandle. Свойства сверяются со снимком плеера на каждом событии (сигнал —
// только по изменившейся группе; TrackChanged — всегда, как _set_track), модель очереди повторяет QueueChange.
// Сеттеры (volume, shuffle, repeat, levelsWanted) меняют значение сразу; «эхо» из плеера, которое старше записи,
// какое-то время не принимается.
use std::pin::Pin;
use std::time::Instant;

use cxx::UniquePtr;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use ym_api::items::TrackItem;
use ym_platform::{SettingKey, SettingValue};
use ym_player::{PlayerEvent, PlayerSnapshot, QueueChange, Repeat};

use super::{Delivery, forward_events, held, model_ptr, with_model};
use crate::context::ctx;
use crate::models::{DictListModel, ModelKind, to_row, to_rows};
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
        #[qproperty(bool, has_track, cxx_name = "hasTrack", READ, NOTIFY = track_changed)]
        #[qproperty(QString, track_id, cxx_name = "trackId", READ, NOTIFY = track_changed)]
        #[qproperty(QString, title, READ, NOTIFY = track_changed)]
        #[qproperty(QString, artist, READ, NOTIFY = track_changed)]
        #[qproperty(QString, album, READ, NOTIFY = track_changed)]
        #[qproperty(QString, cover, READ, NOTIFY = track_changed)]
        #[qproperty(QMap_QString_QVariant, track, READ, NOTIFY = track_changed)]
        #[qproperty(QString, codec, READ, NOTIFY = track_changed)]
        #[qproperty(i32, direction, READ, NOTIFY = track_changed)]
        #[qproperty(QString, cover_file, cxx_name = "coverFile", READ, NOTIFY = cover_file_changed)]
        #[qproperty(bool, playing, READ, NOTIFY = state_changed)]
        #[qproperty(bool, buffering, READ, NOTIFY = state_changed)]
        #[qproperty(i32, position_ms, cxx_name = "positionMs", READ, NOTIFY = position_changed)]
        #[qproperty(i32, duration_ms, cxx_name = "durationMs", READ, NOTIFY = position_changed)]
        #[qproperty(f64, position, READ, NOTIFY = position_changed)]
        #[qproperty(f64, volume, READ, WRITE = set_volume, NOTIFY = volume_changed)]
        #[qproperty(bool, shuffle, READ, WRITE = set_shuffle, NOTIFY = mode_changed)]
        #[qproperty(QString, repeat, READ, WRITE = set_repeat, NOTIFY = mode_changed)]
        #[qproperty(*mut QObject, queue, READ = queue_ptr, CONSTANT)]
        #[qproperty(i32, current_index, cxx_name = "currentIndex", READ, NOTIFY = queue_changed)]
        #[qproperty(QString, source, READ, NOTIFY = queue_changed)]
        #[qproperty(QString, error_text, cxx_name = "errorText", READ, NOTIFY = error_changed)]
        #[qproperty(f64, level, READ, NOTIFY = level_changed)]
        #[qproperty(bool, levels_wanted, cxx_name = "levelsWanted", READ, WRITE = set_levels_wanted, NOTIFY = level_changed)]
        type Player = super::PlayerRust;

        fn queue_ptr(self: &Player) -> *mut QObject;
        fn set_volume(self: Pin<&mut Player>, value: f64);
        fn set_shuffle(self: Pin<&mut Player>, value: bool);
        fn set_repeat(self: Pin<&mut Player>, value: QString);
        fn set_levels_wanted(self: Pin<&mut Player>, value: bool);

        #[qsignal]
        #[cxx_name = "trackChanged"]
        fn track_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "stateChanged"]
        fn state_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "positionChanged"]
        fn position_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "volumeChanged"]
        fn volume_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "modeChanged"]
        fn mode_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "queueChanged"]
        fn queue_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "coverFileChanged"]
        fn cover_file_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "errorChanged"]
        fn error_changed(self: Pin<&mut Player>);
        #[qsignal]
        #[cxx_name = "levelChanged"]
        fn level_changed(self: Pin<&mut Player>);

        /// Поставить в очередь всю модель (DictListModel или TrackFilter) и начать с row.
        #[qinvokable]
        #[cxx_name = "playFrom"]
        unsafe fn play_from(self: Pin<&mut Player>, model: *mut QObject, row: i32);
        /// «Перемешать»: включить shuffle и начать со случайного трека модели.
        #[qinvokable]
        #[cxx_name = "shuffleFrom"]
        unsafe fn shuffle_from(self: Pin<&mut Player>, model: *mut QObject);
        #[qinvokable]
        #[cxx_name = "playIndex"]
        fn play_index(self: &Player, index: i32);
        #[qinvokable]
        #[cxx_name = "togglePlay"]
        fn toggle_play(self: &Player);
        #[qinvokable]
        fn play(self: &Player);
        #[qinvokable]
        fn pause(self: &Player);
        #[qinvokable]
        fn stop(self: &Player);
        #[qinvokable]
        fn next(self: &Player);
        #[qinvokable]
        fn previous(self: &Player);
        #[qinvokable]
        fn seek(self: Pin<&mut Player>, fraction: f64);
        #[qinvokable]
        #[cxx_name = "seekMs"]
        fn seek_ms(self: Pin<&mut Player>, ms: i32);
        #[qinvokable]
        #[cxx_name = "cycleRepeat"]
        fn cycle_repeat(self: Pin<&mut Player>);
    }

    impl cxx_qt::Threading for Player {}
    impl cxx_qt::Initialize for Player {}
}

pub struct PlayerRust {
    has_track: bool,
    track_id: QString,
    title: QString,
    artist: QString,
    album: QString,
    cover: QString,
    track: QVariantMap,
    codec: QString,
    direction: i32,
    cover_file: QString,
    playing: bool,
    buffering: bool,
    position_ms: i32,
    duration_ms: i32,
    position: f64,
    volume: f64,
    shuffle: bool,
    repeat: QString,
    queue: UniquePtr<DictListModel>,
    current_index: i32,
    source: QString,
    error_text: QString,
    level: f64,
    levels_wanted: bool,

    /// трек снимка, по которому заполнены свойства группы trackChanged
    track_item: Option<TrackItem>,
    repeat_mode: Repeat,
    volume_set_at: Option<Instant>,
    mode_set_at: Option<Instant>,
    position_set_at: Option<Instant>,
}

impl Default for PlayerRust {
    fn default() -> Self {
        Self {
            has_track: false,
            track_id: QString::default(),
            title: QString::default(),
            artist: QString::default(),
            album: QString::default(),
            cover: QString::default(),
            track: QVariantMap::default(),
            codec: QString::default(),
            direction: 1,
            cover_file: QString::default(),
            playing: false,
            buffering: false,
            position_ms: 0,
            duration_ms: 0,
            position: 0.0,
            volume: 0.0,
            shuffle: false,
            repeat: QString::from(Repeat::Off.as_str()),
            queue: DictListModel::create(ModelKind::Track),
            current_index: -1,
            source: QString::default(),
            error_text: QString::default(),
            level: 0.0,
            levels_wanted: false,
            track_item: None,
            repeat_mode: Repeat::Off,
            volume_set_at: None,
            mode_set_at: None,
            position_set_at: None,
        }
    }
}

fn clamp_ms(ms: i64) -> i32 {
    ms.clamp(0, i32::MAX as i64) as i32
}

fn fraction(position_ms: i32, duration_ms: i32) -> f64 {
    if duration_ms > 0 { position_ms as f64 / duration_ms as f64 } else { 0.0 }
}

impl qobject::Player {
    fn queue_ptr(&self) -> *mut cxx_qt::QObject {
        model_ptr(&self.queue)
    }

    // --- сеттеры -----------------------------------------------------------

    fn set_volume(mut self: Pin<&mut Self>, value: f64) {
        let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 };
        if value == self.volume {
            return;
        }
        let mut this = self.as_mut().rust_mut();
        this.volume = value;
        this.volume_set_at = Some(Instant::now());
        self.volume_changed();
        let services = &ctx().services;
        services.player.set_volume(value);
        services.settings.set(SettingKey::Volume, SettingValue::Float(value));
    }

    /// У волны не меняется.
    fn set_shuffle(mut self: Pin<&mut Self>, value: bool) {
        if value == self.shuffle || self.source == QString::from("wave") {
            return;
        }
        let mut this = self.as_mut().rust_mut();
        this.shuffle = value;
        this.mode_set_at = Some(Instant::now());
        self.mode_changed();
        ctx().services.player.set_shuffle(value);
    }

    fn set_repeat(self: Pin<&mut Self>, value: QString) {
        if let Some(repeat) = Repeat::from_str(&value.to_string()) {
            self.apply_repeat(repeat);
        }
    }

    fn apply_repeat(mut self: Pin<&mut Self>, repeat: Repeat) {
        if repeat == self.repeat_mode {
            return;
        }
        let mut this = self.as_mut().rust_mut();
        this.repeat_mode = repeat;
        this.repeat = QString::from(repeat.as_str());
        this.mode_set_at = Some(Instant::now());
        self.mode_changed();
        ctx().services.player.set_repeat(repeat);
    }

    fn set_levels_wanted(mut self: Pin<&mut Self>, value: bool) {
        self.as_mut().rust_mut().levels_wanted = value;
        if !value && self.level != 0.0 {
            self.as_mut().rust_mut().level = 0.0;
            self.as_mut().level_changed();
        }
        ctx().services.player.set_levels_wanted(value);
    }

    // --- управление --------------------------------------------------------

    unsafe fn play_from(self: Pin<&mut Self>, model: *mut cxx_qt::QObject, row: i32) {
        let Some((items, context)) = (unsafe { crate::models::tracks_from_model(model) }) else { return };
        let Ok(row) = usize::try_from(row) else { return };
        if row < items.len() {
            ctx().services.player.play_items(items, row, context, None);
        }
    }

    unsafe fn shuffle_from(mut self: Pin<&mut Self>, model: *mut cxx_qt::QObject) {
        let Some((items, context)) = (unsafe { crate::models::tracks_from_model(model) }) else { return };
        if items.is_empty() {
            return;
        }
        if !self.shuffle {
            let mut this = self.as_mut().rust_mut();
            this.shuffle = true;
            this.mode_set_at = Some(Instant::now());
            self.as_mut().mode_changed();
        }
        ctx().services.player.shuffle_from(items, context);
    }

    fn play_index(&self, index: i32) {
        if let Ok(index) = usize::try_from(index) {
            ctx().services.player.play_index(index);
        }
    }

    fn toggle_play(&self) {
        ctx().services.player.toggle_play();
    }

    fn play(&self) {
        ctx().services.player.play();
    }

    fn pause(&self) {
        ctx().services.player.pause();
    }

    fn stop(&self) {
        ctx().services.player.stop();
    }

    fn next(&self) {
        ctx().services.player.next();
    }

    /// После 3 с — в начало трека, иначе предыдущий.
    fn previous(&self) {
        ctx().services.player.previous();
    }

    fn seek(self: Pin<&mut Self>, fraction: f64) {
        if self.duration_ms > 0 && fraction.is_finite() {
            let ms = (fraction * self.duration_ms as f64) as i64;
            self.seek_ms(clamp_ms(ms));
        }
    }

    fn seek_ms(mut self: Pin<&mut Self>, ms: i32) {
        if !self.has_track {
            return;
        }
        let ms = ms.max(0);
        let mut this = self.as_mut().rust_mut();
        this.position_ms = ms;
        this.position = fraction(ms, this.duration_ms);
        this.position_set_at = Some(Instant::now());
        self.position_changed();
        ctx().services.player.seek_ms(ms as i64);
    }

    /// off → all → one → off; у волны нет «конца» — off ↔ one.
    fn cycle_repeat(self: Pin<&mut Self>) {
        let next = if self.source == QString::from("wave") {
            if self.repeat_mode != Repeat::Off { Repeat::Off } else { Repeat::One }
        } else {
            match self.repeat_mode {
                Repeat::Off => Repeat::All,
                Repeat::All => Repeat::One,
                Repeat::One => Repeat::Off,
            }
        };
        self.apply_repeat(next);
    }

    // --- снимок плеера → свойства -------------------------------------------

    fn on_event(mut self: Pin<&mut Self>, delivery: Delivery<PlayerEvent>) {
        let snapshot = ctx().services.player.snapshot();
        let force_track = matches!(delivery, Delivery::Event(PlayerEvent::TrackChanged));
        match delivery {
            Delivery::Event(PlayerEvent::Queue(change)) => self.as_mut().apply_queue(change),
            Delivery::Lagged => self.as_mut().reset_queue(&snapshot),
            Delivery::Event(_) => {}
        }
        self.sync(&snapshot, force_track);
    }

    fn apply_queue(mut self: Pin<&mut Self>, change: QueueChange) {
        with_model(&mut self.as_mut().rust_mut().get_mut().queue, |m| match change {
            QueueChange::Reset(items) => m.reset_rows(to_rows(&items)),
            QueueChange::Append(items) => m.append_rows(to_rows(&items)),
            QueueChange::Remove { at, count } => m.remove_rows(at, count),
        });
    }

    fn reset_queue(mut self: Pin<&mut Self>, snapshot: &PlayerSnapshot) {
        with_model(&mut self.as_mut().rust_mut().get_mut().queue, |mut m| {
            m.as_mut().set_context(snapshot.context.clone());
            m.reset_rows(to_rows(&snapshot.queue));
        });
    }

    fn sync(mut self: Pin<&mut Self>, s: &PlayerSnapshot, force_track: bool) {
        // трек
        let track_changed = self.track_item != s.track || self.codec != QString::from(s.codec.as_str())
            || self.direction != s.direction;
        if track_changed {
            let mut this = self.as_mut().rust_mut();
            let track = s.track.clone().unwrap_or_default();
            this.has_track = s.track.is_some();
            this.track_id = QString::from(track.track_id.as_str());
            this.title = QString::from(track.title.as_str());
            this.artist = QString::from(track.artists.as_str());
            this.album = QString::from(track.album.as_str());
            this.cover = QString::from(track.cover.as_str());
            this.track = if s.track.is_some() { json_to_qvariantmap(&to_row(&track)) } else { QVariantMap::default() };
            this.codec = QString::from(s.codec.as_str());
            this.direction = s.direction;
            if this.track_item.as_ref().map(|t| &t.track_id) != s.track.as_ref().map(|t| &t.track_id) {
                this.position_set_at = None;
            }
            this.track_item = s.track.clone();
        }
        if track_changed || force_track {
            self.as_mut().track_changed();
        }

        // воспроизведение
        if self.playing != s.playing || self.buffering != s.buffering {
            let mut this = self.as_mut().rust_mut();
            this.playing = s.playing;
            this.buffering = s.buffering;
            self.as_mut().state_changed();
        }

        // позиция
        let duration_ms = clamp_ms(s.duration_ms);
        let position_ms = if held(self.position_set_at) { self.position_ms } else { clamp_ms(s.position_ms) };
        if self.position_ms != position_ms || self.duration_ms != duration_ms {
            let mut this = self.as_mut().rust_mut();
            this.position_ms = position_ms;
            this.duration_ms = duration_ms;
            this.position = fraction(position_ms, duration_ms);
            self.as_mut().position_changed();
        }

        // громкость
        if !held(self.volume_set_at) && self.volume != s.volume {
            self.as_mut().rust_mut().volume = s.volume;
            self.as_mut().volume_changed();
        }

        // режимы
        if !held(self.mode_set_at) && (self.shuffle != s.shuffle || self.repeat_mode != s.repeat) {
            let mut this = self.as_mut().rust_mut();
            this.shuffle = s.shuffle;
            this.repeat_mode = s.repeat;
            this.repeat = QString::from(s.repeat.as_str());
            self.as_mut().mode_changed();
        }

        // очередь
        let source = QString::from(s.source());
        let context_changed = self.queue.as_ref().is_some_and(|q| q.context() != &s.context);
        if self.current_index != s.current_index || self.source != source || context_changed {
            let mut this = self.as_mut().rust_mut();
            this.current_index = s.current_index;
            this.source = source;
            with_model(&mut this.queue, |m| m.set_context(s.context.clone()));
            self.as_mut().queue_changed();
        }

        // обложка, ошибка
        let cover_file = QString::from(s.cover_file.as_str());
        if self.cover_file != cover_file {
            self.as_mut().rust_mut().cover_file = cover_file;
            self.as_mut().cover_file_changed();
        }
        let error_text = QString::from(s.error_text.as_str());
        if self.error_text != error_text {
            self.as_mut().rust_mut().error_text = error_text;
            self.as_mut().error_changed();
        }

        // уровень — только пока он нужен QML
        let level = if self.levels_wanted { s.level } else { 0.0 };
        if self.level != level {
            self.as_mut().rust_mut().level = level;
            self.level_changed();
        }
    }
}

impl cxx_qt::Initialize for qobject::Player {
    fn initialize(mut self: Pin<&mut Self>) {
        let player = &ctx().services.player;
        let subscription = player.subscribe();
        let snapshot = player.snapshot();
        self.as_mut().reset_queue(&snapshot);
        self.as_mut().sync(&snapshot, false);
        forward_events(self.qt_thread(), subscription.events, Self::on_event);
    }
}
