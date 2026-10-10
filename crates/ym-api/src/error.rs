use thiserror::Error;

pub type Result<T, E = ApiError> = std::result::Result<T, E>;

/// Ошибки API. Сопоставление со статусами — как в yandex_music/utils/request_base.py.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Нет связи, таймаут, обрыв (reqwest) — «Нет связи с Яндексом…» в UI
    #[error("нет связи: {0}")]
    Network(String),
    /// 401/403 — токен недействителен или отозван
    #[error("{0}")]
    Unauthorized(String),
    /// 400; `code` — имя ошибки ('authorization_pending', 'expired_token'…)
    #[error("{message}")]
    BadRequest { code: String, message: String },
    /// 404
    #[error("{0}")]
    NotFound(String),
    /// Любой другой не-2xx (в т.ч. 409/412 — устаревшая ревизия плейлиста, 429/5xx после повторов)
    #[error("{message} ({status})")]
    Http { status: u16, code: String, message: String },
    /// Ответ не разобрался
    #[error("ошибка разбора ответа: {0}")]
    Decode(String),
    /// Методу нужен вход в аккаунт (нет токена)
    #[error("Войдите в аккаунт")]
    NoToken,
}

impl ApiError {
    /// Сетевая ошибка в смысле Python-версии (NetworkError): связь, 409/413/502, прочие коды.
    pub fn is_network(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Http { status, .. } => !matches!(status, 400 | 401 | 403 | 404),
            _ => false,
        }
    }

    /// HTTP-статус ответа; у `Unauthorized` — 401 (403 не различается).
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Unauthorized(_) => Some(401),
            Self::BadRequest { .. } => Some(400),
            Self::NotFound(_) => Some(404),
            Self::Http { status, .. } => Some(*status),
            Self::Network(_) | Self::Decode(_) | Self::NoToken => None,
        }
    }

    /// Имя ошибки из ответа ('authorization_pending', 'wrong-revision'…), если есть.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::BadRequest { code, .. } | Self::Http { code, .. } if !code.is_empty() => Some(code),
            _ => None,
        }
    }

    /// Устаревшая ревизия плейлиста: 412/409 или 400 с wrong-revision.
    pub fn is_wrong_revision(&self) -> bool {
        match self {
            Self::Http { status: 409 | 412, .. } => true,
            Self::BadRequest { code, message } => {
                code.contains("revision") || message.to_ascii_lowercase().contains("revision")
            }
            Self::Http { code, .. } => code.contains("revision"),
            _ => false,
        }
    }

    /// Ошибка по статусу и телу ответа: у OAuth error — строка (+ error_description),
    /// у API музыки — объект {name, message}.
    pub(crate) fn from_response(status: u16, body: &str) -> Self {
        let (code, text) = error_fields(body);
        let message = if !text.is_empty() {
            text
        } else if !code.is_empty() {
            code.clone()
        } else {
            format!("HTTP {status}")
        };
        match status {
            401 | 403 => Self::Unauthorized(message),
            400 => Self::BadRequest { code, message },
            404 => Self::NotFound(message),
            _ => Self::Http { status, code, message },
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_decode() {
            Self::Decode(error.to_string())
        } else {
            Self::Network(error.to_string())
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::Decode(error.to_string())
    }
}

fn error_fields(body: &str) -> (String, String) {
    let Ok(serde_json::Value::Object(root)) = serde_json::from_str::<serde_json::Value>(body) else {
        return (String::new(), String::new());
    };
    let text = |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    // ошибка бывает и на верхнем уровне, и внутри result
    let error = root
        .get("error")
        .or_else(|| root.get("result").and_then(|r| r.get("error")))
        .filter(|e| !e.is_null());
    match error {
        Some(serde_json::Value::String(code)) => {
            let description = text(root.get("error_description").or_else(|| root.get("errorDescription")));
            (code.clone(), description)
        }
        Some(serde_json::Value::Object(obj)) => (text(obj.get("name")), text(obj.get("message"))),
        _ => (String::new(), text(root.get("message"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_pending() {
        let e = ApiError::from_response(400, r#"{"error":"authorization_pending","error_description":"User has not yet authorized"}"#);
        match &e {
            ApiError::BadRequest { code, message } => {
                assert_eq!(code, "authorization_pending");
                assert_eq!(message, "User has not yet authorized");
            }
            other => panic!("{other:?}"),
        }
        assert!(!e.is_network());
        assert_eq!(e.status(), Some(400));
    }

    #[test]
    fn api_error_object() {
        let e = ApiError::from_response(
            401,
            r#"{"invocationInfo":{},"error":{"name":"session-expired","message":"Your OAuth token is expired"}}"#,
        );
        assert!(matches!(e, ApiError::Unauthorized(ref m) if m == "Your OAuth token is expired"));
        let e = ApiError::from_response(412, r#"{"error":{"name":"wrong-revision","message":"bad"}}"#);
        assert!(e.is_network());
        assert!(e.is_wrong_revision());
        assert_eq!(e.code(), Some("wrong-revision"));
        let e = ApiError::from_response(502, "<html>");
        assert!(matches!(e, ApiError::Http { status: 502, ref message, .. } if message == "HTTP 502"));
        assert!(e.is_network());
        assert!(matches!(ApiError::from_response(404, "{}"), ApiError::NotFound(_)));
    }
}
