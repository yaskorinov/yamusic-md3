import QtQuick
import QtQuick.Effects

// Тень уровня MD3 (0..5) под прямоугольной поверхностью target.
RectangularShadow {
    id: root

    required property Item target
    property int level: 1
    readonly property real _dp: [0, 1, 3, 6, 8, 12][Math.max(0, Math.min(5, level))]

    parent: target.parent
    anchors.fill: target
    z: target.z - 1
    radius: target.radius ?? 0
    offset.y: _dp * 0.6
    blur: _dp * 2.2
    spread: 0
    color: Qt.alpha(Theme.shadow, 0.35)
    visible: level > 0
}
