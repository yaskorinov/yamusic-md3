//! Связки, которые в Python делались в app.py / bind_settings:
//! - настройки → плеер: Volume → set_volume (если отличается от снимка), Crossfade → set_crossfade, Quality → set_quality;
//! - MPRIS (Linux): снимок плеера → MprisState (art_url = file://<coverFile> или cover), MprisCommand → плеер,
//!   Raise/Quit → AppRequest. SeekTo — в мс (/1000), SetVolume — clamp 0..1 и запись Settings.volume.
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc, watch};
use ym_platform::mpris::{MprisCommand, MprisServer, MprisState};
use ym_platform::{SettingKey, SettingValue, SettingsStore};
use ym_player::{PlayerHandle, PlayerSnapshot, Quality, Repeat};

use crate::AppRequest;

fn crossfade(settings: &SettingsStore) -> u32 {
    settings.get_int(SettingKey::Crossfade).clamp(0, i64::from(u32::MAX)) as u32
}

fn apply_setting(settings: &SettingsStore, player: &PlayerHandle, key: SettingKey) {
    match key {
        SettingKey::Volume => {
            let volume = settings.get_f64(SettingKey::Volume).clamp(0.0, 1.0);
            if (player.snapshot().volume - volume).abs() > 1e-6 {
                player.set_volume(volume);
            }
        }
        SettingKey::Crossfade => player.set_crossfade(crossfade(settings)),
        SettingKey::Quality => player.set_quality(Quality::from_setting(&settings.get_str(SettingKey::Quality))),
        _ => {}
    }
}

pub fn spawn_settings_glue(rt: &Handle, settings: Arc<SettingsStore>, player: PlayerHandle) {
    let mut changes = settings.subscribe();
    rt.spawn(async move {
        loop {
            match changes.recv().await {
                Ok(key) => apply_setting(&settings, &player, key),
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    for key in [SettingKey::Volume, SettingKey::Crossfade, SettingKey::Quality] {
                        apply_setting(&settings, &player, key);
                    }
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// Состояние MPRIS по снимку плеера.
pub fn mpris_state(s: &PlayerSnapshot) -> MprisState {
    let track = s.track.clone().unwrap_or_default();
    let art_url = if s.cover_file.is_empty() { track.cover.clone() } else { format!("file://{}", s.cover_file) };
    MprisState {
        has_track: s.has_track(),
        playing: s.playing,
        track_id: track.track_id,
        title: track.title,
        artist: track.artists,
        album: track.album,
        art_url,
        duration_us: s.duration_ms * 1000,
        position_us: s.position_ms * 1000,
        shuffle: s.shuffle,
        repeat: match s.repeat {
            Repeat::Off => "off",
            Repeat::All => "all",
            Repeat::One => "one",
        }
        .to_owned(),
        volume: s.volume,
    }
}

fn handle_command(
    command: MprisCommand,
    settings: &SettingsStore,
    player: &PlayerHandle,
    requests: &broadcast::Sender<AppRequest>,
) {
    match command {
        MprisCommand::Raise => drop(requests.send(AppRequest::RaiseWindow)),
        MprisCommand::Quit => drop(requests.send(AppRequest::Quit)),
        MprisCommand::Next => player.next(),
        MprisCommand::Previous => player.previous(),
        MprisCommand::Pause | MprisCommand::Stop => player.pause(),
        MprisCommand::PlayPause => player.toggle_play(),
        MprisCommand::Play => player.play(),
        MprisCommand::SeekTo(us) => player.seek_ms(us.max(0) / 1000),
        MprisCommand::SetRepeat(value) => {
            if let Some(repeat) = Repeat::from_str(&value) {
                player.set_repeat(repeat);
            }
        }
        MprisCommand::SetShuffle(shuffle) => player.set_shuffle(shuffle),
        MprisCommand::SetVolume(volume) => {
            let volume = volume.clamp(0.0, 1.0);
            player.set_volume(volume);
            settings.set(SettingKey::Volume, SettingValue::Float(volume));
        }
    }
}

/// Остановка сервера MPRIS при выходе.
pub struct MprisStop {
    stop: watch::Sender<bool>,
    done: std::sync::mpsc::Receiver<()>,
}

impl MprisStop {
    /// Попросить сервер отключиться от шины и подождать не дольше `timeout`.
    pub fn stop(self, timeout: Duration) {
        let _ = self.stop.send(true);
        let _ = self.done.recv_timeout(timeout);
    }
}

/// Как [`spawn_mpris`], но с ручкой остановки (Services::shutdown).
pub fn start_mpris(
    rt: &Handle,
    settings: Arc<SettingsStore>,
    player: PlayerHandle,
    requests: broadcast::Sender<AppRequest>,
) -> MprisStop {
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let mut snapshot = player.subscribe().snapshot;
    rt.spawn(async move {
        let (state_tx, state_rx) = watch::channel(mpris_state(&snapshot.borrow_and_update()));
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let server = match MprisServer::start(state_rx, cmd_tx).await {
            Ok(server) => server,
            Err(e) => {
                tracing::warn!("MPRIS недоступен: {e}");
                return;
            }
        };
        loop {
            tokio::select! {
                _ = stop_rx.changed() => break,
                changed = snapshot.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let state = mpris_state(&snapshot.borrow_and_update());
                    state_tx.send_if_modified(|old| {
                        let changed = *old != state;
                        *old = state;
                        changed
                    });
                }
                command = cmd_rx.recv() => match command {
                    Some(command) => handle_command(command, &settings, &player, &requests),
                    None => break,
                },
            }
        }
        server.close().await;
        let _ = done_tx.send(());
    });
    MprisStop { stop: stop_tx, done: done_rx }
}

/// Ошибка подключения к шине — warn «MPRIS недоступен: <e>», приложение работает дальше.
pub fn spawn_mpris(rt: &Handle, settings: Arc<SettingsStore>, player: PlayerHandle, requests: broadcast::Sender<AppRequest>) {
    // ручка не нужна: сервер живёт до конца рантайма
    std::mem::forget(start_mpris(rt, settings, player, requests));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ym_api::items::TrackItem;

    #[test]
    fn mpris_from_snapshot() {
        let mut s = PlayerSnapshot {
            track: Some(TrackItem {
                track_id: "1".into(),
                title: "T".into(),
                artists: "A, B".into(),
                album: "Al".into(),
                cover: "https://c/400x400".into(),
                ..Default::default()
            }),
            playing: true,
            duration_ms: 2000,
            position_ms: 500,
            volume: 0.5,
            ..Default::default()
        };
        let state = mpris_state(&s);
        assert!(state.has_track && state.playing);
        assert_eq!(state.art_url, "https://c/400x400");
        assert_eq!((state.duration_us, state.position_us), (2_000_000, 500_000));
        assert_eq!(state.artist, "A, B");
        s.cover_file = "/tmp/c.jpg".into();
        assert_eq!(mpris_state(&s).art_url, "file:///tmp/c.jpg");
        assert_eq!(mpris_state(&PlayerSnapshot::default()), MprisState { repeat: "off".into(), ..Default::default() });
    }
}
