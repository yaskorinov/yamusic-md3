import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Страница со списком треков (виртуализированный ListView): шапка прокручивается вместе со списком.
Item {
    id: page

    property var router
    property string title
    property string subtitle
    property bool showHeader: true
    property var model: null                  // TrackListModel
    property bool likedMarks: false            // показывать сердечки по Library.isLiked
    property string emptyIcon: "music_note"
    property string emptyTitle: "Здесь пусто"
    property string emptyText: ""
    property Component headerExtra: null       // доп. содержимое шапки (кнопки и т. п.)
    readonly property bool scrolled: list.contentY - list.originY > 48

    signal trackActivated(int row)

    ListView {
        id: list
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        model: page.model
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        acceptedButtons: Qt.NoButton
        reuseItems: true
        cacheBuffer: 640

        // ListView не сдвигает contentY, когда шапка меняет высоту (подзаголовок «N треков»
        // появляется после загрузки) или модель сбрасывается, — и заголовок оказывается обрезан.
        // Пока пользователь не прокручивал, держим список у самого верха.
        property bool userScrolled: false
        function keepTop() { if (!userScrolled) Qt.callLater(() => contentY = originY) }
        onMovementStarted: userScrolled = true   // касание
        Connections {
            target: page.model
            ignoreUnknownSignals: true
            function onModelReset() { list.userScrolled = false; list.keepTop() }
        }

        header: ColumnLayout {
            width: list.width
            spacing: 4
            onImplicitHeightChanged: list.keepTop()
            Item { Layout.preferredHeight: 8 }
            Label {
                visible: page.showHeader
                Layout.fillWidth: true
                Layout.leftMargin: 16
                text: page.title
                type: "displaySmall"
                weight: 600
            }
            Label {
                visible: page.subtitle !== ""
                Layout.fillWidth: true
                Layout.leftMargin: 16
                text: page.subtitle
                type: "bodyLarge"
                color: Theme.fgSurfaceVariant
            }
            Loader {
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.topMargin: 12
                active: page.headerExtra !== null
                sourceComponent: page.headerExtra
            }
            Item { Layout.preferredHeight: 16 }
        }

        delegate: TrackRow {
            required property int index
            required property var model
            width: ListView.view.width
            number: index + 1
            trackId: model.trackId
            title: model.title
            version: model.version
            artists: model.artists
            album: model.album
            cover: model.cover
            durationMs: model.durationMs
            explicit: model.explicit
            available: model.available
            liked: page.likedMarks && Library.isLiked(model.trackId)
            onActivated: page.trackActivated(index)
        }

        footer: Item {
            width: list.width
            height: 120 + (page.model && page.model.loading && page.model.count > 0 ? 64 : 0)
            LoadingIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.top
                visible: page.model && page.model.loading && page.model.count > 0
            }
        }
    }

    SmoothScroll { flickable: list; wheelStep: Settings.wheelStep; onUserScrolled: list.userScrolled = true }

    // Первая загрузка / пусто
    LoadingIndicator {
        anchors.centerIn: parent
        size: 64
        contained: true
        visible: page.model !== null && page.model.loading && page.model.count === 0
    }
    EmptyState {
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 420)
        visible: page.model !== null && !page.model.loading && page.model.count === 0
        icon: page.emptyIcon
        title: page.emptyTitle
        text: page.emptyText
    }
}
