# YaMusic — кастомный клиент Яндекс Музыки в стиле MD3 Expressive

Нативный, лёгкий клиент для Linux (основная цель — Arch + Hyprland/Wayland).
Визуальный референс и разбор: [reference/DESIGN_NOTES.md](reference/DESIGN_NOTES.md).

## Стек

| Слой | Выбор | Почему |
|---|---|---|
| UI | QML (Qt Quick 6.12), свой MD3-кит `Md3` | сцен-граф Qt лёгкий и плавный; Electron/WebEngine исключены |
| Логика | Python 3.14 + PySide6-essentials | готовая библиотека `yandex-music`, быстрая разработка |
| Цвет | `materialyoucolor` (порт material-color-utilities, spec 2025) | dynamic color из обложки |
| Звук | libmpv через `python-mpv` | gapless, все кодеки, PipeWire |
| API | `yandex-music` (MarshalX), неофициальный API | — |
| Сборка | uv; позже PKGBUILD + `.desktop` | — |

## Решения

- **Авторизация**: OAuth device code (код вводится на ya.ru/device) + ручной ввод токена. Токен — в Secret Service (keyring). Без встроенного браузера.
- **Качество**: настройка low / high / lossless. FLAC через зашифрованный транспорт с обязательным откатом на AAC.
- **Цвет**: всё приложение перекрашивается под обложку текущего трека (схема из доминантного цвета, плавная анимация смены). Можно зафиксировать seed-цвет. Светлая/тёмная тема.
- **Окно**: без системного заголовка, MD3 top app bar, без кнопок окна (опция для не-тайлинговых DE).
- **Раскладка**: NavigationRail/сайдбар слева → контент по центру → выдвижная правая панель (очередь / текст) → плавающий мини-плеер-пилюля снизу → полноэкранный Now Playing с блоб-переходом.
- **Системные интеграции**: MPRIS (обязательно), иконка в трее (SNI).

## MVP

1. Вход (device code / токен).
2. Плеер: очередь, shuffle/repeat, громкость, перемотка, MPRIS.
3. Моя волна (с настройками) и «Мне нравится» (лайк/дизлайк).
4. Плейлисты, альбомы, артисты; редактирование своих плейлистов.
5. Поиск с подсказками.
6. Синхронный текст песен.
7. Офлайн-кэш треков и обложек.
8. Трей.

**После MVP**: аудиовизуализатор (Bars/Wave/Circular, выключен по умолчанию; PCM-тап из mpv/PipeWire + FFT), уведомления, Discord RPC.

## Этапы

1. ✅ **MD3-кит + тема**. Галерея компонентов (`yamusic --gallery`).
2. ✅ **Каркас приложения**: окно, навигация, роутинг страниц, хранилище настроек.
3. ✅ **Авторизация + API-слой** (асинхронный, в отдельном потоке) + кэш обложек.
4. ✅ **Плеер** (mpv) + очередь + MPRIS + мини-плеер + Now Playing.
5. ✅ **Моя волна**, лайк/дизлайк, учёт прослушиваний.
6. ✅ Плейлисты / альбомы / артисты, поиск.
7. ✅ Тексты; офлайн-кэш ← следующий.
8. Трей, PKGBUILD.

## Структура

```
src/yamusic/
  __init__.py        точка входа (main)
  app.py             QGuiApplication, шрифты, регистрация типов, загрузка QML
  settings.py        Settings (QSettings → ~/.config/yamusic/YaMusic.conf), свойства из SCHEMA
  theme.py           ThemeEngine: схема MD3 из seed/обложки, следует настройкам
  aio.py             asyncio-цикл в фоновом потоке, колбэки — в GUI-потоке
  auth.py            Auth: device code (ya.ru/device) / токен, состояние входа
  tokens.py          токен: Secret Service, если есть связка по умолчанию, иначе файл 0600
  library.py         Library: плейлисты, «Мне нравится», треки плейлистов (порциями), лайки/дизлайки
  player.py          Player: mpv, очередь с контекстом (откуда треки), события начала/конца трека
  wave.py            Wave: «Моя волна» — сессия rotor, настройки, обратная связь
  history.py         PlayReporter: /play-audio (история и рекомендации)
  mpris.py           MPRIS на dbus-fast
  models.py          TrackListModel / PlaylistListModel (QAbstractListModel)
  images.py          дисковый кэш картинок для QML (~/.cache/yamusic/YaMusic/images, 512 МБ)
  icons.py           IconMetrics: оптическое центрирование глифов Material Symbols
  assets/fonts/      Google Sans (OFL), Material Symbols Rounded (Apache 2.0)
  qml/
    Main.qml         главное окно: Sidebar | контент (TopBar, Router, MiniPlayer) | RightPanel
    app/             оболочка: Router, Page, Sidebar, TopBar, MiniPlayer, RightPanel, ResizeEdges…
    app/pages/       страницы (home, search, wave, liked, settings)
    Md3/             модуль компонентов MD3 Expressive
    gallery/         галерея компонентов
```

## MD3-кит (`import Md3`)

| Компонент | Суть |
|---|---|
| `Theme` (singleton) | анимируемые цветовые роли, типографика, формы, motion-токены |
| `Label`, `Icon` | текст по шкале MD3; иконки Material Symbols (вариативные оси FILL/wght) |
| `StateLayer` | hover/press-слой + ripple в границах скругления |
| `Button`, `IconButton` | filled / tonal / outlined / text; морфинг скругления при нажатии |
| `ButtonGroup` + `SegmentButton` | connected button group |
| `MorphShape`, `MorphImage`, `Shapes.js` | полярные фигуры (circle, square, cookie N, flower, blob) и морфинг между ними; маска для обложек |
| `PlayButton` | play/pause в «печеньке», которая морфится и вращается |
| `WavyProgress`, `WavyDivider` | волнистый прогресс (амплитуда → 0 на паузе) и разделитель |
| `Slider` | expressive: толстый трек, хэндл-полоса с зазорами, stop-точка |
| `Switch` | MD3 switch с иконкой в бегунке |
| `SmoothFlickable` | плавная прокрутка колесом (экспоненциальная доводка), тачпад 1:1, тонкий индикатор |
| `Chip` | filter chip; `selected` управляется снаружи (радио-группы) |
| `NavItem`, `SearchField`, `Surface` | элементы навигации и поверхности |

## Каркас (этап 2)

- **Навигация**: `Router` держит живые экземпляры страниц в стеке (назад сохраняет прокрутку).
  Разделы сайдбара — `reset()` (fade through), переход вглубь — `push()` (shared axis X), `back()`.
- **Горячие клавиши**: Ctrl+F поиск, Alt+← / кнопка мыши «назад», Ctrl+, настройки, Ctrl+B сайдбар,
  Ctrl+L текст, F11 полноэкранный режим, Ctrl+Q выход.
- **Адаптивность**: < 980 px — сайдбар становится рейлом; < 1100 px — правая панель не показывается.
- **Окно**: без рамки; верхняя панель тянет окно, двойной клик — развернуть; края — `startSystemResize`.
- **Цвет по умолчанию**: жёлтый Яндекса `#FFCC00`, пока ничего не играет (настраивается).
- **Плеер** пока `app/DemoPlayer.qml` — заглушка с тем же интерфейсом, что будет у Python-плеера.
- Бесконечные анимации в простое запрещены (держат перерисовку каждый кадр); вращение/волны — только во время игры.

## Вход и API (этап 3)

- Вход: OAuth Device Flow библиотеки `yandex-music` (client_id Android-приложения), код на ya.ru/device,
  5 минут, опрос раз в 5 с. Запасной путь — вставить токен вручную.
- Токен: Secret Service только при наличии коллекции по умолчанию (иначе keyring дёргает системный
  диалог создания связки при каждом запуске), иначе файл в каталоге данных с правами 0600.
- Все запросы — `AsyncRunner.submit(coro, on_done, on_error)`; смена аккаунта отменяет ответы старых запросов (generation).
- Обложки: `https://` + `cover_uri.replace("%%", "400x400")`, грузит сам QML `Image` через NAM с дисковым кэшем.

## Плеер (этап 4)

- `player.py`: libmpv (python-mpv), очередь с shuffle/repeat, `get-file-info` с `transport=raw` —
  FLAC (flac-mp4) приходит незашифрованным, mpv играет его по прямой ссылке; откат на AAC (hq).
- Без пауз: в плейлисте mpv «текущий + следующий», по смене playlist-pos очередь догоняет mpv.
- `mpris.py`: org.mpris.MediaPlayer2.yamusic на dbus-fast в asyncio-потоке (playerctl, waybar, медиаклавиши).
- Обложка играющего трека кэшируется в ~/.cache/yamusic/YaMusic/covers → цвет темы и mpris:artUrl.
- Now Playing раскрывается через «окно» — случайную фигуру MD3E: вращаясь, растёт из обложки мини-плеера до
  средней у центра, затем на весь экран (маска MultiEffect только на время перехода).
- Клавиши: Space, Ctrl+←/→, Shift+←/→ (±10 с), Ctrl+↑/↓ громкость, Ctrl+P плеер, Esc закрыть.
- Тесты без звука: `YAMUSIC_AO=null`. libmpv требует `LC_NUMERIC=C`.
- Синглтоны-экземпляры регистрировать ДО создания QQmlApplicationEngine — иначе ломается весь QML.

## Моя волна и оценки (этап 5)

- Волна — новые сессии rotor: `rotor_session_new(seeds)` → партия треков + `radioSessionId`/`batchId`;
  следующая партия — `rotor_session_tracks(id, queue=последние полученные)`, когда впереди < 3 треков
  (плеер вызывает «подкормку» волны). Протухшая сессия (`unknownSession`) пересоздаётся с теми же сидами.
- Сиды: `user:onyourwave` или сид занятия (`activity:workout`, `genre:dance`, `mood:relaxed`) + настройки
  `settingDiversity:*`, `settingMoodEnergy:*`, `settingLanguage:*`. Список — из `/rotor/wave/settings`,
  при ошибке — запасной в `wave.py`. Выбор хранится в `Settings.waveSeeds`.
- Смена настройки во время игры: новая сессия, всё после текущего трека заменяется.
- Обратная связь волне: radioStarted, trackStarted, trackFinished/skip (сколько секунд проиграно), like/unlike/dislike.
- `/play-audio` для любого трека: в начале (0 с) и в конце (проиграно, позиция конца); `from` — по контексту очереди.
- Лайк меняет интерфейс сразу (`Library.likesRevision` для привязок), при ошибке API — откат.
  «Не рекомендовать» = `users_dislikes_tracks_add` + следующий трек.
- Тесты на настоящем аккаунте: `YAMUSIC_NO_REPORT=1` — история и обратная связь не отправляются, только пишутся в лог.

## Каталог и поиск (этап 6)

- `catalog.py` — синглтон `Catalog`: поиск (`search`, лучший результат + треки/альбомы/исполнители/плейлисты),
  `album(id)` → AlbumData (albums_with_tracks), `artist(id)` → ArtistData (artists_brief_info + direct_albums).
- Переходы — `Catalog.openArtist/openAlbum/openPlaylist/openArtists` → сигнал `openRequested`, его ловит Main
  (закрывает полноэкранный плеер, `router.push`). У трека с несколькими исполнителями — меню `Md3/Menu`.
- Ссылки (`app/LinkLabel`) — исполнитель и альбом в строках треков, мини-плеере, полноэкранном плеере, шапке альбома.
- «Волна» на странице исполнителя — `Wave.playStation("artist:ID")` (сид без настроек «Моей волны»).
- Картинки QML грузятся по HTTP/1.1: по HTTP/2 сервер обложек отвечает REFUSED_STREAM на пачку запросов.

## Фон и движение

- `app/CoverBackdrop.qml` — размытая обложка (`Settings.nowPlayingBlur`) с «плаванием» (`nowPlayingDrift`):
  композиция неподвижна, шейдер `flow.frag` переливает размытые пятна полем смещений. Размытие считается
  один раз, каждый кадр — только выборка. Используется в полноэкранном плеере и как атмосферный фон окна
  (`Settings.ambientBackground`: сайдбар прямо на фоне, панели `Surface.translucency` 0.3).
- Переходы страниц (Router): разделы — всплытие снизу с масштабом 0.94→1, вглубь/назад — сдвиг по X на 140 px.
- «Главной» нет: стартовая страница — «Моя волна».

## Тексты песен

- `lyrics_sources.py` — загрузчик из плагина Word Lyrics (DMS) того же автора: NetEase YRC (по словам) →
  Musixmatch richsync (по словам) → LRCLIB → Musixmatch subtitles → NetEase LRC → Яндекс LRC.
  Построчным текстам время слов интерполируется по длине. Кэш — ~/.cache/yamusic/YaMusic/lyrics.
- `lyrics.py` — синглтон `Lyrics` для играющего трека (поиск в рабочем потоке, Яндекс — через asyncio-цикл).
- `app/LyricsView.qml` — заливка по словам (шейдер `wordfill`), каскадная прокрутка, паузы «• • •»,
  размытие неактивных строк, затухание краёв (`edgefade`), клик по строке — перемотка.
  Позиция между обновлениями плеера (~5/с) интерполируется по часам.

## Производительность (уроки)

- `MorphShape` при создании не морфится (иначе каждая фигура полсекунды пересчитывала путь из круга —
  ~7 мс/кадр на 20 обложек при каждом появлении строк в списке).
- `MorphShape` рисуется белым, цвет — шейдер `Md3/shaders/tint.frag(.qsb)` на слое самой Shape:
  CurveRenderer при смене `fillColor` перестраивает геометрию (≈0,15 мс на фигуру за кадр перекраски).
  Пересборка шейдера: `/usr/lib/qt6/bin/qsb --glsl "100 es,120,150" --hlsl 50 --msl 12 -o tint.frag.qsb tint.frag`
  (qsb из пакета qt6-shadertools; в pyside6-essentials его нет).
- Все вызовы libmpv — в потоке `mpv-cmd`; состояние — из наблюдателей. Синхронный вызов из GUI
  подвешивает кадры, пока mpv открывает сетевой поток.
- `sys.setswitchinterval(0.001)`: фоновый Python иначе держит GIL до 5 мс и GUI-поток ждёт.
- Свойство-`Property` в подклассе с `notify` на сигнал базового класса — segfault PySide; объявлять в базовом.
- Замер стоимости перекраски: `QT_LOGGING_RULES="qt.scenegraph.time.renderloop=true"` → `animations=N ms`.

## Разработка

```bash
uv run yamusic --gallery                         # галерея MD3-кита (пока это и есть точка входа)
uv run yamusic --gallery --light --set startCover=1
uv run yamusic --gallery --set scrollTo=600 --screenshot out.png   # снимок без окна (offscreen + OpenGL)
uv run yamusic --size 900x700 --set startPage=settings --screenshot out.png  # снимок приложения (временный конфиг)
```

Подводные камни:
- **Роли `onX` называются `fgX`** (`Theme.fgPrimary`, `Theme.fgSurfaceVariant`, `inverseOnSurface` → `fgInverseSurface`).
  QML принимает имена вида `onSomething` за обработчики сигналов: `Behavior` на таком свойстве роняет движок, а чтение даёт мусор.
- Вложенные `ColumnLayout`/`RowLayout` по умолчанию `fillWidth: true` — для фиксированных колонок явно ставить `Layout.fillWidth: false`.
- Software-бэкенд Qt Quick не рисует `MultiEffect`/`RectangularShadow`; offscreen-снимки — только с `QT_QUICK_BACKEND=rhi QSG_RHI_BACKEND=opengl` (флаг `--screenshot` делает это сам).
- Шрифт — **Google Sans** (вариативный, вшит). Google Sans Flex из MD3 Expressive не годится: в нём нет кириллицы.
  Вес задаётся только осью `wght` (`Label.weight`, 400..700); `font.weight` заставляет Qt брать статический файл
  семейства из системы, и 600 превращается в 500.
- Иконки — только `Text.NativeRendering`: distance-field рендер дырявит заливку (FILL=1) вариативного Material Symbols.
- Смена темы в окне — «шторка» `Md3/ThemeWipe`: снимок окна (ShaderEffectSource, live: false) со старыми цветами
  кладётся поверх, тема переключается мгновенно, снимок уезжает (шейдер `wipe.frag`) по `Player.direction`:
  вперёд — слева направо, назад — справа налево. Привязки цветов пересчитываются один раз, а не каждый кадр
  (animations = 0 мс во время шторки против 2–5 мс у плавной анимации). Без ThemeWipe (галерея) — прежняя
  плавная анимация прогресса в `Theme` (OKLab). Расчёт схемы — в пуле потоков (~40 мс Python).
  Компонентам нельзя иметь свою `Behavior on color` без `enabled: !Theme.transitioning` — иначе двойная анимация и рывки.
- **60 к/с на 144-герцовом мониторе.** Qt Quick берёт период кадра у `primaryScreen()`, а на Wayland основной —
  первый монитор от композитора (у пользователя 60-герцовый DVI-D-1). Qt считает частые кадры «сломанным vsync» и
  гоняет анимации таймером 60 Гц. `display.py` при старте делает основным самый быстрый монитор через экспортируемую
  `QWindowSystemInterface::handlePrimaryScreenChanged` (ctypes). Отключить: `YAMUSIC_KEEP_PRIMARY_SCREEN=1`.
  Проверка: `--debug-fps` (оверлей) или `QT_LOGGING_RULES="qt.scenegraph.time.renderloop=true"`.
- В окружении пользователя `QSG_USE_SIMPLE_ANIMATION_DRIVER=1` — анимации от таймера ~60 Гц вместо vsync
  (дёргается на 144 Гц). `app.py` убирает переменную для своего процесса. Прокрутка — только через `SmoothFlickable`.
- В окружении пользователя `QT_LOGGING_RULES` глушит `console.log`; для отладки: `QT_LOGGING_RULES="qml.debug=true"`.
  Замер кадров при смене обложек: `--set perfLog=true`.
- Иконки центрируются по фактическому контуру (IconMetrics меряет пиксели в том же кегле и opsz):
  `FontMetrics.tightBoundingRect` для лигатур вариативного шрифта врёт, а часть глифов (favorite) нарисована выше центра.
- Иконка рядом с текстом — `Icon { tight: true }`: ширина по видимому контуру, и отступы MD3 (кнопки,
  чипы, группы) отмеряются от него. Иначе узкие глифы (logout, refresh) дают лишние 2–4 px слева и в зазоре.
- `ListView` с `header` не сдвигает `contentY`, когда шапка меняет высоту, — см. `keepTop()` в `TrackListPage`.
- Шрифт иконок `assets/fonts/MaterialSymbolsRounded.ttf` — полный вариативный (15 МБ); перед упаковкой урезать через `pyftsubset`.
