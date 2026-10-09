import QtQuick
import QtQuick.Layouts
import Md3
import ".."

Page {
    id: page
    title: "Моя волна"
    showHeader: false

    property var picks: ({})   // группа → индекс выбранного чипа (-1 — любой)
    readonly property var groups: [
        { name: "Занятие", items: ["Просыпаюсь", "В дороге", "Работаю", "Тренируюсь", "Засыпаю"] },
        { name: "Характер", items: ["Любимое", "Незнакомое", "Популярное"] },
        { name: "Настроение", items: ["Бодрое", "Весёлое", "Спокойное", "Грустное"] }
    ]

    // Герой: морфящаяся фигура + большая кнопка
    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: 340

        MorphShape {
            id: hero
            width: 300; height: 300
            anchors.centerIn: parent
            shape: heroArea.containsMouse ? "flower8" : "cookie12"
            duration: Theme.motion.spatialSlow
            color: Theme.primaryContainer
        }
        MorphShape {
            width: 210; height: 210
            anchors.centerIn: parent
            shape: heroArea.containsMouse ? "cookie9" : "clover4"
            duration: Theme.motion.spatialSlow
            color: Qt.alpha(Theme.primary, 0.22)
        }
        PlayButton {
            anchors.centerIn: parent
            size: 104
            playingShape: "cookie9"
            pausedShape: "cookie6"
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
        Layout.topMargin: -16
        text: "Подбор станет доступен после входа"
        type: "bodyLarge"
        color: Theme.fgSurfaceVariant
    }

    Repeater {
        model: page.groups
        ColumnLayout {
            id: group
            required property var modelData
            Layout.fillWidth: true
            Layout.maximumWidth: 720
            Layout.alignment: Qt.AlignHCenter
            spacing: 10
            Label { text: group.modelData.name; type: "titleMedium"; color: Theme.fgSurfaceVariant }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: group.modelData.items
                    Chip {
                        required property string modelData
                        required property int index
                        text: modelData
                        selected: (page.picks[group.modelData.name] ?? -1) === index
                        onClicked: {
                            const p = Object.assign({}, page.picks)
                            p[group.modelData.name] = selected ? -1 : index
                            page.picks = p
                        }
                    }
                }
            }
        }
    }
}
