import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Плавающий мини-плеер-пилюля (низ контент-панели).
Item {
    id: root

    readonly property Item coverItem: coverBox
    signal openNowPlaying(point coverCenter)   // центр обложки в координатах окна — отсюда растёт блоб

    implicitHeight: 72

    // Позиция приходит из Python ~5 раз в секунду; между обновлениями — плавно.
    property real progress: Player.position
    Behavior on progress {
        enabled: Player.playing && !seekBar.dragging && !Theme.calm
        NumberAnimation { duration: 220; easing.type: Easing.Linear }
    }

    function _time(ms) {
        const s = Math.max(0, Math.round(ms / 1000))
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0")
    }

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
        IconButton {
            icon: "shuffle"
            checkable: true
            autoToggle: false
            checked: Player.shuffle
            enabled: Player.hasTrack && Player.source !== "wave"
            onClicked: Player.shuffle = !Player.shuffle
        }
        IconButton { icon: "skip_previous"; iconColor: Theme.fgSurface; enabled: Player.hasTrack; onClicked: Player.previous() }
        Item {
            implicitWidth: 52
            implicitHeight: 52
            PlayButton {
                anchors.fill: parent
                size: 52
                playing: Player.playing
                enabled: Player.hasTrack
                onClicked: Player.togglePlay()
            }
            LoadingIndicator {
                anchors.centerIn: parent
                size: 60
                color: Theme.primary
                visible: Player.buffering
                opacity: 0.5
            }
        }
        IconButton { icon: "skip_next"; iconColor: Theme.fgSurface; enabled: Player.hasTrack; onClicked: Player.next() }
        IconButton {
            icon: Player.repeat === "one" ? "repeat_one" : "repeat"
            checkable: true
            autoToggle: false
            checked: Player.repeat !== "off"
            enabled: Player.hasTrack
            onClicked: Player.cycleRepeat()
        }

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
                    id: coverBox
                    Layout.preferredWidth: 48
                    Layout.preferredHeight: 48
                    MorphImage {
                        id: miniCover
                        anchors.fill: parent
                        visible: Player.cover !== ""
                        source: Player.cover
                        shape: Player.playing ? "cookie12" : "softSquare"
                        MaskSpin { target: miniCover; spinning: Settings.coverSpin && Player.playing }
                    }
                    MorphShape {
                        anchors.fill: parent
                        visible: Player.cover === ""
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
                        text: Player.hasTrack ? Player.title : "Ничего не играет"
                        type: "titleSmall"
                    }
                    // Исполнитель · кодек (по правому краю — прямо над временем)
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        LinkLabel {
                            Layout.fillWidth: true
                            linkEnabled: Player.errorText === "" && Player.hasTrack
                            previewArtistId: (Player.track.artistRefs ?? []).length === 1 ? Player.track.artistRefs[0].id : ""
                            onClicked: Catalog.openArtists(Player.track.artistRefs ?? [])
                            text: Player.errorText !== "" ? Player.errorText
                                : Player.hasTrack ? Player.artist : "Выберите трек в «Мне нравится» или плейлисте"
                            type: "bodySmall"
                            color: Player.errorText !== "" ? Theme.error : Theme.fgSurfaceVariant
                        }
                        CodecBadge { Layout.alignment: Qt.AlignVCenter; Layout.bottomMargin: 3 }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        visible: Player.hasTrack
                        spacing: 8
                        WavyProgress {
                            id: seekBar
                            Layout.fillWidth: true
                            Layout.preferredHeight: 14
                            thickness: 3
                            amplitude: 2
                            wavelength: 28
                            value: root.progress
                            wavy: Player.playing
                            interactive: true
                            onCommitted: v => Player.seek(v)
                        }
                        Label {
                            text: root._time(seekBar.dragging ? seekBar.dragValue * Player.durationMs : Player.positionMs)
                                  + " / " + root._time(Player.durationMs)
                            type: "labelSmall"
                            color: Theme.fgSurfaceVariant
                        }
                    }
                }
            }

            TapHandler {
                enabled: Player.hasTrack
                onTapped: root.openNowPlaying(coverBox.mapToItem(null, coverBox.width / 2, coverBox.height / 2))
            }
        }

        // Оценка: «не рекомендовать» — только в волне (там она влияет на подбор), лайк — всегда
        IconButton {
            visible: Player.source === "wave"
            icon: "thumb_down"
            enabled: Player.hasTrack
            onClicked: { Library.dislike(Player.track); Player.next() }
        }
        LikeButton { track: Player.track }
        Item { implicitWidth: 8 }

        // Панели и громкость
        IconButton {
            icon: "title"
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
        Icon { Layout.leftMargin: 8; name: Player.volume > 0.5 ? "volume_up" : Player.volume > 0 ? "volume_down" : "volume_off"; size: 20 }
        Slider {
            Layout.preferredWidth: 96
            trackHeight: 10
            handleHeight: 28
            value: Player.volume
            onMoved: v => Player.volume = v
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
