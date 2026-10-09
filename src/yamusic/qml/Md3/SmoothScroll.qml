import QtQuick

// Плавная прокрутка для любого Flickable/ListView: SmoothScroll { flickable: list }.
// Колесо мыши: щелчок сдвигает цель на wheelStep, contentY догоняет её экспоненциально
// (независимо от частоты кадров — одинаково на 60 и 144 Гц); быстрые щелчки накапливаются.
// Тачпад: пиксели 1:1 без задержки. Плюс тонкий индикатор прокрутки.
// Перетаскивание контента мышью стоит выключить у самого Flickable: acceptedButtons: Qt.NoButton.
Item {
    id: root

    required property Flickable flickable
    property real wheelStep: 220          // px за щелчок колеса
    property real smoothing: 14           // 1/с: больше — быстрее доводка
    property real maxBoost: 3             // быстрые щелчки подряд разгоняют шаг до wheelStep × maxBoost
    property real _boost: 1
    property real _lastWheel: 0
    property bool showScrollBar: true

    readonly property real minY: flickable.originY
    readonly property real maxY: flickable.originY + Math.max(0, flickable.contentHeight - flickable.height)
    property real _target: 0

    signal userScrolled()   // колесо, тачпад или индикатор — не программная прокрутка

    function clampY(y) { return Math.max(minY, Math.min(maxY, y)) }

    function scrollTo(y, animated) {
        _target = clampY(y)
        if (animated === false) {
            smooth.running = false
            flickable.contentY = _target
        } else {
            smooth.running = true
        }
    }

    WheelHandler {
        parent: root.flickable
        target: null
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Vertical
        onWheel: event => {
            root.userScrolled()
            const touchpad = event.device.type === PointerDevice.TouchPad || event.pixelDelta.y !== 0
            if (touchpad) {
                smooth.running = false
                root.flickable.contentY = root.clampY(root.flickable.contentY - event.pixelDelta.y)
                root._target = root.flickable.contentY
            } else {
                // Разгон: щелчки чаще чем раз в ~90 мс увеличивают шаг, пауза сбрасывает.
                const now = Date.now()
                const dt = now - root._lastWheel
                root._lastWheel = now
                root._boost = dt < 90 ? Math.min(root.maxBoost, root._boost * 1.35) : 1
                const base = smooth.running ? root._target : root.flickable.contentY
                root._target = root.clampY(base - event.angleDelta.y / 120 * root.wheelStep * root._boost)
                smooth.running = true
            }
        }
    }

    FrameAnimation {
        id: smooth
        running: false
        onTriggered: {
            const target = root.clampY(root._target)
            const d = target - root.flickable.contentY
            if (Math.abs(d) < 0.5) {
                root.flickable.contentY = target
                running = false
                return
            }
            root.flickable.contentY += d * (1 - Math.exp(-frameTime * root.smoothing))
        }
    }

    // Тонкий индикатор прокрутки: проявляется при движении и наведении, тянется мышью.
    Item {
        id: bar
        parent: root.flickable
        visible: root.showScrollBar && root.flickable.contentHeight > root.flickable.height
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 4
        width: 12

        readonly property bool active: root.flickable.movingVertically || smooth.running || barArea.containsMouse || barArea.pressed
        opacity: active ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: bar.active ? Theme.motion.effectsFast : 600 } }

        readonly property real ratio: root.flickable.height / Math.max(root.flickable.contentHeight, 1)
        readonly property real thumbH: Math.max(32, height * ratio)
        readonly property real pos: root.maxY > root.minY ? (root.flickable.contentY - root.minY) / (root.maxY - root.minY) : 0

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
                root.userScrolled()
                const thumbY = (bar.height - bar.thumbH) * bar.pos
                grabOffset = mouse.y >= thumbY && mouse.y <= thumbY + bar.thumbH ? mouse.y - thumbY : bar.thumbH / 2
                root.scrollTo(yFor(mouse.y), grabOffset === bar.thumbH / 2)
            }
            onPositionChanged: mouse => { if (pressed) root.scrollTo(yFor(mouse.y), false) }
        }
    }
}
