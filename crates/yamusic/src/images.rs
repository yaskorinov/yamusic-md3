//! Дисковый кэш картинок для QML (images.py): QQmlNetworkAccessManagerFactory в C++ (cpp/ym_shell.cpp),
//! каждый NAM — свой QNetworkDiskCache на общий каталог dirs::cache_subdir("images"), 512 МиБ;
//! только HTTP/1.1 (Http2AllowedAttribute = false: по HTTP/2 сервер обложек отвечает REFUSED_STREAM).
use cxx_qt_lib::{QQmlApplicationEngine, QString};
use std::pin::Pin;

pub const CACHE_BYTES: i64 = 512 * 1024 * 1024;

/// Установить фабрику на движок (до load).
pub fn install_caching_nam(engine: Pin<&mut QQmlApplicationEngine>) {
    let dir = ym_platform::dirs::cache_subdir("images");
    let dir = QString::from(dir.to_string_lossy().as_ref());
    crate::qt::ffi::ffi::engine_install_caching_nam(engine, &dir, CACHE_BYTES);
}
