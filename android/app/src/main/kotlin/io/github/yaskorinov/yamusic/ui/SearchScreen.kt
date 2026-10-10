package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.api.Artist
import io.github.yaskorinov.yamusic.api.Best
import io.github.yaskorinov.yamusic.api.SearchResult
import io.github.yaskorinov.yamusic.data.Catalog
import io.github.yaskorinov.yamusic.data.SearchState
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

private val FILTER_OPTIONS = listOf("Всё", "Треки", "Альбомы", "Исполнители").mapIndexed { index, title -> "$index" to title }
private const val ALL = 0
private const val TRACKS = 1
private const val ALBUMS = 2
private const val ARTISTS = 3

/**
 * Поиск (перенос SearchPage.qml): поле с задержкой ввода, фильтры-чипы. «Всё» — лучший результат
 * и по несколько из каждой категории, остальные фильтры — вся категория.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SearchScreen(
    catalog: Catalog,
    player: PlayerConnection,
    playerState: PlayerState,
    contentPadding: PaddingValues,
    onOpenAlbum: (Album) -> Unit,
    onOpenArtist: (Artist) -> Unit,
) {
    val query by catalog.query.collectAsStateWithLifecycle()
    val state by catalog.search.collectAsStateWithLifecycle()
    var filter by rememberSaveable { mutableIntStateOf(ALL) }
    val focus = LocalFocusManager.current
    Column(Modifier.fillMaxSize().padding(top = contentPadding.calculateTopPadding())) {
        SearchField(
            query, catalog::search, "Трек, альбом, исполнитель",
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 12.dp),
            onSearch = { focus.clearFocus() },
        )
        FlowChoice(
            FILTER_OPTIONS, "$filter", { filter = it.toInt() },
            Modifier.padding(start = 16.dp, end = 16.dp, top = 10.dp, bottom = 6.dp),
        )
        val bottom = contentPadding.calculateBottomPadding() + 16.dp
        // Состояние поиска и фильтр сменяются через fade through: новое всплывает снизу
        AnimatedContent(
            targetState = state to filter,
            contentKey = { (search, chosen) -> if (search is SearchState.Found) "found:$chosen:${search.result.hashCode()}" else search::class },
            transitionSpec = {
                (fadeIn(tween(260, delayMillis = 60)) + slideInVertically(tween(460, delayMillis = 60, easing = Motion.EmphasizedDecelerate)) { it / 24 }) togetherWith
                    fadeOut(tween(120))
            },
            label = "search",
        ) { (search, chosen) ->
            when (search) {
                is SearchState.Found -> Results(search.result, chosen, { filter = it }, player, playerState, bottom, onOpenAlbum, onOpenArtist)
                SearchState.Loading -> Box(Modifier.fillMaxSize().padding(top = 64.dp), contentAlignment = Alignment.TopCenter) {
                    LoadingIndicator(Modifier.size(64.dp))
                }
                else -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
                    EmptyState(
                        icon = when (search) {
                            SearchState.Empty -> "search_off"
                            is SearchState.Failed -> "cloud_off"
                            else -> "travel_explore"
                        },
                        title = when (search) {
                            SearchState.Empty -> "Ничего не нашлось"
                            is SearchState.Failed -> "Поиск не ответил"
                            else -> "Что послушаем?"
                        },
                        modifier = Modifier.fillMaxWidth().padding(top = 48.dp),
                        text = when (search) {
                            SearchState.Empty -> "Попробуйте написать иначе"
                            is SearchState.Failed -> search.error
                            else -> "Ищите треки, альбомы и исполнителей"
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun Results(
    result: SearchResult,
    filter: Int,
    onFilter: (Int) -> Unit,
    player: PlayerConnection,
    playerState: PlayerState,
    bottom: androidx.compose.ui.unit.Dp,
    onOpenAlbum: (Album) -> Unit,
    onOpenArtist: (Artist) -> Unit,
) {
    val focus = LocalFocusManager.current
    val all = filter == ALL
    val tracks = if (all) result.tracks.take(5) else result.tracks
    val artists = if (all) result.artists.take(TILE_COLUMNS) else result.artists
    val albums = if (all) result.albums.take(TILE_COLUMNS) else result.albums
    val play: (Int) -> Unit = {
        focus.clearFocus()
        player.play(result.tracks, it, PlayContext.Search)
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = bottom)) {
        val best = result.best
        if (all && best != null) {
            item(key = "best") {
                BestCard(best, playerState, onClick = {
                    when {
                        best.artist != null -> onOpenArtist(best.artist)
                        best.album != null -> onOpenAlbum(best.album)
                        best.track != null -> result.tracks.indexOfFirst { it.id == best.track.id }.let {
                            if (it >= 0) play(it) else player.play(listOf(best.track), 0, PlayContext.Search)
                        }
                    }
                })
            }
        }
        if (tracks.isNotEmpty() && (all || filter == TRACKS)) {
            item(key = "tracks") {
                SectionHeader("Треки", actionText = if (all && result.tracks.size > tracks.size) "Все" else "", onAction = { onFilter(TRACKS) })
            }
            // Один трек может прийти дважды (разные издания) — ключ с номером строки
            itemsIndexed(tracks, key = { index, track -> "$index:${track.id}" }) { index, track ->
                Box(Modifier.padding(horizontal = 8.dp)) {
                    TrackRow(track, current = track.id == playerState.track?.id, onClick = { play(index) })
                }
            }
        }
        if (artists.isNotEmpty() && (all || filter == ARTISTS)) {
            item(key = "artists") {
                SectionHeader("Исполнители", actionText = if (all && result.artists.size > artists.size) "Все" else "", onAction = { onFilter(ARTISTS) })
            }
            items(artists.chunked(TILE_COLUMNS)) { row -> TileRow(row) { artist, modifier -> ArtistTile(artist, modifier, onOpenArtist) } }
        }
        if (albums.isNotEmpty() && (all || filter == ALBUMS)) {
            item(key = "albums") {
                SectionHeader("Альбомы", actionText = if (all && result.albums.size > albums.size) "Все" else "", onAction = { onFilter(ALBUMS) })
            }
            items(albums.chunked(TILE_COLUMNS)) { row -> TileRow(row) { album, modifier -> AlbumTile(album, modifier, artist = true, onOpen = onOpenAlbum) } }
        }
    }
}

/** Лучший результат: крупная карточка с обложкой в фигуре; у трека — кнопка воспроизведения. */
@Composable
private fun BestCard(best: Best, playerState: PlayerState, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val cover = best.track?.cover(400) ?: best.album?.cover(400) ?: best.artist?.cover(400).orEmpty()
    val title = best.track?.title ?: best.album?.title ?: best.artist?.name.orEmpty()
    val subtitle = best.track?.artists ?: best.album?.artists.orEmpty()
    val overline = when {
        best.track != null -> "ТРЕК"
        best.album != null -> best.album.kind.uppercase()
        else -> "ИСПОЛНИТЕЛЬ"
    }
    Surface(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        shape = RoundedCornerShape(28.dp),
        color = colors.surfaceContainerHigh,
    ) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            MorphImage(
                cover, if (best.artist != null) Shapes.Circle else Shapes.SoftSquare, Modifier.size(96.dp),
                placeholder = if (best.artist != null) "person" else "album", placeholderSize = 40.dp,
            )
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Overline(overline)
                Text(title, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
                if (subtitle.isNotEmpty()) {
                    Text(subtitle, style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (best.track != null) {
                Spacer(Modifier.width(8.dp))
                PlayButton(playing = playerState.playing && playerState.track?.id == best.track.id, onClick = onClick, size = 56.dp)
            }
        }
    }
}
