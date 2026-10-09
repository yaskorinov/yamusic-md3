"""Вход в Яндекс: OAuth Device Flow (код на ya.ru/device) или токен вручную. Доступен в QML как Auth."""

from __future__ import annotations

import asyncio
import time

from PySide6.QtCore import Property, QObject, Signal, Slot
from PySide6.QtGui import QGuiApplication
from yandex_music import ClientAsync
from yandex_music.exceptions import DeviceAuthError, NetworkError, UnauthorizedError, YandexMusicError

from . import tokens
from .aio import AsyncRunner

DEVICE_NAME = "YaMusic (Linux)"


class Auth(QObject):
    """Состояния: checking → signedOut | signedIn; signedOut → requestingCode → awaitingUser → signingIn → signedIn.
    error — с текстом в errorText (кнопка «Повторить» в UI)."""

    stateChanged = Signal()
    codeChanged = Signal()
    accountChanged = Signal()
    errorChanged = Signal()
    storageChanged = Signal()
    signedIn = Signal()    # клиент готов (Library подписывается)
    signedOut = Signal()

    def __init__(self, runner: AsyncRunner, parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._client: ClientAsync | None = None
        self._state = "checking"
        self._error = ""
        self._user_code = ""
        self._verification_url = ""
        self._code_expires_at = 0.0
        self._account: dict[str, object] = {}
        self._poll = None
        self._storage = ""

    # --- публичное для Python-сервисов ----------------------------------

    @property
    def client(self) -> ClientAsync | None:
        return self._client

    def start(self) -> None:
        """Восстановить сессию из keyring при запуске."""
        self._set_state("checking")
        async def load() -> tuple[str | None, str]:
            return await asyncio.to_thread(tokens.load), await asyncio.to_thread(tokens.backend)

        self._runner.submit(load(), self._on_token_loaded, self._on_error)

    # --- свойства для QML -------------------------------------------------

    @Property(str, notify=stateChanged)
    def state(self) -> str:
        return self._state

    @Property(str, notify=errorChanged)
    def errorText(self) -> str:
        return self._error

    @Property(str, notify=codeChanged)
    def userCode(self) -> str:
        return self._user_code

    @Property(str, notify=codeChanged)
    def verificationUrl(self) -> str:
        return self._verification_url

    @Property(float, notify=codeChanged)
    def codeExpiresAt(self) -> float:
        """Unix-время (мс), когда код перестанет действовать."""
        return self._code_expires_at * 1000

    @Property(str, notify=storageChanged)
    def tokenStorage(self) -> str:
        """'keyring' | 'file' | '' — где лежит токен (подсказка в настройках)."""
        return self._storage

    @Property("QVariantMap", notify=accountChanged)
    def account(self) -> dict[str, object]:
        """{ uid, login, displayName, hasPlus }"""
        return self._account

    # --- слоты ---------------------------------------------------------------

    @Slot()
    def startDeviceLogin(self) -> None:
        self.cancelLogin()
        self._set_state("requestingCode")
        self._poll = self._runner.submit(self._device_flow(), self._on_token_received, self._on_error)

    @Slot()
    def cancelLogin(self) -> None:
        if self._poll is not None:
            self._poll.cancel()
            self._poll = None
        if self._state in ("requestingCode", "awaitingUser"):
            self._set_code("", "", 0)
            self._set_state("signedOut")

    @Slot(str)
    def signInWithToken(self, token: str) -> None:
        token = token.strip()
        if token.lower().startswith("oauth "):
            token = token[6:].strip()
        if not token:
            return
        self.cancelLogin()
        self._on_token_received(token)

    @Slot()
    def signOut(self) -> None:
        self.cancelLogin()
        self._client = None
        self._account = {}
        self.accountChanged.emit()
        self._runner.submit(asyncio.to_thread(tokens.clear))
        self._set_storage("")
        self._set_state("signedOut")
        self.signedOut.emit()

    @Slot()
    def retry(self) -> None:
        self.start()

    @Slot(str)
    def copyToClipboard(self, text: str) -> None:
        QGuiApplication.clipboard().setText(text)

    # --- поток входа ---------------------------------------------------------

    async def _device_flow(self) -> str:
        client = ClientAsync()
        code = await client.request_device_code(device_name=DEVICE_NAME)
        expires_at = time.time() + code.expires_in
        self._runner.call_in_gui(lambda: self._show_code(code.user_code, code.verification_url, expires_at))
        interval = max(code.interval, 2)
        while time.time() < expires_at:
            await asyncio.sleep(interval)
            token = await client.poll_device_token(code.device_code)
            if token is not None:
                return token.access_token
        raise DeviceAuthError("expired_token")

    def _show_code(self, user_code: str, url: str, expires_at: float) -> None:
        self._set_code(user_code, url, expires_at)
        self._set_state("awaitingUser")

    def _on_token_loaded(self, result: tuple[str | None, str]) -> None:
        token, storage = result
        if token:
            self._set_storage(storage)
            self._sign_in(token, save=False)
        else:
            self._set_state("signedOut")

    def _on_token_received(self, token: str) -> None:
        self._poll = None
        self._set_code("", "", 0)
        self._sign_in(token, save=True)

    def _sign_in(self, token: str, save: bool) -> None:
        self._set_state("signingIn")

        async def init() -> tuple[ClientAsync, dict[str, object], str]:
            client = await ClientAsync(token).init()
            status = client.me
            account = status.account if status else None
            plus = status.plus if status else None
            info = {
                "uid": account.uid if account else 0,
                "login": (account.login if account else "") or "",
                "displayName": (account.display_name or account.full_name or account.login) if account else "",
                "hasPlus": bool(plus and plus.has_plus),
            }
            storage = await asyncio.to_thread(tokens.save, token) if save else self._storage
            return client, info, storage

        def done(result: tuple[ClientAsync, dict[str, object], str]) -> None:
            self._client, self._account, storage = result
            self._set_storage(storage)
            self.accountChanged.emit()
            self._set_state("signedIn")
            self.signedIn.emit()

        def failed(error: BaseException) -> None:
            if isinstance(error, UnauthorizedError):
                self._runner.submit(asyncio.to_thread(tokens.clear))
                self._set_error("Токен недействителен или отозван — войдите заново")
                self._set_state("signedOut")
            else:
                self._on_error(error)

        self._runner.submit(init(), done, failed)

    def _on_error(self, error: BaseException) -> None:
        self._poll = None
        self._set_code("", "", 0)
        if isinstance(error, DeviceAuthError):
            text = "Время действия кода истекло" if "expired" in str(error) else f"Вход отклонён: {error}"
            self._set_error(text)
            self._set_state("signedOut")
            return
        if isinstance(error, NetworkError):
            self._set_error("Нет связи с Яндексом. Проверьте подключение к интернету")
        elif isinstance(error, YandexMusicError):
            self._set_error(f"Ошибка Яндекс Музыки: {error}")
        else:
            self._set_error(f"Непредвиденная ошибка: {error}")
        self._set_state("error")

    # --- служебное -----------------------------------------------------------

    def _set_state(self, state: str) -> None:
        if state != self._state:
            if state in ("requestingCode", "signingIn", "signedIn"):
                self._set_error("")
            self._state = state
            self.stateChanged.emit()

    def _set_storage(self, storage: str) -> None:
        if storage != self._storage:
            self._storage = storage
            self.storageChanged.emit()

    def _set_error(self, text: str) -> None:
        if text != self._error:
            self._error = text
            self.errorChanged.emit()

    def _set_code(self, code: str, url: str, expires_at: float) -> None:
        self._user_code, self._verification_url, self._code_expires_at = code, url, expires_at
        self.codeChanged.emit()
