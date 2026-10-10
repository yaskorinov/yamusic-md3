import QtQuick
import QtQuick.Shapes

// Всплывающее меню MD3: карточка со списком пунктов у точки (x, y). Клик мимо, колесо или Esc — закрыть.
// Кладётся поверх всего окна: Menu { anchors.fill: parent; z: … }; open(items, point).
// Пункт: { id, text, icon, enabled (true), checked (undefined — без галочки), separator: true, submenu: [пункты] }.
// picked(index) — выбран пункт верхнего уровня (индекс в списке); triggered(id, item) — любой выбранный пункт.
Item {
    id: root

    property var items: []
    property bool opened: false
    property point _at: Qt.point(0, 0)
    property Item _subRow: null    // строка-родитель подменю (биндинг: раскладка могла ещё не пройти)

    signal picked(int index)
    signal triggered(string id, var item)

    function open(list, point) {
        closeSub()
        items = list
        main.currentIndex = -1
        _at = point
        opened = true
        main.forceActiveFocus()
    }
    function close() {
        closeSub()
        opened = false
    }
    function closeSub() {
        hoverTimer.stop()
        sub.parentIndex = -1
        sub.shown = false
    }

    function _usable(item) {
        return item && !item.separator && item.enabled !== false
    }

    function _openSub(index, focusFirst) {
        hoverTimer.stop()
        const item = items[index]
        if (!_usable(item) || !item.submenu || item.submenu.length === 0)
            return
        if (sub.parentIndex !== index) {
            sub.model = item.submenu
            sub.parentIndex = index
            sub.currentIndex = -1
            _subRow = main.rowAt(index)
        }
        sub.shown = true
        if (focusFirst) {
            sub.currentIndex = sub.step(-1, 1)
            sub.forceActiveFocus()
        }
    }

    function _activate(card, index) {
        const item = card.model[index]
        if (!_usable(item))
            return
        if (card.level === 0 && item.submenu && item.submenu.length > 0) {
            _openSub(index, true)
            return
        }
        close()
        if (card.level === 0)
            picked(index)
        triggered(item.id ?? "", item)
    }

    visible: opened

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: root.close()
        onWheel: wheel => root.close()
    }

    // Задержка открытия/закрытия подменю по наведению: диагональный путь мыши к подменю его не схлопывает
    Timer {
        id: hoverTimer
        property int index: -1
        interval: 160
        onTriggered: {
            const item = root.items[index]
            if (item && item.submenu && item.submenu.length > 0)
                root._openSub(index, false)
            else
                root.closeSub()
        }
    }

    component MenuCard: Rectangle {
        id: card

        property var model: []
        property int level: 0
        property bool shown: false
        property int currentIndex: -1
        property int parentIndex: -1   // для подменю: пункт-родитель
        readonly property bool hasIcons: (model || []).some(it => (it.icon ?? "") !== "")
        readonly property bool hasTrailing: (model || []).some(it => typeof it.checked === "boolean" || (it.submenu && it.submenu.length > 0))

        function rowAt(i) { return rows.itemAt(i) }
        // Следующий доступный пункт от from в направлении dir (пропуская разделители и выключенные)
        function step(from, dir) {
            const n = (model || []).length
            let i = from < 0 ? (dir > 0 ? -1 : n) : from
            for (let k = 0; k < n; ++k) {
                i = ((i + dir) % n + n) % n
                if (root._usable(model[i]))
                    return i
            }
            return -1
        }

        implicitWidth: list.implicitWidth + 16
        implicitHeight: list.implicitHeight + 16
        width: implicitWidth
        height: implicitHeight
        radius: Theme.shape.large
        color: Qt.alpha(Theme.surfaceContainer, 0.92)
        border.width: 1
        border.color: Qt.alpha(Theme.outlineVariant, 0.5)
        transformOrigin: Item.TopLeft
        visible: opacity > 0
        scale: shown ? 1 : 0.9
        opacity: shown ? 1 : 0
        Behavior on scale { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutBack } }
        Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }

        Keys.onPressed: event => {
            switch (event.key) {
            case Qt.Key_Down:
                currentIndex = step(currentIndex, 1)
                break
            case Qt.Key_Up:
                currentIndex = step(currentIndex, -1)
                break
            case Qt.Key_Right:
                if (level === 0 && currentIndex >= 0)
                    root._openSub(currentIndex, true)
                break
            case Qt.Key_Left:
                if (level === 1) {
                    root.closeSub()
                    main.forceActiveFocus()
                }
                break
            case Qt.Key_Return:
            case Qt.Key_Enter:
            case Qt.Key_Space:
                if (currentIndex >= 0)
                    root._activate(card, currentIndex)
                break
            case Qt.Key_Escape:
                if (level === 1) {
                    root.closeSub()
                    main.forceActiveFocus()
                } else {
                    root.close()
                }
                break
            default:
                return
            }
            event.accepted = true
        }

        Elevation { target: card; level: 2 }

        // Клики по самой карточке не должны закрывать меню через подложку
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons; onWheel: wheel => wheel.accepted = true }

        Column {
            id: list
            x: 8
            y: 8

            Repeater {
                id: rows
                model: card.model

                Item {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property bool isSeparator: modelData.separator === true
                    readonly property bool usable: root._usable(modelData)
                    readonly property bool hasSub: !isSeparator && card.level === 0 && !!modelData.submenu && modelData.submenu.length > 0
                    readonly property bool current: card.currentIndex === index
                    readonly property real contentLeft: 12 + (card.hasIcons ? 20 + 12 : 0)

                    width: isSeparator ? 1 : Math.max(204, contentLeft + label.implicitWidth + (card.hasTrailing ? 16 + 20 : 0) + 12)
                    height: isSeparator ? 9 : 40

                    Rectangle {
                        visible: row.isSeparator
                        x: 4
                        width: list.width - 8
                        height: 1
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.outlineVariant
                        opacity: 0.6
                    }

                    // Подсветка: текущий пункт с клавиатуры и открытый родитель подменю
                    Rectangle {
                        visible: !row.isSeparator
                        width: list.width
                        height: parent.height
                        radius: Theme.shape.medium
                        color: Theme.fgSurface
                        opacity: (row.current && !hit.containsMouse) || (row.hasSub && sub.shown && sub.parentIndex === row.index)
                                 ? Theme.stateLayer.focus : 0
                        Behavior on opacity { NumberAnimation { duration: Theme.motion.effectsFast } }
                    }

                    Item {
                        visible: !row.isSeparator
                        width: list.width
                        height: parent.height
                        opacity: row.usable ? 1 : Theme.stateLayer.disabledContent

                        Icon {
                            visible: (row.modelData.icon ?? "") !== ""
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            name: row.modelData.icon ?? ""
                            size: 20
                            color: Theme.fgSurfaceVariant
                        }
                        Label {
                            id: label
                            x: row.contentLeft
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.modelData.text ?? ""
                            type: "bodyMedium"
                            color: Theme.fgSurface
                        }
                        Icon {
                            visible: row.modelData.checked === true
                            x: parent.width - 12 - 20
                            anchors.verticalCenter: parent.verticalCenter
                            name: "check"
                            size: 20
                            color: Theme.primary
                        }
                        // Шеврон «›»: в урезанном шрифте иконок chevron_right нет
                        Shape {
                            visible: row.hasSub
                            x: parent.width - 12 - 20
                            width: 20
                            height: 20
                            anchors.verticalCenter: parent.verticalCenter
                            preferredRendererType: Shape.CurveRenderer
                            ShapePath {
                                strokeColor: Theme.fgSurfaceVariant
                                strokeWidth: 1.8
                                fillColor: "transparent"
                                capStyle: ShapePath.RoundCap
                                joinStyle: ShapePath.RoundJoin
                                startX: 8; startY: 5
                                PathLine { x: 13; y: 10 }
                                PathLine { x: 8; y: 15 }
                            }
                        }
                    }

                    StateLayer {
                        id: hit
                        visible: !row.isSeparator
                        enabled: row.usable
                        width: list.width
                        height: parent.height
                        radius: Theme.shape.medium
                        onContainsMouseChanged: {
                            if (!containsMouse)
                                return
                            card.currentIndex = row.index
                            if (card.level === 0) {
                                hoverTimer.index = row.index
                                hoverTimer.restart()
                                card.forceActiveFocus()
                            } else {
                                hoverTimer.stop()
                            }
                        }
                        onClicked: root._activate(card, row.index)
                    }
                }
            }
        }
    }

    // Позиции — биндингами: размер карточки известен только после раскладки пунктов
    MenuCard {
        id: main
        level: 0
        model: root.items
        shown: root.opened
        x: Math.max(8, Math.min(root._at.x, root.width - width - 8))
        y: root._at.y + height + 8 > root.height ? Math.max(8, root._at.y - height) : root._at.y
    }

    MenuCard {
        id: sub
        level: 1
        x: {
            const right = main.x + main.width - 4
            return right + width > root.width - 8 ? Math.max(8, main.x - width + 4) : right
        }
        y: Math.max(8, Math.min(main.y + (root._subRow ? root._subRow.y : 0), root.height - height - 8))
        HoverHandler { onHoveredChanged: if (hovered) hoverTimer.stop() }
    }
}
