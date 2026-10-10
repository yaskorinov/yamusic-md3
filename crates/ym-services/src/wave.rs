//! «Моя волна» (src/yamusic/wave.py): сессия rotor, настройки, обратная связь, станции по сиду.
//! Поведение play/play_station/select/_start/_more/_renew/_take/_on_track_*/_on_like_feedback — как в Python;
//! выбор настроек хранится в Settings.waveSeeds (сиды через запятую).
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use ym_api::items::{PlayContext, TrackItem};
use ym_api::models::SequenceItem;
use ym_api::{RotorEvent, RotorFeedback, RotorSessionNew, YmClient};
use ym_platform::{SettingKey, SettingValue, SettingsStore};
use ym_player::{FeederId, PlayerEvent, PlayerHandle, TrackEnded, TrackStarted};

use crate::auth::{AuthEvent, AuthService};
use crate::library::{FeedbackKind, LibraryEvent, LibraryService};
use crate::lock;

pub const WAVE_SEED: &str = "user:onyourwave";
pub const WAVE_FROM: &str = "desktop_win-radio-user-onyourwave-default";
/// сколько последних треков ('id:album') передавать в queue, чтобы волна их не повторяла
pub const RECENT: usize = 60;
/// feeder волны в плеере
pub const FEEDER: FeederId = FeederId(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveItem {
    pub label: String,
    pub seed: String,
}

/// Wave.groups[i]: {key: "context"|"diversity"|"moodEnergy"|"language", title, items: [{label, seed}]}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveGroup {
    pub key: String,
    pub title: String,
    pub items: Vec<WaveItem>,
}

fn group(key: &str, title: &str, items: &[(&str, &str)]) -> WaveGroup {
    WaveGroup {
        key: key.to_owned(),
        title: title.to_owned(),
        items: items.iter().map(|(label, seed)| WaveItem { label: (*label).into(), seed: (*seed).into() }).collect(),
    }
}

/// FALLBACK_GROUPS из wave.py — дословно.
pub fn fallback_groups() -> Vec<WaveGroup> {
    vec![
        group(
            "context",
            "Занятие",
            &[
                ("Танцую", "genre:dance"),
                ("Просыпаюсь", "activity:wake-up"),
                ("В дороге", "activity:road-trip"),
                ("Работаю", "activity:work-background"),
                ("Тренируюсь", "activity:workout"),
                ("Отдыхаю", "mood:relaxed"),
                ("Засыпаю", "activity:fall-asleep"),
            ],
        ),
        group(
            "diversity",
            "Характер",
            &[
                ("Любимое", "settingDiversity:favorite"),
                ("Незнакомое", "settingDiversity:discover"),
                ("Популярное", "settingDiversity:popular"),
            ],
        ),
        group(
            "moodEnergy",
            "Настроение",
            &[
                ("Бодрое", "settingMoodEnergy:active"),
                ("Весёлое", "settingMoodEnergy:fun"),
                ("Спокойное", "settingMoodEnergy:calm"),
                ("Грустное", "settingMoodEnergy:sad"),
            ],
        ),
        group(
            "language",
            "Язык",
            &[
                ("Русский", "settingLanguage:russian"),
                ("Иностранный", "settingLanguage:not-russian"),
                ("Без слов", "settingLanguage:without-words"),
            ],
        ),
    ]
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn id_part(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// _groups_from_api: блоки type == "contexts" → группа context; setting_restrictions diversity/moodEnergy/language →
/// possibleValues (serializedSeed, без unspecified); недостающее — из fallback_groups, в их порядке.
pub fn groups_from_api(settings: &serde_json::Value) -> Vec<WaveGroup> {
    let mut groups: Vec<WaveGroup> = Vec::new();
    for block in settings.get("blocks").and_then(Value::as_array).into_iter().flatten() {
        if block.get("type").and_then(Value::as_str) != Some("contexts") {
            continue;
        }
        let mut items = Vec::new();
        for item in block.get("items").and_then(Value::as_array).into_iter().flatten() {
            let station = item.get("station").filter(|s| s.is_object()).unwrap_or(item);
            let Some(id) = station.get("id").filter(|v| v.is_object()) else { continue };
            let Some(name) = str_field(station, "name") else { continue };
            let (Some(kind), Some(tag)) =
                (id.get("type").and_then(id_part), id.get("tag").and_then(id_part))
            else {
                continue;
            };
            items.push(WaveItem { label: name.to_owned(), seed: format!("{kind}:{tag}") });
        }
        if !items.is_empty() {
            groups.push(WaveGroup { key: "context".into(), title: "Занятие".into(), items });
        }
    }
    let restrictions = settings.get("settingRestrictions");
    for (key, field, title) in
        [("diversity", "diversity", "Характер"), ("moodEnergy", "moodEnergy", "Настроение"), ("language", "language", "Язык")]
    {
        let values = restrictions.and_then(|r| r.get(field)).and_then(|e| e.get("possibleValues")).and_then(Value::as_array);
        let items: Vec<WaveItem> = values
            .into_iter()
            .flatten()
            .filter(|v| !v.get("unspecified").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|v| {
                let seed = str_field(v, "serializedSeed")?;
                let label = v.get("name").and_then(Value::as_str).unwrap_or_default();
                Some(WaveItem { label: label.to_owned(), seed: seed.to_owned() })
            })
            .collect();
        if !items.is_empty() {
            groups.push(WaveGroup { key: key.into(), title: title.into(), items });
        }
    }
    fallback_groups()
        .into_iter()
        .map(|f| groups.iter().find(|g| g.key == f.key).cloned().unwrap_or(f))
        .collect()
}

/// Выбор из Settings.waveSeeds: сид → группа, в которой он есть.
pub fn selection_from_seeds(groups: &[WaveGroup], seeds: &str) -> BTreeMap<String, String> {
    let mut selection = BTreeMap::new();
    for seed in seeds.split(',').filter(|s| !s.is_empty()) {
        for group in groups {
            if group.items.iter().any(|i| i.seed == seed) {
                selection.insert(group.key.clone(), seed.to_owned());
            }
        }
    }
    selection
}

/// Сиды сессии: станция — она одна; иначе занятие (или user:onyourwave) + остальные настройки.
pub fn session_seeds(station: &str, selection: &BTreeMap<String, String>) -> Vec<String> {
    if !station.is_empty() {
        return vec![station.to_owned()];
    }
    let context = selection.get("context").cloned().unwrap_or_else(|| WAVE_SEED.to_owned());
    std::iter::once(context)
        .chain(selection.iter().filter(|(k, _)| k.as_str() != "context").map(|(_, s)| s.clone()))
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaveSnapshot {
    pub groups: Arc<Vec<WaveGroup>>,
    /// key группы → сид (Wave.selection)
    pub selection: BTreeMap<String, String>,
    /// плеер играет волну (Player.source == "wave")
    pub active: bool,
    pub loading: bool,
    pub error_text: String,
    /// волна по исполнителю/треку ("" — «Моя волна»)
    pub station_title: String,
    pub station: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveEvent {
    GroupsChanged,
    SelectionChanged,
    /// active/loading/stationTitle
    StateChanged,
    ErrorChanged,
}

#[derive(Default)]
struct Session {
    snap: WaveSnapshot,
    session_id: String,
    batch_id: String,
    /// id:album полученных треков
    recent: Vec<String>,
    fetching: bool,
    epoch: u64,
}

impl Session {
    fn take(&mut self, sequence: &[SequenceItem]) -> Vec<TrackItem> {
        let items: Vec<TrackItem> = sequence.iter().filter_map(|s| s.track.as_ref()).map(TrackItem::from_track).collect();
        self.recent.extend(items.iter().map(TrackItem::full_id));
        if self.recent.len() > RECENT {
            self.recent.drain(..self.recent.len() - RECENT);
        }
        items
    }

    fn batch(&self) -> Option<String> {
        Some(self.batch_id.clone()).filter(|b| !b.is_empty())
    }
}

pub struct WaveService {
    rt: Handle,
    snapshot: watch::Sender<WaveSnapshot>,
    events: broadcast::Sender<WaveEvent>,
    auth: Arc<AuthService>,
    player: PlayerHandle,
    settings: Arc<SettingsStore>,
    no_report: bool,
    state: Mutex<Session>,
    me: Weak<WaveService>,
}

impl WaveService {
    /// Подписки: AuthEvent::SignedIn → rotor_wave_settings; события плеера (QueueChanged → active,
    /// TrackStarted/TrackEnded → feedback, NeedMore(FEEDER) → следующая партия); LibraryEvent::Feedback → like/unlike/dislike.
    /// Выбор восстанавливается из Settings.waveSeeds.
    pub fn new(
        rt: Handle,
        auth: Arc<AuthService>,
        player: PlayerHandle,
        library: Arc<LibraryService>,
        settings: Arc<SettingsStore>,
        no_report: bool,
    ) -> Arc<Self> {
        let groups = fallback_groups();
        let selection = selection_from_seeds(&groups, &settings.get_str(SettingKey::WaveSeeds));
        let snap = WaveSnapshot { groups: Arc::new(groups), selection, ..WaveSnapshot::default() };
        let mut auth_events = auth.subscribe();
        let mut player_events = player.subscribe().events;
        let mut library_events = library.subscribe();
        let service = Arc::new_cyclic(|me| WaveService {
            rt: rt.clone(),
            snapshot: watch::Sender::new(snap.clone()),
            events: broadcast::channel(64).0,
            auth,
            player,
            settings,
            no_report,
            state: Mutex::new(Session { snap, ..Session::default() }),
            me: me.clone(),
        });

        let weak = Arc::downgrade(&service);
        rt.spawn(async move {
            loop {
                match auth_events.recv().await {
                    Ok(AuthEvent::SignedIn(_)) => match weak.upgrade() {
                        Some(me) => me.load_settings(),
                        None => return,
                    },
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        });

        let weak = Arc::downgrade(&service);
        rt.spawn(async move {
            loop {
                let event = player_events.recv().await;
                let Some(me) = weak.upgrade() else { return };
                match event {
                    Ok(PlayerEvent::QueueChanged) => me.update_active(),
                    Ok(PlayerEvent::TrackStarted(ev)) => me.on_track_started(&ev),
                    Ok(PlayerEvent::TrackEnded(ev)) => me.on_track_ended(&ev),
                    Ok(PlayerEvent::NeedMore(feeder)) if feeder == FEEDER => me.more(),
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => me.update_active(),
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        });

        let weak = Arc::downgrade(&service);
        rt.spawn(async move {
            loop {
                match library_events.recv().await {
                    Ok(LibraryEvent::Feedback { kind, track }) => match weak.upgrade() {
                        Some(me) => me.on_like_feedback(kind, &track),
                        None => return,
                    },
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        });
        service
    }

    /// Большая кнопка: «Моя волна» или пауза/продолжение, если она уже играет (и это не станция).
    pub fn play(&self) {
        let (active, station_empty) = {
            let st = lock(&self.state);
            (st.snap.active, st.snap.station.is_empty())
        };
        if active && station_empty {
            self.player.toggle_play();
        } else {
            self.set_station("", "");
            self.start(false);
        }
    }

    /// Волна от исполнителя/трека: сид artist:ID или track:ID, без настроек «Моей волны».
    pub fn play_station(&self, seed: String, title: String) {
        self.set_station(&seed, &title);
        self.start(false);
    }

    /// Выбрать вариант; повторный выбор того же — снять. Пишет Settings.waveSeeds; если волна играет — пересоздать
    /// сессию (станция сбрасывается) и заменить всё после текущего трека.
    pub fn select(&self, key: String, seed: String) {
        let (seeds, active) = self.update(|st| {
            let selection = &mut st.snap.selection;
            if selection.get(&key) == Some(&seed) {
                selection.remove(&key);
            } else {
                selection.insert(key, seed);
            }
            let seeds = selection.values().cloned().collect::<Vec<_>>().join(",");
            ((seeds, st.snap.active), vec![WaveEvent::SelectionChanged])
        });
        self.settings.set(SettingKey::WaveSeeds, SettingValue::Str(seeds));
        if active {
            self.set_station("", "");
            self.start(true);
        }
    }

    pub fn snapshot(&self) -> WaveSnapshot {
        self.snapshot.borrow().clone()
    }

    pub fn watch(&self) -> watch::Receiver<WaveSnapshot> {
        self.snapshot.subscribe()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WaveEvent> {
        self.events.subscribe()
    }

    // --- сессия ----------------------------------------------------------

    fn start(&self, replace: bool) {
        let Some(client) = self.auth.client() else {
            self.set_error("Войдите в аккаунт, чтобы слушать волну");
            return;
        };
        let Some(me) = self.me.upgrade() else { return };
        let current = if replace { self.player.snapshot().track } else { None };
        let (epoch, seeds) = self.update(|st| {
            st.epoch += 1;
            let mut events = Vec::new();
            if !st.snap.loading {
                st.snap.loading = true;
                events.push(WaveEvent::StateChanged);
            }
            if !st.snap.error_text.is_empty() {
                st.snap.error_text.clear();
                events.push(WaveEvent::ErrorChanged);
            }
            ((st.epoch, session_seeds(&st.snap.station, &st.snap.selection)), events)
        });
        let body = RotorSessionNew {
            seeds,
            queue: current.map(|t| vec![t.full_id()]),
            include_tracks_in_response: true,
        };
        self.rt.spawn(async move {
            let result = client.rotor_session_new(&body).await;
            let mut feedback = None;
            me.update(|st| {
                if st.epoch != epoch {
                    return ((), Vec::new());
                }
                st.snap.loading = false;
                let mut events = vec![WaveEvent::StateChanged];
                let error = match result {
                    Ok(session) if session.sequence.as_ref().is_some_and(|s| !s.is_empty()) => {
                        st.session_id = session.radio_session_id.clone().unwrap_or_default();
                        st.batch_id = session.batch_id.clone().unwrap_or_default();
                        st.fetching = false;
                        let items = st.take(session.sequence.as_deref().unwrap_or_default());
                        if replace && st.snap.active {
                            me.player.replace_upcoming(items, FEEDER);
                        } else {
                            me.player.play_items(items, 0, PlayContext::new("wave", WAVE_FROM), Some(FEEDER));
                        }
                        feedback = Some((st.session_id.clone(), st.batch()));
                        None
                    }
                    Ok(_) => Some("Волна не прислала треков — попробуйте другие настройки".to_owned()),
                    Err(e) => Some(format!("Не удалось запустить волну: {e}")),
                };
                if let Some(text) = error
                    && st.snap.error_text != text
                {
                    st.snap.error_text = text;
                    events.push(WaveEvent::ErrorChanged);
                }
                ((), events)
            });
            if let Some((session_id, batch_id)) = feedback {
                let event = RotorEvent { kind: "radioStarted".into(), ..Self::event_now() };
                me.send_feedback(&client, session_id, RotorFeedback { event, batch_id, from: Some(WAVE_FROM.into()) });
            }
        });
    }

    /// Плееру не хватает треков впереди — следующая партия.
    fn more(&self) {
        let Some(client) = self.auth.client() else { return };
        let Some(me) = self.me.upgrade() else { return };
        let (epoch, session_id, queue) = {
            let mut st = lock(&self.state);
            if st.fetching || st.session_id.is_empty() {
                return;
            }
            st.fetching = true;
            (st.epoch, st.session_id.clone(), st.recent.clone())
        };
        self.rt.spawn(async move {
            let result = client.rotor_session_tracks(&session_id, &queue).await;
            let renew = {
                let mut st = lock(&me.state);
                if st.epoch != epoch {
                    return;
                }
                st.fetching = false;
                match result {
                    Ok(batch)
                        if !batch.unknown_session.unwrap_or(false)
                            && batch.sequence.as_ref().is_some_and(|s| !s.is_empty()) =>
                    {
                        if let Some(id) = batch.batch_id.filter(|b| !b.is_empty()) {
                            st.batch_id = id;
                        }
                        if me.owns_feeder() {
                            let items = st.take(batch.sequence.as_deref().unwrap_or_default());
                            me.player.append_tracks(items, FEEDER);
                        }
                        false
                    }
                    // Сессия протухла (долгая пауза) — новая с теми же настройками, в конец очереди
                    Ok(_) => {
                        st.session_id.clear();
                        true
                    }
                    Err(e) => {
                        tracing::warn!("волна: следующая партия не загрузилась: {e}");
                        false
                    }
                }
            };
            if renew {
                me.renew(client);
            }
        });
    }

    fn renew(&self, client: YmClient) {
        let Some(me) = self.me.upgrade() else { return };
        let (epoch, body) = {
            let st = lock(&self.state);
            let body = RotorSessionNew {
                seeds: session_seeds(&st.snap.station, &st.snap.selection),
                queue: Some(st.recent.clone()),
                include_tracks_in_response: true,
            };
            (st.epoch, body)
        };
        self.rt.spawn(async move {
            match client.rotor_session_new(&body).await {
                Ok(session) => {
                    let mut st = lock(&me.state);
                    if st.epoch != epoch {
                        return;
                    }
                    st.session_id = session.radio_session_id.clone().unwrap_or_default();
                    st.batch_id = session.batch_id.clone().unwrap_or_default();
                    if me.owns_feeder() {
                        let items = st.take(session.sequence.as_deref().unwrap_or_default());
                        me.player.append_tracks(items, FEEDER);
                    }
                }
                Err(e) => tracing::warn!("волна: новая сессия не создалась: {e}"),
            }
        });
    }

    fn owns_feeder(&self) -> bool {
        self.player.snapshot().feeder == Some(FEEDER)
    }

    // --- обратная связь --------------------------------------------------

    fn event_now() -> RotorEvent {
        RotorEvent { timestamp: ym_api::client::utc_now_iso(), ..RotorEvent::default() }
    }

    /// Сессия для обратной связи: клиент есть, сессия есть.
    fn feedback_session(&self) -> Option<(YmClient, String, Option<String>, bool)> {
        let client = self.auth.client()?;
        let st = lock(&self.state);
        if st.session_id.is_empty() {
            return None;
        }
        Some((client, st.session_id.clone(), st.batch(), st.snap.active))
    }

    fn on_track_started(&self, event: &TrackStarted) {
        if event.context.kind != "wave" {
            return;
        }
        let Some((client, session_id, batch_id, _)) = self.feedback_session() else { return };
        let event = RotorEvent { kind: "trackStarted".into(), track_id: Some(event.track.full_id()), ..Self::event_now() };
        self.send_feedback(&client, session_id, RotorFeedback { event, batch_id, from: None });
    }

    fn on_track_ended(&self, event: &TrackEnded) {
        if event.context.kind != "wave" {
            return;
        }
        let Some((client, session_id, batch_id, _)) = self.feedback_session() else { return };
        let kind = if event.natural { "trackFinished" } else { "skip" };
        let event = RotorEvent {
            kind: kind.into(),
            track_id: Some(event.track.full_id()),
            total_played_seconds: Some(event.played),
            ..Self::event_now()
        };
        self.send_feedback(&client, session_id, RotorFeedback { event, batch_id, from: None });
    }

    /// Лайк/дизлайк во время волны — подсказка рекомендациям (коллекцию меняет Library).
    fn on_like_feedback(&self, kind: FeedbackKind, track: &TrackItem) {
        let Some((client, session_id, batch_id, active)) = self.feedback_session() else { return };
        if !active {
            return;
        }
        let player = self.player.snapshot();
        let current = player.track.as_ref().is_some_and(|t| t.track_id == track.track_id);
        let played = if current { player.played_seconds } else { 0.0 };
        let event = RotorEvent {
            kind: kind.as_str().into(),
            track_id: Some(track.full_id()),
            total_played_seconds: (kind == FeedbackKind::Dislike).then(|| (played * 10.0).round() / 10.0),
            ..Self::event_now()
        };
        self.send_feedback(&client, session_id, RotorFeedback { event, batch_id, from: None });
    }

    fn send_feedback(&self, client: &YmClient, session_id: String, feedback: RotorFeedback) {
        if self.no_report {
            tracing::warn!("волна: обратная связь не отправлена (YAMUSIC_NO_REPORT): {}", feedback.event.kind);
            return;
        }
        let client = client.clone();
        self.rt.spawn(async move {
            if let Err(e) = client.rotor_session_feedback(&session_id, &feedback).await {
                tracing::warn!("волна: обратная связь не отправилась: {e}");
            }
        });
    }

    // --- служебное -------------------------------------------------------

    fn load_settings(&self) {
        let Some(client) = self.auth.client() else { return };
        let Some(me) = self.me.upgrade() else { return };
        self.rt.spawn(async move {
            match client.rotor_wave_settings(WAVE_SEED).await {
                Ok(value) if !value.is_null() => {
                    let groups = groups_from_api(&value);
                    me.update(|st| {
                        st.snap.groups = Arc::new(groups);
                        ((), vec![WaveEvent::GroupsChanged])
                    });
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("волна: настройки не загрузились: {e}"),
            }
        });
    }

    fn update_active(&self) {
        let active = self.player.snapshot().source() == "wave";
        self.update(|st| {
            if st.snap.active == active {
                return ((), Vec::new());
            }
            st.snap.active = active;
            if !active {
                // ответы старой сессии больше не нужны
                st.epoch += 1;
                st.fetching = false;
            }
            ((), vec![WaveEvent::StateChanged])
        });
    }

    fn set_station(&self, seed: &str, title: &str) {
        self.update(|st| {
            if st.snap.station == seed && st.snap.station_title == title {
                return ((), Vec::new());
            }
            st.snap.station = seed.to_owned();
            st.snap.station_title = title.to_owned();
            ((), vec![WaveEvent::StateChanged])
        });
    }

    fn set_error(&self, text: &str) {
        self.update(|st| {
            if st.snap.error_text == text {
                return ((), Vec::new());
            }
            st.snap.error_text = text.to_owned();
            ((), vec![WaveEvent::ErrorChanged])
        });
    }

    /// Изменить состояние; при событиях снимок обновляется до их рассылки.
    fn update<R>(&self, f: impl FnOnce(&mut Session) -> (R, Vec<WaveEvent>)) -> R {
        let mut st = lock(&self.state);
        let (result, events) = f(&mut st);
        if !events.is_empty() {
            self.snapshot.send_replace(st.snap.clone());
            for event in events {
                let _ = self.events.send(event);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fallback_shape() {
        let groups = fallback_groups();
        let keys: Vec<_> = groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, ["context", "diversity", "moodEnergy", "language"]);
        assert_eq!(groups[0].items.len(), 7);
        assert_eq!(groups[2].items[1], WaveItem { label: "Весёлое".into(), seed: "settingMoodEnergy:fun".into() });
        assert_eq!(
            serde_json::to_value(&groups[3]).unwrap()["items"][2],
            json!({"label": "Без слов", "seed": "settingLanguage:without-words"})
        );
    }

    #[test]
    fn groups_from_settings() {
        let ws = json!({
            "blocks": [
                {"type": "contexts", "items": [
                    {"id": {"type": "activity", "tag": "workout"}, "name": "Спорт"},
                    {"station": {"id": {"type": "genre", "tag": "rock"}, "name": "Рок"}},
                    {"id": {"type": "x", "tag": "y"}, "name": ""}
                ]},
                {"type": "other", "items": [{"id": {"type": "a", "tag": "b"}, "name": "Нет"}]}
            ],
            "settingRestrictions": {
                "diversity": {"type": "enum", "name": "d", "possibleValues": [
                    {"value": "default", "name": "Любое", "serializedSeed": "settingDiversity:default", "unspecified": true},
                    {"value": "favorite", "name": "Любимое", "serializedSeed": "settingDiversity:favorite"},
                    {"value": "x", "name": "Без сида"}
                ]},
                "language": {"type": "enum", "name": "l", "possibleValues": []}
            }
        });
        let groups = groups_from_api(&ws);
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0].title, "Занятие");
        assert_eq!(
            groups[0].items,
            [
                WaveItem { label: "Спорт".into(), seed: "activity:workout".into() },
                WaveItem { label: "Рок".into(), seed: "genre:rock".into() }
            ]
        );
        assert_eq!(groups[1].items, [WaveItem { label: "Любимое".into(), seed: "settingDiversity:favorite".into() }]);
        // недостающее — из запасного списка
        assert_eq!(groups[2], fallback_groups()[2]);
        assert_eq!(groups[3], fallback_groups()[3]);
        assert_eq!(groups_from_api(&json!({})), fallback_groups());
    }

    #[test]
    fn selection_and_seeds() {
        let groups = fallback_groups();
        let selection = selection_from_seeds(&groups, "settingDiversity:discover,,unknown:1,activity:workout");
        assert_eq!(selection.len(), 2);
        assert_eq!(selection["diversity"], "settingDiversity:discover");
        assert_eq!(selection["context"], "activity:workout");
        assert_eq!(session_seeds("", &selection), ["activity:workout", "settingDiversity:discover"]);
        assert_eq!(session_seeds("track:5", &selection), ["track:5"]);
        assert_eq!(session_seeds("", &BTreeMap::new()), [WAVE_SEED]);
    }

    #[test]
    fn recent_is_bounded() {
        let mut session = Session { recent: (0..RECENT).map(|i| i.to_string()).collect(), ..Session::default() };
        session.take(&[]);
        assert_eq!(session.recent.len(), RECENT);
        assert!(session.batch().is_none());
        session.batch_id = "b".into();
        assert_eq!(session.batch().as_deref(), Some("b"));
    }
}
