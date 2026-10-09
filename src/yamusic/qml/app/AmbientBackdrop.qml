import QtQuick
import Md3

// Атмосферный фон с двойным буфером: новая обложка заранее грузится в скрытый слой, а меняется
// в тот же момент, что и цвета приложения (Theme.schemeApplied): под шторкой — мгновенно, и шторка
// открывает новый фон вместе с новыми цветами; при плавной смене схемы — плавным перекрёстным
// затуханием. Если тема от обложки не зависит (или схема не изменилась) — сам, с затуханием.
Item {
    id: root

    property string source
    property real flow: 0.5
    property bool running: true

    property int _front: 0
    property bool _animated: false
    property bool _waiting: false          // схема уже применена, а новая обложка ещё грузится
    readonly property var _layers: [a, b]
    readonly property var _back: _layers[1 - _front]

    onSourceChanged: {
        _back.source = source
        _waiting = false
        fallback.restart()
    }

    function _swap(animated) {
        const back = _back
        if (back.source !== source)
            return true                     // нечего менять
        if (!back.ready)
            return false
        _animated = animated
        _front = 1 - _front
        fallback.stop()
        _waiting = false
        return true
    }

    Connections {
        target: Theme
        function onSchemeApplied(instant) {
            if (!root._swap(!instant))
                root._waiting = true
        }
    }
    Connections {
        target: root._back
        function onReadyChanged() {
            // первая обложка (впереди ещё пусто) или схема уже сменилась — показываем
            if (root._back.ready && (root._waiting || root._layers[root._front].source === ""))
                root._swap(true)
        }
    }
    // Тема не поменялась (цвет из обложки выключен, или схема та же) — меняем сами
    Timer {
        id: fallback
        interval: 2500
        onTriggered: if (!root._swap(true)) root._waiting = true
    }

    component Layer: CoverBackdrop {
        property real shown: 0
        anchors.fill: parent
        blur: 1
        flow: root.flow
        opacity: shown
        visible: shown > 0 && ready
        running: root.running && visible
        Behavior on shown { enabled: root._animated; NumberAnimation { duration: 700; easing.type: Easing.InOutCubic } }
    }

    Layer { id: a; shown: root._front === 0 ? 1 : 0 }
    Layer { id: b; shown: root._front === 1 ? 1 : 0 }
}
