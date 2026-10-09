import QtQuick
import QtQuick.Layouts
import Md3
import ".."

Page {
    id: page
    title: "Главная"

    // Герой: Моя волна
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: 232
        radius: Theme.shape.extraLarge
        color: Theme.primaryContainer

        // Декор целиком внутри карточки: clip режет по прямоугольнику, а не по скруглению.
        MorphShape {
            width: 200; height: 200
            anchors.right: parent.right
            anchors.rightMargin: 40
            anchors.verticalCenter: parent.verticalCenter
            shape: "flower8"
            color: Qt.alpha(Theme.primary, 0.16)
        }
        MorphShape {
            width: 96; height: 96
            anchors.right: parent.right
            anchors.rightMargin: 192
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 20
            shape: "cookie9"
            color: Qt.alpha(Theme.tertiary, 0.22)
        }
        MorphShape {
            width: 56; height: 56
            anchors.right: parent.right
            anchors.rightMargin: 36
            anchors.top: parent.top
            anchors.topMargin: 20
            shape: "clover4"
            color: Qt.alpha(Theme.tertiary, 0.22)
        }

        ColumnLayout {
            anchors.left: parent.left
            anchors.leftMargin: 32
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width * 0.6
            spacing: 8
            Label { text: "Моя волна"; type: "displaySmall"; weight: 700; color: Theme.fgPrimaryContainer }
            Label {
                Layout.fillWidth: true
                text: "Бесконечный поток музыки, который подстраивается под вас"
                type: "bodyLarge"
                color: Theme.fgPrimaryContainer
                wrapMode: Text.WordWrap
                elide: Text.ElideNone
            }
            Button {
                Layout.topMargin: 12
                text: "Слушать"
                icon: "play_arrow"
                size: "m"
                onClicked: page.router.reset("wave")
            }
        }
    }

    Section { text: "Подборки для вас" }
    Flow {
        Layout.fillWidth: true
        spacing: 20
        Repeater {
            model: ["softSquare", "cookie9", "softSquare", "clover4", "softSquare", "cookie12"]
            SkeletonTile { required property string modelData; shape: modelData }
        }
    }

    Section { text: "Новые релизы" }
    Flow {
        Layout.fillWidth: true
        spacing: 20
        Repeater {
            model: 6
            SkeletonTile {}
        }
    }

    component Section: Label {
        Layout.topMargin: 8
        type: "titleLarge"
        weight: 600
    }
}
