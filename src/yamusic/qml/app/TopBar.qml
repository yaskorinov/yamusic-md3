import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Верхняя панель контента: назад, заголовок прокрученной страницы, кнопки окна (опционально).
// Пустое место тянет окно (startSystemMove), двойной клик — развернуть.
Item {
    id: root

    required property var router
    required property Window window

    implicitHeight: 64

    readonly property var page: router.currentItem
    readonly property bool showTitle: page && (page.scrolled || !page.showHeader) && (page.title ?? "") !== ""

    DragHandler {
        target: null
        grabPermissions: PointerHandler.TakeOverForbidden
        onActiveChanged: if (active) root.window.startSystemMove()
    }
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onDoubleTapped: root.window.visibility = root.window.visibility === Window.Maximized ? Window.Windowed : Window.Maximized
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        spacing: 8

        // Без «назад» кнопка схлопывается, и заголовок стоит у левого края
        Item {
            Layout.preferredWidth: back.enabled ? back.implicitWidth : 0
            // заголовок встаёт по левому краю содержимого страницы (отступ 32)
            Layout.rightMargin: back.enabled ? 0 : 8
            Layout.preferredHeight: back.implicitHeight
            Behavior on Layout.preferredWidth { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutCubic } }
            Behavior on Layout.rightMargin { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutCubic } }
            clip: true
            IconButton {
                id: back
                icon: "arrow_back"
                enabled: root.router.depth > 1
                opacity: enabled ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
                onClicked: root.router.back()
            }
        }

        Label {
            Layout.fillWidth: true
            text: root.page ? root.page.title : ""
            type: "titleLarge"
            weight: 500
            opacity: root.showTitle ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsDefault } }
        }

        Row {
            visible: Settings.windowButtons
            spacing: 4
            IconButton { icon: "remove"; size: "xs"; onClicked: root.window.showMinimized() }
            IconButton {
                icon: root.window.visibility === Window.Maximized ? "collapse_content" : "expand_content"
                size: "xs"
                onClicked: root.window.visibility = root.window.visibility === Window.Maximized ? Window.Windowed : Window.Maximized
            }
            IconButton { icon: "close"; size: "xs"; onClicked: root.window.close() }
        }
    }
}
