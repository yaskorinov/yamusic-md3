import QtQuick
import QtQuick.Effects
import Md3

// Размытая обложка во весь элемент: фон полноэкранного плеера и атмосферная подложка окна.
// blur (0..1) — в два приёма: обложка грузится уменьшенной (на максимуме 12×12 — билинейное
// растяжение само даёт мягкий градиент), MultiEffect доразмывает; края вынесены за элемент.
// flow (0..1) — «плавание»: картинка на месте, переливаются только размытые пятна (шейдер flow).
// Размытый слой считается один раз; каждый кадр — только выборка со смещением.
Item {
    id: root

    property string source
    property real blur: 1
    property real flow: 0.5
    property real saturation: 0.1 + 0.4 * (1 - blur)
    property bool running: visible

    readonly property bool ready: img.status === Image.Ready
    readonly property real _blur: Math.max(0, Math.min(1, blur))
    readonly property bool _flowing: flow > 0 && _blur > 0.2   // чёткую обложку «переливать» нечего
    property real _t: 0

    Image {
        id: img
        anchors.fill: parent
        source: root.source.replace("400x400", root._blur < 0.5 ? "800x800" : "400x400")
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        smooth: true
        visible: false
        sourceSize: {
            const side = Math.round(12 + 788 * Math.pow(1 - root._blur, 3))
            return Qt.size(side, side)
        }
    }

    Item {
        anchors.fill: parent
        visible: root.ready
        layer.enabled: root._flowing
        layer.effect: ShaderEffect {
            property real time: root._t
            property real amp: 0.08 * root._blur
            fragmentShader: Qt.resolvedUrl("shaders/flow.frag.qsb")
        }

        MultiEffect {
            anchors.fill: parent
            anchors.margins: root._blur > 0 ? -120 : 0
            source: img
            autoPaddingEnabled: false
            blurEnabled: root._blur > 0
            blurMax: 64
            blur: root._blur
            saturation: root.saturation
        }
    }

    FrameAnimation {
        running: root.running && root._flowing && root.ready && !Theme.calm
        onTriggered: root._t += frameTime * (0.15 + 0.85 * Math.min(1, root.flow))
    }
}
