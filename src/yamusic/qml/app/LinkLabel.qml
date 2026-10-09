import QtQuick
import Md3

// Текст-ссылка (исполнитель, альбом): подчёркивается при наведении. Кликабелен только сам текст,
// а не пустое место справа — там клик достаётся строке под ним.
Label {
    id: root

    property bool linkEnabled: true
    signal clicked()

    font.underline: area.containsMouse && linkEnabled

    MouseArea {
        id: area
        width: Math.min(root.width, root.implicitWidth)
        height: root.height
        enabled: root.linkEnabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
