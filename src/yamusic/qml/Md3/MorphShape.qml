import QtQuick
import QtQuick.Shapes
import "Shapes.js" as Shapes

// Фигура MD3 Expressive. При смене `shape` плавно морфится из текущей формы в новую
// (в том числе из промежуточной, если предыдущий морф ещё не закончился).
// Корень — Item, а не Shape: Shape пересчитывает свой implicit-размер из пути, а путь зависит
// от размера — в Layout это петля привязок.
Item {
    id: root

    property var shape: "circle"
    property real angle: 0                 // поворот контура, градусы (Item.rotation не трогаем)
    property int samples: 120
    property color color: Theme.primary
    property color strokeColor: "transparent"
    property real strokeWidth: 0
    property int duration: Theme.motion.spatialDefault
    property real overshoot: 1.4
    property real livingSpeed: 1           // скорость «дыхания» blob-фигур, 0 — заморозить

    property var _from: shape
    property var _to: shape
    property real _t: 0
    property real _time: 0

    function morphTo(next) {
        const current = Shapes.blend(Shapes.radii(_from, samples, _time), Shapes.radii(_to, samples, _time), _t)
        _from = current
        _to = next
        _t = 0
        morph.restart()
    }

    onShapeChanged: morphTo(shape)

    implicitWidth: 48
    implicitHeight: 48

    NumberAnimation {
        id: morph
        target: root
        property: "_t"
        from: 0
        to: 1
        duration: root.duration
        easing.type: Easing.OutBack
        easing.overshoot: root.overshoot
    }

    FrameAnimation {
        running: root.visible && root.livingSpeed > 0 && (Shapes.isLiving(root._from) || Shapes.isLiving(root._to))
        onTriggered: root._time += frameTime * root.livingSpeed
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: root.color
            strokeColor: root.strokeColor
            strokeWidth: root.strokeWidth
            joinStyle: ShapePath.RoundJoin
            PathPolyline {
                path: Shapes.outline(root._from, root._to, root._t, root.samples, root.width, root.height,
                                     root.angle * Math.PI / 180, root._time, root.strokeWidth / 2)
            }
        }
    }
}
