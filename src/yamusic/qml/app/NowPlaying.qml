import QtQuick
import QtQuick.Layouts
import QtQuick.Effects
import Md3
import YaMusic.Core

// Полноэкранный плеер. Раскрывается через «окно» — случайную фигуру MD3 Expressive:
// фаза 1 — фигура быстро вращается и растёт из маленькой (на обложке мини-плеера) до средней,
// уходя к центру; фаза 2 — растёт на весь экран, пока весь экран не окажется внутри неё.
// Закрытие — тот же путь в обратную сторону. Маска (MultiEffect) включена только на время перехода.
Item {
    id: root

    property bool open: false
    property point origin: Qt.point(width / 2, height - 60)
    property string tab: "lyrics"          // lyrics | queue
    property real reveal: open ? 1 : 0
    readonly property bool animating: revealAnim.running

    readonly property var _windowShapes: ["cookie9", "cookie12", "cookie7", "flower8", "flower6", "clover4", "sunny", "cookie6"]
    readonly property real _split: 0.45
    readonly property real _a: Math.min(1, reveal / _split)
    readonly property real _b: Math.max(0, (reveal - _split) / (1 - _split))

    function _inOut(x) {
        return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2
    }

    function show(from) {
        if (from)
            origin = from
        if (!open)
            blob.jumpTo(_windowShapes[Math.floor(Math.random() * _windowShapes.length)])
        open = true
        forceActiveFocus()
    }
    function hide() {
        open = false
    }

    visible: reveal > 0
    focus: open
    Keys.onEscapePressed: hide()

    Behavior on reveal {
        NumberAnimation {
            id: revealAnim
            duration: root.open ? 820 : 620
            easing.type: Easing.Linear     // кривые — у каждой фазы свои, см. blob
        }
    }

    function _time(ms) {
        const s = Math.max(0, Math.round(ms / 1000))
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0")
    }

    // Маска раскрытия: фигура-«окно»
    Item {
        id: revealMask
        anchors.fill: parent
        visible: false
        layer.enabled: root.animating || root.reveal < 1
        MorphShape {
            id: blob
            readonly property real e1: root._inOut(root._a)
            readonly property real e2: root._inOut(root._b)
            readonly property real small: 36
            readonly property real medium: 0.42 * Math.min(root.width, root.height)
            // Во впадинах фигура уже описанного круга — берём с запасом, чтобы углы окна оказались внутри
            readonly property real full: 2 * 1.6 * Math.hypot(root.width / 2, root.height / 2)
            readonly property real d: root._b > 0 ? medium + (full - medium) * e2 : small + (medium - small) * e1
            readonly property real cx: root.origin.x + (root.width / 2 - root.origin.x) * e1
            readonly property real cy: root.origin.y + (root.height / 2 - root.origin.y) * e1
            x: cx - d / 2
            y: cy - d / 2
            width: d
            height: d
            rotation: 240 * e1 + 50 * e2
            color: "white"
            tinted: false
        }
    }

    // Содержимое (в самом конце сворачивания гаснет — без «обрубка» в крошечной фигуре)
    Item {
        id: content
        anchors.fill: parent
        opacity: Math.min(1, root.reveal / 0.12)
        layer.enabled: root.reveal < 1
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: revealMask
            maskThresholdMin: 0.5
            maskSpreadAtMin: 1.0
        }

        // Фон: размытая обложка + вуаль цвета темы
        Rectangle { anchors.fill: parent; color: Theme.surface }
        // Режим, сила размытия и «плавание» — из настроек (см. CoverBackdrop); при смене трека фон
        // меняется вместе с цветами темы — под той же шторкой (см. TrackBackdrop)
        TrackBackdrop {
            anchors.fill: parent
            source: Player.cover
            mode: Settings.backdropMode
            blur: Settings.nowPlayingBlur
            flow: Settings.nowPlayingDrift
            followTheme: Settings.accentFromCover
            running: root.visible && Player.playing
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

                // «Дыхание» в такт: уровень звука от mpv; доля над скользящим средним — удар
                Binding {
                    target: Player
                    property: "levelsWanted"
                    value: root.visible && root.open && Settings.coverPulse && Player.playing && !Theme.calm
                }
                property real avgLevel: 0
                property real beat: 0
                Behavior on beat { NumberAnimation { duration: 110; easing.type: Easing.OutQuad } }
                Connections {
                    target: Player
                    function onLevelChanged() {
                        left.avgLevel += (Player.level - left.avgLevel) * 0.06
                        left.beat = Math.max(0, Math.min(1, (Player.level - left.avgLevel) * 4))
                    }
                }

                NowPlayingCover {
                    // Всё остальное в колонке занимает ~360 px — обложка берёт оставшееся
                    readonly property real s: Math.max(160, Math.min(left.width, left.height - 360, 440))
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: s
                    Layout.preferredHeight: s
                    Layout.bottomMargin: 20
                    source: Player.cover.replace("400x400", "800x800")
                    restShape: Player.playing ? "cookie12" : "square"
                    direction: Player.direction
                    pulse: Settings.coverPulse ? left.beat : 0
                }

                // Подписи при смене трека проявляются заново, чуть снизу
                property real infoIn: 1
                Connections {
                    target: Player
                    function onTrackChanged() {
                        if (root.visible && left._lastId !== Player.trackId) {
                            left.infoIn = 0
                            infoAnim.restart()
                        }
                        left._lastId = Player.trackId
                    }
                }
                property string _lastId: ""
                NumberAnimation {
                    id: infoAnim
                    target: left
                    property: "infoIn"
                    to: 1
                    duration: 520
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Theme.motion.emphasizedDecelerate
                }

                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: Player.title
                    type: "headlineMedium"
                    weight: 700
                    opacity: left.infoIn
                    transform: Translate { y: (1 - left.infoIn) * 14 }
                }
                LinkLabel {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: left.width
                    Layout.topMargin: -6
                    horizontalAlignment: Text.AlignHCenter
                    previewArtistId: (Player.track.artistRefs ?? []).length === 1 ? Player.track.artistRefs[0].id : ""
                    onClicked: Catalog.openArtists(Player.track.artistRefs ?? [])
                    opacity: Math.max(0, left.infoIn * 1.3 - 0.3)    // чуть позже названия
                    transform: Translate { y: (1 - left.infoIn) * 18 }
                    text: Player.artist
                    type: "titleMedium"
                    color: Theme.fgSurfaceVariant
                }
                LinkLabel {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: left.width
                    Layout.topMargin: -6
                    horizontalAlignment: Text.AlignHCenter
                    onClicked: Catalog.openAlbum(Player.track.albumId ?? "")
                    opacity: Math.max(0, left.infoIn * 1.3 - 0.3)    // чуть позже названия
                    transform: Translate { y: (1 - left.infoIn) * 18 }
                    text: Player.album + (Player.codec !== "" ? "  ·  " + Player.codec : "")
                    type: "bodySmall"
                    color: Theme.fgSurfaceVariant
                }

                // Прогресс
                property real progress: Player.position
                Behavior on progress {
                    enabled: Player.playing && !bigSeek.dragging && !Theme.calm
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
            // Смена вкладки — shared axis X (порядок как у переключателя: текст слева, очередь справа):
            // уходящая уезжает в свою сторону и гаснет, новая выезжает с другой.
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                component TabShift: NumberAnimation {
                    duration: 460
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Theme.motion.emphasizedDecelerate
                }

                ListView {
                    id: queueList
                    property real shown: root.tab === "queue" ? 1 : 0
                    Behavior on shown { TabShift {} }
                    anchors.fill: parent
                    visible: shown > 0.001
                    opacity: Math.pow(shown, 1.6)
                    transform: Translate { x: (1 - queueList.shown) * 80 }
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
                        artistRefs: model.artistRefs
                        albumId: model.albumId
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

                LyricsView {
                    id: lyricsView
                    property real shown: root.tab === "lyrics" ? 1 : 0
                    Behavior on shown { TabShift {} }
                    anchors.fill: parent
                    visible: shown > 0.001 && root.visible
                    opacity: Math.pow(shown, 1.6)
                    transform: Translate { x: -(1 - lyricsView.shown) * 80 }
                    fontSize: Math.max(26, Math.min(40, root.width / 36))
                }
            }
        }
    }
}
