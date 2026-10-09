import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import "app"

// Главное окно: сайдбар | контент (+ мини-плеер) | правая панель.
Window {
    id: win

    property string startPage: "home"     // --set startPage=settings (для разработки)

    width: Settings.windowWidth
    height: Settings.windowHeight
    minimumWidth: 720
    minimumHeight: 520
    visible: true
    title: "YaMusic"
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

    DemoPlayer { id: player }

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
                        home: Qt.resolvedUrl("app/pages/HomePage.qml"),
                        search: Qt.resolvedUrl("app/pages/SearchPage.qml"),
                        wave: Qt.resolvedUrl("app/pages/WavePage.qml"),
                        liked: Qt.resolvedUrl("app/pages/LikedPage.qml"),
                        settings: Qt.resolvedUrl("app/pages/SettingsPage.qml")
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
                player: player
                z: 3
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

    ResizeEdges { window: win }

    // Навигация мышью «назад» и горячие клавиши
    TapHandler {
        acceptedButtons: Qt.BackButton
        onTapped: router.back()
    }
    Shortcut { sequences: [StandardKey.Back, "Alt+Left"]; onActivated: router.back() }
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
