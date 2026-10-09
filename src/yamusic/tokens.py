"""Хранение OAuth-токена.

Основной вариант — Secret Service (gnome-keyring / KWallet), но только если у пользователя есть
коллекция по умолчанию: иначе keyring пытается создать её через системный диалог при каждом запуске.
Без неё — файл token в каталоге данных приложения (~/.local/share/yamusic/YaMusic) с правами 0600 (как ключи SSH без пароля).
"""

from __future__ import annotations

import os
from pathlib import Path

import secretstorage
from PySide6.QtCore import QStandardPaths
from secretstorage.exceptions import SecretStorageException

_LABEL = "YaMusic: токен Яндекса"
_ATTRS = {"application": "yamusic", "account": "yandex-oauth-token"}


def _file() -> Path:
    base = QStandardPaths.writableLocation(QStandardPaths.StandardLocation.AppDataLocation)
    return Path(base) / "token"


def _collection():
    """Коллекция по умолчанию или None (без попыток её создать)."""
    try:
        bus = secretstorage.dbus_init()
        collection = secretstorage.collection.get_collection_by_alias(bus, "default")
        if collection.is_locked():
            collection.unlock()  # обычный запрос пароля связки ключей, если она заперта
        return None if collection.is_locked() else collection
    except (SecretStorageException, OSError):
        return None


def backend() -> str:
    """'keyring' | 'file' — для подсказки в настройках."""
    return "keyring" if _collection() is not None else "file"


def load() -> str | None:
    collection = _collection()
    if collection is not None:
        for item in collection.search_items(_ATTRS):
            return item.get_secret().decode()
    path = _file()
    if path.exists():
        return path.read_text().strip() or None
    return None


def save(token: str) -> str:
    collection = _collection()
    if collection is not None:
        collection.create_item(_LABEL, _ATTRS, token.encode(), replace=True)
        _file().unlink(missing_ok=True)
        return "keyring"
    path = _file()
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(token)
    os.chmod(path, 0o600)
    return "file"


def clear() -> None:
    collection = _collection()
    if collection is not None:
        for item in collection.search_items(_ATTRS):
            item.delete()
    _file().unlink(missing_ok=True)
