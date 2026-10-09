import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Плавающий мини-плеер-пилюля (низ контент-панели).
// player — объект с полями: hasTrack, playing, title, artist, cover, position (0..1), volume.
Item {
    id: root

    required property var player
    signal openNowPlaying()

    implicitHeight: 72

    Rectangle {
        id: bg
        anchors.fill: parent
        radius: height / 2
        color: Theme.surfaceContainerHighest
    }
    Elevation { target: bg; level: 2 }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 16
        spacing: 4

        // Управление
        IconButton { icon: "shuffle"; checkable: true; enabled: root.player.hasTrack }
        IconButton { icon: "skip_previous"; iconColor: Theme.fgSurface; enabled: root.player.hasTrack }
        PlayButton {
            size: 52
            playing: root.player.playing
            onClicked: root.player.togglePlay()
        }
        IconButton { icon: "skip_next"; iconColor: Theme.fgSurface; enabled: root.player.hasTrack }
        IconButton { icon: "repeat"; checkable: true; enabled: root.player.hasTrack }

        // Трек
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12

            RowLayout {
                anchors.fill: parent
                spacing: 12

                Item {
                    Layout.preferredWidth: 48
                    Layout.preferredHeight: 48
                    MorphImage {
                        anchors.fill: parent
                        visible: root.player.cover !== ""
                        source: root.player.cover
                        shape: root.player.playing ? "cookie12" : "softSquare"
                    }
                    MorphShape {
                        anchors.fill: parent
                        visible: root.player.cover === ""
                        shape: "softSquare"
                        color: Theme.surfaceContainerHigh
                        Icon { anchors.centerIn: parent; name: "music_note"; size: 24; color: Theme.fgSurfaceVariant }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    Label {
                        Layout.fillWidth: true
                        text: root.player.hasTrack ? root.player.title : "Ничего не играет"
                        type: "titleSmall"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: root.player.hasTrack ? root.player.artist : "Включите Мою волну или выберите трек"
                        type: "bodySmall"
                        color: Theme.fgSurfaceVariant
                    }
                    WavyProgress {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 14
                        visible: root.player.hasTrack
                        thickness: 3
                        amplitude: 2
                        wavelength: 28
                        value: root.player.position
                        wavy: root.player.playing
                        interactive: true
                        onCommitted: v => root.player.seek(v)
                    }
                }
            }

            TapHandler { onTapped: root.openNowPlaying() }
        }

        // Панели и громкость
        IconButton {
            icon: "lyrics"
            checkable: true
            autoToggle: false
            checked: Settings.rightPanelOpen && Settings.rightPanelTab === "lyrics"
            onClicked: root._togglePanel("lyrics")
        }
        IconButton {
            icon: "queue_music"
            checkable: true
            autoToggle: false
            checked: Settings.rightPanelOpen && Settings.rightPanelTab === "queue"
            onClicked: root._togglePanel("queue")
        }
        Icon { Layout.leftMargin: 8; name: root.player.volume > 0.5 ? "volume_up" : root.player.volume > 0 ? "volume_down" : "volume_off"; size: 20 }
        Slider {
            Layout.preferredWidth: 96
            trackHeight: 10
            handleHeight: 28
            value: root.player.volume
            onMoved: v => root.player.volume = v
        }
    }

    function _togglePanel(tab) {
        if (Settings.rightPanelOpen && Settings.rightPanelTab === tab) {
            Settings.rightPanelOpen = false
        } else {
            Settings.rightPanelTab = tab
            Settings.rightPanelOpen = true
        }
    }
}
