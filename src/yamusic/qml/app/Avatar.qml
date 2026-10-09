import QtQuick
import Md3

// Аватар-«печенька» с инициалами.
MorphShape {
    id: root
    property string name
    property real size: 40

    width: size
    height: size
    implicitWidth: size
    implicitHeight: size
    shape: "cookie9"
    color: Theme.tertiaryContainer

    readonly property string initials: name.split(/\s+/).filter(p => p.length).slice(0, 2).map(p => p[0].toUpperCase()).join("")

    Label {
        anchors.centerIn: parent
        visible: root.initials !== ""
        text: root.initials
        type: root.size >= 64 ? "headlineSmall" : "titleSmall"
        weight: 600
        color: Theme.fgTertiaryContainer
    }
    Icon {
        anchors.centerIn: parent
        visible: root.initials === ""
        name: "person"
        size: root.size * 0.55
        fill: 1
        color: Theme.fgTertiaryContainer
    }
}
