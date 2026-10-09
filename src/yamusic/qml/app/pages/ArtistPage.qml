import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

// Исполнитель: шапка (фото в фигуре, слушатели, Слушать / Перемешать / Волна), популярные треки,
// альбомы и синглы, релизы с участием, похожие исполнители.
Page {
    id: page

    property string artistId
    readonly property QtObject artist: Catalog.artist(artistId)
    readonly property var info: artist ? artist.info : ({})
    property bool allPopular: false
    property bool allAlbums: false

    title: info.name ?? ""
    showHeader: false

    function listeners(n) {
        if (n >= 1e6) return (n / 1e6).toFixed(n >= 1e7 ? 0 : 1).replace(".", ",") + " млн"
        if (n >= 1e3) return Math.round(n / 1e3) + " тыс."
        return String(n)
    }

    // ---- Шапка ----
    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: heroRow.implicitHeight + 32

        FloatingShapes {
            anchors.fill: parent
            anchors.margins: -16
            seed: page.info.name ?? ""
            opacity: 0.6
            running: page.visible && !page.scrolled
        }

        RowLayout {
            id: heroRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 32

            MorphImage {
                Layout.preferredWidth: page.width > 760 ? 208 : 144
                Layout.preferredHeight: Layout.preferredWidth
                source: (page.info.cover ?? "").replace("400x400", "600x600")
                shape: page.playingHere && Player.playing ? "cookie12" : "circle"
                duration: Theme.motion.spatialSlow
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Label { text: "ИСПОЛНИТЕЛЬ"; type: "labelLarge"; color: Theme.primary; font.letterSpacing: 1.2 }
                Label {
                    Layout.fillWidth: true
                    text: page.info.name ?? ""
                    type: page.width > 760 ? "displayMedium" : "displaySmall"
                    weight: 700
                }
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: [page.info.listeners ? page.listeners(page.info.listeners) + " слушателей за месяц" : "",
                           page.info.genres ?? ""].filter(x => x).join("  ·  ")
                    type: "bodyLarge"
                    color: Theme.fgSurfaceVariant
                }
                RowLayout {
                    Layout.topMargin: 14
                    spacing: 12
                    visible: page.artist && page.artist.popular.count > 0
                    Button {
                        text: "Слушать"
                        icon: "play_arrow"
                        size: "m"
                        onClicked: Player.playFrom(page.artist.popular, 0)
                    }
                    Button {
                        text: "Волна"
                        icon: "graphic_eq"
                        style: "tonal"
                        size: "m"
                        onClicked: Wave.playStation("artist:" + page.artistId, page.info.name ?? "")
                    }
                }
            }
        }
    }

    readonly property bool playingHere: Wave.active && Wave.stationTitle === (info.name ?? "-")
                                        || artist && Player.trackId !== "" && artist.popular.find("trackId", Player.trackId) >= 0

    LoadingIndicator {
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 48
        size: 64
        contained: true
        visible: page.artist && page.artist.loading
    }
    EmptyState {
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 48
        visible: page.artist && page.artist.errorText !== ""
        icon: "person_off"
        title: "Исполнитель не загрузился"
        text: page.artist ? page.artist.errorText : ""
    }

    // ---- Популярные треки ----
    SectionHeader {
        visible: page.artist && page.artist.popular.count > 0
        text: "Популярные треки"
        actionText: page.artist && page.artist.popular.count > 5 ? (page.allPopular ? "Свернуть" : "Все") : ""
        onAction: page.allPopular = !page.allPopular
    }
    ColumnLayout {
        Layout.fillWidth: true
        Layout.topMargin: -12
        spacing: 0
        Repeater {
            model: page.artist ? page.artist.popular : null
            TrackRow {
                required property int index
                required property var model
                visible: page.allPopular || index < 5
                Layout.fillWidth: true
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
                onActivated: Player.playFrom(page.artist.popular, index)
                onLikeClicked: Library.toggleLike(page.artist.popular.get(index))
            }
        }
    }

    // ---- Альбомы ----
    SectionHeader {
        visible: page.artist && page.artist.albums.count > 0
        text: "Альбомы и синглы"
        actionText: page.artist && page.artist.albums.count > tilesPerRow.value * 2 ? (page.allAlbums ? "Свернуть" : "Все") : ""
        onAction: page.allAlbums = !page.allAlbums
    }
    QtObject {
        id: tilesPerRow
        readonly property int value: Math.max(1, Math.floor((page.width - 2 * page.sideMargin + 20) / (168 + 20)))
    }
    Flow {
        Layout.fillWidth: true
        spacing: 20
        Repeater {
            model: page.artist ? page.artist.albums : null
            MediaTile {
                required property int index
                required property var model
                visible: page.allAlbums || index < tilesPerRow.value * 2
                cover: model.cover
                title: model.title
                subtitle: [model.year || "", model.kind].filter(x => x).join(" · ")
                explicit: model.explicit
                onClicked: Catalog.openAlbum(model.albumId)
            }
        }
    }

    SectionHeader {
        visible: page.artist && page.artist.alsoAlbums.count > 0
        text: "С участием"
    }
    Flow {
        Layout.fillWidth: true
        spacing: 20
        Repeater {
            model: page.artist ? page.artist.alsoAlbums : null
            MediaTile {
                required property int index
                required property var model
                visible: index < tilesPerRow.value
                cover: model.cover
                title: model.title
                subtitle: model.artists
                explicit: model.explicit
                onClicked: Catalog.openAlbum(model.albumId)
            }
        }
    }

    // ---- Похожие ----
    SectionHeader {
        visible: page.artist && page.artist.similar.count > 0
        text: "Похожие исполнители"
    }
    Flow {
        Layout.fillWidth: true
        spacing: 20
        Repeater {
            model: page.artist ? page.artist.similar : null
            MediaTile {
                required property int index
                required property var model
                visible: index < tilesPerRow.value
                size: 148
                round: true
                placeholderIcon: "person"
                cover: model.cover
                title: model.name
                onClicked: Catalog.openArtist(model.artistId)
            }
        }
    }
}
