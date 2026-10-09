import QtQuick
import QtQuick.Layouts
import Md3
import ".."

Page {
    title: "Мне нравится"
    subtitle: "Треки, которые вы отметили"

    EmptyState {
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 40
        icon: "favorite"
        shape: "clover4"
        title: "Пока пусто"
        text: "После входа здесь появятся треки с вашими лайками"
    }
}
