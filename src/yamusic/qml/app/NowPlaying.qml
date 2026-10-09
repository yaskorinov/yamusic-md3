import QtQuick
import QtQuick.Layouts
import QtQuick.Effects
import Md3
import YaMusic.Core

// Полноэкранный плеер. Раскрывается морфящимся блобом из обложки мини-плеера (origin) и
// схлопывается обратно. Маска (MultiEffect) включена только на время анимации.
Item {
    id: root

    property bool open: false
    property point origin: Qt.point(width / 2, height - 60)
    property string tab: "queue"           // queue | lyrics
    property real reveal: open ? 1 : 0
    readonly property bool animating: revealAnim.running
    readonly property real maxRadius: 1.25 * Math.max(
        Math.hypot(origin.x, origin.y), Math.hypot(width - origin.x, origin.y),
        Math.hypot(origin.x, height - origin.y), Math.hypot(width - origin.x, height - origin.y))

    function show(from) {
        if (from)
            origin = from
        blob.shape = "flower8"
        open = true
        blob.shape = "circle"
        forceActiveFocus()
    }
    function hide() {
        blob.shape = "flower6"
        open = false
    }

    visible: reveal > 0
    focus: open
    Keys.onEscapePressed: hide()

    Behavior on reveal {
        NumberAnimation {
            id: revealAnim
            duration: root.open ? 650 : 450
            easing.type: Easing.BezierSpline
            easing.bezierCurve: root.open ? Theme.motion.emphasizedDecelerate : Theme.motion.emphasizedAccelerate
        }
    }

    function _time(ms) {
        const s = Math.max(0, Math.round(ms / 1000))
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0")
    }

    // Маска раскрытия: блоб с центром в origin
    Item {
        id: revealMask
        anchors.fill: parent
        visible: false
        layer.enabled: root.animating || root.reveal < 1
        MorphShape {
            id: blob
            readonly property real r: 24 + (root.maxRadius - 24) * root.reveal
            x: root.origin.x - r
            y: root.origin.y - r
            width: 2 * r
            height: 2 * r
            duration: 650
            overshoot: 1.1
            color: "white"
            tinted: false
        }
    }

    // Содержимое (гаснет на последней трети сворачивания — без «обрубка» обложки в конце)
    Item {
        id: content
        anchors.fill: parent
        opacity: Math.min(1, root.reveal / 0.3)
        layer.enabled: root.reveal < 1
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: revealMask
            maskThresholdMin: 0.5
            maskSpreadAtMin: 1.0
        }

        // Фон: размытая обложка + вуаль цвета темы
        Rectangle { anchors.fill: parent; color: Theme.surface }
        // Размытие (Settings.nowPlayingBlur, 0..1) — в два приёма: обложка грузится уменьшенной
        // (на максимуме 12×12 — билинейное растяжение само даёт мягкий градиент), MultiEffect доразмывает.
        // Края вынесены за окно — у размытия они тёмные/прозрачные.
        Image {
            id: bgCover
            readonly property real blur: Math.max(0, Math.min(1, Settings.nowPlayingBlur))
            anchors.fill: parent
            source: Player.cover.replace("400x400", blur < 0.5 ? "800x800" : "400x400")
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            smooth: true
            visible: false
            sourceSize: {
                const side = Math.round(12 + 788 * Math.pow(1 - blur, 3))
                return Qt.size(side, side)
            }
        }
        MultiEffect {
            anchors.fill: parent
            anchors.margins: bgCover.blur > 0 ? -120 : 0
            source: bgCover
            visible: bgCover.status === Image.Ready
            autoPaddingEnabled: false
            blurEnabled: bgCover.blur > 0
            blurMax: 64
            blur: bgCover.blur
            saturation: 0.1 + 0.4 * (1 - bgCover.blur)
        }
        Rectangle {
            anchors.fill: parent
            color: Theme.surface
            opacity: Theme.dark ? 0.62 : 0.55
        }

        // Блокирует клики по приложению под плеером
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons; onWheel: wheel => wheel.accepted = true }

        // Верхняя панель: свернуть + переключатель очередь/текст (над правой колонкой)
        RowLayout {
            id: topRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 16
            spacing: 12
            IconButton { icon: "keyboard_arrow_down"; style: "tonal"; onClicked: root.hide() }
            Item { Layout.fillWidth: true }
            ButtonGroup {
                model: [{ icon: "title" }, { icon: "queue_music" }]
                autoSelect: false
                currentIndex: root.tab === "lyrics" ? 0 : 1
                onActivated: i => root.tab = i === 0 ? "lyrics" : "queue"
            }
        }

        RowLayout {
            anchors.top: topRow.bottom
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 32
            anchors.topMargin: 8
            spacing: 48

            // ---- Колонка плеера (слева, с отступом от края): обложка и управление ----
            ColumnLayout {
                id: left
                Layout.fillHeight: true
                Layout.leftMargin: Math.max(16, root.width * 0.06)
                Layout.preferredWidth: Math.min(460, root.width * 0.42)
                Layout.maximumWidth: Layout.preferredWidth
                spacing: 12

                Item { Layout.fillHeight: true }

                MorphImage {
                    // Всё остальное в колонке занимает ~360 px — обложка берёт оставшееся
                    readonly property real s: Math.max(160, Math.min(left.width, left.height - 360, 440))
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: s
                    Layout.preferredHeight: s
                    Layout.bottomMargin: 20
                    source: Player.cover.replace("400x400", "800x800")
                    shape: Player.playing ? "cookie12" : "square"
                    duration: Theme.motion.spatialSlow
                }

                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: Player.title
                    type: "headlineMedium"
                    weight: 700
                }
                Label {
                    Layout.fillWidth: true
                    Layout.topMargin: -6
                    horizontalAlignment: Text.AlignHCenter
                    text: Player.artist
                    type: "titleMedium"
                    color: Theme.fgSurfaceVariant
                }
                Label {
                    Layout.fillWidth: true
                    Layout.topMargin: -6
                    horizontalAlignment: Text.AlignHCenter
                    text: Player.album
                    type: "bodySmall"
                    color: Theme.fgSurfaceVariant
                }

                // Прогресс
                property real progress: Player.position
                Behavior on progress {
                    enabled: Player.playing && !bigSeek.dragging
                    NumberAnimation { duration: 220; easing.type: Easing.Linear }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    spacing: 12
                    Label {
                        text: root._time(bigSeek.dragging ? bigSeek.dragValue * Player.durationMs : Player.positionMs)
                        type: "labelMedium"
                        color: Theme.fgSurfaceVariant
                    }
                    WavyProgress {
                        id: bigSeek
                        Layout.fillWidth: true
                        thickness: 6
                        amplitude: 4
                        wavelength: 48
                        value: left.progress
                        wavy: Player.playing
                        interactive: true
                        onCommitted: v => Player.seek(v)
                    }
                    CodecBadge { Layout.rightMargin: -4 }
                    Label {
                        text: "−" + root._time(Player.durationMs - Player.positionMs)
                        type: "labelMedium"
                        color: Theme.fgSurfaceVariant
                    }
                }

                // Кнопки
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 4
                    spacing: 12
                    IconButton {
                        icon: "shuffle"; size: "m"
                        checkable: true; autoToggle: false; checked: Player.shuffle
                        enabled: Player.source !== "wave"
                        style: Player.shuffle ? "tonal" : "standard"
                        onClicked: Player.shuffle = !Player.shuffle
                    }
                    IconButton { icon: "skip_previous"; size: "m"; iconColor: Theme.fgSurface; onClicked: Player.previous() }
                    PlayButton {
                        size: 96
                        playing: Player.playing
                        containerColor: Theme.primaryContainer
                        iconColor: Theme.fgPrimaryContainer
                        onClicked: Player.togglePlay()
                    }
                    IconButton { icon: "skip_next"; size: "m"; iconColor: Theme.fgSurface; onClicked: Player.next() }
                    IconButton {
                        icon: Player.repeat === "one" ? "repeat_one" : "repeat"; size: "m"
                        checkable: true; autoToggle: false; checked: Player.repeat !== "off"
                        style: Player.repeat !== "off" ? "tonal" : "standard"
                        onClicked: Player.cycleRepeat()
                    }
                }

                // Не рекомендовать · громкость · лайк
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 4
                    spacing: 12
                    IconButton {
                        icon: "thumb_down"
                        enabled: Player.hasTrack
                        onClicked: { Library.dislike(Player.track); Player.next() }
                    }
                    Icon {
                        Layout.leftMargin: 8
                        name: Player.volume > 0.5 ? "volume_up" : Player.volume > 0 ? "volume_down" : "volume_off"
                        size: 22
                    }
                    Slider {
                        Layout.preferredWidth: 220
                        value: Player.volume
                        trackHeight: 14
                        handleHeight: 36
                        onMoved: v => Player.volume = v
                    }
                    LikeButton { Layout.leftMargin: 8; track: Player.track }
                }

                Item { Layout.fillHeight: true }
            }

            // ---- Колонка очереди / текста (справа) ----
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ListView {
                    id: queueList
                    anchors.fill: parent
                    visible: root.tab === "queue"
                    clip: true
                    model: Player.queue
                    boundsBehavior: Flickable.StopAtBounds
                    acceptedButtons: Qt.NoButton
                    reuseItems: true
                    header: Label {
                        width: queueList.width
                        height: 56
                        leftPadding: 12
                        text: "Очередь · " + Player.queue.count
                        type: "titleLarge"
                        weight: 600
                    }
                    delegate: TrackRow {
                        required property int index
                        required property var model
                        width: ListView.view.width
                        number: index + 1
                        trackId: model.trackId
                        title: model.title
                        version: model.version
                        artists: model.artists
                        album: model.album
                        cover: model.cover
                        durationMs: model.durationMs
                        explicit: model.explicit
                        available: model.available
                        current: index === Player.currentIndex
                        liked: Library.likesRevision >= 0 && Library.isLiked(model.trackId)
                        onLikeClicked: Library.toggleLike(Player.queue.get(index))
                        wide: false
                        onActivated: Player.playIndex(index)
                    }
                    // Держим текущий трек в поле зрения при открытии
                    Connections {
                        target: root
                        function onOpenChanged() { if (root.open) queueList.positionViewAtIndex(Math.max(0, Player.currentIndex - 2), ListView.Beginning) }
                    }
                }
                SmoothScroll { flickable: queueList; wheelStep: Settings.wheelStep; visible: queueList.visible }

                EmptyState {
                    anchors.centerIn: parent
                    width: Math.min(parent.width, 360)
                    visible: root.tab === "lyrics"
                    icon: "title"
                    shape: "flower6"
                    title: "Текст пока недоступен"
                    text: "Синхронный текст будет появляться здесь для играющего трека"
                }
            }
        }
    }
}
