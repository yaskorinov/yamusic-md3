//! Каталог (src/yamusic/catalog.py): данные страниц альбома/исполнителя и поиск. Состояние страниц и поиска
//! (кэш AlbumData/ArtistData, searchState, поколения запросов) — в мосте Catalog; здесь только загрузка.
use std::sync::Arc;

use serde::Serialize;
use ym_api::items::{AlbumInfo, AlbumItem, ArtistInfo, ArtistItem, PlaylistItem, TrackItem};
use ym_api::models::{self, Album, ArtistBriefInfo, Label, Search};
use ym_api::{SearchType, YmClient};

use crate::ServiceError;
use crate::auth::AuthService;

/// Вкладки поиска QML: "all" | "tracks" | "albums" | "artists" | "playlists" (иное → All).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchKind {
    #[default]
    All,
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl SearchKind {
    pub fn from_name(name: &str) -> Self {
        match name {
            "tracks" => SearchKind::Tracks,
            "albums" => SearchKind::Albums,
            "artists" => SearchKind::Artists,
            "playlists" => SearchKind::Playlists,
            _ => SearchKind::All,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SearchKind::All => "all",
            SearchKind::Tracks => "tracks",
            SearchKind::Albums => "albums",
            SearchKind::Artists => "artists",
            SearchKind::Playlists => "playlists",
        }
    }

    pub fn api_type(self) -> SearchType {
        match self {
            SearchKind::All => SearchType::All,
            SearchKind::Tracks => SearchType::Track,
            SearchKind::Albums => SearchType::Album,
            SearchKind::Artists => SearchType::Artist,
            SearchKind::Playlists => SearchType::Playlist,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AlbumPage {
    /// album_to_dict + label (лейблы через ", "), genre, durationMs (сумма треков)
    pub info: AlbumInfo,
    /// треки всех дисков подряд
    pub tracks: Vec<TrackItem>,
}

impl AlbumPage {
    pub fn from_album(album: &Album) -> Self {
        let tracks: Vec<TrackItem> = album.volumes.iter().flatten().flatten().map(TrackItem::from_track).collect();
        let label = album
            .labels
            .iter()
            .flatten()
            .filter_map(|l| match l {
                Label::Name(name) => Some(name.as_str()),
                Label::Object { name, .. } => name.as_deref(),
            })
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        let duration_ms = album.volumes.iter().flatten().flatten().map(|t| t.duration_ms.unwrap_or(0)).sum();
        let info = AlbumInfo {
            item: AlbumItem::from_album(album),
            label,
            genre: album.genre.clone().unwrap_or_default(),
            duration_ms,
        };
        AlbumPage { info, tracks }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ArtistPage {
    /// artist_to_dict + listeners, likes
    pub info: ArtistInfo,
    pub popular: Vec<TrackItem>,
    /// direct-albums (page-size 100; при ошибке — brief.albums), по (year, releaseDate) убыв.
    pub albums: Vec<AlbumItem>,
    pub also_albums: Vec<AlbumItem>,
    pub similar: Vec<ArtistItem>,
}

impl ArtistPage {
    /// `direct` — альбомы из direct-albums (None — запрос не удался или пусто).
    pub fn from_brief(brief: &ArtistBriefInfo, direct: Option<Vec<Album>>) -> Self {
        let artist = brief.artist.clone().unwrap_or_default();
        let info = ArtistInfo {
            item: ArtistItem::from_artist(&artist),
            listeners: brief.stats.as_ref().and_then(|s| s.last_month_listeners).unwrap_or(0),
            likes: artist.likes_count.unwrap_or(0),
        };
        let mut own = direct.unwrap_or_else(|| brief.albums.clone().unwrap_or_default());
        sort_albums(&mut own);
        ArtistPage {
            info,
            popular: brief.popular_tracks.iter().flatten().map(TrackItem::from_track).collect(),
            albums: own.iter().map(AlbumItem::from_album).collect(),
            also_albums: brief.also_albums.iter().flatten().map(AlbumItem::from_album).collect(),
            similar: brief.similar_artists.iter().flatten().map(ArtistItem::from_artist).collect(),
        }
    }
}

/// sorted(key=(year or 0, release_date or ""), reverse=True); сортировка устойчивая, как в Python.
fn sort_albums(albums: &mut [Album]) {
    albums.sort_by(|a, b| {
        let key = |x: &Album| (x.year.unwrap_or(0), x.release_date.clone().unwrap_or_default());
        key(b).cmp(&key(a))
    });
}

/// Catalog.totals: {tracks, albums, artists, playlists} (0, если блока нет)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SearchTotals {
    pub tracks: i64,
    pub albums: i64,
    pub artists: i64,
    pub playlists: i64,
}

/// Catalog.best: {"type": "track"|"album"|"artist"|"playlist", "item": {…}}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", content = "item", rename_all = "lowercase")]
pub enum SearchBest {
    Track(TrackItem),
    Album(AlbumItem),
    Artist(ArtistItem),
    Playlist(PlaylistItem),
}

impl SearchBest {
    /// best из ответа /search: тип и сырой объект; неизвестный тип или битый объект — None.
    pub fn from_api(best: &models::SearchBest) -> Option<Self> {
        let value = best.result.clone().filter(|v| !v.is_null())?;
        match best.kind.as_deref()? {
            "track" => serde_json::from_value(value).ok().map(|t| SearchBest::Track(TrackItem::from_track(&t))),
            "album" => serde_json::from_value(value).ok().map(|a| SearchBest::Album(AlbumItem::from_album(&a))),
            "artist" => serde_json::from_value(value).ok().map(|a| SearchBest::Artist(ArtistItem::from_artist(&a))),
            "playlist" => {
                serde_json::from_value(value).ok().map(|p| SearchBest::Playlist(PlaylistItem::from_playlist(&p)))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SearchResults {
    pub tracks: Vec<TrackItem>,
    pub albums: Vec<AlbumItem>,
    pub artists: Vec<ArtistItem>,
    pub playlists: Vec<PlaylistItem>,
    pub totals: SearchTotals,
    pub best: Option<SearchBest>,
}

impl SearchResults {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.albums.is_empty() && self.artists.is_empty() && self.playlists.is_empty()
    }

    pub fn from_api(search: &Search) -> Self {
        fn results<T>(block: &Option<models::SearchBlock<T>>) -> impl Iterator<Item = &T> {
            block.iter().flat_map(|b| b.results.iter().flatten())
        }
        fn total<T>(block: &Option<models::SearchBlock<T>>) -> i64 {
            block.as_ref().and_then(|b| b.total).unwrap_or(0)
        }
        SearchResults {
            tracks: results(&search.tracks).map(TrackItem::from_track).collect(),
            albums: results(&search.albums).map(AlbumItem::from_album).collect(),
            artists: results(&search.artists).map(ArtistItem::from_artist).collect(),
            playlists: results(&search.playlists).map(PlaylistItem::from_playlist).collect(),
            totals: SearchTotals {
                tracks: total(&search.tracks),
                albums: total(&search.albums),
                artists: total(&search.artists),
                playlists: total(&search.playlists),
            },
            best: search.best.as_ref().and_then(SearchBest::from_api),
        }
    }
}

pub struct CatalogService {
    auth: Arc<AuthService>,
}

impl CatalogService {
    pub fn new(auth: Arc<AuthService>) -> Arc<Self> {
        Arc::new(Self { auth })
    }

    fn client(&self) -> Result<YmClient, ServiceError> {
        self.auth.client().ok_or(ServiceError::NotSignedIn)
    }

    /// GET /albums/{id}/with-tracks
    pub async fn album(&self, album_id: &str) -> Result<AlbumPage, ServiceError> {
        let album = self.client()?.albums_with_tracks(album_id).await?;
        Ok(AlbumPage::from_album(&album))
    }

    /// brief-info + direct-albums
    pub async fn artist(&self, artist_id: &str) -> Result<ArtistPage, ServiceError> {
        let client = self.client()?;
        let brief = client.artists_brief_info(artist_id).await?;
        let direct = match client.artists_direct_albums(artist_id, 0, 100).await {
            Ok(direct) => direct.albums.filter(|a| !a.is_empty()),
            Err(e) => {
                tracing::debug!("direct-albums {artist_id}: {e}");
                None
            }
        };
        Ok(ArtistPage::from_brief(&brief, direct))
    }

    /// /search (nocorrect=false); треки null отбрасываются.
    pub async fn search(&self, query: &str, kind: SearchKind) -> Result<SearchResults, ServiceError> {
        let search = self.client()?.search(query, kind.api_type(), false, 0).await?;
        Ok(SearchResults::from_api(&search))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_kinds() {
        for name in ["all", "tracks", "albums", "artists", "playlists"] {
            assert_eq!(SearchKind::from_name(name).name(), name);
        }
        assert_eq!(SearchKind::from_name("podcasts"), SearchKind::All);
        assert_eq!(SearchKind::Tracks.api_type(), SearchType::Track);
        assert_eq!(SearchKind::All.api_type(), SearchType::All);
    }

    #[test]
    fn albums_sorted_newest_first() {
        let album = |id: &str, year: Option<i64>, date: Option<&str>| Album {
            id: Some(models::IdString(id.into())),
            year,
            release_date: date.map(Into::into),
            ..Album::default()
        };
        let mut albums = vec![
            album("a", Some(2001), None),
            album("b", Some(2020), Some("2020-01-01")),
            album("c", None, None),
            album("d", Some(2020), Some("2020-06-01")),
            album("e", Some(2001), None),
        ];
        sort_albums(&mut albums);
        let ids: Vec<_> = albums.iter().map(|a| a.id.as_ref().unwrap().0.as_str()).collect();
        assert_eq!(ids, ["d", "b", "a", "e", "c"]);
    }

    #[test]
    fn search_totals_serialize() {
        let totals = SearchTotals { tracks: 1, albums: 2, artists: 3, playlists: 4 };
        assert_eq!(
            serde_json::to_value(&totals).unwrap(),
            serde_json::json!({"tracks": 1, "albums": 2, "artists": 3, "playlists": 4})
        );
    }

    #[test]
    fn unknown_best_ignored() {
        let best = models::SearchBest { kind: Some("podcast_episode".into()), result: Some(serde_json::json!({})) };
        assert!(SearchBest::from_api(&best).is_none());
        let best = models::SearchBest { kind: Some("track".into()), result: None };
        assert!(SearchBest::from_api(&best).is_none());
    }
}
