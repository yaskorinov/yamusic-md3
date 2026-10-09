import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Левая навигация. compact — рейл из иконок (узкое окно или свёрнуто вручную).
//
// Геометрия одна для обоих режимов: поля 12 px, все иконки (меню, разделы, обложки плейлистов, аватар)
// центрированы на оси x = 40 — это центр рейла шириной 80. При сворачивании анимируется только ширина
// (её ведёт Main.qml), подписи гаснут, высоты не меняются — поэтому ничего не прыгает.
Item {
    id: root

    required property var router
    property bool compact: false

    readonly property real expandedWidth: 264
    readonly property real railWidth: 80
    readonly property real margin: 12
    readonly property real iconInset: 14     // квадрат иконки 28 px: 12 + 14 + 14 = 40 — ось рейла
    readonly property real labelWidth: expandedWidth - 2 * margin - iconInset - 28 - 8 - 12

    readonly property var sections: [
        { name: "home", icon: "home", text: "Главная" },
        { name: "search", icon: "search", text: "Поиск" },
        { name: "wave", icon: "graphic_eq", text: "Моя волна" },
        { name: "liked", icon: "favorite", text: "Мне нравится" }
    ]

    implicitWidth: compact ? railWidth : expandedWidth
    clip: true

    component FadeLabel: Label {
        opacity: root.compact ? 0 : 1
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        anchors.leftMargin: root.margin
        anchors.rightMargin: root.margin
        spacing: 2

        // Шапка: кнопка меню на оси рейла + название
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 48
            Layout.bottomMargin: 8

            IconButton {
                x: 40 - root.margin - width / 2
                anchors.verticalCenter: parent.verticalCenter
                icon: root.compact ? "menu" : "menu_open"
                onClicked: Settings.sidebarCollapsed = !Settings.sidebarCollapsed
            }
            FadeLabel {
                x: root.iconInset + 28 + 8
                anchors.verticalCenter: parent.verticalCenter
                width: root.labelWidth
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
                iconInset: root.iconInset
                labelWidth: root.labelWidth
                icon: modelData.icon
                text: modelData.text
                selected: root.router.rootName === modelData.name
                onClicked: root.router.reset(modelData.name)
            }
        }

        // Заголовок секции: подпись гаснет, волнистый разделитель остаётся и в рейле
        Item {
            id: sectionHeader
            Layout.fillWidth: true
            Layout.topMargin: 12
            Layout.preferredHeight: 32

            FadeLabel {
                id: sectionLabel
                x: root.iconInset
                anchors.verticalCenter: parent.verticalCenter
                text: "Плейлисты"
                type: "labelMedium"
                color: Theme.fgSurfaceVariant
            }
            WavyDivider {
                x: root.compact ? root.iconInset : sectionLabel.x + sectionLabel.implicitWidth + 10
                width: parent.width - x - (root.compact ? root.iconInset : 40)
                anchors.verticalCenter: parent.verticalCenter
                Behavior on x { NumberAnimation { duration: Theme.motion.spatialDefault; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
            }
            IconButton {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                icon: "add"
                size: "xs"
                iconColor: Theme.primary
                enabled: false
                opacity: root.compact ? 0 : 1
                visible: opacity > 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
            }
        }

        FadeLabel {
            visible: opacity > 0 && Auth.state !== "signedIn"
            Layout.preferredWidth: root.labelWidth + 28 + 8
            Layout.leftMargin: root.iconInset
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
            visible: Library.playlists.loading && Library.playlists.count === 0
            Layout.leftMargin: 40 - root.margin - size / 2
            Layout.topMargin: 8
            size: 32
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
                iconInset: root.iconInset
                labelWidth: root.labelWidth
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

        // В рейле настройки — отдельным пунктом; в развёрнутом виде — кнопкой в карточке аккаунта
        NavItem {
            visible: root.compact
            Layout.fillWidth: true
            Layout.topMargin: 8
            compact: root.compact
            iconInset: root.iconInset
            labelWidth: root.labelWidth
            icon: "settings"
            text: "Настройки"
            selected: root.router.rootName === "settings"
            onClicked: root.router.reset("settings")
        }

        // Карточка аккаунта: аватар на оси рейла, одна высота в обоих режимах
        Item {
            Layout.fillWidth: true
            Layout.topMargin: 8
            Layout.preferredHeight: 64

            Rectangle {
                anchors.fill: parent
                radius: Theme.shape.large
                color: root.router.rootName === "account" ? Theme.secondaryContainer : Theme.surfaceContainer
                Behavior on color { enabled: !Theme.transitioning; ColorAnimation { duration: Theme.motion.effectsDefault } }
            }

            Avatar {
                id: avatar
                x: 40 - root.margin - size / 2
                anchors.verticalCenter: parent.verticalCenter
                size: 40
                name: Auth.state === "signedIn" ? (Auth.account.displayName ?? "") : ""
            }

            Column {
                x: avatar.x + avatar.width + 12
                anchors.verticalCenter: parent.verticalCenter
                width: root.expandedWidth - 2 * root.margin - x - 12 - settingsButton.width
                opacity: root.compact ? 0 : 1
                visible: opacity > 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
                Label {
                    width: parent.width
                    text: Auth.state === "signedIn" ? (Auth.account.displayName ?? "") : "Войти"
                    type: "titleSmall"
                }
                Label {
                    width: parent.width
                    text: Auth.state === "signedIn" ? (Auth.account.hasPlus ? "Плюс" : "Без Плюса")
                        : Auth.state === "awaitingUser" ? "Ждём подтверждения…"
                        : Auth.state === "checking" || Auth.state === "signingIn" ? "Входим…"
                        : "Аккаунт Яндекса"
                    type: "bodySmall"
                    color: Theme.fgSurfaceVariant
                }
            }

            StateLayer {
                anchors.fill: parent
                radius: Theme.shape.large
                onClicked: root.router.reset("account")
            }

            IconButton {
                id: settingsButton
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                icon: "settings"
                checkable: true
                autoToggle: false
                checked: root.router.rootName === "settings"
                opacity: root.compact ? 0 : 1
                visible: opacity > 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
                onClicked: root.router.reset("settings")
            }
        }
    }
}
