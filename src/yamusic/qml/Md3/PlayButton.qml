import QtQuick

// Play/Pause в «печеньке» MD3 Expressive. Во время воспроизведения фигура медленно вращается,
// при нажатии сжимается в более круглую, при смене состояния морфится.
Item {
    id: root

    property bool playing: false
    property real size: 64
    property color containerColor: Theme.primary
    property color iconColor: Theme.fgPrimary
    property var playingShape: "cookie9"
    property var pausedShape: "cookie4"
    property bool spin: true

    signal clicked()

    implicitWidth: size
    implicitHeight: size

    MorphShape {
        id: shape
        anchors.fill: parent
        anchors.margins: area.pressed ? root.size * 0.06 : 0
        Behavior on anchors.margins { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutBack } }
        shape: area.pressed ? "circle" : root.playing ? root.playingShape : root.pausedShape
        duration: Theme.motion.spatialDefault
        color: root.containerColor

        FrameAnimation {
            running: root.spin && root.playing && root.visible
            onTriggered: shape.angle = (shape.angle + frameTime * 30) % 360
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: root.iconColor
        opacity: area.containsMouse && !area.pressed ? Theme.stateLayer.hover : 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
    }

    Icon {
        anchors.centerIn: parent
        name: root.playing ? "pause" : "play_arrow"
        size: root.size * 0.45
        fill: 1
        color: root.iconColor
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
