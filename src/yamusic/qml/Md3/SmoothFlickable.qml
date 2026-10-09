import QtQuick

// Flickable с плавной прокруткой для десктопа.
// Колесо мыши: щелчок сдвигает цель на wheelStep, contentY догоняет её экспоненциально
// (независимо от частоты кадров — одинаково на 60 и 144 Гц); быстрые щелчки накапливаются.
// Тачпад: пиксели 1:1 без задержки. Перетаскивание мышью отключено — только касания.
Flickable {
    id: root

    property real wheelStep: 120          // px за щелчок колеса
    property real smoothing: 14           // 1/с: больше — быстрее доводка
    property bool showScrollBar: true

    readonly property real minY: originY
    readonly property real maxY: originY + Math.max(0, contentHeight - height)
    property real _target: 0

    function clampY(y) { return Math.max(minY, Math.min(maxY, y)) }

    function scrollTo(y, animated) {
        _target = clampY(y)
        if (animated === false) {
            smooth.running = false
            contentY = _target
        } else {
            smooth.running = true
        }
    }

    boundsBehavior: Flickable.StopAtBounds
    acceptedButtons: Qt.NoButton
    flickableDirection: Flickable.VerticalFlick
    clip: true

    WheelHandler {
        parent: root
        target: null
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Vertical
        onWheel: event => {
            const touchpad = event.device.type === PointerDevice.TouchPad || event.pixelDelta.y !== 0
            if (touchpad) {
                smooth.running = false
                root.contentY = root.clampY(root.contentY - event.pixelDelta.y)
                root._target = root.contentY
            } else {
                const base = smooth.running ? root._target : root.contentY
                root._target = root.clampY(base - event.angleDelta.y / 120 * root.wheelStep)
                smooth.running = true
            }
        }
    }

    FrameAnimation {
        id: smooth
        running: false
        onTriggered: {
            const target = root.clampY(root._target)
            const d = target - root.contentY
            if (Math.abs(d) < 0.5) {
                root.contentY = target
                running = false
                return
            }
            root.contentY += d * (1 - Math.exp(-frameTime * root.smoothing))
        }
    }

    // Тонкий индикатор прокрутки: проявляется при движении и наведении, тянется мышью.
    Item {
        id: bar
        parent: root
        visible: root.showScrollBar && root.contentHeight > root.height
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 4
        width: 12

        readonly property bool active: root.movingVertically || smooth.running || barArea.containsMouse || barArea.pressed
        opacity: active ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: bar.active ? Theme.motion.effectsFast : 600 } }

        readonly property real ratio: root.height / Math.max(root.contentHeight, 1)
        readonly property real thumbH: Math.max(32, height * ratio)
        readonly property real pos: root.maxY > root.minY ? (root.contentY - root.minY) / (root.maxY - root.minY) : 0

        Rectangle {
            readonly property bool wide: barArea.containsMouse || barArea.pressed
            anchors.right: parent.right
            width: wide ? 8 : 4
            radius: width / 2
            height: bar.thumbH
            y: (bar.height - height) * bar.pos
            color: Theme.fgSurfaceVariant
            opacity: wide ? 0.7 : 0.45
            Behavior on width { NumberAnimation { duration: Theme.motion.effectsFast } }
        }

        MouseArea {
            id: barArea
            anchors.fill: parent
            hoverEnabled: true
            preventStealing: true
            property real grabOffset: 0
            function yFor(mouseY) {
                const p = (mouseY - grabOffset) / Math.max(1, bar.height - bar.thumbH)
                return root.minY + Math.max(0, Math.min(1, p)) * (root.maxY - root.minY)
            }
            onPressed: mouse => {
                const thumbY = (bar.height - bar.thumbH) * bar.pos
                grabOffset = mouse.y >= thumbY && mouse.y <= thumbY + bar.thumbH ? mouse.y - thumbY : bar.thumbH / 2
                root.scrollTo(yFor(mouse.y), grabOffset === bar.thumbH / 2)
            }
            onPositionChanged: mouse => { if (pressed) root.scrollTo(yFor(mouse.y), false) }
        }
    }
}
