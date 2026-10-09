import QtQuick
import QtQuick.Layouts
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

    onClosing: {
        if (visibility === Window.Windowed) {
            Settings.windowWidth = width
            Settings.windowHeight = height
        }
    }

    // Всё содержимое окна — источник снимка для шторки смены темы (ThemeWipe поверх, вне его)
    Item {
        id: scene
        anchors.fill: parent

        Rectangle { anchors.fill: parent; color: Theme.surface }

        // Атмосферный фон: размытая обложка за всем окном. Сайдбар лежит прямо на ней,
        // панели контента полупрозрачны — там свечение слабее.
        AmbientBackdrop {
            id: ambient
            anchors.fill: parent
            visible: Settings.ambientBackground && Player.cover !== ""
            opacity: Theme.dark ? 0.3 : 0.35
            source: Player.cover
            flow: Settings.nowPlayingDrift
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

    // Смена цвета под новую обложку: шторка слева направо при переходе вперёд, справа налево — назад
    ThemeWipe {
        id: themeWipe
        anchors.fill: parent
        source: scene
        z: 1000
        Component.onCompleted: Theme.wipe = themeWipe
    }
    Binding { target: Theme; property: "wipeDirection"; value: Player.direction }

    // Переходы к исполнителю / альбому / плейлисту — откуда угодно (строки треков, плееры)
    Connections {
        target: Catalog
        function onOpenRequested(name, props) {
            artistMenu.close()
            if (nowPlaying.open)
                nowPlaying.hide()
            router.push(name, props)
        }
        function onChooseArtist(refs) {
            artistMenu.refs = refs
            artistMenu.open(refs.map(r => ({ text: r.name, icon: "person" })), cursor.point.position)
        }
    }
    HoverHandler { id: cursor }
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
            router.reset("search")
            Qt.callLater(() => router.currentItem && router.currentItem.focusSearch && router.currentItem.focusSearch())
        }
    }
    Shortcut { sequence: "Ctrl+,"; onActivated: router.reset("settings") }
    Shortcut { sequence: "Ctrl+B"; onActivated: Settings.sidebarCollapsed = !Settings.sidebarCollapsed }
    Shortcut { sequence: "Ctrl+L"; onActivated: { Settings.rightPanelTab = "lyrics"; Settings.rightPanelOpen = !Settings.rightPanelOpen } }
    Shortcut { sequences: [StandardKey.Quit]; onActivated: win.close() }
    Shortcut { sequence: "F11"; onActivated: win.visibility = win.visibility === Window.FullScreen ? Window.Windowed : Window.FullScreen }
}
