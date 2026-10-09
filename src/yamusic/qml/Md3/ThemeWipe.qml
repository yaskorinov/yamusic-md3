import QtQuick

// Смена темы «шторкой»: снимок source со старыми цветами кладётся поверх, тема переключается
// мгновенно под ним, и снимок уезжает в сторону direction. Дешевле плавной анимации цветов:
// все привязки пересчитываются один раз, а не каждый кадр, дальше — один текстурный слой.
// Подключение: Theme.wipe = themeWipe (source — всё содержимое окна, сам ThemeWipe — вне его, поверх).
Item {
    id: root

    property Item source
    property int duration: 650
    readonly property bool running: _phase !== 0

    property int _phase: 0          // 0 — нет, 1 — снимаем кадр, 2 — шторка едет
    property int _frames: 0
    property real _dir: 1
    property var _apply: null

    // Запуск: apply() переключает тему; вызывается, когда снимок со старыми цветами уже готов.
    function start(direction, apply) {
        anim.stop()
        _dir = direction < 0 ? -1 : 1
        _apply = apply
        _frames = 0
        _phase = 1
        snap.scheduleUpdate()
    }

    ShaderEffectSource {
        id: snap
        sourceItem: root._phase !== 0 ? root.source : null
        live: false
        hideSource: false
        visible: false
    }

    // Видна и во время снимка (с shown: 0): текстура источника обновляется, только когда её кто-то рисует.
    ShaderEffect {
        anchors.fill: parent
        visible: root._phase !== 0
        property variant source: snap
        property real progress: 0
        property real dir: root._dir
        property real edge: 0.22
        property real skew: 0.12
        property real shown: root._phase === 2 ? 1 : 0
        fragmentShader: Qt.resolvedUrl("shaders/wipe.frag.qsb")

        NumberAnimation on progress {
            id: anim
            running: false
            from: 0
            to: 1
            duration: root.duration
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.2, 0.0, 0.0, 1.0, 1.0, 1.0]
            onFinished: root._phase = 0
        }
    }

    // Снимок рендерится в синхронизации ближайшего кадра; на втором тике он точно готов.
    FrameAnimation {
        running: root._phase === 1
        onTriggered: {
            if (++root._frames < 2)
                return
            root._phase = 2
            if (root._apply)
                root._apply()
            root._apply = null
            anim.restart()
        }
    }
}
