package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.data.ArtistData
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

/**
 * Исполнитель (перенос ArtistPage.qml): шапка — фото в фигуре (пока играет его музыка, круг морфится
 * в «печеньку»), Слушать / Перемешать; популярные треки; альбомы и синглы плитками. За шапкой плывут фигуры.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ArtistScreen(
    data: ArtistData,
    player: PlayerConnection,
    playerState: PlayerState,
    contentPadding: PaddingValues,
    onRetry: () -> Unit,
    onBack: () -> Unit,
    onOpenAlbum: (Album) -> Unit,
) {
    val artist by data.info.collectAsStateWithLifecycle()
    val albums by data.albums.collectAsStateWithLifecycle()
    val tracks by data.popular.tracks.collectAsStateWithLifecycle()
    val loading by data.popular.loading.collectAsStateWithLifecycle()
    val error by data.popular.error.collectAsStateWithLifecycle()
    var allPopular by rememberSaveable { mutableStateOf(false) }
    var allAlbums by rememberSaveable { mutableStateOf(false) }
    val name = artist?.name.orEmpty()
    val playingHere = playerState.playing && tracks.any { it.id == playerState.track?.id }

    val listState = rememberLazyListState()
    val threshold = with(LocalDensity.current) { 48.dp.toPx() }
    val scrolled by remember { derivedStateOf { listState.firstVisibleItemIndex > 0 || listState.firstVisibleItemScrollOffset > threshold } }
    var headerHeight by remember { mutableFloatStateOf(0f) }
    Box(Modifier.fillMaxSize().padding(top = contentPadding.calculateTopPadding())) {
        HeroShapes(
            seed = name,
            top = 56.dp,
            headerHeight = { headerHeight },
            scroll = { if (listState.firstVisibleItemIndex == 0) listState.firstVisibleItemScrollOffset.toFloat() else Float.MAX_VALUE },
            opacity = 0.6f,
        )
        Column {
            TopBar(name, showTitle = scrolled, onBack = onBack)
            LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + 16.dp)) {
                item(key = "header") {
                    Column(Modifier.fillMaxWidth().onSizeChanged { headerHeight = it.height.toFloat() }.padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 16.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            MorphImage(
                                artist?.cover(600).orEmpty(),
                                if (playingHere) Shapes.Cookie12 else Shapes.Circle,
                                Modifier.size(144.dp),
                                placeholder = "person",
                                placeholderSize = 56.dp,
                                duration = Motion.SpatialSlow,
                            )
                            Spacer(Modifier.width(20.dp))
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Overline("ИСПОЛНИТЕЛЬ")
                                Text(
                                    name,
                                    style = MaterialTheme.typography.headlineLarge,
                                    fontWeight = FontWeight.Bold,
                                    maxLines = 3,
                                    overflow = TextOverflow.Ellipsis,
                                    autoSize = TextAutoSize.StepBased(minFontSize = 22.sp, maxFontSize = 34.sp, stepSize = 2.sp),
                                )
                            }
                        }
                        if (tracks.isNotEmpty()) {
                            Spacer(Modifier.height(18.dp))
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                val padding = PaddingValues(start = 14.dp, end = 18.dp)
                                Button(onClick = { player.play(tracks, 0, PlayContext.Artist) }, contentPadding = padding) {
                                    Symbol("play_arrow", size = 18.dp, filled = true)
                                    Spacer(Modifier.width(6.dp))
                                    Text("Слушать")
                                }
                                FilledTonalButton(
                                    onClick = { player.play(tracks, tracks.indices.random(), PlayContext.Artist, shuffle = true) },
                                    contentPadding = padding,
                                ) {
                                    Symbol("shuffle", size = 18.dp)
                                    Spacer(Modifier.width(6.dp))
                                    Text("Перемешать")
                                }
                            }
                        }
                    }
                }
                if (error.isNotEmpty()) {
                    item(key = "error") {
                        EmptyState("person_off", "Исполнитель не загрузился", Modifier.fillMaxWidth().padding(top = 24.dp), text = error, actionText = "Повторить", onAction = onRetry)
                    }
                }
                if (tracks.isNotEmpty()) {
                    item(key = "popular") {
                        SectionHeader(
                            "Популярные треки",
                            actionText = if (tracks.size > POPULAR) (if (allPopular) "Свернуть" else "Все") else "",
                            onAction = { allPopular = !allPopular },
                        )
                    }
                    itemsIndexed(if (allPopular) tracks else tracks.take(POPULAR), key = { _, track -> track.id }) { index, track ->
                        Box(Modifier.padding(horizontal = 8.dp)) {
                            TrackRow(track, current = track.id == playerState.track?.id, onClick = { player.play(tracks, index, PlayContext.Artist) }, number = index + 1)
                        }
                    }
                }
                if (albums.isNotEmpty()) {
                    val shown = if (allAlbums) albums else albums.take(TILE_COLUMNS * 2)
                    item(key = "albums") {
                        SectionHeader(
                            "Альбомы и синглы",
                            actionText = if (albums.size > TILE_COLUMNS * 2) (if (allAlbums) "Свернуть" else "Все") else "",
                            onAction = { allAlbums = !allAlbums },
                        )
                    }
                    items(shown.chunked(TILE_COLUMNS)) { row -> TileRow(row) { album, modifier -> AlbumTile(album, modifier, onOpen = onOpenAlbum) } }
                }
                if (loading) {
                    item(key = "loading") {
                        Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { LoadingIndicator(Modifier.size(64.dp)) }
                    }
                }
            }
        }
    }
}

private const val POPULAR = 5
