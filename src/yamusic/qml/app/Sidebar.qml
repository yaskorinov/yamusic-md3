import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Левая навигация. compact — рейл из иконок (узкое окно или свёрнуто вручную).
Item {
    id: root

    required property var router
    property bool compact: false

    readonly property var sections: [
        { name: "home", icon: "home", text: "Главная" },
        { name: "search", icon: "search", text: "Поиск" },
        { name: "wave", icon: "graphic_eq", text: "Моя волна" },
        { name: "liked", icon: "favorite", text: "Мне нравится" }
    ]

    implicitWidth: compact ? 80 : 264

    ColumnLayout {
        anchors.fill: parent
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        anchors.leftMargin: root.compact ? 12 : 8
        anchors.rightMargin: root.compact ? 12 : 8
        spacing: 2

        // Шапка: сворачивание + название
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 48
            Layout.bottomMargin: 8
            spacing: 4
            IconButton {
                Layout.leftMargin: root.compact ? 4 : 4
                icon: root.compact ? "menu" : "menu_open"
                onClicked: Settings.sidebarCollapsed = !Settings.sidebarCollapsed
            }
            Label {
                visible: !root.compact
                Layout.fillWidth: true
                text: "YaMusic"
                type: "titleLarge"
                weight: 600
            }
        }

        Repeater {
            model: root.sections
            NavItem {
                required property var modelData
                Layout.fillWidth: true
                compact: root.compact
                icon: modelData.icon
                text: modelData.text
                selected: root.router.rootName === modelData.name
                onClicked: root.router.reset(modelData.name)
            }
        }

        // Плейлисты
        RowLayout {
            visible: !root.compact
            Layout.fillWidth: true
            Layout.topMargin: 14
            Layout.leftMargin: 16
            Layout.rightMargin: 4
            spacing: 10
            Label { text: "Плейлисты"; type: "labelMedium"; color: Theme.fgSurfaceVariant }
            WavyDivider { Layout.fillWidth: true }
            IconButton { icon: "add"; size: "xs"; iconColor: Theme.primary; enabled: false }
        }
        Label {
            visible: !root.compact
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.topMargin: 4
            text: "Войдите, чтобы увидеть свои плейлисты"
            type: "bodySmall"
            color: Theme.fgSurfaceVariant
            wrapMode: Text.WordWrap
            elide: Text.ElideNone
            lineHeightMode: Text.ProportionalHeight
            lineHeight: 1.2
        }

        Item { Layout.fillHeight: true }

        NavItem {
            Layout.fillWidth: true
            compact: root.compact
            icon: "settings"
            text: "Настройки"
            selected: root.router.rootName === "settings"
            onClicked: root.router.reset("settings")
        }

        // Карточка аккаунта (вход — этап 3)
        Rectangle {
            Layout.fillWidth: true
            Layout.topMargin: 8
            Layout.preferredHeight: 64
            radius: root.compact ? height / 2 : Theme.shape.large
            color: Theme.surfaceContainer
            visible: !root.compact

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 8
                spacing: 12
                MorphShape {
                    Layout.preferredWidth: 40
                    Layout.preferredHeight: 40
                    shape: "cookie9"
                    color: Theme.tertiaryContainer
                    Icon { anchors.centerIn: parent; name: "person"; size: 22; fill: 1; color: Theme.fgTertiaryContainer }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    Label { Layout.fillWidth: true; text: "Гость"; type: "titleSmall" }
                    Label { Layout.fillWidth: true; text: "Вход не выполнен"; type: "bodySmall"; color: Theme.fgSurfaceVariant }
                }
            }
        }
    }
}
