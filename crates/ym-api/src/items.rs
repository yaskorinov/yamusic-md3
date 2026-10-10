//! Плоские словари для QML — ключи и значения ровно как в src/yamusic/models.py.
//! Сериализуются serde в camelCase и превращаются в QVariantMap одной функцией в оболочке.
use serde::{Deserialize, Serialize};

use crate::models::{Album, Artist, ArtistBriefInfo, Cover, IdString, Label, Playlist, SearchBest, Track};

pub const COVER_SIZE: &str = "400x400";
pub const PLAYLIST_COVER_SIZE: &str = "200x200";

/// 'avatars.yandex.net/get-music-content/…/%%' → https-URL нужного размера; пусто → "".
pub fn cover_url(uri: Option<&str>, size: &str) -> String {
    match uri {
        None | Some("") => String::new(),
        Some(uri) => {
            let uri = uri.replace("%%", size);
            if uri.starts_with("https://") || uri.starts_with("http://") {
                uri
            } else {
                format!("https://{uri}")
            }
        }
    }
}

/// Непустая строка или None (`x or y` в Python).
fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

fn id_str(id: &Option<IdString>) -> String {
    id.as_ref().map(|id| id.0.clone()).unwrap_or_default()
}

fn artist_names(artists: &[Artist]) -> String {
    artists.iter().filter_map(|a| non_empty(&a.name)).collect::<Vec<_>>().join(", ")
}

fn artist_refs(artists: &[Artist]) -> Vec<ArtistRef> {
    artists
        .iter()
        .filter_map(|a| {
            let id = a.id.as_ref().map(|id| id.0.as_str()).filter(|id| !id.is_empty() && *id != "0")?;
            Some(ArtistRef { id: id.to_owned(), name: non_empty(&a.name)?.to_owned() })
        })
        .collect()
}

/// cover.uri || cover.itemsUri[0] || ogImage
fn cover_uri<'a>(cover: &'a Option<Cover>, og_image: &'a Option<String>) -> Option<&'a str> {
    cover.as_ref().and_then(Cover::best_uri).or_else(|| non_empty(og_image))
}

fn to_map<T: Serialize>(value: &T) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
}

/// track_to_dict: {trackId, albumId, title, version, artists, artistRefs, album, cover, durationMs, explicit, available}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackItem {
    pub track_id: String,
    /// первый альбом трека или ""
    pub album_id: String,
    pub title: String,
    pub version: String,
    /// имена через ", "
    pub artists: String,
    /// только исполнители с id и именем
    pub artist_refs: Vec<ArtistRef>,
    pub album: String,
    /// cover_url(track.coverUri || album.coverUri, "400x400")
    pub cover: String,
    pub duration_ms: i64,
    /// contentWarning == "explicit"
    pub explicit: bool,
    /// available != false
    pub available: bool,
}

impl TrackItem {
    pub fn from_track(track: &Track) -> Self {
        let album = track.albums.as_ref().and_then(|albums| albums.first());
        let artists = track.artists.as_deref().unwrap_or_default();
        Self {
            track_id: id_str(&track.id),
            album_id: album.map(|a| id_str(&a.id)).unwrap_or_default(),
            title: track.title.clone().unwrap_or_default(),
            version: track.version.clone().unwrap_or_default(),
            artists: artist_names(artists),
            artist_refs: artist_refs(artists),
            album: album.and_then(|a| a.title.clone()).unwrap_or_default(),
            cover: cover_url(non_empty(&track.cover_uri).or_else(|| album.and_then(|a| non_empty(&a.cover_uri))), COVER_SIZE),
            duration_ms: track.duration_ms.unwrap_or(0),
            explicit: track.content_warning.as_deref() == Some("explicit"),
            available: track.available != Some(false),
        }
    }

    /// 'id:albumId' (или просто 'id' без альбома) — так API лайков и волны однозначно находит трек.
    pub fn full_id(&self) -> String {
        if self.album_id.is_empty() {
            self.track_id.clone()
        } else {
            format!("{}:{}", self.track_id, self.album_id)
        }
    }

    pub fn to_row(&self) -> serde_json::Map<String, serde_json::Value> {
        to_map(self)
    }
}

/// Тип альбома для подписи: правило album_to_dict.
pub fn album_kind_label(kind: Option<&str>, track_count: Option<i64>) -> String {
    let label = match kind.unwrap_or_default() {
        "single" => "Сингл",
        "compilation" => "Сборник",
        "podcast" => "Подкаст",
        "audiobook" => "Аудиокнига",
        _ => "Альбом",
    };
    match track_count {
        Some(count) if label == "Альбом" && count > 0 && count <= 6 && kind != Some("compilation") => {
            if count > 1 { "EP" } else { "Сингл" }.to_owned()
        }
        _ => label.to_owned(),
    }
}

/// album_to_dict: {albumId, title, version, artists, artistRefs, year, kind, trackCount, cover, explicit}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumItem {
    pub album_id: String,
    pub title: String,
    pub version: String,
    pub artists: String,
    pub artist_refs: Vec<ArtistRef>,
    pub year: i64,
    /// «Альбом» | «Сингл» | «Сборник» | «Подкаст» | «Аудиокнига» | «EP» — правило из models.py
    pub kind: String,
    pub track_count: i64,
    /// cover_url(coverUri || ogImage, "400x400")
    pub cover: String,
    pub explicit: bool,
}

impl AlbumItem {
    pub fn from_album(album: &Album) -> Self {
        let artists = album.artists.as_deref().unwrap_or_default();
        Self {
            album_id: id_str(&album.id),
            title: album.title.clone().unwrap_or_default(),
            version: album.version.clone().unwrap_or_default(),
            artists: artist_names(artists),
            artist_refs: artist_refs(artists),
            year: album.year.unwrap_or(0),
            kind: album_kind_label(album.kind.as_deref(), album.track_count),
            track_count: album.track_count.unwrap_or(0),
            cover: cover_url(non_empty(&album.cover_uri).or_else(|| non_empty(&album.og_image)), COVER_SIZE),
            explicit: album.content_warning.as_deref() == Some("explicit"),
        }
    }

    pub fn to_row(&self) -> serde_json::Map<String, serde_json::Value> {
        to_map(self)
    }
}

/// artist_to_dict: {artistId, name, cover, genres}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistItem {
    pub artist_id: String,
    pub name: String,
    /// cover.uri || cover.itemsUri[0] || ogImage, "400x400"
    pub cover: String,
    /// жанры через ", "
    pub genres: String,
}

impl ArtistItem {
    pub fn from_artist(artist: &Artist) -> Self {
        Self {
            artist_id: id_str(&artist.id),
            name: artist.name.clone().unwrap_or_default(),
            cover: cover_url(cover_uri(&artist.cover, &artist.og_image), COVER_SIZE),
            genres: artist.genres.as_deref().unwrap_or_default().join(", "),
        }
    }

    pub fn to_row(&self) -> serde_json::Map<String, serde_json::Value> {
        to_map(self)
    }
}

/// playlist_to_dict: {uid, kind, title, trackCount, cover, owner}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaylistItem {
    /// playlist.uid || owner.uid || 0
    pub uid: i64,
    pub kind: i64,
    pub title: String,
    pub track_count: i64,
    /// cover.uri || cover.itemsUri[0] || ogImage, "200x200"
    pub cover: String,
    /// owner.name || owner.login || ""
    pub owner: String,
}

impl PlaylistItem {
    pub fn from_playlist(playlist: &Playlist) -> Self {
        let owner = playlist.owner.as_ref();
        Self {
            uid: playlist.owner_uid().unwrap_or(0),
            kind: playlist.kind.unwrap_or(0),
            title: playlist.title.clone().unwrap_or_default(),
            track_count: playlist.track_count.unwrap_or(0),
            cover: cover_url(cover_uri(&playlist.cover, &playlist.og_image), PLAYLIST_COVER_SIZE),
            owner: owner
                .and_then(|o| non_empty(&o.name).or_else(|| non_empty(&o.login)))
                .unwrap_or_default()
                .to_owned(),
        }
    }

    pub fn to_row(&self) -> serde_json::Map<String, serde_json::Value> {
        to_map(self)
    }
}

/// Catalog.album(id).info: album_to_dict + label, genre, durationMs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumInfo {
    #[serde(flatten)]
    pub item: AlbumItem,
    /// названия лейблов через ", "
    pub label: String,
    pub genre: String,
    /// сумма длительностей треков
    pub duration_ms: i64,
}

impl AlbumInfo {
    /// Из ответа /albums/{id}/with-tracks (как done() в catalog.py).
    pub fn from_album(album: &Album) -> Self {
        let labels: Vec<&str> =
            album.labels.iter().flatten().filter_map(Label::name).filter(|name| !name.is_empty()).collect();
        Self {
            item: AlbumItem::from_album(album),
            label: labels.join(", "),
            genre: album.genre.clone().unwrap_or_default(),
            duration_ms: album.all_tracks().map(|t| t.duration_ms.unwrap_or(0)).sum(),
        }
    }

    pub fn to_row(&self) -> serde_json::Map<String, serde_json::Value> {
        to_map(self)
    }
}

/// Catalog.artist(id).info: artist_to_dict + listeners, likes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistInfo {
    #[serde(flatten)]
    pub item: ArtistItem,
    /// stats.lastMonthListeners || 0
    pub listeners: i64,
    /// artist.likesCount || 0
    pub likes: i64,
}

impl ArtistInfo {
    /// Из ответа /artists/{id}/brief-info.
    pub fn from_brief(brief: &ArtistBriefInfo) -> Self {
        let artist = brief.artist.clone().unwrap_or_default();
        Self {
            item: ArtistItem::from_artist(&artist),
            listeners: brief.stats.as_ref().and_then(|s| s.last_month_listeners).unwrap_or(0),
            likes: artist.likes_count.unwrap_or(0),
        }
    }

    pub fn to_row(&self) -> serde_json::Map<String, serde_json::Value> {
        to_map(self)
    }
}

/// Лучший результат поиска → (тип, словарь) для Catalog.best = {type, item}; неизвестный тип — None.
pub fn best_item(best: &SearchBest) -> Option<(String, serde_json::Map<String, serde_json::Value>)> {
    let kind = best.kind.as_deref()?;
    let raw = best.result.clone().filter(|v| v.is_object())?;
    let row = match kind {
        "track" => TrackItem::from_track(&serde_json::from_value(raw).ok()?).to_row(),
        "album" => AlbumItem::from_album(&serde_json::from_value(raw).ok()?).to_row(),
        "artist" => ArtistItem::from_artist(&serde_json::from_value(raw).ok()?).to_row(),
        "playlist" => PlaylistItem::from_playlist(&serde_json::from_value(raw).ok()?).to_row(),
        _ => return None,
    };
    Some((kind.to_owned(), row))
}

/// Альбомы исполнителя по убыванию (year, releaseDate) — сортировка из catalog.py.
pub fn sort_albums_desc(albums: &mut [Album]) {
    albums.sort_by(|a, b| {
        let key = |x: &Album| (x.year.unwrap_or(0), x.release_date.clone().unwrap_or_default());
        key(b).cmp(&key(a))
    });
}

/// Контекст очереди (откуда треки): {"type": wave|liked|playlist|album|artist|search|restored, "from": …, "playlistId": "uid:kind"?}.
/// Ключи сериализации совпадают с Python (session.json совместим).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayContext {
    #[serde(rename = "type")]
    pub kind: String,
    pub from: String,
    #[serde(rename = "playlistId", skip_serializing_if = "Option::is_none")]
    pub playlist_id: Option<String>,
}

impl PlayContext {
    pub fn new(kind: &str, from: &str) -> Self {
        Self { kind: kind.to_owned(), from: from.to_owned(), playlist_id: None }
    }
}

/// Контексты списков — значения `from` как у официального клиента.
pub mod contexts {
    pub const LIKED: (&str, &str) = ("liked", "desktop_win-own_tracks-track-default");
    pub const PLAYLIST: (&str, &str) = ("playlist", "desktop_win-playlist-track-default");
    pub const ALBUM: (&str, &str) = ("album", "desktop_win-album-track-default");
    pub const ARTIST: (&str, &str) = ("artist", "desktop_win-artist-track-default");
    pub const SEARCH: (&str, &str) = ("search", "desktop_win-search-track-default");
    pub const WAVE: (&str, &str) = ("wave", "desktop_win-radio-user-onyourwave-default");
    /// play-audio без контекста
    pub const DEFAULT_FROM: &str = "desktop_win-own_tracks-track-default";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Search;
    use serde_json::json;

    #[test]
    fn cover() {
        assert_eq!(cover_url(Some("avatars.yandex.net/get-music-content/1/a/%%"), "400x400"),
                   "https://avatars.yandex.net/get-music-content/1/a/400x400");
        assert_eq!(cover_url(None, COVER_SIZE), "");
        assert_eq!(cover_url(Some(""), COVER_SIZE), "");
    }

    #[test]
    fn track_dict() {
        let track: Track = serde_json::from_value(json!({
            "id": 101, "title": "Песня", "durationMs": 180000, "contentWarning": "explicit",
            "artists": [{"id": 1, "name": "Один"}, {"name": "Без id"}, {"id": 3, "name": ""}],
            "albums": [{"id": 55, "title": "Альбом", "coverUri": "avatars.yandex.net/a/%%"}]
        }))
        .unwrap();
        let item = TrackItem::from_track(&track);
        let row = item.to_row();
        assert_eq!(serde_json::Value::Object(row), json!({
            "trackId": "101", "albumId": "55", "title": "Песня", "version": "",
            "artists": "Один, Без id", "artistRefs": [{"id": "1", "name": "Один"}],
            "album": "Альбом", "cover": "https://avatars.yandex.net/a/400x400",
            "durationMs": 180000, "explicit": true, "available": true
        }));
        assert_eq!(item.full_id(), "101:55");
        let bare = TrackItem::from_track(&serde_json::from_value(json!({"id": "7", "available": false})).unwrap());
        assert_eq!(bare.full_id(), "7");
        assert!(!bare.available);
        assert_eq!(bare.cover, "");
        // из QML-словаря без части полей
        let partial: TrackItem = serde_json::from_value(json!({"trackId": "9", "albumId": "1"})).unwrap();
        assert_eq!(partial.full_id(), "9:1");
    }

    #[test]
    fn album_kinds() {
        let kind = |t: Option<&str>, n: Option<i64>| album_kind_label(t, n);
        assert_eq!(kind(Some("single"), Some(1)), "Сингл");
        assert_eq!(kind(Some("compilation"), Some(3)), "Сборник");
        assert_eq!(kind(Some("podcast"), None), "Подкаст");
        assert_eq!(kind(Some("audiobook"), Some(2)), "Аудиокнига");
        assert_eq!(kind(None, Some(12)), "Альбом");
        assert_eq!(kind(None, Some(5)), "EP");
        assert_eq!(kind(None, Some(1)), "Сингл");
        assert_eq!(kind(None, Some(0)), "Альбом");
        assert_eq!(kind(None, None), "Альбом");
    }

    #[test]
    fn album_info() {
        let album: Album = serde_json::from_value(json!({
            "id": 9, "title": "A", "year": 2021, "trackCount": 10, "genre": "rock", "ogImage": "og/%%",
            "artists": [{"id": 2, "name": "B"}], "labels": ["L1", {"id": 1, "name": "L2"}, {"id": 2}],
            "volumes": [[{"id": 1, "durationMs": 1000}], [{"id": 2, "durationMs": 2500}]]
        }))
        .unwrap();
        let row = AlbumInfo::from_album(&album).to_row();
        assert_eq!(serde_json::Value::Object(row), json!({
            "albumId": "9", "title": "A", "version": "", "artists": "B", "artistRefs": [{"id": "2", "name": "B"}],
            "year": 2021, "kind": "Альбом", "trackCount": 10, "cover": "https://og/400x400", "explicit": false,
            "label": "L1, L2", "genre": "rock", "durationMs": 3500
        }));
    }

    #[test]
    fn artist_and_playlist() {
        let brief: ArtistBriefInfo = serde_json::from_value(json!({
            "artist": {"id": 4, "name": "Art", "cover": {"type": "from-artist-photos", "itemsUri": ["c/%%"]},
                       "genres": ["pop", "rock"], "likesCount": 77},
            "stats": {"lastMonthListeners": 1234}
        }))
        .unwrap();
        let info = ArtistInfo::from_brief(&brief);
        assert_eq!(serde_json::Value::Object(info.to_row()), json!({
            "artistId": "4", "name": "Art", "cover": "https://c/400x400", "genres": "pop, rock",
            "listeners": 1234, "likes": 77
        }));

        let playlist: Playlist = serde_json::from_value(json!({
            "kind": 3, "title": "Мой", "trackCount": 12, "owner": {"uid": 42, "login": "me", "name": ""},
            "cover": {"type": "mosaic", "itemsUri": ["m1/%%", "m2/%%"]}
        }))
        .unwrap();
        assert_eq!(PlaylistItem::from_playlist(&playlist), PlaylistItem {
            uid: 42, kind: 3, title: "Мой".into(), track_count: 12, cover: "https://m1/200x200".into(), owner: "me".into()
        });
    }

    #[test]
    fn search_best() {
        let search: Search = serde_json::from_value(json!({
            "best": {"type": "album", "result": {"id": 1, "title": "X", "type": "single", "trackCount": 1}}
        }))
        .unwrap();
        let (kind, row) = best_item(search.best.as_ref().unwrap()).unwrap();
        assert_eq!(kind, "album");
        assert_eq!(row["kind"], "Сингл");
        assert!(best_item(&SearchBest { kind: Some("video".into()), result: Some(json!({})) }).is_none());
    }

    #[test]
    fn albums_sorted() {
        let mut albums: Vec<Album> = serde_json::from_value(json!([
            {"id": 1, "year": 2019}, {"id": 2, "year": 2021, "releaseDate": "2021-01"},
            {"id": 3, "year": 2021, "releaseDate": "2021-05"}, {"id": 4}
        ]))
        .unwrap();
        sort_albums_desc(&mut albums);
        let ids: Vec<_> = albums.iter().map(|a| a.id.clone().unwrap().0).collect();
        assert_eq!(ids, ["3", "2", "1", "4"]);
    }

    #[test]
    fn context_json() {
        let mut ctx = PlayContext::new(contexts::PLAYLIST.0, contexts::PLAYLIST.1);
        assert_eq!(serde_json::to_value(&ctx).unwrap(), json!({"type": "playlist", "from": "desktop_win-playlist-track-default"}));
        ctx.playlist_id = Some("1:2".into());
        assert_eq!(serde_json::to_value(&ctx).unwrap()["playlistId"], "1:2");
    }
}
