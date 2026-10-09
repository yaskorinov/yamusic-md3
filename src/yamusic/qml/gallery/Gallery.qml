import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Галерея MD3-кита: все компоненты + управление темой. Картинку можно перетащить в окно — тема возьмёт цвет из неё.
Window {
    id: win

    readonly property url coversDir: Qt.resolvedUrl("../../../../docs/reference/covers/")
    readonly property var covers: ["panchiko.png", "insomniac.png", "cannibal.png"]
    property int coverIndex: 0
    property bool playing: true
    property real progress: 0.32
    property real scrollTo: 0
    property int startCover: 0
    property bool perfLog: false        // --set perfLog=true: менять обложки по кругу и писать время кадров
    property bool perfScroll: false     // --set perfScroll=true: вместо обложек — прокрутка вверх-вниз

    Timer {
        interval: 1600
        repeat: true
        running: win.perfLog
        onTriggered: {
            if (perf.frames > 0)
                console.log(`perf: frames=${perf.frames} avg=${(perf.total / perf.frames).toFixed(1)}ms max=${perf.max.toFixed(1)}ms slow(>20ms)=${perf.slow}`)
            perf.frames = 0; perf.total = 0; perf.max = 0; perf.slow = 0
            if (win.perfScroll)
                flick.scrollTo(flick.contentY > flick.maxY / 2 ? 0 : flick.maxY)
            else
                win.useCover((win.coverIndex + 1) % win.covers.length)
        }
    }

    FrameAnimation {
        id: perf
        property int frames: 0
        property real total: 0
        property real max: 0
        property int slow: 0
        running: win.perfLog
        onTriggered: {
            const ms = frameTime * 1000
            frames++; total += ms; max = Math.max(max, ms); if (ms > 20) slow++
        }
    }

    width: 1280
    height: 800
    visible: true
    title: "YaMusic — MD3 Gallery"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.surfaceContainerLowest

    function useCover(i) {
        coverIndex = i
        ThemeEngine.setSeedFromImage(coversDir + covers[i])
    }

    Component.onCompleted: {
        useCover(startCover)
        Qt.callLater(() => flick.contentY = Math.min(scrollTo, Math.max(0, flick.contentHeight - flick.height)))
    }

    Timer {
        interval: 100
        repeat: true
        running: win.playing && !seek.dragging
        onTriggered: win.progress = (win.progress + 0.002) % 1
    }

    DropArea {
        anchors.fill: parent
        onDropped: drop => { if (drop.hasUrls) ThemeEngine.setSeedFromImage(drop.urls[0]) }
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        // ---------------- Сайдбар ----------------
        ColumnLayout {
            Layout.preferredWidth: 260
            Layout.fillWidth: false
            Layout.fillHeight: true
            spacing: 2

            Item { Layout.preferredHeight: 12 }
            NavItem { Layout.fillWidth: true; icon: "home"; text: "Главная" }
            NavItem { Layout.fillWidth: true; icon: "search"; text: "Поиск"; selected: true }

            SectionHeader { text: "Коллекция" }
            NavItem { Layout.fillWidth: true; icon: "favorite"; text: "Мне нравится" }
            NavItem { Layout.fillWidth: true; icon: "graphic_eq"; text: "Моя волна" }

            SectionHeader { text: "Плейлисты"; action: "add" }
            Repeater {
                model: win.covers
                NavItem {
                    required property string modelData
                    required property int index
                    Layout.fillWidth: true
                    text: ["Panchiko", "Insomniac", "Cannibal"][index]
                    imageSource: win.coversDir + modelData
                    selected: win.coverIndex === index
                    trailingIcon: "download_done"
                    onClicked: win.useCover(index)
                }
            }
            Item { Layout.fillHeight: true }
        }

        // ---------------- Контент ----------------
        Surface {
            Layout.fillWidth: true
            Layout.fillHeight: true
            level: "container"
            clip: true

            SmoothFlickable {
                id: flick
                anchors.fill: parent
                contentHeight: col.implicitHeight + 48

                ColumnLayout {
                    id: col
                    x: 24
                    y: 24
                    width: flick.width - 48
                    spacing: 28

                    RowLayout {
                        Layout.fillWidth: true
                        IconButton { icon: "arrow_back" }
                        Item { Layout.fillWidth: true }
                        SearchField { Layout.preferredWidth: 380; placeholder: "Трек, альбом, исполнитель"; text: "death metal" }
                        Item { Layout.fillWidth: true }
                        IconButton { icon: "more_vert" }
                    }

                    Label { text: "MD3 Expressive"; type: "displaySmall"; weight: 600 }

                    // --- Плеер ---
                    Surface {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 220
                        level: "high"

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 24
                            spacing: 28

                            MorphImage {
                                id: bigCover
                                Layout.preferredWidth: 148
                                Layout.preferredHeight: 148
                                source: win.coversDir + win.covers[win.coverIndex]
                                shape: win.playing ? "cookie12" : "square"
                                duration: Theme.motion.spatialSlow
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Label { text: ["D>E>A>T>H>M>E>T>A>L", "Insomniac", "Dead Embryonic Cells"][win.coverIndex]; type: "headlineSmall"; weight: 700; Layout.fillWidth: true }
                                Label { text: ["Panchiko", "Memo Boy, Chakra Efendi", "Sepultura"][win.coverIndex]; type: "bodyLarge"; color: Theme.fgSurfaceVariant }

                                RowLayout {
                                    Layout.fillWidth: true
                                    Layout.topMargin: 10
                                    spacing: 12
                                    Label { text: "1:46"; type: "labelMedium"; color: Theme.fgSurfaceVariant }
                                    WavyProgress {
                                        id: seek
                                        Layout.fillWidth: true
                                        interactive: true
                                        value: win.progress
                                        wavy: win.playing
                                        onCommitted: v => win.progress = v
                                    }
                                    Label { text: "-2:35"; type: "labelMedium"; color: Theme.fgSurfaceVariant }
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8
                                    IconButton { icon: "shuffle"; checkable: true; checked: true; style: "tonal"; size: "s" }
                                    IconButton { icon: "skip_previous"; iconColor: Theme.fgSurface }
                                    PlayButton { size: 64; playing: win.playing; onClicked: win.playing = !win.playing }
                                    IconButton { icon: "skip_next"; iconColor: Theme.fgSurface }
                                    IconButton { icon: "repeat"; checkable: true; size: "s" }
                                    Icon { name: "volume_up"; size: 20; Layout.leftMargin: 8 }
                                    Slider { Layout.fillWidth: true; Layout.minimumWidth: 80; value: 0.7; trackHeight: 12; handleHeight: 32 }
                                }
                            }
                        }
                    }

                    // --- Кнопки ---
                    Section { title: "Кнопки" }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 12
                        Button { text: "Filled"; icon: "play_arrow" }
                        Button { text: "Tonal"; style: "tonal"; icon: "shuffle" }
                        Button { text: "Outlined"; style: "outlined" }
                        Button { text: "Elevated"; style: "elevated"; icon: "add" }
                        Button { text: "Text"; style: "text" }
                        Button { text: "Disabled"; enabled: false }
                        Button { text: "Toggle"; style: "tonal"; checkable: true; icon: "favorite" }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 12
                        Button { text: "XS"; size: "xs" }
                        Button { text: "Small"; size: "s" }
                        Button { text: "Medium"; size: "m"; icon: "graphic_eq" }
                        Button { text: "Моя волна"; size: "l"; icon: "graphic_eq"; style: "tonal" }
                    }

                    Section { title: "Иконки-кнопки" }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 12
                        IconButton { icon: "favorite"; checkable: true }
                        IconButton { icon: "favorite"; checkable: true; style: "filled" }
                        IconButton { icon: "lyrics"; checkable: true; checked: true; style: "tonal" }
                        IconButton { icon: "queue_music"; style: "outlined" }
                        IconButton { icon: "bedtime"; checkable: true; style: "outlined" }
                        IconButton { icon: "skip_next"; style: "filled"; size: "m"; widthMode: "wide" }
                        IconButton { icon: "play_arrow"; style: "filled"; size: "l" }
                        IconButton { icon: "graphic_eq"; style: "tonal"; size: "l"; widthMode: "narrow" }
                    }

                    Section { title: "Группа кнопок" }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 24
                        ButtonGroup { model: ["Normal", "Circular"] }
                        ButtonGroup { model: ["Bars", "Wave"]; currentIndex: 1; showCheck: true }
                        ButtonGroup {
                            model: [{ icon: "photo" }, { icon: "lyrics" }, { icon: "queue_music" }, { icon: "bedtime" }, { icon: "tune" }]
                            currentIndex: 1
                        }
                    }

                    Section { title: "Фигуры (клик — морф)" }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 16
                        Repeater {
                            model: ["circle", "square", "softSquare", "cookie4", "cookie6", "cookie9", "cookie12", "scallop", "clover4", "flower6", "flower8", "sunny", "blob", "puffy"]
                            Column {
                                id: cell
                                required property string modelData
                                required property int index
                                spacing: 6
                                MorphShape {
                                    id: ms
                                    width: 72
                                    height: 72
                                    shape: cell.modelData
                                    color: [Theme.primary, Theme.secondary, Theme.tertiary, Theme.primaryContainer][cell.index % 4]
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: ms.shape = ms.shape === cell.modelData ? "circle" : cell.modelData
                                    }
                                }
                                Label { text: cell.modelData; type: "labelSmall"; color: Theme.fgSurfaceVariant; anchors.horizontalCenter: parent.horizontalCenter }
                            }
                        }
                    }

                    Section { title: "Поля и переключатели" }
                    RowLayout {
                        spacing: 24
                        Switch { checked: true }
                        Switch { checked: false }
                        Switch { checked: false; icons: false }
                        Slider { Layout.preferredWidth: 280; value: 0.6; from: 0; to: 2; valueText: v => Math.round(v * 100) + "%" }
                    }

                    Section { title: "Цветовые роли" }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: ["primary", "fgPrimary", "primaryContainer", "fgPrimaryContainer", "secondary", "secondaryContainer", "tertiary", "tertiaryContainer",
                                    "surface", "surfaceContainerLow", "surfaceContainer", "surfaceContainerHigh", "surfaceContainerHighest", "fgSurface", "fgSurfaceVariant", "outline", "outlineVariant", "error"]
                            Rectangle {
                                required property string modelData
                                width: 132
                                height: 56
                                radius: Theme.shape.medium
                                color: Theme[modelData]
                                border.width: 1
                                border.color: Theme.outlineVariant
                                Label {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    verticalAlignment: Text.AlignBottom
                                    text: parent.modelData
                                    type: "labelSmall"
                                    color: Qt.colorEqual(parent.color, "transparent") ? Theme.fgSurface : (parent.color.hslLightness > 0.5 ? "black" : "white")
                                }
                            }
                        }
                    }
                    Item { Layout.preferredHeight: 24 }
                }
            }
        }

        // ---------------- Панель темы ----------------
        Surface {
            Layout.preferredWidth: 300
            Layout.fillHeight: true
            level: "container"

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 24
                spacing: 16

                Label { text: "Тема"; type: "titleLarge" }

                RowLayout {
                    Layout.fillWidth: true
                    Label { text: "Тёмная"; type: "bodyLarge"; Layout.fillWidth: true }
                    Switch { checked: ThemeEngine.dark; onToggled: c => ThemeEngine.dark = c }
                }

                Label { text: "Цвет из обложки"; type: "labelLarge"; color: Theme.fgSurfaceVariant }
                Row {
                    spacing: 12
                    Repeater {
                        model: win.covers
                        MorphImage {
                            required property string modelData
                            required property int index
                            width: 72
                            height: 72
                            source: win.coversDir + modelData
                            shape: win.coverIndex === index ? "cookie9" : "softSquare"
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: win.useCover(parent.index) }
                        }
                    }
                }

                Label { text: "Или seed-цвет"; type: "labelLarge"; color: Theme.fgSurfaceVariant }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    Repeater {
                        model: ["#FFCC00", "#1DB954", "#6750A4", "#E8175D", "#00A3FF", "#FF6D00"]
                        MorphShape {
                            required property string modelData
                            width: 32
                            height: 32
                            shape: Qt.colorEqual(ThemeEngine.seed, modelData) ? "cookie6" : "circle"
                            color: modelData
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: ThemeEngine.setSeed(parent.modelData) }
                        }
                    }
                }

                Label { text: "Схема"; type: "labelLarge"; color: Theme.fgSurfaceVariant }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    Repeater {
                        model: ThemeEngine.variants
                        Chip {
                            required property string modelData
                            text: modelData
                            selected: ThemeEngine.variant === modelData
                            onClicked: ThemeEngine.variant = modelData
                        }
                    }
                }

                Item { Layout.fillHeight: true }
                Label {
                    Layout.fillWidth: true
                    text: "Перетащи любую картинку в окно — тема возьмёт цвет из неё."
                    type: "bodySmall"
                    wrapMode: Text.WordWrap
                    elide: Text.ElideNone
                    lineHeightMode: Text.ProportionalHeight
                    lineHeight: 1.2
                    color: Theme.fgSurfaceVariant
                }
            }
        }
    }

    component Section: Label {
        property string title
        text: title
        type: "titleMedium"
        color: Theme.primary
    }

    component SectionHeader: RowLayout {
        property alias text: lbl.text
        property string action
        Layout.fillWidth: true
        Layout.topMargin: 14
        Layout.leftMargin: 16
        Layout.rightMargin: 8
        spacing: 10
        Label { id: lbl; type: "labelMedium"; color: Theme.fgSurfaceVariant }
        WavyDivider { Layout.fillWidth: true }
        IconButton { visible: parent.action !== ""; icon: parent.action; size: "xs"; iconColor: Theme.primary }
    }
}
