import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Страница со списком треков (виртуализированный ListView): шапка прокручивается вместе со списком.
// Поиск по списку (кнопка в шапке или Ctrl+F): пока в поле есть текст, список показывает найденное,
// и «Слушать», «Перемешать», клик по треку ставят в очередь именно найденное.
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
    property var heroArtistRefs: []            // исполнители-ссылки под названием (альбом)
    readonly property bool scrolled: list.contentY - list.originY > 48

    property bool searchOpen: false
    property string searchText
    readonly property var shownModel: model ? filter : null   // то, что в списке: всё или найденное

    // Ctrl+F: открыть поиск по списку. false — искать тут нечего или поле уже в фокусе
    // (тогда повторный Ctrl+F уходит в общий поиск).
    function findInPage() {
        if (!model || model.count === 0 || !showHeader)
            return false
        const field = list.headerItem ? list.headerItem.searchField : null
        if (searchOpen && field && field.inputItem.activeFocus)
            return false
        searchOpen = true
        list.userScrolled = false
        list.positionViewAtBeginning()
        Qt.callLater(() => list.headerItem && list.headerItem.searchField.inputItem.forceActiveFocus())
        return true
    }
    function closeSearch() {
        searchOpen = false
        searchText = ""
    }

    TrackFilter {
        id: filter
        source: page.model
        query: page.searchText
    }

    signal trackActivated(int row)

    function _duration(ms) {
        const m = Math.round(ms / 60000)
        return m >= 60 ? Math.floor(m / 60) + " ч " + (m % 60) + " мин" : m + " мин"
    }

    // Медленно плывущие фигуры за шапкой. Слой вне ListView (иначе его clip режет фигуры по краю
    // списка) и заходит под прозрачную верхнюю панель; сверху и снизу фигуры растворяются,
    // поэтому видимой рамки нет. Едет вместе с шапкой, стоит, когда она уехала из виду.
    HeroShapes {
        width: page.width
        active: page.showHeader && page.visible
        headerHeight: list.headerItem ? list.headerItem.height : 320
        scroll: list.contentY - list.originY
        contentX: list.x
        contentWidth: list.width
        seed: page.title
    }

    ListView {
        id: list
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        model: page.shownModel
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
            target: page.shownModel
            ignoreUnknownSignals: true
            function onModelReset() { list.userScrolled = false; list.keepTop() }
        }

        header: Item {
            id: headerItem
            readonly property alias searchField: searchField
            width: list.width
            implicitHeight: hero.implicitHeight + 56
            onImplicitHeightChanged: list.keepTop()

            RowLayout {
                id: hero
                visible: page.showHeader
                x: 16
                y: 24     // запас сверху: фигура за обложкой больше неё и не должна упираться в край списка
                width: parent.width - 32
                spacing: 28

                // Обложка или фигура с иконкой; за обложкой — медленно вращающаяся фигура
                Item {
                    Layout.preferredWidth: page.width > 760 ? 184 : 128
                    Layout.preferredHeight: Layout.preferredWidth
                    Layout.alignment: Qt.AlignTop
                    MorphShape {
                        id: coverBackdrop
                        visible: page.heroImage !== ""
                        anchors.centerIn: parent
                        width: parent.width * 1.3
                        height: width
                        shape: "cookie9"
                        color: Theme.surfaceContainerHighest
                    }
                    FrameAnimation {
                        running: coverBackdrop.visible && page.visible && !Theme.calm && list.contentY - list.originY < 300
                        onTriggered: coverBackdrop.rotation = (coverBackdrop.rotation + frameTime * 6) % 360
                    }
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
                    Flow {
                        visible: (page.heroArtistRefs ?? []).length > 0
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: page.heroArtistRefs
                            LinkLabel {
                                required property var modelData
                                required property int index
                                text: modelData.name + (index < page.heroArtistRefs.length - 1 ? "," : "")
                                type: "titleLarge"
                                weight: 600
                                onClicked: Catalog.openArtist(modelData.id)
                            }
                        }
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
                            enabled: page.shownModel !== null && page.shownModel.count > 0
                            onClicked: Player.playFrom(page.shownModel, 0)
                        }
                        Button {
                            text: "Перемешать"
                            icon: "shuffle"
                            style: "tonal"
                            size: "m"
                            enabled: page.shownModel !== null && page.shownModel.count > 0
                            onClicked: Player.shuffleFrom(page.shownModel)
                        }
                        Loader {
                            active: page.headerExtra !== null
                            sourceComponent: page.headerExtra
                        }
                        IconButton {
                            icon: "search"
                            size: "m"
                            style: page.searchOpen ? "tonal" : "standard"
                            checkable: true
                            autoToggle: false
                            checked: page.searchOpen
                            onClicked: page.searchOpen ? page.closeSearch() : page.findInPage()
                        }
                    }
                    RowLayout {
                        visible: page.searchOpen
                        Layout.fillWidth: true
                        Layout.topMargin: 10
                        spacing: 16
                        SearchField {
                            id: searchField
                            Layout.fillWidth: true
                            Layout.maximumWidth: 420
                            placeholder: "Найти в списке"
                            text: page.searchText
                            onTextChanged: page.searchText = text
                            Keys.onEscapePressed: page.closeSearch()
                        }
                        Label {
                            visible: filter.active
                            text: filter.count + " из " + (page.model ? page.model.count : 0)
                            type: "bodyMedium"
                            color: Theme.fgSurfaceVariant
                        }
                        Item { Layout.fillWidth: true }
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
            artistRefs: model.artistRefs
            albumId: model.albumId
            album: model.album
            cover: model.cover
            durationMs: model.durationMs
            explicit: model.explicit
            available: model.available
            liked: Library.likesRevision >= 0 && Library.isLiked(model.trackId)
            current: model.trackId === Player.trackId
            onActivated: {
                page.trackActivated(index)
                Player.playFrom(page.shownModel, index)
            }
            onLikeClicked: Library.toggleLike(page.shownModel.get(index))
        }

        footer: Item {
            width: list.width
            height: 120 + (page.model && page.model.loading && page.model.count > 0 ? 64 : 0)
            LoadingIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.top
                visible: page.model && page.model.loading && page.model.count > 0
            }
            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.top
                anchors.topMargin: 8
                width: Math.min(parent.width - 32, 520)
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                visible: filter.active && filter.count === 0 && !(page.model && page.model.loading)
                text: "По запросу «" + page.searchText.trim() + "» в этом списке ничего нет"
                type: "bodyLarge"
                color: Theme.fgSurfaceVariant
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
