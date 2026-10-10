//! Учёт прослушиваний /play-audio (src/yamusic/history.py): отметка в начале трека (0 с, новый play-id uuid4)
//! и в конце, если проиграно ≥ 1 с (played, end). from = context.from || DEFAULT_FROM; playlist-id = context.playlistId;
//! длина = round(durationMs/1000). no_report — только tracing::warn!. Ошибки — warn, без повторов.
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::sync::broadcast;
use ym_api::PlayAudio;
use ym_api::items::contexts::DEFAULT_FROM;
use ym_api::items::{PlayContext, TrackItem};
use ym_player::{PlayerEvent, PlayerHandle};

use crate::auth::AuthService;

/// Форма play-audio для трека; uid — аккаунта.
pub fn play_audio(track: &TrackItem, context: &PlayContext, play_id: &str, uid: i64, played: f64, end: f64) -> PlayAudio {
    PlayAudio {
        track_id: track.track_id.clone(),
        from: if context.from.is_empty() { DEFAULT_FROM.to_owned() } else { context.from.clone() },
        album_id: track.album_id.clone(),
        playlist_id: context.playlist_id.clone().filter(|p| !p.is_empty()),
        play_id: play_id.to_owned(),
        uid,
        // round() в Python — банковское округление
        track_length_seconds: (track.duration_ms as f64 / 1000.0).round_ties_even() as i64,
        total_played_seconds: played,
        end_position_seconds: end,
    }
}

struct Reporter {
    rt: Handle,
    auth: Arc<AuthService>,
    no_report: bool,
    play_id: String,
}

impl Reporter {
    fn send(&self, track: &TrackItem, context: &PlayContext, played: f64, end: f64) {
        let Some((client, account)) = self.auth.session() else { return };
        if track.track_id.is_empty() {
            return;
        }
        if self.no_report {
            tracing::warn!("play-audio {}: {played:.1} с, конец {end:.1} с", track.track_id);
            return;
        }
        let report = play_audio(track, context, &self.play_id, account.uid, played, end);
        self.rt.spawn(async move {
            if let Err(e) = client.play_audio(&report).await {
                tracing::warn!("учёт прослушиваний: {e}");
            }
        });
    }
}

/// Запустить фоновую задачу; живёт до конца рантайма.
pub fn spawn(rt: &Handle, auth: Arc<AuthService>, player: PlayerHandle, no_report: bool) {
    let mut events = player.subscribe().events;
    let mut reporter = Reporter { rt: rt.clone(), auth, no_report, play_id: String::new() };
    rt.spawn(async move {
        loop {
            match events.recv().await {
                Ok(PlayerEvent::TrackStarted(ev)) => {
                    reporter.play_id = uuid::Uuid::new_v4().to_string();
                    reporter.send(&ev.track, &ev.context, 0.0, 0.0);
                }
                Ok(PlayerEvent::TrackEnded(ev)) if ev.played >= 1.0 => {
                    reporter.send(&ev.track, &ev.context, ev.played, ev.end);
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_fields() {
        let track = TrackItem { track_id: "10".into(), album_id: "20".into(), duration_ms: 182_500, ..Default::default() };
        let context = PlayContext { kind: "playlist".into(), from: String::new(), playlist_id: Some("1:3".into()) };
        let form = play_audio(&track, &context, "pid", 5, 12.5, 13.0);
        assert_eq!(form.from, DEFAULT_FROM);
        assert_eq!(form.album_id, "20");
        assert_eq!(form.playlist_id.as_deref(), Some("1:3"));
        assert_eq!(form.track_length_seconds, 182);
        assert_eq!(form.uid, 5);
        assert_eq!((form.total_played_seconds, form.end_position_seconds), (12.5, 13.0));
        let track = TrackItem { duration_ms: 183_500, ..track };
        let context = PlayContext::new("wave", "desktop_win-radio");
        let form = play_audio(&track, &context, "pid", 5, 0.0, 0.0);
        assert_eq!(form.track_length_seconds, 184);
        assert_eq!(form.from, "desktop_win-radio");
        assert!(form.playlist_id.is_none());
    }
}
