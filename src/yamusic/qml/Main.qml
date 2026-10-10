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
    minimumWidth: 720
    minimumHeight: 520
    visible: true
    title: Player.hasTrack ? Player.title + " — " + Player.artist + " · YaMusic" : "YaMusic"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.surface

    readonly property bool compactSidebar: Settings.sidebarCollapsed || width < 980
    readonly property bool panelFits: width >= 1100
    readonly property bool panelShown: Settings.rightPanelOpen && panelFits

    // Закрытие окна прячет его в трей (если это включено и трей в системе есть) — музыка играет дальше.
    // Выход — из меню значка или Ctrl+Q.
    readonly property bool trayWanted: typeof yamusicTray !== "undefined" && yamusicTray && Settings.closeToTray
    readonly property bool trayReady: tray.object !== null && tray.object.available

    function showWindow() {
        show()
        raise()
        requestActivate()
    }

    onClosing: close => {
        if (visibility === Window.Windowed) {
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
                else if (reason !== Platform.SystemTrayIcon.Context)
                    win.visible ? win.hide() : win.showWindow()
            }
            menu: Platform.Menu {
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
                Layout.fillHeight: true
                Layout.preferredWidth: implicitWidth
                Behavior on Layout.preferredWidth { NumberAnimation { duration: Theme.motion.spatialDefault; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
                router: router
                compact: win.compactSidebar
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
            z: 100
        }
    }

    // Смена цвета (и фона — он меняется под снимком) под новую обложку. Способ — из настроек;
    // направление: вперёд по очереди — слева направо, назад — справа налево; волна идёт от обложки
    ThemeWipe {
        id: themeWipe
        anchors.fill: parent
        source: scene
        mode: Settings.trackTransition
        originItem: nowPlaying.open ? nowPlaying.coverItem : miniPlayer.coverItem
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
        onActivated: nowPlaying.open ? nowPlaying.hide() : Player.hasTrack && nowPlaying.show(null)
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
