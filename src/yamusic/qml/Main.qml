import QtQuick
import QtQuick.Layouts
import QtQml.Models
import Qt.labs.platform as Platform
import Md3
import YaMusic.Core
import "app"

// Главное окно: сайдбар | контент (+ мини-плеер) | правая панель.
Window {
    id: win

    property string startPage: "wave"     // --set startPage=settings (для разработки)

    width: Settings.windowWidth
    height: Settings.windowHeight
    // Меньше обычной раскладки окно уходит в «компактную» (сайдбар — шторкой), ещё меньше — в мини-плеер
    minimumWidth: 280
    minimumHeight: 120
    visible: true
    title: Player.hasTrack ? Player.title + " — " + Player.artist + " · YaMusic" : "YaMusic"
    // «Поверх всех окон» (кнопка мини-плеера, меню трека); не запоминается между запусками
    property bool pinnedOnTop: false
    flags: Qt.Window | Qt.FramelessWindowHint | (pinnedOnTop ? Qt.WindowStaysOnTopHint : 0)
    color: Theme.surface

    // Раскладка по размеру окна. Порог с гистерезисом: вход в режим — у порога, выход — на 24 px дальше,
    // поэтому у границы режим не дёргается туда-обратно. Сжатие переключает сразу (старая раскладка уже
    // не помещается), расширение — когда размер постоит 150 мс, а не на каждом шаге перетаскивания.
    property string layoutMode: _modeFor(width, height, "")
    readonly property bool mini: layoutMode === "mini"
    readonly property var _modeRank: ({ mini: 0, compact: 1, full: 2 })
    function _modeFor(w, h, cur) {
        const band = 24
        const miniNow = cur === "mini" ? (w < 480 + band || h < 400 + band) : (w < 480 || h < 400)
        if (miniNow)
            return "mini"
        return (cur === "full" || cur === "") ? (w < 720 ? "compact" : "full") : (w < 720 + band ? "compact" : "full")
    }
    function _reevaluateMode() {
        const next = _modeFor(width, height, layoutMode)
        if (next === layoutMode)
            modeSettle.stop()
        else if (_modeRank[next] < _modeRank[layoutMode]) {
            modeSettle.stop()
            layoutMode = next
        } else
            modeSettle.restart()
    }
    Component.onCompleted: layoutMode = _modeFor(width, height, "")   // разорвать привязку: дальше — только _reevaluateMode
    onWidthChanged: _reevaluateMode()
    onHeightChanged: _reevaluateMode()
    Timer {
        id: modeSettle
        interval: 150
        onTriggered: win.layoutMode = win._modeFor(win.width, win.height, win.layoutMode)
    }

    // 0 — обычная сцена, 1 — мини-плеер; между ними — перекрёстное растворение с лёгким масштабом
    property real miniProgress: mini ? 1 : 0
    Behavior on miniProgress {
        NumberAnimation { duration: 320; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized }
    }
    property bool _miniUsed: false
    onMiniChanged: {
        if (mini) {
            _miniUsed = true
            if (nowPlaying.open)
                nowPlaying.hide()
        }
    }

    readonly property bool panelFits: width >= 1100
    readonly property bool panelShown: Settings.rightPanelOpen && panelFits && layoutMode === "full"
    // Открытая правая панель забирает 340 px: в окне уже 1400 боковая панель сворачивается в рейку,
    // иначе контенту остаётся меньше, чем в самом узком окне, и мини-плеер с «орбитой» в него не влезают
    readonly property bool compactSidebar: Settings.sidebarCollapsed || width < 980 || (panelShown && width < 1400)

    // Закрытие окна прячет его в трей (если это включено и трей в системе есть) — музыка играет дальше.
    // Выход — из меню значка или Ctrl+Q.
    readonly property bool trayWanted: typeof yamusicTray !== "undefined" && yamusicTray && Settings.closeToTray
    readonly property bool trayReady: tray.object !== null && tray.object.available

    // Из мини-плеера — обратно к обычному размеру (последний сохранённый, но не меньше обычной раскладки)
    function expandFromMini() {
        if (visibility !== Window.Windowed)
            visibility = Window.Windowed
        const w = Math.max(Settings.windowWidth, 960), h = Math.max(Settings.windowHeight, 640)
        const s = screen
        x = Math.max(s.virtualX, Math.min(x, s.virtualX + s.width - w))
        y = Math.max(s.virtualY, Math.min(y, s.virtualY + s.height - h))
        width = w
        height = h
    }

    function showWindow() {
        show()
        raise()
        requestActivate()
    }

    onClosing: close => {
        // размер мини-плеера не запоминаем: следующий запуск — в обычной раскладке
        if (visibility === Window.Windowed && !mini) {
            Settings.windowWidth = width
            Settings.windowHeight = height
        }
        if (trayWanted && trayReady) {
            close.accepted = false
            hide()
        } else {
            Qt.quit()
        }
    }

    Instantiator {
        id: tray
        active: win.trayWanted
        delegate: Platform.SystemTrayIcon {
            visible: true
            icon.source: Qt.resolvedUrl("../assets/yamusic.svg")
            tooltip: Player.hasTrack ? Player.title + " — " + Player.artist : "YaMusic"
            onActivated: reason => {
                if (reason === Platform.SystemTrayIcon.MiddleClick)
                    Player.togglePlay()
                else if (reason === Platform.SystemTrayIcon.Context) {
                    // на Windows меню трея — виджетное, к значку Qt его не привязывает: открываем сами у курсора
                    if (Qt.platform.os === "windows")
                        trayMenu.open()
                } else
                    win.visible ? win.hide() : win.showWindow()
            }
            menu: Platform.Menu {
                id: trayMenu
                Platform.MenuItem {
                    text: win.visible ? "Скрыть окно" : "Показать окно"
                    onTriggered: win.visible ? win.hide() : win.showWindow()
                }
                Platform.MenuSeparator {}
                Platform.MenuItem {
                    text: Player.playing ? "Пауза" : "Играть"
                    enabled: Player.hasTrack
                    onTriggered: Player.togglePlay()
                }
                Platform.MenuItem { text: "Следующий трек"; enabled: Player.hasTrack; onTriggered: Player.next() }
                Platform.MenuItem { text: "Предыдущий трек"; enabled: Player.hasTrack; onTriggered: Player.previous() }
                Platform.MenuSeparator {}
                Platform.MenuItem { text: "Выход"; onTriggered: Qt.quit() }
            }
        }
    }

    // Всё содержимое окна — источник снимка для шторки смены темы (ThemeWipe поверх, вне его)
    Item {
        id: scene
        anchors.fill: parent
        // в мини-плеере сцена не выгружается (страницы и прокрутка сохраняются), а только гаснет и не рисуется
        visible: win.miniProgress < 1
        enabled: !win.mini
        // уходящая раскладка гаснет быстрее, чем проявляется новая
        opacity: Math.max(0, 1 - win.miniProgress * 1.6)
        scale: 1 - 0.04 * win.miniProgress
        layer.enabled: win.miniProgress > 0 && win.miniProgress < 1

        Rectangle { anchors.fill: parent; color: Theme.surface }

        // Атмосферный фон: размытая обложка за всем окном. Сайдбар лежит прямо на ней,
        // панели контента полупрозрачны — там свечение слабее.
        TrackBackdrop {
            id: ambient
            anchors.fill: parent
            visible: Settings.ambientBackground && Player.cover !== ""
            opacity: Theme.dark ? 0.3 : 0.35
            source: Player.cover
            // зерно «стекла» под всем интерфейсом — лишнее
            mode: Settings.backdropMode === "glass" ? "gauss" : Settings.backdropMode
            flow: Settings.nowPlayingDrift
            followTheme: Settings.accentFromCover
            running: visible && Player.playing && !nowPlaying.open
        }

        RowLayout {
            anchors.fill: parent
            anchors.margins: 8
            anchors.leftMargin: 0
            spacing: 8

            Sidebar {
                id: sidebar
                Layout.fillHeight: true
                Layout.preferredWidth: implicitWidth
                Behavior on Layout.preferredWidth { NumberAnimation { duration: Theme.motion.spatialDefault; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
                router: router
                compact: win.compactSidebar
                drawer: win.layoutMode !== "full"
            }

            // Контент
            Surface {
                id: content
                Layout.fillWidth: true
                Layout.fillHeight: true
                level: "container"
                translucency: ambient.visible ? 0.3 : 0
                clip: true

                TopBar {
                    id: topBar
                    anchors.left: parent.left
                    anchors.right: parent.right
                    router: router
                    window: win
                    showMenuButton: win.layoutMode !== "full"
                    onMenuClicked: sidebar.openDrawer()
                    z: 2
                }

                Item {
                    id: pageHost
                    anchors.top: topBar.bottom
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom

                    Router {
                        id: router
                        pages: ({
                            search: Qt.resolvedUrl("app/pages/SearchPage.qml"),
                            wave: Qt.resolvedUrl("app/pages/WavePage.qml"),
                            liked: Qt.resolvedUrl("app/pages/LikedPage.qml"),
                            settings: Qt.resolvedUrl("app/pages/SettingsPage.qml"),
                            account: Qt.resolvedUrl("app/pages/AccountPage.qml"),
                            playlist: Qt.resolvedUrl("app/pages/PlaylistPage.qml"),
                            album: Qt.resolvedUrl("app/pages/AlbumPage.qml"),
                            artist: Qt.resolvedUrl("app/pages/ArtistPage.qml")
                        })
                        anchors.fill: parent
                        Component.onCompleted: reset(win.startPage)
                    }
                }

                MiniPlayer {
                    id: miniPlayer
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(parent.width - 32, 1080)
                    compact: win.layoutMode !== "full" || win.width < 860
                    z: 3
                    onOpenNowPlaying: center => nowPlaying.show(center)
                }
            }

            // Правая панель
            Item {
                Layout.fillHeight: true
                Layout.preferredWidth: win.panelShown ? 340 : 0
                Layout.leftMargin: win.panelShown ? 0 : -8   // убирает зазор RowLayout у закрытой панели
                Behavior on Layout.preferredWidth { NumberAnimation { duration: Theme.motion.spatialDefault; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
                Behavior on Layout.leftMargin { NumberAnimation { duration: Theme.motion.spatialDefault; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
                clip: true

                RightPanel {
                    width: 340
                    height: parent.height
                    opacity: win.panelShown ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsDefault } }
                }
            }
        }

        NowPlaying {
            id: nowPlaying
            anchors.fill: parent
            compact: win.layoutMode !== "full"
            z: 100
        }
    }

    Loader {
        id: miniMode
        anchors.fill: parent
        // однажды созданный мини-плеер остаётся (без пересоздания при каждом сжатии), невидимый — не рисуется
        active: win.mini || win._miniUsed
        visible: win.miniProgress > 0
        enabled: win.mini
        opacity: Math.max(0, (win.miniProgress - 0.25) / 0.75)
        scale: 0.96 + 0.04 * win.miniProgress
        layer.enabled: win.miniProgress > 0 && win.miniProgress < 1
        sourceComponent: MiniMode {
            window: win
            onExpandRequested: win.expandFromMini()
        }
    }

    // Смена цвета (и фона — он меняется под снимком) под новую обложку. Способ — из настроек;
    // направление: вперёд по очереди — слева направо, назад — справа налево; волна идёт от обложки
    ThemeWipe {
        id: themeWipe
        anchors.fill: parent
        source: win.mini && miniMode.item ? miniMode.item : scene
        mode: Settings.trackTransition
        originItem: win.mini && miniMode.item ? miniMode.item.coverItem
                  : nowPlaying.open ? nowPlaying.coverItem : miniPlayer.coverItem
        z: 1000
        Component.onCompleted: Theme.wipe = themeWipe
    }
    Binding { target: Theme; property: "wipeDirection"; value: Player.direction }
    // Окно не в фокусе (или свёрнуто) — декоративные анимации стоят
    Binding { target: Theme; property: "calm"; value: Settings.calmWhenInactive && (!win.active || win.visibility === Window.Minimized) }

    // Переходы к исполнителю / альбому / плейлисту — откуда угодно (строки треков, плееры)
    Connections {
        target: Catalog
        function onOpenRequested(name, props) {
            if (win.mini)
                win.expandFromMini()
            artistMenu.close()
            artistPreview.hide()
            if (nowPlaying.open)
                nowPlaying.hide()
            router.push(name, props)
        }
        function onPreviewRequested(id, x, y, w) { artistPreview.show(id, x, y, w) }
        function onPreviewCancelled() { artistPreview.hideSoon() }
        function onChooseArtist(refs) {
            artistMenu.refs = refs
            artistMenu.open(refs.map(r => ({ text: r.name, icon: "person" })), cursor.point.position)
        }
        function onTrackMenuRequested(track, x, y) {
            artistPreview.hide()
            trackMenu.show(track, Qt.point(x, y))
        }
    }
    Connections {
        target: Library
        function onNotice(text) { snackbar.show(text) }
    }

    function trackMenuAction(id, track, item) {
        const seed = "track:" + track.trackId
        // очередь и настройки живут в обычной раскладке
        if (win.mini && (id === "queue" || id === "sound"))
            win.expandFromMini()
        if (id === "pin") {
            if (Library.isWavePinned(seed))
                Library.unpinWave(seed)
            else
                Library.pinWave(seed)
        } else if (id === "wave") {
            Wave.playStation(seed, track.title ?? "")
        } else if (id === "repeat") {
            Player.cycleRepeat()
        } else if (id === "queue") {
            if (nowPlaying.open)
                nowPlaying.hide()
            Settings.rightPanelTab = "queue"
            Settings.rightPanelOpen = true
        } else if (id === "sound") {
            if (nowPlaying.open)
                nowPlaying.hide()
            router.reset("settings")
        } else if (id === "playlist:new") {
            newPlaylistDialog.open(track)
        } else if (id.startsWith("playlist:")) {
            Library.addToPlaylist(track, Number(id.slice("playlist:".length)))
        } else if (id === "share:track") {
            Catalog.copyLink(Catalog.trackUrl(track))
            snackbar.show("Ссылка скопирована")
        } else if (id.startsWith("share:artist:")) {
            Catalog.copyLink("https://music.yandex.ru/artist/" + id.slice("share:artist:".length))
            snackbar.show("Ссылка скопирована")
        } else if (id === "goto:track") {
            Catalog.openAlbum(track.albumId, track.trackId)
        } else if (id === "goto:album") {
            Catalog.openAlbum(track.albumId)
        } else if (id === "goto:artist") {
            Catalog.openArtists(track.artistRefs ?? [])
        }
    }
    HoverHandler { id: cursor }
    ArtistPreview {
        id: artistPreview
        anchors.fill: parent
        z: 1900
    }
    Menu {
        id: artistMenu
        property var refs: []
        anchors.fill: parent
        z: 2000
        onPicked: i => Catalog.openArtist(refs[i].id)
    }
    TrackMenu {
        id: trackMenu
        z: 2000
        onTriggered: (id, track, item) => win.trackMenuAction(id, track, item)
    }
    NewPlaylistDialog {
        id: newPlaylistDialog
        z: 2100
    }
    Snackbar {
        id: snackbar
        anchors.fill: parent
        z: 2200
    }

    ResizeEdges { window: win }

    Loader {
        active: typeof yamusicDebugFps !== "undefined" && yamusicDebugFps
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 16
        z: 10000
        sourceComponent: FpsOverlay {}
    }

    // Навигация мышью «назад» и горячие клавиши
    TapHandler {
        acceptedButtons: Qt.BackButton
        onTapped: nowPlaying.open ? nowPlaying.hide() : router.back()
    }
    Shortcut { sequences: [StandardKey.Back, "Alt+Left"]; onActivated: nowPlaying.open ? nowPlaying.hide() : router.back() }

    // Плеер (пробел не перехватывается, пока фокус в поле ввода)
    Shortcut { sequence: "Space"; onActivated: Player.togglePlay() }
    Shortcut { sequences: ["Ctrl+Right", "Media Next"]; onActivated: Player.next() }
    Shortcut { sequences: ["Ctrl+Left", "Media Previous"]; onActivated: Player.previous() }
    Shortcut { sequence: "Ctrl+Up"; onActivated: Player.volume = Math.min(1, Player.volume + 0.05) }
    Shortcut { sequence: "Ctrl+Down"; onActivated: Player.volume = Math.max(0, Player.volume - 0.05) }
    Shortcut { sequence: "Shift+Right"; onActivated: Player.seekMs(Player.positionMs + 10000) }
    Shortcut { sequence: "Shift+Left"; onActivated: Player.seekMs(Player.positionMs - 10000) }
    Shortcut {
        sequence: "Ctrl+P"
        onActivated: nowPlaying.open ? nowPlaying.hide() : !win.mini && Player.hasTrack && nowPlaying.show(null)
    }
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: {
            // в списке треков — сначала поиск по нему; повторное нажатие — общий поиск
            if (!nowPlaying.open && router.currentItem && router.currentItem.findInPage && router.currentItem.findInPage())
                return
            if (nowPlaying.open)
                nowPlaying.hide()
            router.reset("search")
            Qt.callLater(() => router.currentItem && router.currentItem.focusSearch && router.currentItem.focusSearch())
        }
    }
    Shortcut { sequence: "Ctrl+,"; onActivated: router.reset("settings") }
    Shortcut { sequence: "Ctrl+B"; onActivated: Settings.sidebarCollapsed = !Settings.sidebarCollapsed }
    Shortcut { sequence: "Ctrl+L"; onActivated: { Settings.rightPanelTab = "lyrics"; Settings.rightPanelOpen = !Settings.rightPanelOpen } }
    Shortcut { sequences: [StandardKey.Quit]; onActivated: Qt.quit() }
    Shortcut { sequence: "F11"; onActivated: win.visibility = win.visibility === Window.FullScreen ? Window.Windowed : Window.FullScreen }
}
