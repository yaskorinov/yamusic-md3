"""asyncio-цикл в фоновом потоке + доставка результатов обратно в GUI-поток Qt.

API Яндекс Музыки асинхронный; QML/Qt живут в GUI-потоке. Корутины запускаются в отдельном
потоке-цикле, а колбэки (on_done/on_error) всегда вызываются в GUI-потоке через очередь сигналов.
"""

from __future__ import annotations

import asyncio
import threading
import traceback
from concurrent.futures import Future
from typing import Any, Awaitable, Callable

from PySide6.QtCore import QObject, Qt, Signal, Slot


class _Bridge(QObject):
    deliver = Signal(object)

    def __init__(self) -> None:
        super().__init__()
        self.deliver.connect(self._run, Qt.ConnectionType.QueuedConnection)

    @Slot(object)
    def _run(self, fn: Callable[[], None]) -> None:
        fn()


class AsyncRunner:
    def __init__(self) -> None:
        self.loop = asyncio.new_event_loop()
        self._bridge = _Bridge()  # создаётся в GUI-потоке → слоты выполняются в нём
        self._thread = threading.Thread(target=self._run_loop, name="yamusic-asyncio", daemon=True)
        self._thread.start()

    def _run_loop(self) -> None:
        asyncio.set_event_loop(self.loop)
        self.loop.run_forever()

    def submit(
        self,
        coro: Awaitable[Any],
        on_done: Callable[[Any], None] | None = None,
        on_error: Callable[[BaseException], None] | None = None,
    ) -> Future:
        """Запустить корутину в фоне; колбэки придут в GUI-потоке. Отмена — future.cancel()."""
        future = asyncio.run_coroutine_threadsafe(coro, self.loop)

        def done(f: Future) -> None:
            if f.cancelled():
                return
            error = f.exception()
            if error is not None:
                if on_error is not None:
                    self._bridge.deliver.emit(lambda: on_error(error))
                else:
                    tb = "".join(traceback.format_exception(error))
                    self._bridge.deliver.emit(lambda: print(f"yamusic: фоновая задача упала:\n{tb}"))
            elif on_done is not None:
                result = f.result()
                self._bridge.deliver.emit(lambda: on_done(result))

        future.add_done_callback(done)
        return future

    def call_in_gui(self, fn: Callable[[], None]) -> None:
        """Выполнить fn в GUI-потоке (можно звать из потока цикла)."""
        self._bridge.deliver.emit(fn)

    def stop(self) -> None:
        """Отменить незавершённые задачи (опрос кода, загрузки) и остановить цикл."""

        async def shutdown() -> None:
            tasks = [t for t in asyncio.all_tasks() if t is not asyncio.current_task()]
            for task in tasks:
                task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)

        try:
            asyncio.run_coroutine_threadsafe(shutdown(), self.loop).result(timeout=2)
        except Exception:
            pass
        self.loop.call_soon_threadsafe(self.loop.stop)
        self._thread.join(timeout=2)
