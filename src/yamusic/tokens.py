"""Хранение OAuth-токена.

Linux: Secret Service (gnome-keyring / KWallet) → файл token в каталоге данных (0600).
Windows: Windows Credential Manager (через keyring) → файл token как запасной вариант.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

from PySide6.QtCore import QStandardPaths

_LABEL = "YaMusic: токен Яндекса"
_ATTRS = {"application": "yamusic", "account": "yandex-oauth-token"}


def _file() -> Path:
    base = QStandardPaths.writableLocation(QStandardPaths.StandardLocation.AppDataLocation)
    return Path(base) / "token"


if sys.platform == "win32":
    import keyring
    import keyring.errors

    _KR_SERVICE = "YaMusic"
    _KR_USER = "yandex-oauth-token"

    def backend() -> str:
        return "keyring"

    def load() -> str | None:
        try:
            secret = keyring.get_password(_KR_SERVICE, _KR_USER)
            if secret:
                return secret
        except Exception:
            pass
        path = _file()
        if path.exists():
            return path.read_text().strip() or None
        return None

    def save(token: str) -> str:
        try:
            keyring.set_password(_KR_SERVICE, _KR_USER, token)
            _file().unlink(missing_ok=True)
            return "keyring"
        except Exception:
            pass
        path = _file()
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(token)
        return "file"

    def clear() -> None:
        try:
            keyring.delete_password(_KR_SERVICE, _KR_USER)
        except Exception:
            pass
        _file().unlink(missing_ok=True)

else:
    import secretstorage
    from secretstorage.exceptions import SecretStorageException

    def _collection():
        """Коллекция по умолчанию или None (без попыток её создать)."""
        try:
            bus = secretstorage.dbus_init()
            collection = secretstorage.collection.get_collection_by_alias(bus, "default")
            if collection.is_locked():
                collection.unlock()
            return None if collection.is_locked() else collection
        except (SecretStorageException, OSError):
            return None

    def backend() -> str:
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
