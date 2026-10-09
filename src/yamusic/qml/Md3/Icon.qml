import QtQuick

// Иконка Material Symbols Rounded по имени лигатуры: Icon { name: "play_arrow" }
Text {
    id: root

    property string name
    property real size: 24
    property real fill: 0          // 0..1, анимируемая ось FILL
    property real weight: 400      // 100..700
    property real grade: 0         // -25..200

    Behavior on fill { NumberAnimation { duration: Theme.motion.effectsDefault } }

    text: name
    color: Theme.fgSurfaceVariant
    width: size
    height: size
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
    textFormat: Text.PlainText
    renderType: Text.NativeRendering  // distance-field (QtRendering) дырявит заливку вариативного шрифта
    font.family: Theme.iconFamily
    font.pixelSize: size
    font.variableAxes: ({ "FILL": fill, "wght": weight, "GRAD": grade, "opsz": Math.max(20, Math.min(48, size)) })
}
