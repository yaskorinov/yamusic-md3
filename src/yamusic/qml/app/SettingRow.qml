import QtQuick
import QtQuick.Layouts
import Md3

// Строка настройки: заголовок + пояснение слева, контрол справа (кладётся внутрь).
// Если контрол не помещается рядом с подписью (узкое окно) — он уходит под неё.
GridLayout {
    id: root

    property string title
    property string description
    default property alias control: slot.data

    Layout.fillWidth: true
    // ширину задаёт родитель: иначе неявная ширина двух колонок распирает строку и она не переносится
    Layout.preferredWidth: 0
    columns: width > 0 && width < slot.implicitWidth + 200 ? 1 : 2
    columnSpacing: 24
    rowSpacing: 10

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 2
        Label { Layout.fillWidth: true; text: root.title; type: "bodyLarge" }
        Label {
            visible: root.description !== ""
            Layout.fillWidth: true
            text: root.description
            type: "bodySmall"
            color: Theme.fgSurfaceVariant
            wrapMode: Text.WordWrap
            elide: Text.ElideNone
            lineHeightMode: Text.ProportionalHeight
            lineHeight: 1.2
        }
    }
    Item {
        id: slot
        implicitWidth: childrenRect.width
        implicitHeight: childrenRect.height
    }
}
