//! Публичная ручка плеера и актор. Поведение каждой команды — как одноимённый метод Player в player.py.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use sha1::{Digest, Sha1};
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::Instant;
use ym_api::items::{PlayContext, TrackItem};

use crate::mpv::{self, Mpv, MpvEvent, MpvLib, PropertyValue};
use crate::session::{self, SessionData};
use crate::state::{PlayerState, Previous};
use crate::{
    AUTOSAVE_MS, FADE_STEP_MS, FEED_AHEAD, FeederId, POSITION_POLL_MS, PREFETCH_DELAY_MS, PlayerConfig, PlayerError,
    PlayerEvent, PlayerSnapshot, PlayerSubscription, Quality, QueueChange, Repeat, SAVE_DEBOUNCE_MS, StreamInfo,
    StreamResolver, TrackEnded, TrackStarted, codec_label, level_from_db, mpv_volume,
};

/// Команды актора (внутреннее; публичный API — методы PlayerHandle).
#[derive(Debug)]
pub(crate) enum Command {
    PlayItems { items: Vec<TrackItem>, row: usize, context: PlayContext, feeder: Option<FeederId> },
    ShuffleFrom { items: Vec<TrackItem>, context: PlayContext },
    AppendTracks { items: Vec<TrackItem>, feeder: FeederId },
    ReplaceUpcoming { items: Vec<TrackItem>, feeder: FeederId },
    PlayIndex(usize),
    TogglePlay,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek(f64),
    SeekMs(i64),
    CycleRepeat,
    SetRepeat(Repeat),
    SetShuffle(bool),
    SetVolume(f64),
    SetLevelsWanted(bool),
    SetCrossfade(u32),
    SetQuality(Quality),
    RestoreSession,
    Shutdown(std::sync::mpsc::Sender<()>),
}

/// Дешёвый Clone; все методы неблокирующие (кроме shutdown), вызывать можно из любого потока.
#[derive(Clone)]
pub struct PlayerHandle {
    commands: mpsc::UnboundedSender<Command>,
    snapshot: watch::Receiver<PlayerSnapshot>,
    events: broadcast::Sender<PlayerEvent>,
}

impl PlayerHandle {
    /// Загрузить libmpv, создать главный экземпляр (второй — лениво, для перехода) и запустить актор на `rt`.
    /// Вызывать после создания QApplication (на unix сначала setlocale(LC_NUMERIC, "C") — его делает start).
    pub fn start(rt: &Handle, config: PlayerConfig, resolver: Arc<dyn StreamResolver>) -> Result<Self, PlayerError> {
        // libmpv отказывается работать с не-C LC_NUMERIC, а QGuiApplication выставил локаль из системы
        #[cfg(unix)]
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }
        let lib = mpv::load_library(config.libmpv_path.as_deref())?;
        let volume = config.volume.clamp(0.0, 1.0);

        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared::default());
        let mut pool = MpvPool {
            lib,
            audio_output: config.audio_output.clone(),
            instances: Vec::new(),
            threads: Vec::new(),
            msgs: msg_tx.clone(),
            shared: shared.clone(),
        };
        pool.spawn_instance(volume, false)?;
        let cmd = Cmd::spawn(pool)?;

        let snapshot = PlayerSnapshot { volume, current_index: -1, direction: 1, ..Default::default() };
        let (watch_tx, watch_rx) = watch::channel(snapshot.clone());
        let (events, _) = broadcast::channel(512);
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let actor = Actor {
            state: PlayerState::default(),
            snap: snapshot,
            queue_dirty: false,
            cover_dir: config.cover_dir,
            session_file: config.session_file,
            crossfade_s: config.crossfade_s,
            quality: config.quality,
            paused: false,
            idle_active: true,
            load_token: 0,
            prefetched_for: None,
            prefetched_codec: String::new(),
            next_ready: None,
            last_time: None,
            track_live: false,
            cur: 0,
            has_second: false,
            fade: None,
            resume_ms: 0,
            cover_seq: 0,
            poll_at: None,
            fade_at: None,
            save_at: None,
            autosave_at: None,
            cmd,
            msgs: msg_tx,
            shared,
            resolver,
            http: reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap_or_default(),
            watch: watch_tx,
            events: events.clone(),
        };
        rt.spawn(actor.run(cmd_rx, msg_rx));
        Ok(Self { commands: cmd_tx, snapshot: watch_rx, events })
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        self.snapshot.borrow().clone()
    }

    pub fn subscribe(&self) -> PlayerSubscription {
        PlayerSubscription { snapshot: self.snapshot.clone(), events: self.events.subscribe() }
    }

    /// owns_feeder: очередь сейчас принадлежит этой «подкормке».
    pub fn owns_feeder(&self, feeder: FeederId) -> bool {
        self.snapshot.borrow().feeder == Some(feeder)
    }

    /// play_items: очередь = items, начать с row, контекст и (для волны) feeder.
    pub fn play_items(&self, items: Vec<TrackItem>, row: usize, context: PlayContext, feeder: Option<FeederId>) {
        self.send(Command::PlayItems { items, row, context, feeder });
    }

    /// shuffleFrom: включить shuffle и начать со случайного трека.
    pub fn shuffle_from(&self, items: Vec<TrackItem>, context: PlayContext) {
        self.send(Command::ShuffleFrom { items, context });
    }

    /// append_tracks: только если очередь всё ещё принадлежит `feeder`; встала на последнем — играть дальше.
    pub fn append_tracks(&self, items: Vec<TrackItem>, feeder: FeederId) {
        self.send(Command::AppendTracks { items, feeder });
    }

    /// replace_upcoming: заменить всё после текущего (волна с новыми настройками).
    pub fn replace_upcoming(&self, items: Vec<TrackItem>, feeder: FeederId) {
        self.send(Command::ReplaceUpcoming { items, feeder });
    }

    pub fn play_index(&self, index: usize) {
        self.send(Command::PlayIndex(index));
    }

    pub fn toggle_play(&self) {
        self.send(Command::TogglePlay);
    }

    pub fn play(&self) {
        self.send(Command::Play);
    }

    pub fn pause(&self) {
        self.send(Command::Pause);
    }

    pub fn stop(&self) {
        self.send(Command::Stop);
    }

    pub fn next(&self) {
        self.send(Command::Next);
    }

    /// После 3 с — в начало трека, иначе предыдущий.
    pub fn previous(&self) {
        self.send(Command::Previous);
    }

    /// Доля 0..1 длительности.
    pub fn seek(&self, fraction: f64) {
        self.send(Command::Seek(fraction));
    }

    pub fn seek_ms(&self, ms: i64) {
        self.send(Command::SeekMs(ms));
    }

    /// off → all → one → off; у волны off ↔ one.
    pub fn cycle_repeat(&self) {
        self.send(Command::CycleRepeat);
    }

    pub fn set_repeat(&self, repeat: Repeat) {
        self.send(Command::SetRepeat(repeat));
    }

    /// У волны не меняется.
    pub fn set_shuffle(&self, shuffle: bool) {
        self.send(Command::SetShuffle(shuffle));
    }

    /// 0..1 (сам плеер настройки не пишет — это делает вызывающий).
    pub fn set_volume(&self, volume: f64) {
        self.send(Command::SetVolume(volume));
    }

    pub fn set_levels_wanted(&self, wanted: bool) {
        self.send(Command::SetLevelsWanted(wanted));
    }

    /// Settings.crossfade изменился → перезапросить следующий трек (force).
    pub fn set_crossfade(&self, seconds: u32) {
        self.send(Command::SetCrossfade(seconds));
    }

    pub fn set_quality(&self, quality: Quality) {
        self.send(Command::SetQuality(quality));
    }

    /// Очередь и позиция прошлого запуска — на паузе (restore_session).
    pub fn restore_session(&self) {
        self.send(Command::RestoreSession);
    }

    /// Синхронно сохранить сессию, остановить mpv. Блокирует до 2 с.
    pub fn shutdown(&self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.send(Command::Shutdown(tx));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    }
}

// --- mpv: поток команд и потоки событий ----------------------------------------

/// Что нужно потокам событий без обращения к актору (фильтр уровня ~20 раз в секунду).
#[derive(Default)]
struct Shared {
    levels_wanted: AtomicBool,
    cur: AtomicUsize,
}

/// Сообщения актору от потоков mpv и фоновых задач.
enum Msg {
    Mpv(usize, MpvEvent),
    Level(usize, f64),
    Time(Option<f64>),
    Resolved { token: u64, target: Target, result: Result<StreamInfo, String> },
    Cover { seq: u64, path: Option<String> },
    PrefetchTimer(u64),
    SecondReady,
}

enum Target {
    Current { resume_ms: i64 },
    Prefetch { index: usize },
}

/// Экземпляры mpv — во владении потока mpv-cmd.
struct MpvPool {
    lib: Arc<MpvLib>,
    audio_output: Option<String>,
    instances: Vec<Arc<Mpv>>,
    threads: Vec<JoinHandle<()>>,
    msgs: mpsc::UnboundedSender<Msg>,
    shared: Arc<Shared>,
}

const OBSERVED: &[&str] = &["pause", "idle-active", "paused-for-cache", "duration", "playlist-pos", "af-metadata/lvl"];
const LEVEL_KEY: &str = "lavfi.astats.Overall.RMS_level";

impl MpvPool {
    /// _make_mpv(i): новый экземпляр с громкостью и loop-file, наблюдатели и свой поток событий.
    fn spawn_instance(&mut self, volume_ui: f64, loop_one: bool) -> Result<(), PlayerError> {
        let index = self.instances.len();
        let m = Arc::new(Mpv::new(self.lib.clone(), self.audio_output.as_deref())?);
        if let Err(e) = m.set_property_f64("volume", mpv_volume(volume_ui)) {
            tracing::debug!("mpv volume: {e}");
        }
        if let Err(e) = m.set_property_string("loop-file", if loop_one { "inf" } else { "no" }) {
            tracing::debug!("mpv loop-file: {e}");
        }
        for (id, name) in OBSERVED.iter().enumerate() {
            m.observe(id as u64 + 1, name)?;
        }
        let (events, tx, shared) = (m.clone(), self.msgs.clone(), self.shared.clone());
        let thread = std::thread::Builder::new()
            .name(format!("mpv-events-{index}"))
            .spawn(move || event_loop(index, events, tx, shared))
            .map_err(|e| PlayerError::Mpv(e.to_string()))?;
        self.instances.push(m);
        self.threads.push(thread);
        Ok(())
    }

    fn get(&self, i: usize) -> Option<&Mpv> {
        self.instances.get(i).map(|m| m.as_ref())
    }

    fn shutdown(&mut self) {
        // фильтры снимаем, пока живы все ядра: после уничтожения первого ffmpeg пишет сводку astats в stderr
        for m in &self.instances {
            let _ = m.set_property_string("af", "");
        }
        for m in &self.instances {
            m.close();
        }
        self.instances.clear();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

fn event_loop(index: usize, m: Arc<Mpv>, tx: mpsc::UnboundedSender<Msg>, shared: Arc<Shared>) {
    loop {
        let msg = match m.wait_event(-1.0) {
            None if m.is_closing() => break,
            None => continue,
            Some(MpvEvent::Shutdown) => break,
            Some(MpvEvent::PropertyChange { name, value }) if name == "af-metadata/lvl" => {
                if !shared.levels_wanted.load(Ordering::Relaxed) || shared.cur.load(Ordering::Relaxed) != index {
                    continue;
                }
                match level_from_metadata(&value) {
                    Some(level) => Msg::Level(index, level),
                    None => continue,
                }
            }
            Some(event) => Msg::Mpv(index, event),
        };
        if tx.send(msg).is_err() {
            break;
        }
    }
}

/// af-metadata/lvl → 0..1; None — метаданных нет или значение не число.
fn level_from_metadata(value: &PropertyValue) -> Option<f64> {
    let PropertyValue::Map(entries) = value else { return None };
    if entries.is_empty() {
        return None;
    }
    let db: f64 = value.map_get(LEVEL_KEY).unwrap_or("-inf").trim().parse().ok()?;
    Some(level_from_db(db))
}

/// _drop_queued_sync: убрать из плейлиста mpv всё после текущего.
fn drop_queued_sync(m: &Mpv) {
    loop {
        let count = m.get_property_i64("playlist-count").unwrap_or(0);
        let pos = m.get_property_i64("playlist-pos").unwrap_or(0);
        if count <= 1 + pos || m.command(&["playlist-remove", &(count - 1).to_string()]).is_err() {
            break;
        }
    }
}

type Job = Box<dyn FnOnce(&mut MpvPool) + Send>;

enum CmdMsg {
    Job(Job),
    Shutdown(std::sync::mpsc::Sender<()>),
}

/// Поток mpv-cmd: команды libmpv по порядку. Актор никогда не ждёт mpv.
struct Cmd {
    tx: std::sync::mpsc::Sender<CmdMsg>,
    cancel: Arc<AtomicBool>,
}

impl Cmd {
    fn spawn(mut pool: MpvPool) -> Result<Self, PlayerError> {
        let (tx, rx) = std::sync::mpsc::channel::<CmdMsg>();
        let cancel = Arc::new(AtomicBool::new(false));
        let skip = cancel.clone();
        std::thread::Builder::new()
            .name("mpv-cmd".into())
            .spawn(move || {
                for msg in rx {
                    match msg {
                        CmdMsg::Job(job) if !skip.load(Ordering::SeqCst) => job(&mut pool),
                        CmdMsg::Job(_) => {}
                        CmdMsg::Shutdown(done) => {
                            pool.shutdown();
                            let _ = done.send(());
                            return;
                        }
                    }
                }
                pool.shutdown();
            })
            .map_err(|e| PlayerError::Mpv(e.to_string()))?;
        Ok(Self { tx, cancel })
    }

    fn run(&self, job: impl FnOnce(&mut MpvPool) + Send + 'static) {
        let _ = self.tx.send(CmdMsg::Job(Box::new(job)));
    }

    /// Команда экземпляру `i`; ошибки libmpv только в лог (как traceback в Python).
    fn on(&self, i: usize, f: impl FnOnce(&Mpv) -> Result<(), PlayerError> + Send + 'static) {
        self.run(move |pool| {
            if let Some(m) = pool.get(i)
                && let Err(e) = f(m)
            {
                tracing::debug!("mpv[{i}]: {e}");
            }
        });
    }

    fn shutdown(&self, done: std::sync::mpsc::Sender<()>) {
        self.cancel.store(true, Ordering::SeqCst);
        let _ = self.tx.send(CmdMsg::Shutdown(done));
    }
}

// --- актор ------------------------------------------------------------------

struct Fade {
    old: usize,
    new: usize,
    t0: Instant,
    length: f64,
}

struct Actor {
    state: PlayerState,
    /// свойства для QML; производные от state поля обновляет publish
    snap: PlayerSnapshot,
    queue_dirty: bool,
    cover_dir: PathBuf,
    session_file: Option<PathBuf>,
    crossfade_s: u32,
    quality: Quality,

    // состояние mpv — из наблюдателей главного экземпляра
    paused: bool,
    idle_active: bool,

    /// отменяет устаревшие ответы resolve
    load_token: u64,
    /// индекс очереди, чья ссылка уже стоит в mpv вторым номером (или готова для перехода)
    prefetched_for: Option<usize>,
    prefetched_codec: String,
    /// (индекс, url, подпись кодека) следующего трека для перехода
    next_ready: Option<(usize, String, String)>,
    last_time: Option<f64>,
    /// TrackStarted отправлен, TrackEnded — ещё нет
    track_live: bool,
    /// главный экземпляр mpv (второй — для плавного перехода)
    cur: usize,
    has_second: bool,
    fade: Option<Fade>,
    /// восстановленная позиция: с неё начнётся первый запуск
    resume_ms: i64,
    cover_seq: u64,

    poll_at: Option<Instant>,
    fade_at: Option<Instant>,
    save_at: Option<Instant>,
    autosave_at: Option<Instant>,

    cmd: Cmd,
    msgs: mpsc::UnboundedSender<Msg>,
    shared: Arc<Shared>,
    resolver: Arc<dyn StreamResolver>,
    http: reqwest::Client,
    watch: watch::Sender<PlayerSnapshot>,
    events: broadcast::Sender<PlayerEvent>,
}

async fn sleep_until_opt(deadline: Option<Instant>) {
    match deadline {
        Some(t) => tokio::time::sleep_until(t).await,
        None => std::future::pending().await,
    }
}

fn ms(v: u64) -> Duration {
    Duration::from_millis(v)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

impl Actor {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>, mut msgs: mpsc::UnboundedReceiver<Msg>) {
        loop {
            let deadline = [self.poll_at, self.fade_at, self.save_at, self.autosave_at].into_iter().flatten().min();
            tokio::select! {
                biased;
                command = commands.recv() => match command {
                    Some(Command::Shutdown(done)) => {
                        self.shutdown(done);
                        return;
                    }
                    Some(command) => self.command(command),
                    None => {
                        let (done, _) = std::sync::mpsc::channel();
                        self.shutdown(done);
                        return;
                    }
                },
                Some(msg) = msgs.recv() => self.message(msg),
                () = sleep_until_opt(deadline) => self.timers(),
            }
        }
    }

    fn shutdown(&mut self, done: std::sync::mpsc::Sender<()>) {
        self.save_session(true);
        self.cmd.shutdown(done);
    }

    // --- снимок и события ---

    fn publish(&mut self) {
        if self.queue_dirty {
            self.snap.queue = Arc::new(self.state.queue.clone());
            self.queue_dirty = false;
        }
        self.snap.shuffle = self.state.shuffle;
        self.snap.repeat = self.state.repeat;
        self.snap.current_index = self.state.current_index();
        self.snap.context = self.state.context.clone();
        self.snap.feeder = self.state.feeder;
        self.watch.send_replace(self.snap.clone());
    }

    fn emit(&mut self, event: PlayerEvent) {
        if matches!(event, PlayerEvent::QueueChanged | PlayerEvent::TrackChanged | PlayerEvent::ModeChanged) {
            self.schedule_save();
        }
        self.publish();
        let _ = self.events.send(event);
    }

    fn queue_changed(&mut self, change: QueueChange) {
        self.queue_dirty = true;
        self.publish();
        let change = match change {
            QueueChange::Reset(_) => QueueChange::Reset(self.snap.queue.clone()),
            other => other,
        };
        let _ = self.events.send(PlayerEvent::Queue(change));
    }

    fn set_error(&mut self, text: String) {
        if text != self.snap.error_text {
            self.snap.error_text = text;
            self.emit(PlayerEvent::ErrorChanged);
        }
    }

    fn has_track(&self) -> bool {
        self.snap.track.is_some()
    }

    // --- команды ---

    fn command(&mut self, command: Command) {
        match command {
            Command::PlayItems { items, row, context, feeder } => self.play_items(items, row, context, feeder),
            Command::ShuffleFrom { items, context } => {
                if items.is_empty() {
                    return;
                }
                if !self.state.shuffle {
                    self.state.shuffle = true;
                    self.emit(PlayerEvent::ModeChanged);
                }
                let row = fastrand::usize(..items.len());
                self.play_items(items, row, context, None);
            }
            Command::AppendTracks { items, feeder } => {
                if self.state.feeder == Some(feeder) {
                    self.append_tracks(items);
                }
            }
            Command::ReplaceUpcoming { items, feeder } => {
                if self.state.feeder == Some(feeder) {
                    self.replace_upcoming(items);
                }
            }
            Command::PlayIndex(index) => {
                if let Some(pos) = self.state.pos_of(index) {
                    self.snap.direction = if self.state.pos.is_some_and(|p| pos < p) { -1 } else { 1 };
                    self.state.pos = Some(pos);
                    self.emit(PlayerEvent::QueueChanged);
                    self.start_current(false);
                }
            }
            Command::TogglePlay => {
                if !self.has_track() {
                    return;
                }
                self.finish_fade();
                // трек закончился и очередь встала (или восстановлен) — играть
                if self.idle_active {
                    self.start_current(false);
                } else {
                    self.cmd.on(self.cur, |m| m.command(&["cycle", "pause"]));
                }
            }
            Command::Play => {
                if self.has_track() {
                    self.cmd.on(self.cur, |m| m.set_property_flag("pause", false));
                }
            }
            Command::Pause => {
                self.finish_fade();
                self.cmd.on(self.cur, |m| m.set_property_flag("pause", true));
            }
            Command::Stop => self.cmd.on(self.cur, |m| m.command(&["stop"])),
            Command::Next => self.next(),
            Command::Previous => match self.state.previous(self.snap.position_ms) {
                Previous::Restart => self.seek_ms(0),
                Previous::Pos(pos) => {
                    self.state.pos = Some(pos);
                    self.snap.direction = -1;
                    self.emit(PlayerEvent::QueueChanged);
                    self.start_current(false);
                }
            },
            Command::Seek(fraction) => {
                if self.snap.duration_ms > 0 {
                    self.seek_ms((fraction * self.snap.duration_ms as f64) as i64);
                }
            }
            Command::SeekMs(ms) => self.seek_ms(ms),
            Command::CycleRepeat => self.set_repeat(self.state.cycled_repeat()),
            Command::SetRepeat(repeat) => self.set_repeat(repeat),
            Command::SetShuffle(shuffle) => {
                if self.state.set_shuffle(shuffle) {
                    self.emit(PlayerEvent::ModeChanged);
                    self.prefetch_next(true);
                }
            }
            Command::SetVolume(volume) => {
                let volume = if volume.is_nan() { 0.0 } else { volume.clamp(0.0, 1.0) };
                self.snap.volume = volume;
                let v = mpv_volume(volume);
                self.cmd.on(self.cur, move |m| m.set_property_f64("volume", v));
                self.emit(PlayerEvent::VolumeChanged);
            }
            Command::SetLevelsWanted(wanted) => {
                self.snap.levels_wanted = wanted;
                self.shared.levels_wanted.store(wanted, Ordering::Relaxed);
                if !wanted && self.snap.level != 0.0 {
                    self.snap.level = 0.0;
                    self.emit(PlayerEvent::LevelChanged);
                } else {
                    self.publish();
                }
            }
            Command::SetCrossfade(seconds) => {
                self.crossfade_s = seconds;
                self.prefetch_next(true);
            }
            Command::SetQuality(quality) => self.quality = quality,
            Command::RestoreSession => self.restore_session(),
            Command::Shutdown(_) => unreachable!("обрабатывается в run"),
        }
    }

    fn play_items(&mut self, items: Vec<TrackItem>, row: usize, context: PlayContext, feeder: Option<FeederId>) {
        if items.is_empty() || row >= items.len() {
            return;
        }
        self.state.context = context;
        self.state.feeder = feeder;
        self.snap.direction = 1;
        self.state.reset(items, row);
        self.queue_changed(QueueChange::Reset(Arc::default()));
        self.emit(PlayerEvent::QueueChanged);
        self.start_current(false);
    }

    /// Догрузка в конец очереди (волна). Если очередь уже встала на последнем треке — играем дальше.
    fn append_tracks(&mut self, items: Vec<TrackItem>) {
        if items.is_empty() {
            return;
        }
        let last = self.state.order.len().checked_sub(1);
        let stalled = self.has_track() && self.idle_active && self.state.pos == last;
        self.state.append(items.clone());
        self.queue_changed(QueueChange::Append(items));
        self.emit(PlayerEvent::QueueChanged);
        if stalled {
            self.next();
        } else {
            self.prefetch_next(false);
        }
    }

    fn replace_upcoming(&mut self, items: Vec<TrackItem>) {
        let Some((at, count)) = self.state.truncate_after_current() else {
            return;
        };
        if count > 0 {
            self.queue_changed(QueueChange::Remove { at, count });
        }
        self.drop_queued();
        self.append_tracks(items);
    }

    fn next(&mut self) {
        if let Some(pos) = self.state.next_pos(true) {
            self.state.pos = Some(pos);
            self.snap.direction = 1;
            self.emit(PlayerEvent::QueueChanged);
            self.start_current(false);
        }
    }

    fn seek_ms(&mut self, ms: i64) {
        if !self.has_track() {
            return;
        }
        let ms = ms.max(0);
        let seconds = format!("{:.3}", ms as f64 / 1000.0);
        self.cmd.on(self.cur, move |m| m.command(&["seek", &seconds, "absolute"]));
        // восстановленный, но ещё не запущенный трек начнётся с новой позиции
        if self.resume_ms > 0 && self.idle_active {
            self.resume_ms = ms;
        }
        self.snap.position_ms = ms;
        self.emit(PlayerEvent::PositionChanged);
    }

    fn set_repeat(&mut self, repeat: Repeat) {
        if repeat == self.state.repeat {
            return;
        }
        self.state.repeat = repeat;
        let value = if repeat == Repeat::One { "inf" } else { "no" };
        self.cmd.run(move |pool| {
            for m in &pool.instances {
                if let Err(e) = m.set_property_string("loop-file", value) {
                    tracing::debug!("mpv loop-file: {e}");
                }
            }
        });
        self.emit(PlayerEvent::ModeChanged);
        self.prefetch_next(true);
    }

    // --- очередь и загрузка ---

    fn start_current(&mut self, natural: bool) {
        let Some(track) = self.state.current_track().cloned() else {
            return;
        };
        self.finish_fade();
        let resume = std::mem::take(&mut self.resume_ms);
        self.set_track(track.clone(), natural);
        if resume > 0 {
            self.snap.position_ms = resume;
            self.emit(PlayerEvent::PositionChanged);
        }
        self.load_token += 1;
        self.prefetched_for = None;
        self.next_ready = None;
        self.resolve(track.track_id.clone(), Target::Current { resume_ms: resume });
        self.fetch_cover(track.cover);
    }

    fn resolve(&self, track_id: String, target: Target) {
        let future = self.resolver.resolve(track_id, self.quality);
        let (token, tx) = (self.load_token, self.msgs.clone());
        tokio::spawn(async move {
            let result = future.await;
            let _ = tx.send(Msg::Resolved { token, target, result });
        });
    }

    fn schedule_prefetch(&self) {
        let (token, tx) = (self.load_token, self.msgs.clone());
        tokio::spawn(async move {
            tokio::time::sleep(ms(PREFETCH_DELAY_MS)).await;
            let _ = tx.send(Msg::PrefetchTimer(token));
        });
    }

    fn on_resolved(&mut self, token: u64, target: Target, result: Result<StreamInfo, String>) {
        if token != self.load_token {
            return;
        }
        match (target, result) {
            (Target::Current { resume_ms }, Ok(info)) => {
                self.snap.codec = codec_label(&info.codec, info.bitrate);
                self.emit(PlayerEvent::TrackChanged);
                let start = (resume_ms > 0).then(|| resume_ms as f64 / 1000.0);
                let url = info.url;
                self.cmd.on(self.cur, move |m| {
                    m.loadfile(&url, "replace", start)?;
                    m.set_property_flag("pause", false)
                });
                // следующий трек — через пару секунд, чтобы запрос не пришёлся на анимацию смены трека
                self.schedule_prefetch();
            }
            (Target::Current { .. }, Err(e)) => {
                self.set_error(format!("Не удалось получить трек: {e}"));
                self.update_playing();
            }
            (Target::Prefetch { index }, Ok(info)) => {
                self.prefetched_for = Some(index);
                let codec = codec_label(&info.codec, info.bitrate);
                let cur = self.cur;
                if self.crossfade_s > 0 {
                    self.next_ready = Some((index, info.url, codec));
                    // в плейлисте mpv — только текущий
                    self.cmd.on(cur, |m| {
                        drop_queued_sync(m);
                        Ok(())
                    });
                    self.ensure_second();
                    return;
                }
                self.next_ready = None;
                self.prefetched_codec = codec;
                let url = info.url;
                self.cmd.on(cur, move |m| {
                    drop_queued_sync(m);
                    m.loadfile(&url, "append", None)
                });
            }
            (Target::Prefetch { .. }, Err(e)) => tracing::debug!("ссылка на следующий трек: {e}"),
        }
    }

    /// Второй экземпляр mpv — лениво, в потоке mpv-cmd (создание libmpv занимает десятки мс).
    fn ensure_second(&self) {
        if self.has_second {
            return;
        }
        let (volume, loop_one, tx) = (self.snap.volume, self.state.repeat == Repeat::One, self.msgs.clone());
        self.cmd.run(move |pool| {
            if pool.instances.len() < 2
                && let Err(e) = pool.spawn_instance(volume, loop_one)
            {
                tracing::warn!("второй экземпляр mpv: {e}");
                return;
            }
            let _ = tx.send(Msg::SecondReady);
        });
    }

    /// Поставить в mpv вторым номером следующий трек — для перехода без паузы.
    fn prefetch_next(&mut self, force: bool) {
        let next = self.state.next_pos(false);
        let index = next.and_then(|p| self.state.order.get(p).copied());
        let (Some(index), false, true) = (index, self.state.repeat == Repeat::One, self.has_track()) else {
            self.drop_queued();
            return;
        };
        if Some(index) == self.prefetched_for && !force {
            return;
        }
        let Some(track) = self.state.queue.get(index) else {
            return;
        };
        self.resolve(track.track_id.clone(), Target::Prefetch { index });
    }

    fn drop_queued(&mut self) {
        self.prefetched_for = None;
        self.next_ready = None;
        self.cmd.on(self.cur, |m| {
            drop_queued_sync(m);
            Ok(())
        });
    }

    fn fetch_cover(&mut self, url: String) {
        self.cover_seq += 1;
        let (seq, tx, client, dir) = (self.cover_seq, self.msgs.clone(), self.http.clone(), self.cover_dir.clone());
        tokio::spawn(async move {
            let path = match download_cover(&client, &dir, &url).await {
                Ok(path) => Some(path),
                Err(e) => {
                    tracing::debug!("обложка {url}: {e}");
                    None
                }
            };
            let _ = tx.send(Msg::Cover { seq, path });
        });
    }

    fn set_track(&mut self, track: TrackItem, natural: bool) {
        self.end_current(natural);
        self.snap.duration_ms = track.duration_ms;
        self.snap.position_ms = 0;
        self.snap.played_seconds = 0.0;
        self.snap.track = Some(track.clone());
        self.last_time = None;
        self.set_error(String::new());
        self.emit(PlayerEvent::TrackChanged);
        self.emit(PlayerEvent::PositionChanged);
        self.track_live = true;
        let started = TrackStarted { track, context: self.state.context.clone() };
        self.emit(PlayerEvent::TrackStarted(started));
        self.feed();
    }

    /// Сообщить о конце текущего трека (смена трека или очередь кончилась).
    /// natural — трек доигран (плавный переход начинается раньше конца, но это не пропуск).
    fn end_current(&mut self, natural: bool) {
        if !self.track_live {
            return;
        }
        self.track_live = false;
        let Some(track) = self.snap.track.clone() else {
            return;
        };
        let end = self.snap.position_ms as f64 / 1000.0;
        let duration = self.snap.duration_ms as f64 / 1000.0;
        let ended = TrackEnded {
            track,
            context: self.state.context.clone(),
            played: round1(self.snap.played_seconds),
            end: round1(end),
            natural: natural || (duration > 0.0 && end >= duration - 3.0),
        };
        self.emit(PlayerEvent::TrackEnded(ended));
    }

    fn feed(&mut self) {
        if let Some(feeder) = self.state.feeder
            && self.state.ahead() < FEED_AHEAD as isize
        {
            self.emit(PlayerEvent::NeedMore(feeder));
        }
    }

    // --- события mpv (только от главного экземпляра) ---

    fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Mpv(i, event) if i == self.cur => self.mpv_event(event),
            Msg::Mpv(..) => {}
            Msg::Level(i, level) => {
                if i == self.cur && self.snap.levels_wanted && (level - self.snap.level).abs() > 0.005 {
                    self.snap.level = level;
                    self.emit(PlayerEvent::LevelChanged);
                }
            }
            Msg::Time(seconds) => self.on_time(seconds),
            Msg::Resolved { token, target, result } => self.on_resolved(token, target, result),
            Msg::Cover { seq, path } => {
                if let Some(path) = path
                    && seq == self.cover_seq
                    && path != self.snap.cover_file
                {
                    self.snap.cover_file = path;
                    self.emit(PlayerEvent::CoverFileChanged);
                }
            }
            Msg::PrefetchTimer(token) => {
                if token == self.load_token {
                    self.prefetch_next(false);
                }
            }
            Msg::SecondReady => self.has_second = true,
        }
    }

    fn mpv_event(&mut self, event: MpvEvent) {
        match event {
            MpvEvent::PropertyChange { name, value } => match name.as_str() {
                "pause" => {
                    self.paused = value.as_flag().unwrap_or(false);
                    self.update_playing();
                }
                "idle-active" => {
                    self.idle_active = value.as_flag().unwrap_or(false);
                    if self.idle_active && self.track_live && self.snap.played_seconds > 0.0 {
                        self.end_current(false); // очередь доиграла до конца
                    }
                    self.update_playing();
                }
                "paused-for-cache" => {
                    let buffering = value.as_flag().unwrap_or(false);
                    if buffering != self.snap.buffering {
                        self.snap.buffering = buffering;
                        self.emit(PlayerEvent::StateChanged);
                    }
                }
                "duration" => {
                    if let Some(seconds) = value.as_f64().filter(|s| *s != 0.0) {
                        self.snap.duration_ms = (seconds * 1000.0) as i64;
                        self.emit(PlayerEvent::PositionChanged);
                    }
                }
                "playlist-pos" => self.on_playlist_pos(value.as_i64()),
                _ => {}
            },
            MpvEvent::EndFile { reason } => self.on_end_file(reason),
            MpvEvent::Shutdown => {}
        }
    }

    fn update_playing(&mut self) {
        let playing = self.has_track() && !self.paused && !self.idle_active;
        if playing == self.snap.playing {
            return;
        }
        self.snap.playing = playing;
        self.emit(PlayerEvent::StateChanged);
        let now = Instant::now();
        if playing {
            self.poll_at = Some(now + ms(POSITION_POLL_MS));
            self.autosave_at = Some(now + ms(AUTOSAVE_MS));
        } else {
            self.poll_at = None;
            self.autosave_at = None;
            self.poll_time(); // точная позиция на паузе
            self.schedule_save();
        }
    }

    fn poll_time(&self) {
        let (i, tx) = (self.cur, self.msgs.clone());
        self.cmd.run(move |pool| {
            let seconds = pool.get(i).and_then(|m| m.get_property_f64("time-pos"));
            let _ = tx.send(Msg::Time(seconds));
        });
    }

    fn on_time(&mut self, seconds: Option<f64>) {
        let Some(seconds) = seconds else {
            return;
        };
        let last = self.last_time.replace(seconds);
        // перемотка не считается прослушиванием
        if let Some(last) = last
            && 0.0 < seconds - last
            && seconds - last < 1.0
        {
            self.snap.played_seconds += seconds - last;
        }
        self.snap.position_ms = (seconds * 1000.0) as i64;
        self.emit(PlayerEvent::PositionChanged);
        // плавный переход: за crossfade секунд до конца
        let cf = self.crossfade_s;
        if cf > 0
            && self.fade.is_none()
            && self.next_ready.is_some()
            && self.snap.duration_ms > 2000 * i64::from(cf)
            && (self.snap.duration_ms - self.snap.position_ms) as f64 / 1000.0 <= f64::from(cf)
        {
            self.begin_crossfade(f64::from(cf));
        }
    }

    /// mpv сам перешёл на заранее поставленный трек (gapless) → догоняем очередь.
    fn on_playlist_pos(&mut self, pos: Option<i64>) {
        if pos.is_none_or(|p| p <= 0) || self.prefetched_for.is_none() {
            return;
        }
        let Some(next) = self.state.next_pos(false) else {
            return;
        };
        self.state.pos = Some(next);
        self.snap.direction = 1;
        self.prefetched_for = None;
        self.cmd.on(self.cur, |m| m.command(&["playlist-remove", "0"]));
        self.snap.codec = std::mem::take(&mut self.prefetched_codec);
        self.emit(PlayerEvent::QueueChanged);
        if let Some(track) = self.state.current_track().cloned() {
            let cover = track.cover.clone();
            self.set_track(track, false);
            self.fetch_cover(cover);
        }
        self.prefetch_next(false);
    }

    fn on_end_file(&mut self, reason: i32) {
        self.update_playing();
        // EOF в режиме перехода: в плейлисте mpv нет следующего (короткий трек, ссылка не успела) — переходим сами.
        // В режиме gapless следующий уже стоит в mpv.
        if reason == 0 && self.crossfade_s > 0 && self.fade.is_none() {
            if let Some(next) = self.state.next_pos(false)
                && self.state.repeat != Repeat::One
            {
                self.state.pos = Some(next);
                self.snap.direction = 1;
                self.emit(PlayerEvent::QueueChanged);
                self.start_current(true);
            }
            return;
        }
        if reason == 4 {
            self.set_error("Не удалось воспроизвести трек — пропускаю".into());
            if self.state.next_pos(false).is_some() {
                self.next();
            }
        }
    }

    // --- плавный переход ---

    fn begin_crossfade(&mut self, length: f64) {
        let Some((index, url, codec)) = self.next_ready.take() else {
            return;
        };
        let Some(next) = self.state.next_pos(false) else {
            return;
        };
        // очередь поменялась или второй mpv ещё не готов — без перехода
        if self.state.order.get(next) != Some(&index) || !self.has_second {
            return;
        }
        let (old, new) = (self.cur, 1 - self.cur);
        let loop_file = if self.state.repeat == Repeat::One { "inf" } else { "no" };
        self.cmd.on(new, move |m| {
            m.set_property_f64("volume", 0.0)?;
            m.set_property_string("loop-file", loop_file)?;
            m.loadfile(&url, "replace", None)?;
            m.set_property_flag("pause", false)
        });
        let now = Instant::now();
        self.fade = Some(Fade { old, new, t0: now, length });
        self.cur = new;
        self.shared.cur.store(new, Ordering::Relaxed);
        self.prefetched_for = None;
        self.state.pos = Some(next);
        self.snap.direction = 1;
        self.snap.codec = codec;
        self.load_token += 1;
        self.emit(PlayerEvent::QueueChanged);
        if let Some(track) = self.state.current_track().cloned() {
            let cover = track.cover.clone();
            self.set_track(track, true);
            self.fetch_cover(cover);
        }
        self.schedule_prefetch();
        self.fade_at = Some(now + ms(FADE_STEP_MS));
    }

    fn fade_step(&mut self) {
        let Some(fade) = &self.fade else {
            self.fade_at = None;
            return;
        };
        let t = fade.t0.elapsed().as_secs_f64() / fade.length;
        if t >= 1.0 {
            self.finish_fade();
            return;
        }
        // равномощная кривая по амплитуде; громкость mpv кубическая — отсюда корень третьей степени
        let base = mpv_volume(self.snap.volume);
        let angle = t * std::f64::consts::FRAC_PI_2;
        let v_old = base * angle.cos().cbrt();
        let v_new = base * angle.sin().cbrt();
        let (old, new) = (fade.old, fade.new);
        self.cmd.run(move |pool| {
            if let (Some(o), Some(n)) = (pool.get(old), pool.get(new)) {
                let _ = o.set_property_f64("volume", v_old);
                let _ = n.set_property_f64("volume", v_new);
            }
        });
    }

    fn finish_fade(&mut self) {
        self.fade_at = None;
        let Some(fade) = self.fade.take() else {
            return;
        };
        let base = mpv_volume(self.snap.volume);
        self.cmd.run(move |pool| {
            if let Some(o) = pool.get(fade.old) {
                let _ = o.command(&["stop"]);
            }
            if let Some(n) = pool.get(fade.new) {
                let _ = n.set_property_f64("volume", base);
            }
        });
    }

    // --- таймеры ---

    fn timers(&mut self) {
        let now = Instant::now();
        if self.poll_at.is_some_and(|t| t <= now) {
            self.poll_at = Some(now + ms(POSITION_POLL_MS));
            self.poll_time();
        }
        if self.fade_at.is_some_and(|t| t <= now) {
            self.fade_at = Some(now + ms(FADE_STEP_MS));
            self.fade_step();
        }
        if self.autosave_at.is_some_and(|t| t <= now) {
            self.autosave_at = Some(now + ms(AUTOSAVE_MS));
            self.schedule_save();
        }
        if self.save_at.is_some_and(|t| t <= now) {
            self.save_at = None;
            self.save_session(false);
        }
    }

    fn schedule_save(&mut self) {
        self.save_at = Some(Instant::now() + ms(SAVE_DEBOUNCE_MS));
    }

    // --- сессия ---

    fn session_data(&self) -> Option<(PathBuf, SessionData)> {
        let path = self.session_file.clone()?;
        if !self.has_track() || self.state.queue.is_empty() {
            return None;
        }
        let data = SessionData {
            items: self.state.queue.clone(),
            order: self.state.order.clone(),
            pos: self.state.pos?,
            position_ms: self.snap.position_ms,
            context: SessionData::saved_context(&self.state.context),
            shuffle: self.state.shuffle,
            repeat: self.state.repeat.as_str().to_owned(),
        };
        Some((path, data))
    }

    fn save_session(&self, sync: bool) {
        let Some((path, data)) = self.session_data() else {
            return;
        };
        if sync {
            if let Err(e) = session::save(&path, &data) {
                tracing::warn!("сессия: {e}");
            }
        } else {
            tokio::task::spawn_blocking(move || {
                if let Err(e) = session::save(&path, &data) {
                    tracing::warn!("сессия: {e}");
                }
            });
        }
    }

    /// Очередь и позиция прошлого запуска — на паузе; Play продолжит с той же секунды.
    fn restore_session(&mut self) {
        let Some(data) = self.session_file.as_deref().and_then(session::load) else {
            return;
        };
        self.state.queue = data.items;
        self.state.order = data.order;
        self.state.pos = Some(data.pos);
        self.state.context = data.context;
        self.state.feeder = None;
        self.state.shuffle = data.shuffle;
        self.state.repeat = Repeat::from_str(&data.repeat).unwrap_or_default();
        let Some(track) = self.state.current_track().cloned() else {
            return;
        };
        self.snap.duration_ms = track.duration_ms;
        let cover = track.cover.clone();
        self.snap.track = Some(track);
        self.resume_ms = data.position_ms.max(0);
        self.snap.position_ms = self.resume_ms;
        let loop_one = self.state.repeat == Repeat::One;
        self.cmd.on(self.cur, move |m| m.set_property_string("loop-file", if loop_one { "inf" } else { "no" }));
        self.queue_changed(QueueChange::Reset(Arc::default()));
        self.emit(PlayerEvent::QueueChanged);
        self.emit(PlayerEvent::ModeChanged);
        self.emit(PlayerEvent::TrackChanged);
        self.emit(PlayerEvent::PositionChanged);
        self.fetch_cover(cover);
    }
}

/// Обложка → cover_dir/<sha1(url)>.jpg (есть — не качать; .part + rename). Пустой url → "".
async fn download_cover(client: &reqwest::Client, dir: &Path, url: &str) -> Result<String, String> {
    if url.is_empty() {
        return Ok(String::new());
    }
    let path = dir.join(format!("{}.jpg", hex::encode(Sha1::digest(url.as_bytes()))));
    let text = path.to_string_lossy().into_owned();
    if tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(text);
    }
    tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
    let response = client.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let tmp = path.with_extension("part");
    tokio::fs::write(&tmp, &bytes).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(&tmp, &path).await.map_err(|e| e.to_string())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_metadata() {
        let map = |v: &str| PropertyValue::Map(vec![(LEVEL_KEY.into(), v.into())]);
        assert_eq!(level_from_metadata(&map("-inf")), Some(0.0));
        assert_eq!(level_from_metadata(&map("-6.0")), Some(1.0));
        assert_eq!(level_from_metadata(&map("-42")), Some(0.0));
        assert!((level_from_metadata(&map("-24")).unwrap() - 0.5).abs() < 1e-9);
        assert_eq!(level_from_metadata(&map("abc")), None);
        assert_eq!(level_from_metadata(&PropertyValue::Map(vec![])), None);
        assert_eq!(level_from_metadata(&PropertyValue::Map(vec![("x".into(), "1".into())])), Some(0.0));
        assert_eq!(level_from_metadata(&PropertyValue::None), None);
    }

    #[test]
    fn rounding() {
        assert_eq!(round1(12.345), 12.3);
        assert_eq!(round1(0.06), 0.1);
    }

    #[tokio::test]
    async fn cover_cached_file_reused() {
        let dir = std::env::temp_dir().join(format!("ym-player-covers-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://example.invalid/cover/400x400";
        let path = dir.join(format!("{}.jpg", hex::encode(Sha1::digest(url.as_bytes()))));
        std::fs::write(&path, b"jpg").unwrap();
        let client = reqwest::Client::new();
        assert_eq!(download_cover(&client, &dir, url).await.unwrap(), path.to_string_lossy());
        assert_eq!(download_cover(&client, &dir, "").await.unwrap(), "");
        let _ = std::fs::remove_dir_all(dir);
    }
}
