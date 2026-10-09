import QtQuick
import QtQuick.Layouts
import Md3

// Пустое состояние: иконка в «печеньке», заголовок, пояснение, необязательная кнопка.
ColumnLayout {
    id: root

    property string icon: "music_note"
    property string title
    property string text
    property string actionText
    property string actionIcon
    property var shape: "cookie9"

    signal action()

    spacing: 12

    Item {
        Layout.alignment: Qt.AlignHCenter
        Layout.preferredWidth: 112
        Layout.preferredHeight: 112
        Layout.bottomMargin: 8

        // Статично: бесконечные анимации в простое заставляют окно перерисовываться каждый кадр.
        MorphShape {
            anchors.fill: parent
            shape: root.shape
            color: Theme.secondaryContainer
        }
        Icon {
            anchors.centerIn: parent
            name: root.icon
            size: 44
            fill: 1
            color: Theme.fgSecondaryContainer
        }
    }

    Label {
        Layout.alignment: Qt.AlignHCenter
        Layout.fillWidth: true
        Layout.maximumWidth: 420
        text: root.title
        wrapMode: Text.WordWrap
        elide: Text.ElideNone
        type: "titleLarge"
        horizontalAlignment: Text.AlignHCenter
    }
    Label {
        visible: root.text !== ""
        Layout.alignment: Qt.AlignHCenter
        Layout.fillWidth: true
        Layout.maximumWidth: 420
        text: root.text
        type: "bodyMedium"
        color: Theme.fgSurfaceVariant
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        elide: Text.ElideNone
        lineHeightMode: Text.ProportionalHeight
        lineHeight: 1.25
    }
    Button {
        visible: root.actionText !== ""
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 8
        text: root.actionText
        icon: root.actionIcon
        style: "tonal"
        onClicked: root.action()
    }
}
