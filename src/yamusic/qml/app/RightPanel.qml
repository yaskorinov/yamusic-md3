import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Правая выдвижная панель: очередь / текст песни.
Surface {
    id: root

    level: "container"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            ButtonGroup {
                model: [{ text: "Очередь", icon: "queue_music" }, { text: "Текст", icon: "lyrics" }]
                autoSelect: false
                currentIndex: Settings.rightPanelTab === "lyrics" ? 1 : 0
                onActivated: i => Settings.rightPanelTab = i === 1 ? "lyrics" : "queue"
            }
            Item { Layout.fillWidth: true }
            IconButton { icon: "close"; onClicked: Settings.rightPanelOpen = false }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            EmptyState {
                anchors.centerIn: parent
                width: Math.min(parent.width, 280)
                visible: Settings.rightPanelTab === "queue"
                icon: "queue_music"
                shape: "cookie6"
                title: "Очередь пуста"
                text: "Треки появятся здесь, когда начнётся воспроизведение"
            }
            EmptyState {
                anchors.centerIn: parent
                width: Math.min(parent.width, 280)
                visible: Settings.rightPanelTab === "lyrics"
                icon: "lyrics"
                shape: "flower6"
                title: "Нет текста"
                text: "Синхронный текст покажется здесь для играющего трека"
            }
        }
    }
}
