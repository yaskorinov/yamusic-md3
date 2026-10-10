//! Подписи запросов (yandex_music/utils/sign_request.py, download_info.py).
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha2::Sha256;

/// Ключ подписи текста песни (Android-клиент)
pub const DEFAULT_SIGN_KEY: &str = "p93jhgh689SBReK6ghtw62";
/// Ключ подписи get-file-info (десктопный клиент Windows; парный заголовок — DESKTOP_CLIENT_HEADER)
pub const DESKTOP_SIGN_KEY: &str = "kzqU4XhfCaY6B6JTHODeq5";
/// Соль прямой ссылки старого /download-info
pub const DIRECT_LINK_SALT: &str = "XGRlBW9FXlekgbPrRHuSiA";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sign {
    /// Unix-время, секунды
    pub timestamp: i64,
    pub value: String,
}

/// get-file-info: base64(HMAC-SHA256(DESKTOP_SIGN_KEY, f"{ts}{track_id}{quality}{codecs.join('')}{transport}")) без последнего символа.
pub fn file_info_sign(track_id: &str, quality: &str, codecs: &[&str], transport: &str, timestamp: i64) -> Sign {
    let message = format!("{timestamp}{track_id}{quality}{}{transport}", codecs.concat());
    let mut value = hmac_base64(DESKTOP_SIGN_KEY, &message);
    value.pop();
    Sign { timestamp, value }
}

/// Текст песни: base64(HMAC-SHA256(DEFAULT_SIGN_KEY, f"{numeric_track_id}{ts}")) целиком;
/// numeric_track_id — часть до ':'.
pub fn lyrics_sign(track_id: &str, timestamp: i64) -> Sign {
    let numeric = numeric_track_id(track_id);
    Sign { timestamp, value: hmac_base64(DEFAULT_SIGN_KEY, &format!("{numeric}{timestamp}")) }
}

/// 'id:albumId' → 'id' (convert_track_id_to_number; ведущие нули и пробелы отбрасываются, как у int()).
pub fn numeric_track_id(track_id: &str) -> String {
    let head = track_id.split(':').next().unwrap_or_default().trim();
    head.parse::<u64>().map(|n| n.to_string()).unwrap_or_else(|_| head.to_owned())
}

/// Текущее Unix-время в секундах.
pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Прямая ссылка старого /download-info по XML {host, path, ts, s}:
/// https://{host}/get-mp3/{md5(SALT + path[1:] + s)}/{ts}{path}
pub fn direct_link(host: &str, path: &str, ts: &str, s: &str) -> String {
    let tail = path.get(1..).unwrap_or_default();
    let mut md5 = Md5::new();
    md5.update(format!("{DIRECT_LINK_SALT}{tail}{s}").as_bytes());
    let sign = hex::encode(md5.finalize());
    format!("https://{host}/get-mp3/{sign}/{ts}{path}")
}

fn hmac_base64(key: &str, message: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC принимает ключ любой длины");
    mac.update(message.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::FILE_INFO_CODECS;

    // эталоны посчитаны формулами yandex_music/utils/sign_request.py при ts = 1760000000
    #[test]
    fn file_info() {
        let sign = file_info_sign("12345678", "lossless", &FILE_INFO_CODECS, "raw", 1_760_000_000);
        assert_eq!(sign.timestamp, 1_760_000_000);
        assert_eq!(sign.value, "i5MftICBzwjRK+Kruk8XoKvITTFc9SV3whcWzG55r5w");
    }

    #[test]
    fn lyrics() {
        let expected = "31bcOJ7KyB1tDbO7fqEVBVVmmQSE5yGKHQ5ihhwtzXQ=";
        assert_eq!(lyrics_sign("12345678", 1_760_000_000).value, expected);
        assert_eq!(lyrics_sign("12345678:999", 1_760_000_000).value, expected);
    }

    #[test]
    fn numeric_id() {
        assert_eq!(numeric_track_id("42:7"), "42");
        assert_eq!(numeric_track_id("042"), "42");
        assert_eq!(numeric_track_id("abc"), "abc");
    }

    #[test]
    fn legacy_direct_link() {
        let link = direct_link("s123vla.storage.yandex.net", "/rmusic/U2FsdGVkX1/abc/def", "0005f1e2d3c4b5a6", "0123456789abcdef");
        assert_eq!(
            link,
            "https://s123vla.storage.yandex.net/get-mp3/531e032aa3ad30bf518f6dbdf32fedfb/0005f1e2d3c4b5a6/rmusic/U2FsdGVkX1/abc/def"
        );
    }
}
