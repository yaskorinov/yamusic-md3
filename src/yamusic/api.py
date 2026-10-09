"""HTTP-слой клиента Яндекс Музыки с защитой от ограничений API.

Яндекс отвечает 429 «Concurrency limit exceeded», если с одним токеном одновременно идёт слишком
много запросов (старт приложения: аккаунт, плейлисты, лайки, настройки волны, тексты…).
Поэтому: не больше MAX_CONCURRENT запросов разом, а 429 и 5xx повторяются с нарастающей паузой.
"""

from __future__ import annotations

import asyncio
import logging
import random
import re

from yandex_music.exceptions import NetworkError
from yandex_music.utils.request_async import Request

log = logging.getLogger(__name__)

MAX_CONCURRENT = 4
RETRIES = 5          # паузы 0,8 → 12,8 с: всего до ~25 с
_RETRY_STATUS = re.compile(r"\((429|50[0-4])\)")


class RetryingRequest(Request):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._gate: asyncio.Semaphore | None = None    # создаётся в цикле клиента

    async def _request_wrapper(self, *args, **kwargs) -> bytes:
        if self._gate is None:
            self._gate = asyncio.Semaphore(MAX_CONCURRENT)
        delay = 0.8
        for attempt in range(RETRIES + 1):
            async with self._gate:
                try:
                    return await super()._request_wrapper(*args, **kwargs)
                except NetworkError as error:
                    if attempt == RETRIES or not _RETRY_STATUS.search(str(error)):
                        raise
                    log.info("API занят (%s), повтор через %.1f с", str(error)[:60], delay)
            await asyncio.sleep(delay * (1 + 0.25 * random.random()))
            delay *= 2
        raise AssertionError("unreachable")


def make_client(token: str | None = None):
    from yandex_music import ClientAsync
    return ClientAsync(token, request=RetryingRequest())
