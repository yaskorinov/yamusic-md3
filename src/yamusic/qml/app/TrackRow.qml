import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Строка трека: номер · обложка · название/исполнители · альбом · лайк · длительность.
Item {
    id: root

    property int number: 0
    property string trackId
    property string title
    property string version
    property string artists
    property var artistRefs: []        // [{ id, name }] — для перехода к исполнителю
    property string albumId
    property string album
    property string cover
    property int durationMs: 0
    property bool explicit: false
    property bool available: true
    property bool liked: false
    property bool current: false       // играет сейчас (этап 4)
    property bool wide: width > 720

    signal activated()                 // клик — воспроизвести
    signal likeClicked()               // клик по сердцу

    implicitHeight: 64

    function _time(ms) {
        const s = Math.round(ms / 1000)
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0")
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.shape.large
        color: root.current ? Qt.alpha(Theme.primary, 0.12) : "transparent"
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 16
        spacing: 16
        opacity: root.available ? 1 : Theme.stateLayer.disabledContent

        Label {
            visible: root.number > 0
            Layout.preferredWidth: 28
            text: root.number
            type: "bodyMedium"
            color: Theme.fgSurfaceVariant
            horizontalAlignment: Text.AlignRight
        }

        MorphImage {
            Layout.preferredWidth: 48
            Layout.preferredHeight: 48
            source: root.cover
            shape: root.current ? "cookie9" : "softSquare"
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.preferredWidth: 3
            spacing: 2
            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                Label {
                    Layout.fillWidth: implicitWidth > width
                    Layout.maximumWidth: implicitWidth
                    text: root.title
                    type: "bodyLarge"
                    weight: 500
                    color: root.current ? Theme.primary : Theme.fgSurface
                }
                Label {
                    visible: root.version !== ""
                    Layout.fillWidth: implicitWidth > width
                    Layout.maximumWidth: implicitWidth
                    text: root.version
                    type: "bodyLarge"
                    color: Theme.fgSurfaceVariant
                }
                Rectangle {
                    visible: root.explicit
                    implicitWidth: 16
                    implicitHeight: 16
                    radius: 4
                    color: Theme.surfaceContainerHighest
                    Label { anchors.centerIn: parent; text: "E"; type: "labelSmall"; color: Theme.fgSurfaceVariant }
                }
                Item { Layout.fillWidth: true }
            }
            LinkLabel {
                Layout.fillWidth: true
                text: root.artists
                type: "bodyMedium"
                color: Theme.fgSurfaceVariant
                linkEnabled: (root.artistRefs ?? []).length > 0
                previewArtistId: (root.artistRefs ?? []).length === 1 ? root.artistRefs[0].id : ""
                onClicked: Catalog.openArtists(root.artistRefs)
            }
        }

        LinkLabel {
            visible: root.wide
            Layout.fillWidth: true
            Layout.preferredWidth: 2
            linkEnabled: root.albumId !== ""
            onClicked: Catalog.openAlbum(root.albumId)
            text: root.album
            type: "bodyMedium"
            color: Theme.fgSurfaceVariant
        }

        // Сердце: залитое у лайкнутых, контур — при наведении на строку
        Item {
            implicitWidth: 32
            implicitHeight: 32
            visible: root.available
            Icon {
                anchors.centerIn: parent
                name: "favorite"
                size: 20
                fill: root.liked ? 1 : 0
                color: root.liked ? Theme.primary : heartArea.containsMouse ? Theme.fgSurface : Theme.fgSurfaceVariant
                opacity: root.liked || rowHover.hovered ? 1 : 0
                scale: heartArea.pressed ? 0.85 : 1
                Behavior on scale { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutBack } }
            }
            StateLayer {
                id: heartArea
                anchors.fill: parent
                radius: width / 2
                visible: root.liked || rowHover.hovered
                onClicked: root.likeClicked()
            }
        }

        Label {
            Layout.preferredWidth: 44
            text: root._time(root.durationMs)
            type: "bodyMedium"
            color: Theme.fgSurfaceVariant
            horizontalAlignment: Text.AlignRight
        }
    }

    HoverHandler { id: rowHover }

    // Под содержимым: клики по сердцу достаются ему, по остальной строке — сюда
    StateLayer {
        z: -1
        anchors.fill: parent
        radius: Theme.shape.large
        onClicked: root.activated()
    }
}
