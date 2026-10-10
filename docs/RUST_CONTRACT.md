# YaMusic: контракт переписывания бэкенда на Rust (cxx-qt)

Документ — единственный источник истины для 9 параллельных исполнителей. QML (`src/yamusic/qml`, 63 файла) переиспользуется
**без изменений**: тот же модуль `YaMusic.Core 1.0`, те же имена синглтонов, свойств, сигналов, методов, ролей моделей
и ключей словарей. Python-код (`src/yamusic/*.py`) и `android/` не трогаем — это эталоны поведения
(`android/app/src/main/kotlin/.../api/YandexApi.kt` — компактный эталон запросов API).

**Нормативный код.** Скелет рабочего пространства (`Cargo.toml`, `crates/*`) уже содержит все публичные типы и сигнатуры,
тела — `todo!()`. Сигнатуры в скелете — часть контракта: менять их можно только расширением (новые методы/поля),
а не изменением существующих; любое отклонение — в отчёт. Поведение, которое не видно из сигнатур, описано здесь
и в doc-комментариях скелета (они тоже нормативны).

Правила для всех: идиоматичный Rust 2024, tokio для ввода-вывода, `thiserror` в библиотеках, `anyhow` только в бинарнике/
`Services::start`; комментарии — минимальные, по-русски, как в репозитории; платформенный код — `cfg(target_os = …)`;
никаких git-операций; не трогать Python-файлы и `android/`; не трогать чужие файлы (таблица §2).

---

## 0. Сборка и окружение (проверено)

| что | версия / значение |
|---|---|
| Rust | 1.98.1 (msvc), edition 2024, `resolver = "3"` |
| MSVC | Build Tools 2022 (находится `cc` сам, `cl` в PATH не нужен) |
| Qt | 6.12.0 MSVC, `C:/Users/Fatyzzz/Qt/6.12.0` |
| cxx / cxx-qt / cxx-qt-lib / cxx-qt-build | 1.0.204 / 0.10.0 / 0.10.0 (features `qt_gui`, `qt_qml`, `serde`) / 0.10.0 |
| tokio / reqwest (rustls) / serde_json | 1.53.2 / 0.12.28 / 1.0.152 |
| libloading / keyring / secret-service / zbus | 0.8.9 / 3.6.3 / 5.2.0 / 5.19.0 |
| material-colors | 0.5.0 (есть `color_spec_2025`) |

Точные версии — в `Cargo.lock`; общие зависимости только через `[workspace.dependencies]` (`dep.workspace = true`).
Добавить новую зависимость можно в свой `crates/<свой>/Cargo.toml`; в корневой `Cargo.toml` — только если она нужна
нескольким крейтам (сообщите в отчёте).

Сборка (Git Bash):
```sh
export QMAKE=C:/Users/Fatyzzz/Qt/6.12.0/bin/qmake6.exe
export PATH="/c/Users/Fatyzzz/Qt/6.12.0/bin:$PATH"
cargo check --workspace            # проходит на скелете
cargo build -p yamusic
```
PowerShell: `$env:QMAKE = "C:/Users/Fatyzzz/Qt/6.12.0/bin/qmake6.exe"; $env:PATH = "C:\Users\Fatyzzz\Qt\6.12.0\bin;" + $env:PATH`.
Запуск: Qt DLL берутся из PATH; `console.log` QML в Windows виден только с `QT_FORCE_STDERR_LOGGING=1`.
Крейты без Qt (`ym-*`) собираются и тестируются без этих переменных: `cargo test -p ym-api`.

**Проба cxx-qt (пройдена):** `target/debug/yamusic.exe crates/yamusic/tests/probe.qml` при `QT_QPA_PLATFORM=offscreen`
печатает `PROBE OK`, код 0. Доказано:
1. QApplication (Qt Widgets) из C++-помощника, регистрация `YaMusic.Core` вручную (`cpp/register.cpp`), загрузка QML с диска;
2. `DictListModel` (база `QAbstractListModel`, переопределены `rowCount/data/roleNames`, `#[inherit] begin/end*Rows`)
   создаётся из Rust (`make_unique`), отдаётся из Q_INVOKABLE как `QObject*`, в QML работают `count`, `totalDurationMs`, `get`, `find`, роли;
3. свойство типа `QObject*` (`#[qproperty(*mut QObject, queue, READ = queue_ptr, CONSTANT)]`) на модель во владении Rust;
4. параметр `QObject*` из QML и `downcast::<DictListModel>()` (`cxx_qt::casting::Downcast`);
5. `CxxQtThread::queue` из фонового потока меняет свойство и шлёт сигнал;
6. **перегрузка Q_INVOKABLE**: два `#[qinvokable]` с одним `#[cxx_name = "openAlbum"]` и разными параметрами — QML
   выбирает по числу аргументов (`openAlbum(id)` и `openAlbum(id, trackId)`);
7. сигнал с аргументом `QVariantMap` доходит до JS.

Найденные ограничения cxx-qt 0.10 (учесть всем, кто пишет мосты):
- `CxxQtBuilder::new_qml_module` требует, чтобы **все** файлы мостов лежали в одном каталоге (QTBUG-93443). Поэтому
  QML-модуль не используется: атрибуты `#[qml_element]`, `#[qml_singleton]`, `#[qml_uncreatable]` **не ставить**;
  типы регистрирует `cpp/register.cpp` (`qmlRegisterSingletonInstance` — как в app.py).
- В каждом `#[cxx_qt::bridge]` с `#[qobject]` тип `QObject` объявлен неявно — **не объявлять** `type QObject` повторно
  (ошибка «defined multiple times»); в Rust-коде вне моста — `cxx_qt::QObject`.
- `Upcast`/`Downcast` — в `cxx_qt::casting`; для базы `QAbstractListModel` объявить её в `extern "C++Qt"` с `#[qobject]`
  и вызвать `cxx_qt::impl_transitive_cast!(Мой, QAbstractListModel, cxx_qt::QObject)` (см. `models/list_model.rs`).
- Создание экземпляра cxx-qt-объекта из Rust:
  `#[namespace = "rust::cxxqtlib1"] #[cxx_name = "make_unique"] fn x_make_unique() -> UniquePtr<X>;` в `unsafe extern "C++"`
  моста с `include!("cxx-qt-lib/common.h")`. После создания — `crate::qt::set_cpp_ownership(ptr)`.
- Сгенерированный заголовок моста: `"yamusic/src/<путь>/<файл>.cxxqt.h"` (например `yamusic/src/bridges/player.cxxqt.h`).
- `build.rs` сам находит все файлы `src/**/*.rs`, где строка начинается с `#[cxx_qt::bridge` — новый мост не требует правки `build.rs`.

---

## 1. Архитектура

```
                     ┌────────────── yamusic (бинарник) ───────────────┐
 QML (без изменений) │ shell: main/engine/cli/context/qt/models/icons/ │
 YaMusic.Core 1.0 ◄──┤        images/display/winframe, cpp/*           │
                     │ bridges: src/bridges/* — 8 синглтонов + данные  │
                     └───────┬─────────────────────────────────────────┘
                             │ AppContext { rt, services, flags, system_dark }
                     ┌───────▼────────┐
                     │  ym-services   │ auth, library(+меню), catalog, wave, lyrics, reporter, resolver, glue, trackfilter
                     └─┬────┬────┬──┬─┘
          ┌────────────┘    │    │  └──────────────┐
     ┌────▼───┐  ┌──────────▼┐ ┌─▼────────┐  ┌──────▼──────┐   ┌──────────┐
     │ ym-api │◄─┤ ym-player │ │ ym-lyrics│  │ ym-platform │   │ ym-theme │ (тему зовёт мост ThemeEngine)
     └────────┘  └───────────┘ └──────────┘  └─────────────┘   └──────────┘
```
Зависимости: `ym-api` — ни от кого; `ym-player` → `ym-api` (только `items::TrackItem/PlayContext`);
`ym-lyrics`, `ym-theme`, `ym-platform` — ни от кого из наших; `ym-services` → api, player, lyrics, platform;
`yamusic` → все.

**Потоки.**
- GUI-поток Qt: все QObject (мосты, модели), все вызовы Qt.
- tokio multi-thread рантайм (создаёт оболочка, 2–4 воркера, имя потока `yamusic-rt`): сервисы, HTTP, таймеры.
- ym-player: актор (tokio-задача) + поток `mpv-cmd` (команды libmpv по порядку) + поток событий на каждый экземпляр mpv.
- Мост → сервис: обычный вызов метода (`ctx().services.player.next()`), методы сервисов неблокирующие; долгие операции
  сервисы сами запускают на `rt`. Сервис → мост: `watch` (снимок) и `broadcast` (события); мост в `cxx_qt::Initialize`
  берёт `self.qt_thread()` и запускает на `ctx().rt` задачу-слушатель, которая переносит изменения через
  `qt_thread.queue(move |mut qobj| …)`. Если `queue` вернул ошибку (объект уничтожен) — задача завершается.
- `broadcast::error::RecvError::Lagged` — мост перечитывает снимок/списки сервиса и сбрасывает модели (resync).
- Ничего в GUI-потоке не ждёт сеть/mpv (как в Python: «GUI-поток никогда не ждёт mpv»).

**Что живёт в сервисах, а что в мостах.** Всё, что в Python работало независимо от QML (учёт прослушиваний,
обратная связь волне, MPRIS, громкость/переход/качество из настроек, текст для текущего трека), — в ym-services и
запускается `Services::start`. Чисто интерфейсное состояние (навигационные сигналы Catalog, состояние поиска, кэш
AlbumData/ArtistData, Qt-модели) — в мостах.

---

## 2. Исполнители и владение файлами

| # | исполнитель | владеет файлами |
|---|---|---|
| 1 | **api** | `crates/ym-api/**` |
| 2 | **player** | `crates/ym-player/**` |
| 3 | **theme** | `crates/ym-theme/**` |
| 4 | **lyrics** | `crates/ym-lyrics/**` |
| 5 | **services-core** | `crates/ym-services/Cargo.toml`, `src/lib.rs`, `auth.rs`, `library.rs`, `catalog.rs`, `resolver.rs`, `trackfilter.rs` |
| 6 | **services-wave** | `crates/ym-services/src/wave.rs`, `lyrics.rs`, `reporter.rs`, `glue.rs` |
| 7 | **platform** | `crates/ym-platform/**` |
| 8 | **shell** | `crates/yamusic/` всё, кроме `src/bridges/**`: `Cargo.toml`, `build.rs`, `cpp/**`, `src/main.rs`, `cli.rs`, `context.rs`, `engine.rs`, `display.rs`, `images.rs`, `icons.rs`, `winframe.rs`, `qt/**`, `models/**`, `tests/**` |
| 9 | **bridges** | `crates/yamusic/src/bridges/**` (включая `mod.rs`) |

Корневой `Cargo.toml` и этот документ — у архитектора; правки — только через отчёт.
`crates/yamusic/Cargo.toml` — у shell; если bridges нужна зависимость, она уже есть или её просят у shell.

### 2.1. Python-модуль → владелец

| Python | что | Rust (владелец) |
|---|---|---|
| `__init__.py`, `__main__.py` | точка входа | `yamusic/src/main.rs` (shell) |
| `app.py` | CLI, запуск, движок, контекстные свойства, снимок, трей, raise | `cli.rs`, `engine.rs`, `cpp/ym_shell.*` (shell) |
| `aio.py` | asyncio-поток + доставка в GUI | tokio-рантайм (shell) + `CxxQtThread` (bridges) |
| `api.py` | HTTP с семафором и повторами | `ym-api::client` (api) |
| `auth.py` | вход: device flow, токен, состояния | `ym-services::auth` (services-core) + `bridges/auth.rs` (bridges) |
| `tokens.py` | keyring + файл | `ym-platform::tokens` (platform) |
| `library.py` | плейлисты, лайки, треки плейлистов | `ym-services::library` (services-core) + `bridges/library.rs` |
| `catalog.py` | навигация, альбом/исполнитель, поиск | `ym-services::catalog` (services-core) + `bridges/catalog.rs`, `bridges/data.rs` |
| `models.py` | DictListModel и dict-конвертеры | конвертеры: `ym-api::items` (api); модель: `models/list_model.rs` (shell) |
| `trackfilter.py` | QML-тип TrackFilter | логика: `ym-services::trackfilter` (services-core); тип: `models/track_filter.rs` (shell) |
| `player.py` | очередь, mpv, gapless, переход, сессия | `ym-player` (player) + `bridges/player.rs` |
| `history.py` | /play-audio | `ym-services::reporter` (services-wave) |
| `wave.py` | «Моя волна» | `ym-services::wave` (services-wave) + `bridges/wave.rs` |
| `lyrics.py` | текст текущего трека, кэш | `ym-services::lyrics` (services-wave) + `ym-lyrics` (кэш, lyrics) + `bridges/lyrics.rs` |
| `lyrics_sources.py` | источники текстов | `ym-lyrics` (lyrics) |
| `settings.py` | схема настроек + QML Settings | `ym-platform::settings` (platform) + `bridges/settings.rs` |
| `theme.py` | Material You + ThemeEngine | `ym-theme` (theme) + `bridges/theme.rs` |
| `mpris.py` | MPRIS2 | `ym-platform::mpris` (platform) + `ym-services::glue::spawn_mpris` (services-wave) |
| `icons.py` | IconMetrics | `icons.rs` + C++ замер (shell) |
| `images.py` | NAM с дисковым кэшем, cache_dir | `images.rs` + C++ (shell); `ym-platform::dirs` (platform) |
| `display.py` | самый быстрый монитор | `display.rs` + C++ (shell) |
| `winframe.py` | рамка окна Windows (черновик) | `winframe.rs`, `cpp/winframe.*` (shell) |

---

## 3. Интерфейс shell ↔ bridges

### 3.1. Порядок запуска (engine::run, shell)
1. Переменные окружения как в app.py (см. шапку `engine.rs`); имена приложения; `qt::ffi::app_create(args)` (QApplication).
2. Главный режим и не `--new-instance`: `ym_platform::single_instance::raise_running_instance()` (на `rt.block_on`) → true — выход 0.
3. Иконка окна, `display::prefer_fastest_screen()` (до окон), шрифты `assets/fonts/*.ttf`, tracing.
4. tokio Runtime → `SettingsStore::open(config_dir)` → `Services::start(&rt, settings, ServicesOptions{persist_session,
   enable_mpris, no_report})` → `context::init(AppContext{…})`. Ошибка `Services::start` (нет libmpv) — сообщение в stderr и код 1.
5. `qt::ffi::register_qml_types()` — создаёт синглтоны **в порядке** Settings, ThemeEngine, IconMetrics, Auth, Library,
   Player, Wave, Catalog, Lyrics и регистрирует TrackFilter, DictListModel, AlbumData, ArtistData (`cpp/register.cpp` — уже есть).
6. QQmlApplicationEngine: NAM-фабрика, import path = `qml_dir()`, контекстные свойства, initial properties, load.
7. Не `--offline` → `ctx().services.auth.start()`. Трей, winframe, AppRequest, снимок, `exec`, `services.shutdown()`.

### 3.2. AppContext (`src/context.rs`, shell; уже реализован)
```rust
pub struct AppContext { pub rt: Handle, pub services: ym_services::Services, pub flags: StartupFlags, pub system_dark: watch::Receiver<bool> }
pub fn init(context: AppContext);           // один раз, до register_qml_types
pub fn ctx() -> &'static AppContext;        // мосты: в Default/Initialize и далее
pub struct StartupFlags { offline, screenshot, light, seed_image: Option<PathBuf>, debug_wheel, debug_fps }
```
`system_dark` поддерживает shell: C++ подключает `QStyleHints::colorSchemeChanged` и обновляет watch (через `extern "Rust"`
обратный вызов или опрос — на выбор shell); начальное значение — `qt::system_dark()`.

### 3.3. Помощники Qt (`src/qt/mod.rs`, shell; уже реализованы)
```rust
pub type QVariantMap = QMap<QMapPair_QString_QVariant>;   pub type QVariantList = QList<QVariant>;
pub fn json_to_qvariant(&Value) -> QVariant;  json_to_qvariantmap(&Map) -> QVariantMap;  json_to_qvariantlist(&[Value]) -> QVariantList;
pub fn qvariant_to_json(&QVariant) -> Value;  qvariantmap_to_json(&QVariantMap) -> Map<String, Value>;
pub fn copy_to_clipboard(&str);  pub unsafe fn set_cpp_ownership(*mut QObject);  pub fn system_dark() -> bool;  pub fn quit_app();
```
Числа: целое, влезающее в i32, → `int`, иначе `qlonglong`; дробное → `double`. Объекты/массивы — рекурсивно.
Shell может добавлять функции, не меняя эти.

### 3.4. Модели (`src/models`, shell; DictListModel уже реализован)
```rust
pub type Row = Arc<serde_json::Map<String, Value>>;
pub enum ModelKind { Track, Album, Artist, Playlist }      // TRACK_KEYS/ALBUM_KEYS/ARTIST_KEYS/PLAYLIST_KEYS — роли (UserRole + i)
impl DictListModel {                                         // qobject-тип, база QAbstractListModel
    pub fn create(kind: ModelKind) -> UniquePtr<DictListModel>;   // CppOwnership уже выставлен
    pub fn kind(&self) -> ModelKind;  pub fn rows(&self) -> &[Row];
    pub fn context(&self) -> &PlayContext;  pub fn set_context(self: Pin<&mut Self>, PlayContext);
    pub fn as_qobject_ptr(self: Pin<&mut Self>) -> *mut QObject;     // для свойств/возврата в QML
    pub fn reset_rows / append_rows(Vec<Row>) / insert_rows(at, Vec<Row>) / remove_rows(at, count) / set_loading(bool);
    pub fn find_row(&self, key: &str, value: &Value) -> Option<usize>;
}
// QML: count (int, countChanged), loading (bool, loadingChanged), totalDurationMs (qint64, countChanged),
//      get(int) -> QVariantMap ({} вне диапазона), find(key, value) -> int (-1); числа сравниваются по значению.
pub fn to_row<T: Serialize>(&T) -> Row;  pub fn to_rows<T: Serialize>(&[T]) -> Vec<Row>;
pub fn apply_list_change<T: Serialize>(Pin<&mut DictListModel>, &ym_services::ListChange<T>);
pub unsafe fn tracks_from_model(*mut QObject) -> Option<(Vec<TrackItem>, PlayContext)>;  // DictListModel или TrackFilter
```
Мост держит свои модели полями `UniquePtr<DictListModel>` (создаются в `Default`), отдаёт их в QML как `*mut QObject`
(`CONSTANT`-свойство с `READ = getter` или возврат из invokable). Модели живут столько же, сколько синглтон (до конца процесса).

### 3.5. Правила написания мостов (bridges)
- Один файл — один синглтон (+ `data.rs` для AlbumData/ArtistData); имена типов C++ = имена в QML:
  `Auth`, `Library`, `Player`, `Wave`, `Catalog`, `Lyrics`, `Settings`, `ThemeEngine`, `AlbumData`, `ArtistData`
  (их ждёт `cpp/register.cpp`; переименование = правка shell).
- Пространство имён моста — глобальное (`#[cxx_qt::bridge]` без namespace), имена C++ — через `#[cxx_name = "camelCase"]`.
- `impl cxx_qt::Threading for X {}` и `impl cxx_qt::Initialize for X {}` — подписки на сервисы в `initialize`.
- Конструктор по умолчанию (`Default` Rust-структуры) может звать `crate::context::ctx()`: синглтоны создаются после `context::init`.
- Типы свойств/параметров: `QString`, `bool`, `i32` (QML int), `i64` (qint64), `f64` (qreal), `QVariantMap`
  (`QMap_QString_QVariant`, include `cxx-qt-lib/qmap.h`), `QVariantList` (`QList_QVariant`, include `cxx-qt-lib/qlist.h`),
  `QStringList`, `QColor`, `QRectF`, `*mut QObject`.
- Сеттеры, которые QML ждёт «сразу» (Player.volume/shuffle/repeat/levelsWanted, ThemeEngine.dark/variant, все Settings):
  меняют локальное значение и шлют сигнал синхронно, затем передают в сервис; эхо из сервиса — без лишнего сигнала, если значение не изменилось.
- Сигналы шлются только при реальном изменении значения (как `_set_*` в Python), кроме событий-фактов (openRequested и т. п.).

### 3.6. Что делает shell для QML помимо мостов
- Контекстные свойства: `yamusicTray` (bool = не снимок и не галерея), `yamusicDebugWheel`, `yamusicDebugFps`.
- Initial properties корня: `--size WxH` → `width`, `height`; `--set KEY=VALUE` (true/false → bool, int, float, иначе строка).
- Корень с `trayWanted` → `QGuiApplication::setQuitOnLastWindowClosed(false)` (выход решает Main.qml).
- `AppRequest::RaiseWindow` → `show(); raise(); requestActivate()` корня; `AppRequest::Quit` → `app_quit()`.
- `--screenshot PNG`: через `--delay` мс `QQuickWindow::grabWindow().save(PNG)` и выход; конфиг — временный каталог,
  если не задан `--config-dir`.
- Каталог QML: `$YAMUSIC_QML_DIR` → `<exe>/qml` → `src/yamusic/qml`, найденный вверх от exe и от cwd; `assets` — рядом.
- **IconMetrics** (singleton, `icons.rs`): `glyph(QString name) -> QString` — символ по коду из `assets/fonts/icons.json`
  (нет имени — "" и сообщение в stderr «иконки «X» нет в урезанном шрифте — запустите tools/subset_fonts.py»);
  `ink(QString name, double size) -> QRectF` — замер по пикселям как `_measure` в icons.py (шрифт "Material Symbols Rounded",
  pixelSize, оси FILL=1 и opsz=clamp(size,20,48), порог альфы 96), кэш по (имя, round(size)). Замер — C++-функция в `cpp/`.
- **TrackFilter** (создаваемый тип, `models/track_filter.rs`): база QAbstractListModel, роли TRACK_KEYS;
  `source: QObject*` (rw, `sourceChanged`; принимает DictListModel, иное → null), `query: QString` (rw, `queryChanged`),
  `active: bool` (`queryChanged`), `count`, `loading` (всегда false), `totalDurationMs`, `get(row)`, `find(key, value)` — как
  DictListModel; контекст = контекст источника; правила поиска — `ym_services::trackfilter`; при изменении источника —
  инкрементально (вставка/удаление одного куска, иначе reset) по алгоритму `_refresh` из trackfilter.py; строки сравниваются
  по `Arc::ptr_eq`. Подписка на источник — через сгенерированный cxx-qt `on_count_changed` (guard хранить в фильтре).
  `tracks_from_filter` — найденные строки + контекст.
- **Кэширующая NAM-фабрика**, **самый быстрый монитор**, **рамка Windows** — §7 и шапки `images.rs`, `display.rs`, `winframe.rs`.

---

## 4. Публичный API крейтов

Сигнатуры — в скелете (нормативно). Здесь — поведение и то, что легко сделать не так.

### 4.1. ym-api (api)
- `YmClient::with_config` строит `reqwest::Client` (rustls, gzip, таймаут из конфига), семафор `MAX_CONCURRENT=4`.
  Повтор: статусы 429 и 500–504, до `RETRIES=5` раз, пауза `0.8·2ⁿ·(1+0.25·rand)` с; семафор держится только на время запроса.
- Ответ: JSON-конверт; если у объекта нет `result` — результатом считается весь объект (`_parse` в request_base.py).
  Ошибки: `error` строкой (OAuth: + `error_description`) или объектом `{name, message}`. Маппинг статусов → `ApiError`:
  401/403 → `Unauthorized`, 400 → `BadRequest{code}`, 404 → `NotFound`, прочие → `Http{status, code, message}`;
  `is_network()` — `Network`, а также `Http` с 409/413/502/иными (как NetworkError в Python), но не 400/401/403/404.
- Формы: `application/x-www-form-urlencoded`, bool как `True`/`False` (Python `str(bool)`), списки id — через запятую.
- `request_device_code` / `poll_device_token` — на `oauth_url`, без заголовков API (только User-Agent); `authorization_pending` → `Ok(None)`.
- `account_status`: `GET /account/status` → `{account{uid, login, displayName, fullName}, plus{hasPlus}}`.
- `users_likes_tracks`: `result.library.tracks[]` (`{id, albumId, timestamp}`).
- `tracks_file_info`: подпись и параметры — `sign.rs` и шапка метода; кодеки `FILE_INFO_CODECS`; ссылка `urls[0] || url`.
- `tracks_lyrics(LRC)`: 404 → `Ok(None)`; текст по `downloadUrl` — `download_text`.
- `play_audio`: форма как в шапке; `timestamp` = `client-now` = `utc_now_iso()`.
- `rotor_*`: тела JSON как в `rotor_sessions.py` (`includeTracksInResponse`, `queue`, `seeds`; feedback `{event{type,timestamp,trackId?,totalPlayedSeconds?}, batchId?, from?}`).
- **Пины (проверено в yandex_music 3.2.1, `_client_async/pins.py`):** `GET /pins`, `PUT /pin/wave` и `DELETE /pin/wave`
  с JSON `{"seeds": "<seed>"}`; ответ DELETE — `"ok"`.
- **Плейлисты (проверено, `_client_async/playlists.py`):** `POST /users/{uid}/playlists/create` (форма `title`, `visibility`);
  `POST /users/{uid}/playlists/{kind}/change` (форма `kind`, `revision`, `diff`=JSON); элемент вставки в diff —
  `{"id": "<trackId>", "albumId": "<albumId>"}` (**ключ `albumId`, не `album_id`** — так формирует `utils/difference.py`).
- `items::*::from_*` — правила `models.py` дословно (виды альбомов: single→«Сингл», compilation→«Сборник», podcast→«Подкаст»,
  audiobook→«Аудиокнига», иначе «Альбом»; «Альбом» с trackCount ≤ 6 и не compilation → «EP» при > 1 треке, иначе «Сингл»).
- Тесты: подписи (`file_info_sign`/`lyrics_sign` с фиксированным ts против Python), конвертеры на JSON-фикстурах, разбор конверта.

### 4.2. ym-player (player)
Порт `player.py` один-в-один по правилам очереди, gapless, плавного перехода, уровня, сессии. Ключевое:
- **gapless** (crossfade == 0): в плейлисте mpv «текущий + следующий»; ссылка на следующий запрашивается через
  `PREFETCH_DELAY_MS` после старта трека; mpv сам переходит → по `playlist-pos > 0` сдвинуть очередь, `playlist-remove 0`.
- **переход** (crossfade > 0): второй экземпляр mpv создаётся лениво в потоке mpv-cmd; за `crossfade` с до конца
  (если длительность > 2·crossfade с и следующий готов) — старт второго с громкостью 0, громкости по равномощной кривой
  `base·cos(t·π/2)^(1/3)` / `base·sin(t·π/2)^(1/3)` шагом `FADE_STEP_MS`, затем `stop` старого; главным становится второй;
  события обрабатываются только от главного; EOF без готового следующего — переход вручную (natural).
- `end-file` с reason ERROR → `error_text` «Не удалось воспроизвести трек — пропускаю» и `next`, если есть следующий.
  Ошибка ссылки → «Не удалось получить трек: <e>».
- Позиция — опрос `time-pos` каждые `POSITION_POLL_MS`, пока играет; `played_seconds` растёт только при шаге 0 < Δ < 1 с.
- `playing = has_track && !pause && !idle-active`; при переходе в паузу — точная позиция и сохранение сессии.
- Уровень: только если `levels_wanted`; `db = af-metadata/lvl["lavfi.astats.Overall.RMS_level"]`, `level = clamp((db+42)/36)`,
  `-inf` → 0; событие при изменении > 0.005; `levels_wanted = false` обнуляет уровень.
- Обложка: при смене трека скачать `cover` в `cover_dir/<sha1(url)>.jpg` (есть — не качать; .part + rename) → `cover_file`.
- События — `PlayerEvent` в том же порядке, что сигналы Python: `TrackChanged` при смене трека и при приходе кодека,
  `PositionChanged` при опросе/seek/длительности и т. д. `TrackStarted` / `TrackEnded` — как `trackStarted`/`trackEnded`
  (`natural = natural || (duration > 0 && end >= duration - 3)`). `NeedMore(feeder)` — когда впереди меньше `FEED_AHEAD`.
- Сессия: `session.rs`; сохранение через `SAVE_DEBOUNCE_MS` после QueueChanged/TrackChanged/ModeChanged и раз в `AUTOSAVE_MS`
  при игре; `shutdown` — синхронно. `restore_session`: очередь, порядок, позиция (на паузе), shuffle/repeat; первый запуск
  трека начнётся с сохранённой секунды (`loadfile … start=<s>`).
- libmpv: только динамически (`mpv.rs`); на unix `setlocale(LC_NUMERIC, "C")` перед `mpv_create`.
- Тесты: `state.rs` (порядок/next_pos/repeat/shuffle/волна), `session.rs` (совместимость JSON с Python), `codec_label`, `mpv_volume`.

### 4.3. ym-theme (theme)
- `build_scheme(seed, dark, variant, contrast)` — spec **2025**, platform phone, 59 ролей `ROLE_NAMES`.
  Эталонные значения: `crates/ym-theme/tests/vectors.json` (56 схем: 4 seed × тёмная/светлая × 7 вариантов, сгенерированы
  `theme.build_scheme` Python) — тест должен совпасть **точно**.
- `seed_from_image`: эталон для `yamusic.png` (корень репозитория) — `#FFFFDB00` (Python, Qt-масштабирование; допустимо
  отличие из-за другой интерполяции, но тест должен его фиксировать и документировать).
- `parse_color`: как `QColor(str)` для "#RGB"/"#RRGGBB"/"#AARRGGBB".

### 4.4. ym-lyrics (lyrics)
Порт `lyrics_sources.py` + кэш из `lyrics.py`. Формат JSON результата/кэша совместим (ключи, «gap», округление до 3 знаков).
Ключ кэша: `sha1(f"{CACHE_VERSION}|{track_id}")` hex + `.json`. Тесты: `lines_from_lrc`, `build_from_timed_lines`,
`parse_yrc`, `add_gaps`, `is_placeholder`, `ne_eapi_params` — против выходов Python на фикстурах (сгенерируйте их
`.venv/Scripts/python.exe -I` из `src/yamusic/lyrics_sources.py`).

### 4.5. ym-platform (platform)
- `dirs` — таблица в шапке `dirs.rs` (совпадает с QStandardPaths Python-версии).
- `settings` — §5. `tokens` — шапка `tokens.rs` (совместимость с сохранённым Python-токеном обязательна).
- `mpris` — шапка `mpris.rs`; `single_instance` — шапка файла. На не-Linux: `MprisServer::start` → `Err(Unsupported)`,
  `raise_running_instance` → `false`.

### 4.6. ym-services (services-core, services-wave)
- `Services::start` (services-core): `TokenStore` → `AuthService` → `LibraryService` → `CatalogService` →
  `PlayerHandle::start(PlayerConfig{cover_dir: dirs::cache_subdir("covers"), session_file: persist_session.then(dirs::session_file),
  audio_output: $YAMUSIC_AO, libmpv_path: $YAMUSIC_LIBMPV, volume/crossfade/quality из настроек}, ApiResolver)` →
  `restore_session()` (если persist) → `WaveService` → `LyricsService(LyricsFetcher::new(dirs::cache_subdir("lyrics")))` →
  `reporter::spawn` → `glue::spawn_settings_glue` → `glue::spawn_mpris` (если enable_mpris и Linux).
- Шапки модулей описывают поведение и тексты ошибок дословно. Снимки (`watch`) обновляются **до** рассылки события.
- `AuthService::client()` — клиент с токеном после успешного входа; до входа и после выхода — `None`.
- Меню трека — §7.

---

## 5. QML-объекты YaMusic.Core (bridges, кроме IconMetrics/TrackFilter)

Обозначения: **R** — только чтение, **RW** — запись из QML, **C** — CONSTANT. Тип в скобках — Rust/cxx-qt.

### 5.1. Settings (`bridges/settings.rs`)
24 свойства **RW**, у каждого сигнал `<имя>Changed()`; чтение/запись — `ctx().services.settings` (§6). Подписка на
`SettingsStore::subscribe()` шлёт сигналы для изменений из других мест (Player.volume, Wave.select).
**Windows:** геттер `windowButtons` всегда возвращает `true` (запись сохраняется, но не влияет).

### 5.2. ThemeEngine (`bridges/theme.rs`)
| свойство | тип | доступ | сигнал |
|---|---|---|---|
| colors | QVariantMap: роль (59 имён) → QColor | R | schemeChanged |
| seed | QColor | R | seedChanged |
| dark | bool | RW (перестроить схему) | darkChanged |
| variant | QString (имя из variants; иное игнорируется) | RW | variantChanged |
| variants | QStringList `[content, tonalSpot, vibrant, expressive, fidelity, neutral, monochrome]` | C | — |

Invokable: `setCover(QString source)`, `setSeed(QColor)`, `setSeedFromImage(QString pathOrFileUrl)`.
Поведение — theme.py: стартовая схема строится синхронно в `initialize` по настройкам (themeMode system → `ctx().system_dark`,
schemeVariant, customSeed) и шлёт все четыре сигнала; изменения themeMode/schemeVariant/accentFromCover/customSeed/системной темы —
`_apply_settings`; пересчёт — `rt.spawn_blocking` с номером запроса (устаревший результат отбрасывается, ещё не разобранная
обложка не теряется); `PlayerEvent::CoverFileChanged` → `setCover(snapshot.cover_file)`; флаги `--light` (dark = false) и
`--seed-image` применяются один раз после привязки. URL `file://` → локальный путь.

### 5.3. Auth (`bridges/auth.rs`) — зеркало `AuthService`
| свойство | тип | сигнал |
|---|---|---|
| state | QString (`AuthState::as_str`) | stateChanged |
| errorText | QString | errorChanged |
| userCode, verificationUrl | QString | codeChanged |
| codeExpiresAt | double, Unix-мс | codeChanged |
| tokenStorage | QString `keyring`/`file`/`""` | storageChanged |
| account | QVariantMap `{uid, login, displayName, hasPlus}` или `{}` | accountChanged |

Сигналы-факты: `signedIn()`, `signedOut()`. Invokable: `startDeviceLogin()`, `cancelLogin()`, `signInWithToken(QString)`,
`signOut()`, `retry()`, `copyToClipboard(QString)` (через `crate::qt::copy_to_clipboard`).

### 5.4. Library (`bridges/library.rs`)
| свойство | тип | доступ | сигнал |
|---|---|---|---|
| playlists | QObject* (DictListModel Playlist) | C | — |
| liked | QObject* (DictListModel Track, контекст `liked`) | C | — |
| errorText | QString | R | errorChanged |
| likesRevision | int | R | likesChanged |
| pinsRevision | int | R | pinsChanged |

Invokable: `refresh()`; `playlistTracks(qint64 uid, qint64 kind) -> QObject*` (модель на ключ `uid:kind` кэшируется, каждый вызов —
`load_playlist`; контекст `{type: playlist, from: desktop_win-playlist-track-default, playlistId: "uid:kind"}`);
`isLiked(QString trackId) -> bool`; `toggleLike(QVariantMap track)`; `dislike(QVariantMap track)`;
меню (§7): `addToPlaylist(QVariantMap track, qint64 kind)`, `createPlaylist(QString title)`,
`createPlaylist(QString title, QVariantMap track)` (перегрузка), `pinWave(QString seed)`, `unpinWave(QString seed)`,
`isWavePinned(QString seed) -> bool`. Сигнал `notice(QString text)` — `LibraryEvent::Notice`.
Словарь трека из QML → `TrackItem` через `qvariantmap_to_json` + `serde_json::from_value` (неполный словарь — поля по умолчанию;
нет trackId — игнор).

### 5.5. Player (`bridges/player.rs`) — зеркало `PlayerHandle`
| свойство | тип | доступ | сигнал |
|---|---|---|---|
| hasTrack | bool | R | trackChanged |
| trackId, title, artist (= track.artists), album, cover | QString | R | trackChanged |
| track | QVariantMap (TrackItem; `{}` без трека) | R | trackChanged |
| codec | QString (`codec_label`) | R | trackChanged |
| direction | int (+1/−1) | R | trackChanged |
| coverFile | QString (локальный путь) | R | coverFileChanged |
| playing, buffering | bool | R | stateChanged |
| positionMs, durationMs | int | R | positionChanged |
| position | double 0..1 | R | positionChanged |
| volume | double 0..1 (запись → плеер + Settings.volume) | RW | volumeChanged |
| shuffle | bool (у волны не меняется) | RW | modeChanged |
| repeat | QString off/all/one | RW | modeChanged |
| queue | QObject* (DictListModel Track) | C | — |
| currentIndex | int | R | queueChanged |
| source | QString (context.type) | R | queueChanged |
| errorText | QString | R | errorChanged |
| level | double 0..1 | R | levelChanged |
| levelsWanted | bool | RW | levelChanged |

Invokable: `playFrom(QObject* model, int row)` и `shuffleFrom(QObject* model)` (через `crate::models::tracks_from_model`;
пустая/чужая модель — игнор), `playIndex(int)`, `togglePlay()`, `play()`, `pause()`, `stop()`, `next()`, `previous()`,
`seek(double fraction)`, `seekMs(int ms)`, `cycleRepeat()`. Модель `queue` повторяет `PlayerEvent::Queue(QueueChange)`.

### 5.6. Wave (`bridges/wave.rs`) — зеркало `WaveService`
| свойство | тип | сигнал |
|---|---|---|
| groups | QVariantList `[{key, title, items: [{label, seed}]}]` | groupsChanged |
| selection | QVariantMap key → seed | selectionChanged |
| active, loading | bool | stateChanged |
| stationTitle | QString | stateChanged |
| errorText | QString | errorChanged |

Invokable: `play()`, `playStation(QString seed, QString title)`, `select(QString key, QString seed)`.

### 5.7. Catalog (`bridges/catalog.rs`, `bridges/data.rs`)
Сигналы: `openRequested(QString page, QVariantMap props)`, `chooseArtist(QVariantList refs)`,
`previewRequested(QString artistId, double x, double y, double width)`, `previewCancelled()`, `searchChanged()`,
**новый** `trackMenuRequested(QVariantMap track, double x, double y)`.

| свойство | тип | сигнал |
|---|---|---|
| searchState | QString idle/loading/ok/empty/error | searchChanged |
| searchQuery | QString | searchChanged |
| best | QVariantMap `{type, item}` или `{}` | searchChanged |
| totals | QVariantMap `{tracks, albums, artists, playlists}` (`{}` после очистки) | searchChanged |
| searchTracks / searchAlbums / searchArtists / searchPlaylists | QObject* (контекст треков `search`) | C |

Invokable (поведение catalog.py):
- `openArtist(QString id)` → `openRequested("artist", {artistId})` (пустой id — ничего);
- `openAlbum(QString id)` → `openRequested("album", {albumId})`; **перегрузка** `openAlbum(QString albumId, QString trackId)` →
  `{albumId, trackId}` (пустой trackId не кладётся) — страница альбома прокручивает к треку и подсвечивает его;
- `openArtists(QVariantList refs)` — refs с id: один → openArtist, несколько → `chooseArtist(refs)`;
- `previewArtist(QString id, double x, double y, double width)` → `previewRequested`; `cancelPreview()` → `previewCancelled`;
- `openPlaylist(QVariantMap p)` → `openRequested("playlist", {uid: int, kind: int, playlistTitle: p.title, cover: p.cover})`;
- `album(QString id) -> QObject*` (AlbumData), `artist(QString id) -> QObject*` (ArtistData): кэш по id, повтор при ошибке;
  без входа — объект с `loading=false`, `errorText="Войдите в аккаунт"`; ошибка — «Не удалось загрузить: <e>»;
- `search(QString query, QString kind)`: trim; тот же запрос и вид в loading/ok/empty — ничего; пусто → очистка (idle);
  без входа → error; поколение отбрасывает устаревшие ответы; `AuthEvent::SignedOut` → очистка;
- **новые:** `copyLink(QString url)` → буфер обмена; `trackUrl(QVariantMap track) -> QString` —
  `https://music.yandex.ru/album/<albumId>/track/<trackId>` или `https://music.yandex.ru/track/<trackId>` без альбома;
  `requestTrackMenu(QVariantMap track, double x, double y)` → `trackMenuRequested(track, x, y)`.

**AlbumData:** `info` (QVariantMap AlbumInfo), `loading` (bool), `errorText` (QString) — все с сигналом `changed()`;
`tracks` (QObject*, C; контекст `album`; `loading=true` до ответа).
**ArtistData:** `info` (ArtistInfo), `loading`, `errorText` (`changed()`); `popular` (контекст `artist`), `albums`, `alsoAlbums`,
`similar` (QObject*, C).
Создаются из Rust через `make_unique` (как DictListModel) с CppOwnership; хранятся в кэше моста.

### 5.8. Lyrics (`bridges/lyrics.rs`) — зеркало `LyricsService`
Свойства (все R, сигнал `changed()`): `lines` (QVariantList строк `{t, e, w: [{x, s, e, p?}], i}` / `{gap: true, t, e}`),
`trackId`, `status` (idle/loading/ok/notfound/error), `source`, `kind` (word/line/"").

### 5.9. Ключи словарей (сводка)
- трек: `trackId, albumId, title, version, artists, artistRefs[{id,name}], album, cover, durationMs, explicit, available`;
- альбом: `albumId, title, version, artists, artistRefs, year, kind, trackCount, cover, explicit` (+ info: `label, genre, durationMs`);
- исполнитель: `artistId, name, cover, genres` (+ info: `listeners, likes`);
- плейлист: `uid, kind, title, trackCount, cover, owner`;
- контекст очереди: `{type, from, playlistId?}`.

---

## 6. Настройки (ym-platform::settings + bridges/settings.rs)

| ключ | тип QML | по умолчанию | примечание |
|---|---|---|---|
| themeMode | string | `"dark"` | system / light / dark |
| schemeVariant | string | `"content"` | ThemeEngine.variants |
| accentFromCover | bool | `true` | |
| customSeed | string | `"#FFCC00"` | |
| windowButtons | bool | `false` | **Windows: геттер всегда true** |
| trackTransition | string | `"wipe"` | wipe / ripple / dissolve / liquid |
| backdropMode | string | `"gauss"` | gauss / glass / palette / blobs |
| nowPlayingBlur | real | `1.0` | 0..1 |
| nowPlayingDrift | real | `0.5` | 0..1 |
| ambientBackground | bool | `true` | |
| coverSpin | bool | `true` | |
| coverPulse | bool | `true` | |
| calmWhenInactive | bool | `true` | |
| rightPanelOpen | bool | `false` | |
| rightPanelTab | string | `"queue"` | queue / lyrics |
| sidebarCollapsed | bool | `false` | |
| windowWidth | int | `1280` | |
| windowHeight | int | `800` | |
| quality | string | `"lossless"` | lossless / high / low |
| volume | real | `0.7` | |
| crossfade | int | `4` | секунды, 0 — gapless |
| waveSeeds | string | `""` | сиды через запятую |
| closeToTray | bool | `true` | |
| wheelStep | int | `150` | |

Хранилище и совместимость — шапка `settings.rs`. Запись равного значения — без события.

---

## 7. Окно на Windows (shell: `cpp/winframe.*`, `winframe.rs`)

QML не меняется (`flags: Qt.Window | Qt.FramelessWindowHint`, `ResizeEdges`, `TopBar` с `DragHandler → startSystemMove`).
После `load` оболочка ставит на корневое окно нативный фильтр (`QAbstractNativeEventFilter` на приложение, фильтр по HWND):
1. **Стиль:** добавить `WS_THICKFRAME | WS_CAPTION | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_SYSMENU` (Aero Snap, Win+стрелки,
   анимации, сворачивание кликом по панели задач), затем `SetWindowPos(SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER)`.
   Фильтр ставится **до** SetWindowPos (ловит WM_NCCALCSIZE от FRAMECHANGED).
2. **WM_NCCALCSIZE** (wParam = TRUE): вернуть 0 — вся площадь клиентская, Windows не рисует хром. Если окно развёрнуто
   (`IsZoomed`), сузить `rgrc[0]` на толщину рамки (`GetSystemMetricsForDpi(SM_CXSIZEFRAME) + SM_CXPADDEDBORDER`, по
   вертикали `SM_CYSIZEFRAME + SM_CXPADDEDBORDER`), иначе края уходят за монитор; панель задач с автоскрытием — оставить 1 px снизу.
3. **WM_NCHITTEST:** в развёрнутом/полноэкранном состоянии — `HTCLIENT`; иначе по краю шириной `6 · dpi / 96` от
   `GetWindowRect` — `HTLEFT/HTRIGHT/HTTOP/HTBOTTOM/HTTOPLEFT/HTTOPRIGHT/HTBOTTOMLEFT/HTBOTTOMRIGHT` (нативный плавный ресайз,
   курсоры ставит Windows), внутри — `HTCLIENT`. Заголовок (`HTCAPTION`) **не** возвращать: перетаскивание делает TopBar
   (`startSystemMove`), а сайдбар и кнопки в той же полосе должны получать клики.
4. Тень и скругления Win11: `DwmExtendFrameIntoClientArea(hwnd, {0, 0, 0, 1})` (dwmapi уже линкуется в build.rs).
5. Кнопки окна видны: `Settings.windowButtons` на Windows всегда true (§6); `showMinimized`, развернуть/восстановить, `close`
   работают через QML как сейчас.
Проверка вручную: ресайз за все края и углы без мерцания, Aero Snap (перетаскивание к краю, Win+←/→/↑), двойной клик по
TopBar разворачивает, развёрнутое окно не обрезано, перетаскивание по пустому месту TopBar.

---

## 8. Меню трека (планируемое; бэкенд делается сейчас)

Пины **существуют** в yandex_music 3.2.1 — реализуем. Цепочка:
- **ym-api:** `pins()`, `pin_wave(seeds)`, `unpin_wave(seeds)`, `users_playlists_create(uid, title, visibility)`,
  `users_playlist(uid, kind)` (ревизия), `users_playlists_change(uid, kind, revision, diff)`,
  `users_playlists_insert_track(uid, kind, revision, at, track_id, album_id)`; diff `[{"op":"insert","at":0,"tracks":[{"id":…,"albumId":…}]}]`.
- **ym-services::library:** `add_to_playlist(track, kind)` (повтор при устаревшей ревизии 412/409/400 wrong-revision —
  один раз, с перечитыванием), `create_playlist(title, Option<track>)`, `pin_wave/unpin_wave/is_wave_pinned/pins_revision`,
  `LibraryEvent::{PinsChanged, Notice}`; пины грузятся после входа и очищаются при выходе.
- **Library (мост):** `addToPlaylist(track, kind)`, `createPlaylist(title)`, `createPlaylist(title, track)`,
  `pinWave(seed)`, `unpinWave(seed)`, `isWavePinned(seed)`, `pinsRevision` + `pinsChanged`, `notice(text)`.
  В QML привязка вида `Library.pinsRevision, Library.isWavePinned("track:" + id)` пересчитывается при изменении.
- **Catalog (мост):** `copyLink(url)`, `trackUrl(track)`, `openAlbum(albumId, trackId)`, `requestTrackMenu(track, x, y)` →
  сигнал `trackMenuRequested(track, x, y)` (его слушает главное окно и показывает меню у точки).
- **Существующее, которым пользуется меню:** `Wave.playStation("track:" + id, title)`, `Player.cycleRepeat()` / `Player.repeat`,
  `Catalog.openAlbum`, `Catalog.openArtists(refs)`, `Library.toggleLike`, `Library.dislike`.
- Список своих плейлистов для подменю — `Library.playlists` (у своих `uid == Auth.account.uid`).

---

## 9. Отклонения от постановки и решения архитектора

1. **QML-модуль cxx-qt не используется** (ограничение одного каталога, §0): регистрация в `cpp/register.cpp`
   через `qmlRegisterSingletonInstance` — это и есть поведение app.py. Атрибуты `qml_*` в мостах запрещены.
2. **Одна модель `DictListModel`** вместо четырёх классов (`TrackListModel` и т. д.): QML не обращается к именам классов,
   набор ролей задаёт `ModelKind`.
3. **Ключ альбома в diff плейлиста — `albumId`** (в постановке `album_id`; так в `utils/difference.py`).
4. `Catalog.openAlbum` с trackId — **перегрузка** того же имени (доказано пробой), а не необязательный параметр.
5. Добавлены: `Library.pinsRevision/pinsChanged/isWavePinned/notice`, `Catalog.trackUrl` — без них меню не показать состояние/итог.
6. `ym-services` разделён на двух исполнителей (services-core / services-wave) — 9 пакетов работ.
7. Настройки читаются из того же места, что у QSettings Python-версии (INI на Linux/macOS, реестр на Windows), а не из нового
   формата — чтобы не потерять настройки пользователей; токен — из тех же записей keyring.
8. Таймаут HTTP API — 10 с вместо 5 с Python (у Python бывали обрывы на медленной сети); повторы — как в Python.
9. Секрет-сервис на Linux — крейт `secret-service` 5 (атрибуты как в Python), а не `keyring` (у него свои атрибуты).
10. `crates/yamusic/src/bridges/*.rs`, `main.rs`, `tests/probe.qml` сейчас — проба архитектора; bridges и shell заменяют их
    целиком (probe.qml после этого можно удалить — shell).

## 10. Готовность (критерии приёмки каждого пакета)
- `cargo check --workspace` и `cargo clippy -p <свой> -- -D warnings` (для Qt-крейта — с переменными §0) чисто;
- `cargo test -p <свой>` проходит (крейты ym-*: модульные тесты на эталонах Python, без сети; сетевые тесты — `#[ignore]`);
- нет `todo!()` в своих файлах; публичные сигнатуры скелета не изменены (только дополнены);
- shell + bridges: `yamusic.exe` загружает `src/yamusic/qml/Main.qml` без ошибок QML в консоли
  (`QT_FORCE_STDERR_LOGGING=1`), `--gallery`, `--screenshot out.png` работают; вход, лайки, плеер, волна, тексты — вручную.
