//! Ссылки на треки для плеера (Player._resolve): tracks_file_info(track_id, quality); если качество не hq и запрос
//! упал — повтор с "hq". Нет клиента → Err("Войдите в аккаунт, чтобы слушать музыку").
use std::sync::Arc;

use futures::FutureExt;
use futures::future::BoxFuture;
use ym_api::models::DownloadInfo;
use ym_player::{Quality, StreamInfo, StreamResolver};

use crate::auth::AuthService;

pub const NOT_SIGNED_IN: &str = "Войдите в аккаунт, чтобы слушать музыку";

pub struct ApiResolver {
    auth: Arc<AuthService>,
}

impl ApiResolver {
    pub fn new(auth: Arc<AuthService>) -> Arc<Self> {
        Arc::new(Self { auth })
    }
}

fn stream_info(info: DownloadInfo) -> Result<StreamInfo, String> {
    let url = info.best_url().filter(|u| !u.is_empty()).ok_or("нет ссылки на файл")?.to_owned();
    Ok(StreamInfo { url, codec: info.codec, bitrate: info.bitrate })
}

impl StreamResolver for ApiResolver {
    fn resolve(&self, track_id: String, quality: Quality) -> BoxFuture<'static, Result<StreamInfo, String>> {
        let client = self.auth.client();
        async move {
            let client = client.ok_or_else(|| NOT_SIGNED_IN.to_owned())?;
            let quality = quality.api_value();
            let info = match client.tracks_file_info(&track_id, quality).await {
                Ok(info) => info,
                Err(e) if quality == "hq" => return Err(e.to_string()),
                // Lossless/низкое качество недоступно или API изменился — откат на AAC/MP3
                Err(_) => client.tracks_file_info(&track_id, "hq").await.map_err(|e| e.to_string())?,
            };
            stream_info(info)
        }
        .boxed()
    }
}
