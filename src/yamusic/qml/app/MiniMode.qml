import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Мини-плеер на всё окно (окно сжато меньше обычной раскладки). Высокое окно — «карточка»
// (обложка над названием или слева от него, если окно широкое), низкое — «полоска» в одну строку.
// Фон тянет окно, двойной клик — развернуть обратно.
Item {
    id: root

    required property Window window
    signal expandRequested()

    readonly property Item coverItem: strip ? stripCover : cardCover
    readonly property bool strip: height < 200
    // Широкая невысокая карточка: обложка слева, остальное справа
    readonly property bool landscape: !strip && width > height * 1.35

    // Позиция приходит ~5 раз в секунду; между обновлениями — плавно (как в мини-плеере)
    property real progress: Player.position
    Behavior on progress {
        enabled: Player.playing && !cardSeek.dragging && !stripSeek.dragging && !Theme.calm
        NumberAnimation { duration: 220; easing.type: Easing.Linear }
    }

    function _time(ms) {
        const s = Math.max(0, Math.round(ms / 1000))
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0")
    }

    // ---- Фон: размытая обложка + вуаль цвета темы (как в полноэкранном плеере) ----
    Rectangle { anchors.fill: parent; color: Theme.surface }
    TrackBackdrop {
        anchors.fill: parent
        visible: Player.cover !== ""
        source: Player.cover
        mode: Settings.backdropMode
        blur: Settings.nowPlayingBlur
        flow: Settings.nowPlayingDrift
        followTheme: Settings.accentFromCover
        running: root.visible && Player.playing
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.surface
        opacity: Player.cover === "" ? 0 : Theme.dark ? 0.6 : 0.52
    }

    // Перетаскивание окна за фон (кнопки и полоса перемотки забирают нажатие сами)
    DragHandler {
        property bool manual: false
        target: null
        grabPermissions: PointerHandler.TakeOverForbidden
        onActiveChanged: if (active) manual = !root.window.startSystemMove()
        onCentroidChanged: if (active && manual && root.window.visibility === Window.Windowed) {
            root.window.x += centroid.scenePosition.x - centroid.scenePressPosition.x
            root.window.y += centroid.scenePosition.y - centroid.scenePressPosition.y
        }
    }
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onDoubleTapped: root.expandRequested()
    }
    TapHandler {
        id: menuTap
        // без трека меню тоже нужно: в нём перемешивание, повтор, «поверх всех окон», «развернуть»
        acceptedButtons: Qt.RightButton
        onTapped: point => {
            const p = root.mapToItem(null, point.position.x, point.position.y)
            Catalog.requestTrackMenu(Player.track, p.x, p.y)
        }
    }

    // ---- Общие куски ----
    component Cover: Item {
        id: cover
        property real size: 120
        implicitWidth: size
        implicitHeight: size
        MorphImage {
            id: img
            anchors.fill: parent
            visible: Player.cover !== ""
            source: Player.cover
            shape: Player.playing ? "cookie12" : "softSquare"
            MaskSpin { target: img; spinning: Settings.coverSpin && Player.playing }
        }
        MorphShape {
            anchors.fill: parent
            visible: Player.cover === ""
            shape: cover.size >= 96 ? "cookie9" : "softSquare"
            color: Theme.secondaryContainer
            Icon { anchors.centerIn: parent; name: "music_note"; size: Math.max(20, cover.size * 0.32); color: Theme.fgSecondaryContainer }
        }
    }

    component PlayControl: Item {
        id: pc
        property real size: 56
        implicitWidth: size
        implicitHeight: size
        PlayButton {
            anchors.fill: parent
            size: pc.size
            // без трека — запустить «Мою волну»
            playing: Player.hasTrack ? Player.playing : Wave.active && Player.playing
            enabled: Player.hasTrack || Auth.state === "signedIn"
            onClicked: Player.hasTrack ? Player.togglePlay() : Wave.play()
        }
        LoadingIndicator {
            anchors.centerIn: parent
            size: pc.size * 1.15
            color: Theme.primary
            visible: Player.buffering || Wave.loading
            opacity: 0.5
        }
    }

    // Закрепить поверх всех окон — общее состояние окна (его же переключает меню трека)
    component PinButton: IconButton {
        icon: "push_pin"
        size: "xs"
        checkable: true
        autoToggle: false
        checked: root.window.pinnedOnTop
        iconColor: checked ? Theme.fgSecondaryContainer : Theme.fgSurface
        onClicked: root.window.pinnedOnTop = !root.window.pinnedOnTop
    }

    component WindowButtons: Row {
        spacing: 2
        PinButton {}
        IconButton { icon: "remove"; size: "xs"; iconColor: Theme.fgSurface; onClicked: root.window.showMinimized() }
        IconButton { icon: "open_in_full"; size: "xs"; iconColor: Theme.fgSurface; onClicked: root.expandRequested() }
        IconButton { icon: "close"; size: "xs"; iconColor: Theme.fgSurface; onClicked: root.window.close() }
    }

    readonly property string titleText: Player.hasTrack ? Player.title : "Ничего не играет"
    readonly property string subtitleText: Player.errorText !== "" ? Player.errorText
        : Player.hasTrack ? Player.artist : Auth.state === "signedIn" ? "Запустите «Мою волну»" : "Войдите, чтобы слушать"

    // ---- Карточка ----
    Item {
        id: card
        anchors.fill: parent
        visible: !root.strip

        WindowButtons {
            id: cardButtons
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: 6
            z: 2
        }

        GridLayout {
            anchors.fill: parent
            anchors.margins: 16
            anchors.topMargin: root.landscape ? 16 : 40
            columns: root.landscape ? 2 : 1
            columnSpacing: 16
            rowSpacing: 10

            Cover {
                id: cardCover
                Layout.alignment: Qt.AlignHCenter | Qt.AlignVCenter
                // «карточка»: обложка забирает всё, что осталось над названием и кнопками
                size: root.landscape
                      ? Math.max(64, Math.min(card.height - 32, card.width * 0.42))
                      : Math.max(56, Math.min(card.width - 48, card.height - 40 - 16 - 172, 320))
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: root.landscape
                Layout.topMargin: root.landscape ? 24 : 0     // под кнопками окна
                spacing: 6

                Item { Layout.fillHeight: true; visible: root.landscape }

                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: root.landscape ? Text.AlignLeft : Text.AlignHCenter
                    text: root.titleText
                    type: card.width >= 340 ? "titleLarge" : "titleMedium"
                    weight: 600
                    elide: Text.ElideRight
                }
                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: root.landscape ? Text.AlignLeft : Text.AlignHCenter
                    text: root.subtitleText
                    type: "bodyMedium"
                    color: Player.errorText !== "" ? Theme.error : Theme.fgSurfaceVariant
                    elide: Text.ElideRight
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    visible: Player.hasTrack
                    spacing: 8
                    Label {
                        text: root._time(cardSeek.dragging ? cardSeek.dragValue * Player.durationMs : Player.positionMs)
                        type: "labelSmall"
                        color: Theme.fgSurfaceVariant
                    }
                    WavyProgress {
                        id: cardSeek
                        Layout.fillWidth: true
                        Layout.preferredHeight: 18
                        thickness: 4
                        amplitude: 2.5
                        wavelength: 30
                        value: root.progress
                        wavy: Player.playing
                        interactive: true
                        onCommitted: v => Player.seek(v)
                    }
                    Label {
                        text: root._time(Player.durationMs)
                        type: "labelSmall"
                        color: Theme.fgSurfaceVariant
                    }
                }

                RowLayout {
                    Layout.alignment: root.landscape ? Qt.AlignLeft : Qt.AlignHCenter
                    spacing: card.width >= 340 ? 8 : 2
                    IconButton {
                        visible: Player.source === "wave" && card.width >= 300
                        icon: "thumb_down"
                        iconColor: Theme.fgSurface
                        enabled: Player.hasTrack
                        onClicked: { Library.dislike(Player.track); Player.next() }
                    }
                    IconButton { icon: "skip_previous"; iconColor: Theme.fgSurface; enabled: Player.hasTrack; onClicked: Player.previous() }
                    PlayControl { size: card.width >= 340 ? 60 : 52 }
                    IconButton { icon: "skip_next"; iconColor: Theme.fgSurface; enabled: Player.hasTrack; onClicked: Player.next() }
                    LikeButton { track: Player.track; visible: card.width >= 260 }
                }

                Item { Layout.fillHeight: true; visible: root.landscape }
            }
        }
    }

    // ---- Полоска ----
    Item {
        id: stripBox
        anchors.fill: parent
        visible: root.strip

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 4
            anchors.topMargin: 6
            anchors.bottomMargin: 10
            spacing: 6

            Cover {
                id: stripCover
                Layout.alignment: Qt.AlignVCenter
                size: Math.max(40, Math.min(stripBox.height - 28, 72, stripBox.width * 0.2))
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 4
                Layout.alignment: Qt.AlignVCenter
                spacing: 0
                Label {
                    Layout.fillWidth: true
                    text: root.titleText
                    type: "titleSmall"
                    weight: 600
                    elide: Text.ElideRight
                }
                Label {
                    Layout.fillWidth: true
                    text: root.subtitleText
                    type: "bodySmall"
                    color: Player.errorText !== "" ? Theme.error : Theme.fgSurfaceVariant
                    elide: Text.ElideRight
                }
            }
            IconButton {
                visible: stripBox.width >= 360
                icon: "skip_previous"; iconColor: Theme.fgSurface; enabled: Player.hasTrack; onClicked: Player.previous()
            }
            PlayControl { size: 48 }
            IconButton {
                visible: stripBox.width >= 300
                icon: "skip_next"; iconColor: Theme.fgSurface; enabled: Player.hasTrack; onClicked: Player.next()
            }
            LikeButton { track: Player.track; visible: stripBox.width >= 420 }
            // кнопки окна — квадратом 2×2 у правого края (столбик из четырёх не влез бы в низкое окно)
            Grid {
                Layout.alignment: Qt.AlignVCenter
                columns: 2
                spacing: 0
                PinButton {}
                IconButton { icon: "close"; size: "xs"; iconColor: Theme.fgSurface; onClicked: root.window.close() }
                IconButton { icon: "remove"; size: "xs"; iconColor: Theme.fgSurface; onClicked: root.window.showMinimized() }
                IconButton { icon: "open_in_full"; size: "xs"; iconColor: Theme.fgSurface; onClicked: root.expandRequested() }
            }
        }

        // Перемотка — тонкой волной по нижнему краю
        WavyProgress {
            id: stripSeek
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            height: 12
            visible: Player.hasTrack
            thickness: 3
            amplitude: 1.5
            wavelength: 24
            value: root.progress
            wavy: Player.playing
            interactive: true
            onCommitted: v => Player.seek(v)
        }
    }
}
