import QtQuick
import QtQuick.Layouts
import Md3

// Плитка альбома / исполнителя / плейлиста: обложка в фигуре MD3E, название, подпись.
// При наведении фигура морфится, у исполнителей — круг.
Item {
    id: root

    property string cover
    property string title
    property string subtitle
    property bool round: false          // исполнитель
    property bool explicit: false
    property string placeholderIcon: "album"
    property real size: 168

    signal clicked()

    implicitWidth: size
    implicitHeight: col.implicitHeight

    ColumnLayout {
        id: col
        width: root.size
        spacing: 8

        Item {
            Layout.preferredWidth: root.size
            Layout.preferredHeight: root.size

            MorphImage {
                anchors.fill: parent
                visible: root.cover !== ""
                source: root.cover
                shape: area.containsMouse ? (root.round ? "cookie12" : "cookie9") : (root.round ? "circle" : "softSquare")
                duration: Theme.motion.spatialDefault
            }
            MorphShape {
                anchors.fill: parent
                visible: root.cover === ""
                shape: area.containsMouse ? "cookie9" : (root.round ? "circle" : "softSquare")
                color: Theme.surfaceContainerHigh
                Icon { anchors.centerIn: parent; name: root.placeholderIcon; size: root.size * 0.3; color: Theme.fgSurfaceVariant }
            }
        }

        RowLayout {
            Layout.preferredWidth: root.size
            spacing: 4
            Label {
                Layout.fillWidth: implicitWidth > width
                Layout.maximumWidth: implicitWidth
                Layout.alignment: root.round ? Qt.AlignHCenter : Qt.AlignLeft
                text: root.title
                type: "titleSmall"
                horizontalAlignment: root.round ? Text.AlignHCenter : Text.AlignLeft
            }
            Rectangle {
                visible: root.explicit
                implicitWidth: 16
                implicitHeight: 16
                radius: 4
                color: Theme.surfaceContainerHighest
                Label { anchors.centerIn: parent; text: "E"; type: "labelSmall"; color: Theme.fgSurfaceVariant }
            }
            Item { Layout.fillWidth: true; visible: !root.round }
        }
        Label {
            visible: root.subtitle !== ""
            Layout.preferredWidth: root.size
            Layout.topMargin: -6
            text: root.subtitle
            type: "bodySmall"
            color: Theme.fgSurfaceVariant
            horizontalAlignment: root.round ? Text.AlignHCenter : Text.AlignLeft
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
