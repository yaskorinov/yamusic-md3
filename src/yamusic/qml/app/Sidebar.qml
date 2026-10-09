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
            visible: !root.compact && Auth.state !== "signedIn"
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
        LoadingIndicator {
            visible: !root.compact && Library.playlists.loading && Library.playlists.count === 0
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 8
            size: 36
        }

        // Плейлисты пользователя: своя прокрутка, если их много
        ListView {
            id: playlistList
            visible: Auth.state === "signedIn" && count > 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.topMargin: 4
            clip: true
            spacing: 2
            boundsBehavior: Flickable.StopAtBounds
            acceptedButtons: Qt.NoButton
            model: Library.playlists
            delegate: NavItem {
                required property var model
                width: ListView.view.width
                compact: root.compact
                text: model.title
                icon: "queue_music"
                imageSource: model.cover
                selected: root.router.currentName === "playlist" && root.router.current.props.kind === model.kind
                          && root.router.current.props.uid === model.uid
                onClicked: root.router.reset("playlist", { uid: model.uid, kind: model.kind, playlistTitle: model.title, cover: model.cover })
            }
            SmoothScroll { flickable: playlistList; wheelStep: Settings.wheelStep; showScrollBar: false }
        }

        Item { Layout.fillHeight: true; visible: !playlistList.visible }

        NavItem {
            Layout.fillWidth: true
            compact: root.compact
            icon: "settings"
            text: "Настройки"
            selected: root.router.rootName === "settings"
            onClicked: root.router.reset("settings")
        }

        // Карточка аккаунта
        Item {
            Layout.fillWidth: true
            Layout.topMargin: 8
            Layout.preferredHeight: root.compact ? 56 : 64

            Rectangle {
                anchors.fill: parent
                radius: root.compact ? height / 2 : Theme.shape.large
                color: root.router.rootName === "account" ? Theme.secondaryContainer : Theme.surfaceContainer
                Behavior on color { enabled: !Theme.transitioning; ColorAnimation { duration: Theme.motion.effectsDefault } }
            }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: root.compact ? 8 : 12
                anchors.rightMargin: 8
                spacing: 12
                Avatar {
                    size: 40
                    name: Auth.state === "signedIn" ? (Auth.account.displayName ?? "") : ""
                }
                ColumnLayout {
                    visible: !root.compact
                    Layout.fillWidth: true
                    spacing: 0
                    Label {
                        Layout.fillWidth: true
                        text: Auth.state === "signedIn" ? (Auth.account.displayName ?? "") : "Войти"
                        type: "titleSmall"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: Auth.state === "signedIn" ? (Auth.account.hasPlus ? "Плюс" : "Без Плюса")
                            : Auth.state === "awaitingUser" ? "Ждём подтверждения…"
                            : Auth.state === "checking" || Auth.state === "signingIn" ? "Входим…"
                            : "Аккаунт Яндекса"
                        type: "bodySmall"
                        color: Theme.fgSurfaceVariant
                    }
                }
            }

            StateLayer {
                anchors.fill: parent
                radius: root.compact ? height / 2 : Theme.shape.large
                onClicked: root.router.reset("account")
            }
        }
    }
}
