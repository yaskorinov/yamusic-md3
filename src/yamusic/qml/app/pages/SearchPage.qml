import QtQuick
import QtQuick.Layouts
import Md3
import ".."

Page {
    id: page
    title: "Поиск"
    showHeader: false

    property int filter: 0
    readonly property var filters: ["Всё", "Треки", "Альбомы", "Исполнители", "Плейлисты", "Подкасты"]

    function focusSearch() { field.inputItem.forceActiveFocus() }
    Component.onCompleted: Qt.callLater(focusSearch)

    SearchField {
        id: field
        Layout.fillWidth: true
        Layout.maximumWidth: 720
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 8
        implicitHeight: 56
        placeholder: "Трек, альбом, исполнитель, подкаст"
    }

    Flow {
        Layout.fillWidth: true
        Layout.maximumWidth: 720
        Layout.alignment: Qt.AlignHCenter
        spacing: 8
        Repeater {
            model: page.filters
            Chip {
                required property string modelData
                required property int index
                text: modelData
                selected: page.filter === index
                onClicked: page.filter = index
            }
        }
    }

    EmptyState {
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 48
        icon: field.text ? "cloud_off" : "travel_explore"
        shape: field.text ? "flower6" : "cookie9"
        title: field.text ? "Поиск станет доступен после входа" : "Что послушаем?"
        text: field.text ? "Сейчас клиент не подключён к Яндекс Музыке" : "Ищите треки, альбомы, исполнителей и подкасты"
    }
}
