//! Текст играющего трека (src/yamusic/lyrics.py): по PlayerEvent::TrackChanged с задержкой 350 мс
//! (пролистывание подряд не запускает поиск на каждый трек); тот же трек со статусом ok/loading/notfound — не ищется.
//! Запрос: title (+ " (version)"), artists, album, durationMs/1000; Яндекс — tracks_lyrics(LRC) + download_text.
//! Статус: ok | notfound (ok=false, details пуст) | error (details не пуст или паника/ошибка) | idle (нет трека) | loading.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use ym_api::items::TrackItem;
use ym_api::{LyricsFormat, YmClient};
use ym_lyrics::{Line, LyricsFetcher, LyricsQuery, LyricsResult, YandexLrc};
use ym_player::{PlayerEvent, PlayerHandle};

use crate::auth::AuthService;
use crate::lock;

/// Задержка поиска после смены трека
pub const DEBOUNCE_MS: u64 = 350;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LyricsStatus {
    #[default]
    Idle,
    Loading,
    Ok,
    NotFound,
    Error,
}

impl LyricsStatus {
    /// "idle" | "loading" | "ok" | "notfound" | "error"
    pub fn as_str(self) -> &'static str {
        match self {
            LyricsStatus::Idle => "idle",
            LyricsStatus::Loading => "loading",
            LyricsStatus::Ok => "ok",
            LyricsStatus::NotFound => "notfound",
            LyricsStatus::Error => "error",
        }
    }

    /// Статус по результату fetch.
    pub fn of(result: &LyricsResult) -> Self {
        if result.ok {
            LyricsStatus::Ok
        } else if result.details.is_empty() {
            LyricsStatus::NotFound
        } else {
            LyricsStatus::Error
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LyricsSnapshot {
    /// трек, к которому относятся lines/status (отстаёт на время поиска)
    pub track_id: String,
    pub status: LyricsStatus,
    /// "NetEase" | "Musixmatch" | "LRCLIB" | "Яндекс" | ""
    pub source: String,
    /// "word" | "line" | ""
    pub kind: String,
    pub lines: Arc<Vec<Line>>,
}

/// Запрос для трека: title (+ " (version)"), artists, album, длительность в секундах.
pub fn query_for(track: &TrackItem) -> LyricsQuery {
    let mut title = track.title.clone();
    if !track.version.is_empty() {
        title.push_str(&format!(" ({})", track.version));
    }
    LyricsQuery {
        track_id: track.track_id.clone(),
        title,
        artist: track.artists.clone(),
        album: track.album.clone(),
        duration: track.duration_ms as f64 / 1000.0,
    }
}

/// LRC от Яндекса: 404 или нет ссылки — None.
fn yandex_lrc(client: YmClient, track_id: String) -> YandexLrc {
    Arc::new(move || {
        let (client, track_id) = (client.clone(), track_id.clone());
        async move {
            let info = client.tracks_lyrics(&track_id, LyricsFormat::Lrc).await.map_err(|e| e.to_string())?;
            let Some(url) = info.and_then(|i| i.download_url).filter(|u| !u.is_empty()) else {
                return Ok(None);
            };
            client.download_text(&url).await.map(Some).map_err(|e| e.to_string())
        }
        .boxed()
    })
}

pub struct LyricsService {
    snapshot: watch::Sender<LyricsSnapshot>,
    rt: Handle,
    auth: Arc<AuthService>,
    player: PlayerHandle,
    fetcher: LyricsFetcher,
    epoch: Mutex<u64>,
}

impl LyricsService {
    pub fn new(rt: Handle, auth: Arc<AuthService>, player: PlayerHandle, fetcher: LyricsFetcher) -> Arc<Self> {
        let mut events = player.subscribe().events;
        let service = Arc::new(LyricsService {
            snapshot: watch::Sender::new(LyricsSnapshot::default()),
            rt: rt.clone(),
            auth,
            player,
            fetcher,
            epoch: Mutex::new(0),
        });
        let weak = Arc::downgrade(&service);
        rt.spawn(async move {
            let mut deadline: Option<tokio::time::Instant> = None;
            loop {
                let at = deadline;
                let wait = async move {
                    match at {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    event = events.recv() => match event {
                        Ok(PlayerEvent::TrackChanged) | Err(broadcast::error::RecvError::Lagged(_)) => {
                            deadline = Some(tokio::time::Instant::now() + Duration::from_millis(DEBOUNCE_MS));
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Closed) => return,
                    },
                    () = wait => {
                        deadline = None;
                        let Some(me) = weak.upgrade() else { return };
                        me.load();
                    }
                }
            }
        });
        service
    }

    /// Мост Lyrics: одно Qt-событие changed() на каждое изменение снимка.
    pub fn watch(&self) -> watch::Receiver<LyricsSnapshot> {
        self.snapshot.subscribe()
    }

    pub fn snapshot(&self) -> LyricsSnapshot {
        self.snapshot.borrow().clone()
    }

    fn load(self: Arc<Self>) {
        let track = self.player.snapshot().track;
        let track_id = track.as_ref().map(|t| t.track_id.clone()).unwrap_or_default();
        {
            let current = self.snapshot.borrow();
            let settled = matches!(current.status, LyricsStatus::Ok | LyricsStatus::Loading | LyricsStatus::NotFound);
            if current.track_id == track_id && settled {
                return;
            }
        }
        let epoch = {
            let mut epoch = lock(&self.epoch);
            *epoch += 1;
            *epoch
        };
        let Some(track) = track.filter(|_| !track_id.is_empty()) else {
            self.set(track_id, LyricsStatus::Idle, None);
            return;
        };
        self.set(track_id.clone(), LyricsStatus::Loading, None);
        let query = query_for(&track);
        let yandex = self.auth.client().map(|c| yandex_lrc(c, track_id.clone()));
        let fetcher = self.fetcher.clone();
        let task = self.rt.spawn(async move { fetcher.fetch_cached(&query, yandex).await });
        let me = self.clone();
        self.rt.spawn(async move {
            let result = task.await;
            if *lock(&me.epoch) != epoch {
                return;
            }
            match result {
                Ok(result) => {
                    let status = LyricsStatus::of(&result);
                    tracing::info!("текст {track_id}: {} {}", if result.source.is_empty() { "—" } else { &result.source }, result.kind);
                    me.set(track_id, status, (status == LyricsStatus::Ok).then_some(result));
                }
                Err(e) => {
                    tracing::warn!("текст не загрузился: {e}");
                    me.set(track_id, LyricsStatus::Error, None);
                }
            }
        });
    }

    fn set(&self, track_id: String, status: LyricsStatus, result: Option<LyricsResult>) {
        let result = result.unwrap_or_default();
        self.snapshot.send_replace(LyricsSnapshot {
            track_id,
            status,
            source: result.source,
            kind: result.kind,
            lines: Arc::new(result.lines),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses() {
        assert_eq!(LyricsStatus::NotFound.as_str(), "notfound");
        assert_eq!(LyricsStatus::of(&LyricsResult { ok: true, ..Default::default() }), LyricsStatus::Ok);
        assert_eq!(LyricsStatus::of(&LyricsResult::default()), LyricsStatus::NotFound);
        let failed = LyricsResult { details: vec!["NetEase: 503".into()], ..Default::default() };
        assert_eq!(LyricsStatus::of(&failed), LyricsStatus::Error);
    }

    #[test]
    fn query_title_with_version() {
        let track = TrackItem {
            track_id: "1".into(),
            title: "Song".into(),
            version: "Live".into(),
            artists: "A, B".into(),
            album: "Al".into(),
            duration_ms: 90_500,
            ..Default::default()
        };
        let q = query_for(&track);
        assert_eq!(q.title, "Song (Live)");
        assert_eq!((q.artist.as_str(), q.album.as_str(), q.duration), ("A, B", "Al", 90.5));
        let q = query_for(&TrackItem { version: String::new(), ..track });
        assert_eq!(q.title, "Song");
    }
}
