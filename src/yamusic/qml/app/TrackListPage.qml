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
    property string emptyIcon: "music_note"
    property string emptyTitle: "Здесь пусто"
    property string emptyText: ""
    property Component headerExtra: null       // доп. содержимое шапки (кнопки и т. п.)
    // Шапка-«герой»: обложка + надзаголовок + название + «N треков · длительность» + Слушать/Перемешать
    property string overline                   // «ПЛЕЙЛИСТ», «КОЛЛЕКЦИЯ»
    property string heroImage                  // обложка (url); если пусто — фигура с иконкой
    property string heroIcon: "queue_music"
    property var heroShape: "cookie9"
    readonly property bool scrolled: list.contentY - list.originY > 48

    signal trackActivated(int row)

    function _duration(ms) {
        const m = Math.round(ms / 60000)
        return m >= 60 ? Math.floor(m / 60) + " ч " + (m % 60) + " мин" : m + " мин"
    }

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

        header: Item {
            width: list.width
            implicitHeight: hero.implicitHeight + 40
            onImplicitHeightChanged: list.keepTop()

            RowLayout {
                id: hero
                visible: page.showHeader
                x: 16
                y: 8
                width: parent.width - 32
                spacing: 28

                // Обложка или фигура с иконкой
                Item {
                    Layout.preferredWidth: page.width > 760 ? 184 : 128
                    Layout.preferredHeight: Layout.preferredWidth
                    Layout.alignment: Qt.AlignTop
                    MorphImage {
                        anchors.fill: parent
                        visible: page.heroImage !== ""
                        source: page.heroImage.replace("200x200", "400x400")
                        shape: "softSquare"
                    }
                    MorphShape {
                        anchors.fill: parent
                        visible: page.heroImage === ""
                        shape: page.heroShape
                        color: Theme.primaryContainer
                        Icon {
                            anchors.centerIn: parent
                            name: page.heroIcon
                            size: parent.width * 0.38
                            fill: 1
                            color: Theme.fgPrimaryContainer
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignBottom
                    spacing: 4
                    Label {
                        visible: page.overline !== ""
                        text: page.overline
                        type: "labelLarge"
                        color: Theme.primary
                        font.letterSpacing: 1.2
                    }
                    Label {
                        Layout.fillWidth: true
                        text: page.title
                        type: page.width > 760 ? "displayMedium" : "displaySmall"
                        weight: 700
                    }
                    Label {
                        visible: text !== ""
                        Layout.fillWidth: true
                        text: page.subtitle + (page.model && page.model.totalDurationMs > 0 && page.subtitle !== ""
                                               ? "  ·  " + page._duration(page.model.totalDurationMs) : "")
                        type: "bodyLarge"
                        color: Theme.fgSurfaceVariant
                    }
                    RowLayout {
                        Layout.topMargin: 14
                        spacing: 12
                        visible: page.model !== null && page.model.count > 0
                        Button {
                            text: "Слушать"
                            icon: "play_arrow"
                            size: "m"
                            onClicked: Player.playFrom(page.model, 0)
                        }
                        Button {
                            text: "Перемешать"
                            icon: "shuffle"
                            style: "tonal"
                            size: "m"
                            onClicked: Player.shuffleFrom(page.model)
                        }
                        Loader {
                            active: page.headerExtra !== null
                            sourceComponent: page.headerExtra
                        }
                    }
                }
            }
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
            liked: Library.likesRevision >= 0 && Library.isLiked(model.trackId)
            current: model.trackId === Player.trackId
            onActivated: {
                page.trackActivated(index)
                Player.playFrom(page.model, index)
            }
            onLikeClicked: Library.toggleLike(page.model.get(index))
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
