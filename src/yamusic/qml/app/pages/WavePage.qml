import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

Page {
    id: page
    title: "Моя волна"
    showHeader: false

    readonly property bool signedIn: Auth.state === "signedIn"
    readonly property bool playing: Wave.active && Wave.stationTitle === "" && Player.playing

    // Герой: морфящиеся фигуры + большая кнопка. Пока волна играет — фигуры медленно вращаются.
    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: 340

        MorphShape {
            id: hero
            width: 300; height: 300
            anchors.centerIn: parent
            shape: page.playing ? "flower8" : heroArea.containsMouse ? "cookie9" : "cookie12"
            duration: Theme.motion.spatialSlow
            color: Theme.primaryContainer
        }
        MorphShape {
            id: inner
            width: 210; height: 210
            anchors.centerIn: parent
            shape: page.playing ? "cookie6" : heroArea.containsMouse ? "cookie9" : "clover4"
            duration: Theme.motion.spatialSlow
            color: Qt.alpha(Theme.primary, 0.22)
        }
        FrameAnimation {
            running: page.playing && page.visible && !Theme.calm
            onTriggered: {
                hero.angle = (hero.angle + frameTime * 8) % 360
                inner.angle = (inner.angle - frameTime * 12 + 360) % 360
            }
        }
        PlayButton {
            anchors.centerIn: parent
            size: 104
            playing: page.playing
            spin: false
            playingShape: "cookie9"
            pausedShape: "cookie6"
            enabled: page.signedIn
            onClicked: Wave.play()
        }
        LoadingIndicator {
            anchors.centerIn: parent
            size: 128
            color: Theme.primary
            visible: Wave.loading || (Wave.active && Player.buffering)
            opacity: 0.6
        }
        MouseArea { id: heroArea; anchors.fill: hero; hoverEnabled: true; acceptedButtons: Qt.NoButton }
    }

    Label {
        Layout.alignment: Qt.AlignHCenter
        text: "Моя волна"
        type: "displaySmall"
        weight: 700
    }
    Label {
        Layout.alignment: Qt.AlignHCenter
        Layout.maximumWidth: 640
        Layout.topMargin: -16
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: !page.signedIn ? "Подбор станет доступен после входа"
            : Wave.errorText !== "" ? Wave.errorText
            : Wave.active && Player.hasTrack ? Player.title + " — " + Player.artist
            : "Бесконечный поток музыки под ваш вкус. Лайки и «не рекомендовать» сразу меняют подбор"
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

    // Настройки: connected button group, как «Качество звука». Повторный клик снимает выбор («любое»).
    // Во время игры смена настройки сразу пересобирает следующие треки.
    Repeater {
        model: Wave.groups
        ColumnLayout {
            id: group
            required property var modelData
            readonly property var seeds: modelData.items.map(i => i.seed)
            Layout.fillWidth: true
            Layout.maximumWidth: 860
            Layout.alignment: Qt.AlignHCenter
            spacing: 10
            Label { text: group.modelData.title; type: "titleMedium"; color: Theme.fgSurfaceVariant }
            ButtonGroup {
                model: group.modelData.items.map(i => i.label)
                autoSelect: false
                currentIndex: group.seeds.indexOf(Wave.selection[group.modelData.key] ?? "")
                onActivated: i => Wave.select(group.modelData.key, group.seeds[i])
            }
        }
    }
}
