import QtQuick

// Пункт навигации сайдбара: пилюля во всю ширину; у выбранного — иконка в квадратике цвета primary.
// compact: true — режим рейла (только иконка по центру).
Item {
    id: root

    property string icon
    property string text
    property bool selected: false
    property bool compact: false
    property string trailingIcon
    property color trailingColor: Theme.primary
    property alias imageSource: thumb.source

    signal clicked()

    implicitHeight: 48
    implicitWidth: compact ? 56 : 240

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.selected ? Theme.secondaryContainer : "transparent"
        Behavior on color { enabled: !Theme.transitioning; ColorAnimation { duration: Theme.motion.effectsDefault } }
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: root.compact ? (root.width - 28) / 2 : 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8   // иконка 20 в боксе 28 → видимый зазор ~12
        Behavior on anchors.leftMargin { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutCubic } }

        Item {
            width: 28
            height: 28
            anchors.verticalCenter: parent.verticalCenter

            Rectangle {
                anchors.fill: parent
                radius: 8
                color: Theme.primary
                scale: root.selected && thumb.source == "" ? 1 : 0.4
                opacity: root.selected && thumb.source == "" ? 1 : 0
                Behavior on scale { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutBack; easing.overshoot: 2 } }
                Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
            }
            Icon {
                anchors.centerIn: parent
                visible: thumb.source == ""
                name: root.icon
                size: 20
                fill: root.selected ? 1 : 0
                color: root.selected ? Theme.fgPrimary : Theme.fgSurfaceVariant
            }
            MorphImage {
                id: thumb
                anchors.fill: parent
                visible: source != ""
                shape: root.selected ? "cookie9" : "softSquare"
            }
        }

        Label {
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            type: "labelLarge"
            color: root.selected ? Theme.fgSecondaryContainer : Theme.fgSurfaceVariant
            width: Math.max(0, root.width - 12 - 28 - 8 - (root.trailingIcon !== "" ? 40 : 16))
            opacity: root.compact ? 0 : 1
            visible: opacity > 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
        }
    }

    Icon {
        visible: root.trailingIcon !== "" && !root.compact
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        name: root.trailingIcon
        size: 18
        fill: 1
        color: root.trailingColor
    }

    StateLayer {
        anchors.fill: parent
        radius: height / 2
        color: Theme.fgSurface
        onClicked: root.clicked()
    }
}
