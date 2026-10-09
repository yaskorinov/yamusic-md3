import QtQuick
import Md3
import YaMusic.Core

// Кодек играющего трека (FLAC, AAC 256…) — маленькая плашка рядом со временем.
Rectangle {
    visible: Player.hasTrack && Player.codec !== ""
    implicitWidth: codec.implicitWidth + 10
    implicitHeight: 18
    radius: 5
    color: Theme.surfaceContainerHigh

    Label {
        id: codec
        anchors.centerIn: parent
        text: Player.codec
        type: "labelSmall"
        weight: 600
        color: Theme.fgSurfaceVariant
    }
}
