"""Дисковый кэш картинок для QML (обложки грузятся по https через Image { source })."""

from __future__ import annotations

from PySide6.QtCore import QObject, QStandardPaths
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkDiskCache, QNetworkRequest
from PySide6.QtQml import QQmlNetworkAccessManagerFactory

CACHE_BYTES = 512 * 1024 * 1024


def cache_dir(sub: str) -> str:
    base = QStandardPaths.writableLocation(QStandardPaths.StandardLocation.CacheLocation)
    return f"{base}/{sub}"


class _Http1Nam(QNetworkAccessManager):
    """Только HTTP/1.1: по HTTP/2 сервер обложек Яндекса отвечает REFUSED_STREAM, когда страница
    разом просит десятки картинок («Server refused a stream»), и часть обложек не появляется."""

    def createRequest(self, op, request, data=None):
        request = QNetworkRequest(request)
        request.setAttribute(QNetworkRequest.Attribute.Http2AllowedAttribute, False)
        return super().createRequest(op, request, data)


class CachingNamFactory(QQmlNetworkAccessManagerFactory):
    """QML создаёт NAM в своих потоках загрузки — у каждого свой QNetworkDiskCache на общий каталог
    (QNetworkDiskCache безопасен для нескольких экземпляров на одном каталоге)."""

    def __init__(self) -> None:
        super().__init__()
        self._dir = cache_dir("images")

    def create(self, parent: QObject) -> QNetworkAccessManager:
        nam = _Http1Nam(parent)
        disk = QNetworkDiskCache(nam)
        disk.setCacheDirectory(self._dir)
        disk.setMaximumCacheSize(CACHE_BYTES)
        nam.setCache(disk)
        return nam
