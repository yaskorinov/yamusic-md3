import QtQuick
import QtQuick.Shapes
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

// «Моя волна» — одна сцена на всю страницу («орбита»). В центре — фигура с кнопкой, вокруг неё по
// эллипсу плавают настройки волны: кружки, собранные в дуги по группам. Выбранная настройка
// притягивается к центру, прилипает к фигуре и получает цвет и форму своей группы; повторный клик
// отпускает её обратно («любое»). Центр тоже отзывается: его форму задаёт настроение, цвет — характер.
// Движение — только трансформации (x/y/rotation), контуры фигур каждый кадр не пересчитываются.
Item {
    id: page

    property var router
    readonly property string title: "Моя волна"
    readonly property bool showHeader: false
    readonly property bool scrolled: false

    readonly property bool signedIn: Auth.state === "signedIn"
    readonly property bool playing: Wave.active && Wave.stationTitle === "" && Player.playing

    // ---- Геометрия сцены ----
    readonly property real sceneHeight: height - 104            // внизу — плавающий мини-плеер
    readonly property real cx: width / 2
    readonly property real cy: sceneHeight / 2
    readonly property real rx: Math.max(240, Math.min(width / 2 - 100, 640))
    readonly property real ry: Math.max(170, Math.min(sceneHeight / 2 - 70, 340))
    readonly property real perimeter: 2 * Math.PI * Math.sqrt((rx * rx + ry * ry) / 2)
    readonly property real spacing: perimeter / (flat.length + 3)
    readonly property real bubble: Math.max(78, Math.min(spacing - 10, 116))
    // Соседи стоят чуть ближе и чуть дальше от центра — живее, чем ровная нитка. В тесном окне разбег
    // больше: кружки не мельчают, а расходятся на два кольца.
    readonly property real zig: Math.max(0.06, Math.sqrt(Math.max(0, Math.pow(bubble + 6, 2) - spacing * spacing))
                                               / (2 * Math.min(rx, ry)))
    // между центром и внутренним кольцом должна помещаться прилипшая фигура
    readonly property real heroSize: flat.length === 0 ? Math.min(ry * 1.2, 300)     // без входа настроек нет
        : Math.max(140, Math.min(2 * (ry * (1 - zig) - bubble * 1.4 - 6), rx * 0.8, 320))

    // ---- Что летает: группы → плоский список (заголовки групп тоже стоят на орбите) ----
    readonly property var groupStyle: ({
        context:    { shape: "cookie9", color: "primary", fg: "fgPrimary" },
        diversity:  { shape: "cookie4", color: "tertiary", fg: "fgTertiary" },
        moodEnergy: { shape: "flower8", color: "secondary", fg: "fgSecondary" },
        language:   { shape: "cookie6", color: "inverseSurface", fg: "fgInverseSurface" }
    })
    readonly property var flat: {
        const out = []
        const groups = page.signedIn ? Wave.groups : []
        for (let g = 0; g < groups.length; g++) {
            out.push({ caption: true, group: g, label: groups[g].title })
            for (const item of groups[g].items)
                out.push({ caption: false, group: g, key: groups[g].key, label: item.label, seed: item.seed })
        }
        return out
    }

    // Места на орбите: равные шаги по длине дуги эллипса, от низа по часовой стрелке; внизу по центру
    // оставлена свободная дуга — там подпись. У каждой группы своё направление «причала» у центра.
    readonly property var slots: {
        const n = flat.length
        if (n === 0)
            return { at: [], dock: [], from: 0, sweep: 0, gap: 0 }
        const steps = 720, xs = [], ys = [], acc = [0]
        for (let k = 0; k <= steps; k++) {
            const a = Math.PI / 2 + 2 * Math.PI * k / steps
            xs.push(rx * Math.cos(a))
            ys.push(ry * Math.sin(a))
            if (k > 0)
                acc.push(acc[k - 1] + Math.hypot(xs[k] - xs[k - 1], ys[k] - ys[k - 1]))
        }
        const total = n + 3
        const at = [], sums = {}, angles = []
        let k = 0
        for (let i = 0; i < n; i++) {
            const target = acc[steps] * (i + 2) / total
            while (k < steps - 1 && acc[k + 1] < target)
                k++
            const f = (target - acc[k]) / Math.max(1e-6, acc[k + 1] - acc[k])
            const x = xs[k] + (xs[k + 1] - xs[k]) * f, y = ys[k] + (ys[k + 1] - ys[k]) * f
            const s = flat[i].caption ? 1 : (i % 2 ? 1 + zig : 1 - zig)
            at.push(Qt.point(x * s, y * s))
            angles.push(90 + 360 * (k + f) / steps)
            if (!flat[i].caption) {
                const sum = sums[flat[i].group] ?? (sums[flat[i].group] = { x: 0, y: 0 })
                sum.x += x / rx
                sum.y += y / ry
            }
        }
        const dock = {}
        for (const g in sums) {
            const len = Math.hypot(sums[g].x, sums[g].y) || 1
            dock[g] = Qt.point(sums[g].x / len, sums[g].y / len)
        }
        // пунктир орбиты — от первой фигуры до последней; ширина свободной дуги внизу — под подпись
        return { at: at, dock: dock, from: angles[0], sweep: angles[n - 1] - angles[0],
                 gap: Math.abs(at[n - 1].x - at[0].x) - bubble - 56 }
    }

    // ---- Центр отзывается на выбор ----
    function _name(key) { return (Wave.selection[key] ?? "").split(":").pop() }
    readonly property var heroShape: ({ active: "sunny", fun: "flower8", calm: "scallop", sad: "blob" })[_name("moodEnergy")] ?? "cookie12"
    readonly property string heroRole: ({ discover: "tertiary", popular: "secondary" })[_name("diversity")] ?? "primary"

    // ---- Движение ----
    property real time: 0
    property real intro: 0
    FrameAnimation {
        running: page.visible && !Theme.calm
        onTriggered: {
            page.time += frameTime
            if (page.playing) {
                hero.rotation = (hero.rotation + frameTime * 8) % 360
                inner.rotation = (inner.rotation - frameTime * 12 + 360) % 360
            }
        }
    }
    NumberAnimation on intro { id: introAnim; from: 0; to: 1; duration: 1100; easing.type: Easing.Linear }
    Connections {
        target: Wave
        function onGroupsChanged() { introAnim.restart() }
    }

    // Орбита — пунктир
    Shape {
        x: page.cx
        y: page.cy
        opacity: page.flat.length > 0 ? Math.min(1, page.intro * 2) : 0
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: Theme.outlineVariant
            strokeWidth: 1.5
            fillColor: "transparent"
            strokeStyle: ShapePath.DashLine
            dashPattern: [1, 5]
            capStyle: ShapePath.RoundCap
            PathAngleArc {
                radiusX: page.rx
                radiusY: page.ry
                startAngle: page.slots.from
                sweepAngle: page.slots.sweep
            }
        }
    }

    // ---- Центр: фигуры и кнопка ----
    Item {
        id: heroBox
        x: page.cx - width / 2
        y: page.cy - height / 2
        width: page.heroSize
        height: page.heroSize
        z: 1

        MorphShape {
            id: hero
            anchors.fill: parent
            shape: heroArea.containsMouse && !page.playing && page.heroShape === "cookie12" ? "cookie9" : page.heroShape
            duration: Theme.motion.spatialSlow
            color: Theme[page.heroRole + "Container"]
        }
        MorphShape {
            id: inner
            width: parent.width * 0.7
            height: width
            anchors.centerIn: parent
            shape: page.playing ? "cookie6" : "clover4"
            duration: Theme.motion.spatialSlow
            color: Qt.alpha(Theme[page.heroRole], 0.22)
        }
        MouseArea { id: heroArea; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
        PlayButton {
            anchors.centerIn: parent
            size: parent.width * 0.35
            playing: page.playing
            spin: false
            playingShape: "cookie9"
            pausedShape: "cookie6"
            enabled: page.signedIn
            onClicked: Wave.play()
        }
        LoadingIndicator {
            anchors.centerIn: parent
            size: parent.width * 0.43
            color: Theme.primary
            visible: Wave.loading || (Wave.active && Player.buffering)
            opacity: 0.6
        }
    }

    // ---- Настройки на орбите ----
    Repeater {
        model: page.flat

        Item {
            id: body
            required property var modelData
            required property int index

            readonly property bool caption: modelData.caption
            readonly property var style: page.groupStyle[modelData.key] ?? page.groupStyle.context
            readonly property bool selected: !caption && Wave.selection[modelData.key] === modelData.seed
            readonly property point slot: page.slots.at[index] ?? Qt.point(0, 0)
            readonly property point dir: page.slots.dock[modelData.group] ?? Qt.point(0, -1)

            // вылет из центра при появлении: по очереди вдоль орбиты
            readonly property real _a: Math.max(0, Math.min(1, page.intro * 1.9 - index / Math.max(1, page.flat.length) * 0.9))
            readonly property real appear: 1 - Math.pow(1 - _a, 3)
            // 0 — на орбите, 1 — прилип к центру
            property real docked: selected ? 1 : 0
            Behavior on docked {
                NumberAnimation { duration: 620; easing.type: Easing.OutBack; easing.overshoot: 1.3 }
            }
            // лёгкое покачивание на месте; у центра затихает
            readonly property real phase: index * 1.7
            readonly property real bobX: 7 * Math.sin(page.time * 0.45 + phase) * (1 - docked)
            readonly property real bobY: 6 * Math.cos(page.time * 0.37 + phase * 1.3) * (1 - docked)
            readonly property real reach: page.heroSize / 2 + page.bubble * 0.36

            width: caption ? Math.max(page.bubble, captionLabel.implicitWidth) : page.bubble
            height: page.bubble
            x: page.cx + (slot.x * appear + bobX) * (1 - docked) + dir.x * reach * docked - width / 2
            y: page.cy + (slot.y * appear + bobY) * (1 - docked) + dir.y * reach * docked - height / 2
            z: selected ? 2 : 0
            opacity: appear
            scale: (0.4 + 0.6 * appear) * (area.pressed ? 0.94 : area.containsMouse ? 1.06 : 1)
            Behavior on scale { NumberAnimation { duration: Theme.motion.spatialFast; easing.type: Easing.OutBack } }

            Label {
                id: captionLabel
                visible: body.caption
                anchors.centerIn: parent
                text: body.modelData.label
                type: "titleSmall"
                weight: 600
                color: Theme.fgSurfaceVariant
            }

            MorphShape {
                visible: !body.caption
                anchors.fill: parent
                shape: body.selected ? body.style.shape : area.containsMouse ? "cookie12" : "circle"
                color: body.selected ? Theme[body.style.color]
                     : area.containsMouse ? Theme.surfaceBright : Theme.surfaceContainerHighest
                rotation: body.selected ? page.time * 6 : 0
            }
            Label {
                visible: !body.caption
                anchors.centerIn: parent
                width: parent.width * 0.8
                horizontalAlignment: Text.AlignHCenter
                text: body.modelData.label
                type: page.bubble >= 96 ? "labelLarge" : "labelMedium"
                weight: body.selected ? 700 : 500
                fontSizeMode: Text.HorizontalFit
                minimumPixelSize: 10
                elide: Text.ElideNone
                color: body.selected ? Theme[body.style.fg] : Theme.fgSurface
            }
            MouseArea {
                id: area
                anchors.fill: parent
                enabled: !body.caption
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Wave.select(body.modelData.key, body.modelData.seed)
            }
        }
    }

    // ---- Подпись в свободной дуге внизу орбиты: что играет / подсказка ----
    ColumnLayout {
        x: page.cx - width / 2
        y: page.flat.length > 0 ? page.cy + page.ry - height / 2 : page.cy + page.heroSize / 2 + 24
        width: page.flat.length > 0 ? Math.max(200, Math.min(page.slots.gap, 440)) : Math.min(page.width - 64, 440)
        opacity: page.flat.length > 0 ? Math.min(1, page.intro * 2) : 1
        spacing: 12
        Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            maximumLineCount: 2
            text: !page.signedIn ? "Подбор станет доступен после входа"
                : Wave.errorText !== "" ? Wave.errorText
                : Wave.active && Player.hasTrack ? Player.title + " — " + Player.artist
                : "Бесконечный поток под ваш вкус"
            type: "bodyLarge"
            color: Wave.errorText !== "" && page.signedIn ? Theme.error : Theme.fgSurfaceVariant
        }
        Button {
            visible: !page.signedIn
            Layout.alignment: Qt.AlignHCenter
            text: "Войти"
            icon: "login"
            onClicked: page.router.reset("account")
        }
    }
}
