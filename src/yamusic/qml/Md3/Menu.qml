import QtQuick

// Всплывающее меню MD3: карточка со списком пунктов у точки (x, y). Клик мимо или Esc — закрыть.
// Кладётся поверх всего окна: Menu { anchors.fill: parent; z: … }; open([{ text, icon }], point).
Item {
    id: root

    property var items: []
    property bool opened: false

    signal picked(int index)

    function open(list, point) {
        items = list
        const w = card.implicitWidth, h = card.implicitHeight
        card.x = Math.max(8, Math.min(point.x, width - w - 8))
        card.y = point.y + h + 8 > height ? Math.max(8, point.y - h) : point.y
        opened = true
        card.forceActiveFocus()
    }
    function close() { opened = false }

    visible: opened

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: root.close()
        onWheel: wheel => root.close()
    }

    Rectangle {
        id: card
        implicitWidth: Math.max(200, list.implicitWidth + 16)
        implicitHeight: list.implicitHeight + 16
        radius: Theme.shape.large
        color: Theme.surfaceContainer
        transformOrigin: Item.TopLeft
        scale: root.opened ? 1 : 0.9
        opacity: root.opened ? 1 : 0
        Behavior on scale { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutBack } }
        Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
        Keys.onEscapePressed: root.close()

        Elevation { target: card; level: 2 }

        Column {
            id: list
            x: 8
            y: 8
            Repeater {
                model: root.items
                Item {
                    required property var modelData
                    required property int index
                    width: Math.max(184, row.implicitWidth + 32)
                    height: 48
                    Row {
                        id: row
                        x: 16
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 12
                        Icon { visible: (modelData.icon ?? "") !== ""; name: modelData.icon ?? ""; size: 22; color: Theme.fgSurfaceVariant; anchors.verticalCenter: parent.verticalCenter }
                        Label { text: modelData.text; type: "bodyLarge"; anchors.verticalCenter: parent.verticalCenter }
                    }
                    StateLayer {
                        anchors.fill: parent
                        radius: Theme.shape.medium
                        onClicked: { root.close(); root.picked(index) }
                    }
                }
            }
        }
    }
}
