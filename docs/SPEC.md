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
3. **Авторизация + API-слой** (асинхронный, в отдельном потоке) + кэш обложек ← следующий.
4. Плеер (mpv) + очередь + MPRIS + мини-плеер + Now Playing.
5. Моя волна, «Мне нравится».
6. Плейлисты / альбомы / артисты, поиск.
7. Тексты, офлайн-кэш.
8. Трей, PKGBUILD.

## Структура

```
src/yamusic/
  __init__.py        точка входа (main)
  app.py             QGuiApplication, шрифты, регистрация типов, загрузка QML
  settings.py        Settings (QSettings → ~/.config/yamusic/YaMusic.conf), свойства из SCHEMA
  theme.py           ThemeEngine: схема MD3 из seed/обложки, следует настройкам
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
- Смена темы — одна анимация прогресса в `Theme` (OKLab), расчёт схемы — в пуле потоков (~40 мс Python).
  Компонентам нельзя иметь свою `Behavior on color` без `enabled: !Theme.transitioning` — иначе двойная анимация и рывки.
- В окружении пользователя `QSG_USE_SIMPLE_ANIMATION_DRIVER=1` — анимации от таймера ~60 Гц вместо vsync
  (дёргается на 144 Гц). `app.py` убирает переменную для своего процесса. Прокрутка — только через `SmoothFlickable`.
- В окружении пользователя `QT_LOGGING_RULES` глушит `console.log`; для отладки: `QT_LOGGING_RULES="qml.debug=true"`.
  Замер кадров при смене обложек: `--set perfLog=true`.
- Шрифт иконок `assets/fonts/MaterialSymbolsRounded.ttf` — полный вариативный (15 МБ); перед упаковкой урезать через `pyftsubset`.
