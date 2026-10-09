import QtQuick
import QtQuick.Effects

// Картинка (обложка), обрезанная по фигуре MD3 с морфингом.
Item {
    id: root

    property alias source: image.source
    property alias status: image.status
    property alias shape: mask.shape
    property alias angle: mask.angle
    property alias duration: mask.duration
    property alias livingSpeed: mask.livingSpeed
    property color placeholderColor: Theme.surfaceContainerHighest

    // Подложка, пока картинка грузится (та же фигура).
    MorphShape {
        anchors.fill: parent
        shape: mask.shape
        angle: mask.angle
        duration: mask.duration
        color: root.placeholderColor
        visible: image.status !== Image.Ready
    }

    Image {
        id: image
        anchors.fill: parent
        visible: false
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(Math.ceil(root.width * Screen.devicePixelRatio), Math.ceil(root.height * Screen.devicePixelRatio))
    }

    MorphShape {
        id: mask
        anchors.fill: parent
        visible: false
        layer.enabled: true
        layer.smooth: true
        color: "white"
    }

    MultiEffect {
        anchors.fill: parent
        source: image
        visible: image.status === Image.Ready
        maskEnabled: true
        maskSource: mask
        maskThresholdMin: 0.5
        maskSpreadAtMin: 1.0
    }
}
