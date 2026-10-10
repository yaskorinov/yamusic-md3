//! Живая проверка с настоящей libmpv (ao=null, источники lavfi). Без libmpv тесты пропускаются.
//! Путь: $YAMUSIC_LIBMPV или libmpv-2.dll / libmpv.so.2 из корня репозитория / системный поиск.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::sync::broadcast;
use ym_api::items::{PlayContext, TrackItem};
use ym_player::{FeederId, PlayerConfig, PlayerEvent, PlayerHandle, Quality, Repeat, StreamInfo, StreamResolver};

/// trackId "sine-<секунды>" → синус нужной длины; "bad" → ошибка ссылки; "broken" → несуществующий файл.
struct LavfiResolver;

impl StreamResolver for LavfiResolver {
    fn resolve(&self, track_id: String, _quality: Quality) -> BoxFuture<'static, Result<StreamInfo, String>> {
        Box::pin(async move {
            if track_id == "bad" {
                return Err("нет ссылки".into());
            }
            if track_id == "broken" {
                return Ok(StreamInfo { url: "file:///nonexistent/ym-player.flac".into(), codec: "flac".into(), bitrate: None });
            }
            let secs: f64 = track_id.trim_start_matches("sine-").split('#').next().unwrap().parse().unwrap();
            Ok(StreamInfo {
                url: wav(secs).to_string_lossy().into_owned(),
                codec: "mp3".into(),
                bitrate: Some(320),
            })
        })
    }
}

/// Синус 440 Гц, 8 кГц моно 16 бит — перематываемый файл (lavfi не перематывается).
fn wav(secs: f64) -> PathBuf {
    let dir = std::env::temp_dir().join("ym-player-test-wav");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("sine-{secs}.wav"));
    if path.is_file() {
        return path;
    }
    let rate = 8000u32;
    let samples = (secs * f64::from(rate)) as u32;
    let mut data = Vec::with_capacity(44 + samples as usize * 2);
    let mut put = |b: &[u8]| data.extend_from_slice(b);
    put(b"RIFF");
    put(&(36 + samples * 2).to_le_bytes());
    put(b"WAVEfmt ");
    put(&16u32.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&rate.to_le_bytes());
    put(&(rate * 2).to_le_bytes());
    put(&2u16.to_le_bytes());
    put(&16u16.to_le_bytes());
    put(b"data");
    put(&(samples * 2).to_le_bytes());
    for i in 0..samples {
        let v = (f64::from(i) * 440.0 * std::f64::consts::TAU / f64::from(rate)).sin() * 8000.0;
        put(&(v as i16).to_le_bytes());
    }
    let tmp = path.with_extension(format!("{}.part", fastrand::u32(..)));
    std::fs::write(&tmp, data).unwrap();
    let _ = std::fs::rename(&tmp, &path);
    path
}

fn libmpv() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let explicit = ["libmpv-2.dll", "libmpv.so.2", "libmpv.2.dylib"].iter().map(|n| root.join(n)).find(|p| p.is_file());
    ym_player::mpv::load_library(explicit.as_deref()).ok().map(|lib| lib.path.clone())
}

fn track(id: &str, secs: i64) -> TrackItem {
    TrackItem { track_id: id.into(), title: id.into(), duration_ms: secs * 1000, available: true, ..Default::default() }
}

fn start(crossfade_s: u32, session_file: Option<PathBuf>) -> Option<PlayerHandle> {
    let Some(path) = libmpv() else {
        eprintln!("libmpv не найдена — пропуск");
        return None;
    };
    let config = PlayerConfig {
        cover_dir: std::env::temp_dir().join("ym-player-test-covers"),
        session_file,
        audio_output: Some("null".into()),
        libmpv_path: Some(path),
        volume: 0.5,
        crossfade_s,
        quality: Quality::High,
    };
    Some(PlayerHandle::start(&tokio::runtime::Handle::current(), config, Arc::new(LavfiResolver)).expect("start"))
}

async fn wait_for<T>(
    events: &mut broadcast::Receiver<PlayerEvent>,
    secs: u64,
    mut f: impl FnMut(&PlayerEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            match events.recv().await {
                Ok(ev) => {
                    if let Some(v) = f(&ev) {
                        return v;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(e) => panic!("{e}"),
            }
        }
    })
    .await
    .expect("событие не пришло")
}

fn started(id: &'static str) -> impl FnMut(&PlayerEvent) -> Option<()> {
    move |ev| matches!(ev, PlayerEvent::TrackStarted(s) if s.track.track_id == id).then_some(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gapless_queue_and_levels() {
    let Some(player) = start(0, None) else { return };
    let mut sub = player.subscribe();
    player.set_levels_wanted(true);
    let ctx = PlayContext::new("playlist", "test");
    player.play_items(vec![track("sine-4", 4), track("sine-3.5", 4)], 0, ctx.clone(), None);
    wait_for(&mut sub.events, 5, started("sine-4")).await;
    wait_for(&mut sub.events, 5, |ev| matches!(ev, PlayerEvent::StateChanged).then_some(())).await;
    let snap = player.snapshot();
    assert!(snap.playing, "{snap:?}");
    assert_eq!(snap.codec, "MP3 320");
    assert_eq!(snap.current_index, 0);
    // уровень «дыхания» обложки приходит, пока играет
    wait_for(&mut sub.events, 5, |ev| matches!(ev, PlayerEvent::LevelChanged).then_some(())).await;
    assert!(player.snapshot().level > 0.0);

    // gapless: mpv сам переходит на заранее поставленный следующий
    let ended = wait_for(&mut sub.events, 8, |ev| match ev {
        PlayerEvent::TrackEnded(e) => Some(e.clone()),
        _ => None,
    })
    .await;
    assert_eq!(ended.track.track_id, "sine-4");
    assert!(ended.natural, "{ended:?}");
    assert!(ended.played > 3.0, "{ended:?}");
    let snap = player.snapshot();
    assert_eq!(snap.current_index, 1);
    assert_eq!(snap.track.as_ref().unwrap().track_id, "sine-3.5");
    assert_eq!(snap.codec, "MP3 320");

    // конец очереди → trackEnded последнего, не играет
    let ended = wait_for(&mut sub.events, 8, |ev| match ev {
        PlayerEvent::TrackEnded(e) => Some(e.clone()),
        _ => None,
    })
    .await;
    assert_eq!(ended.track.track_id, "sine-3.5");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!player.snapshot().playing);

    player.set_levels_wanted(false);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(player.snapshot().level, 0.0);
    player.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crossfade_switches_instances() {
    let Some(player) = start(1, None) else { return };
    let mut sub = player.subscribe();
    player.play_items(vec![track("sine-4", 4), track("sine-4#b", 4)], 0, PlayContext::new("album", "t"), None);
    wait_for(&mut sub.events, 5, started("sine-4")).await;
    let t0 = std::time::Instant::now();
    let ended = wait_for(&mut sub.events, 8, |ev| match ev {
        PlayerEvent::TrackEnded(e) => Some(e.clone()),
        _ => None,
    })
    .await;
    // переход начинается за 1 с до конца
    assert!(ended.natural);
    assert!(ended.end >= 2.5 && ended.end <= 3.5, "{ended:?}");
    assert!(t0.elapsed() < Duration::from_millis(3800), "{:?}", t0.elapsed());
    assert_eq!(player.snapshot().current_index, 1);
    // второй (теперь главный) экземпляр играет и шлёт позицию
    wait_for(&mut sub.events, 3, |ev| matches!(ev, PlayerEvent::PositionChanged).then_some(())).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let snap = player.snapshot();
    assert!(snap.playing, "{snap:?}");
    assert!(snap.position_ms > 500, "{snap:?}");
    player.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn controls_errors_and_session() {
    let file = std::env::temp_dir().join(format!("ym-player-live-{}", fastrand::u64(..))).join("session.json");
    let Some(player) = start(0, Some(file.clone())) else { return };
    let mut sub = player.subscribe();
    let items = vec![track("bad", 10), track("sine-20", 20), track("broken", 5), track("sine-20#2", 20)];
    player.play_items(items.clone(), 0, PlayContext::new("wave", "radio"), Some(FeederId(1)));
    wait_for(&mut sub.events, 3, |ev| matches!(ev, PlayerEvent::ErrorChanged).then_some(())).await;
    assert_eq!(player.snapshot().error_text, "Не удалось получить трек: нет ссылки");

    player.next();
    wait_for(&mut sub.events, 3, started("sine-20")).await;
    // волна просит догрузку: впереди меньше FEED_AHEAD
    wait_for(&mut sub.events, 3, |ev| matches!(ev, PlayerEvent::NeedMore(FeederId(1))).then_some(())).await;
    assert_eq!(player.snapshot().error_text, "");
    wait_for(&mut sub.events, 5, |ev| matches!(ev, PlayerEvent::StateChanged).then_some(())).await;
    // перемотка — после открытия файла (первый опрос позиции)
    wait_for(&mut sub.events, 3, |ev| matches!(ev, PlayerEvent::PositionChanged).then_some(())).await;
    player.seek_ms(10_000);
    tokio::time::sleep(Duration::from_millis(600)).await;
    let pos = player.snapshot().position_ms;
    assert!((10_000..12_000).contains(&pos), "{pos}");

    // волна: repeat off ↔ one, shuffle не включается
    player.cycle_repeat();
    player.set_shuffle(true);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snap = player.snapshot();
    assert_eq!(snap.repeat, Repeat::One);
    assert!(!snap.shuffle);
    player.cycle_repeat();

    player.pause();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!player.snapshot().playing);

    // ошибка воспроизведения → пропуск на следующий
    player.play_index(2);
    wait_for(&mut sub.events, 3, started("broken")).await;
    wait_for(&mut sub.events, 5, started("sine-20#2")).await;
    assert_eq!(player.snapshot().error_text, "");

    // догрузка волны и замена хвоста
    player.append_tracks(vec![track("sine-5", 5)], FeederId(1));
    player.append_tracks(vec![track("sine-6", 6)], FeederId(2)); // чужая подкормка — игнор
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(player.snapshot().queue.len(), 5);
    player.replace_upcoming(vec![track("sine-7", 7), track("sine-8", 8)], FeederId(1));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let ids: Vec<_> = player.snapshot().queue.iter().map(|t| t.track_id.clone()).collect();
    assert_eq!(ids, ["bad", "sine-20", "broken", "sine-20#2", "sine-7", "sine-8"]);

    player.seek_ms(7_000);
    player.pause();
    tokio::time::sleep(Duration::from_millis(400)).await;
    player.shutdown();
    let saved = ym_player::session::load(&file).expect("session saved");
    assert_eq!(saved.items.len(), 6);
    assert_eq!(saved.pos, 3);
    assert_eq!(saved.context.kind, "restored");
    assert!((7_000..8_000).contains(&saved.position_ms), "{}", saved.position_ms);

    // восстановление: на паузе, с той же секунды
    let Some(player) = start(0, Some(file.clone())) else { return };
    let mut sub = player.subscribe();
    player.restore_session();
    wait_for(&mut sub.events, 3, |ev| matches!(ev, PlayerEvent::PositionChanged).then_some(())).await;
    let snap = player.snapshot();
    assert_eq!(snap.current_index, 3);
    assert!(!snap.playing);
    assert_eq!(snap.source(), "restored");
    assert!((7_000..8_000).contains(&snap.position_ms));
    player.toggle_play();
    wait_for(&mut sub.events, 3, started("sine-20#2")).await;
    wait_for(&mut sub.events, 5, |ev| matches!(ev, PlayerEvent::StateChanged).then_some(())).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    let pos = player.snapshot().position_ms;
    assert!((7_000..9_500).contains(&pos), "{pos}");
    player.shutdown();
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}
