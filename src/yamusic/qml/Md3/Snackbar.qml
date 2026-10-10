import QtQuick

// Короткое уведомление MD3 внизу по центру; само прячется. Кладётся поверх окна:
// Snackbar { id: snackbar; anchors.fill: parent; z: … }; snackbar.show("Ссылка скопирована").
Item {
    id: root

    property real bottomMargin: 112   // над мини-плеером
    property int duration: 2500
    property bool shown: false

    function show(text) {
        label.text = text
        shown = true
        hideTimer.restart()
    }
    function hide() { shown = false }

    Timer { id: hideTimer; interval: root.duration; onTriggered: root.hide() }

    Rectangle {
        id: pill
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.height - root.bottomMargin - height + (root.shown ? 0 : 16)
        width: Math.min(label.implicitWidth + 48, root.width - 32)
        height: 48
        radius: height / 2
        color: Theme.inverseSurface
        opacity: root.shown ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsDefault } }
        Behavior on y { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutCubic } }

        Elevation { target: pill; level: 3 }

        Label {
            id: label
            anchors.centerIn: parent
            width: Math.min(implicitWidth, parent.width - 48)
            elide: Text.ElideRight
            type: "bodyMedium"
            color: Theme.fgInverseSurface
        }
    }
}
