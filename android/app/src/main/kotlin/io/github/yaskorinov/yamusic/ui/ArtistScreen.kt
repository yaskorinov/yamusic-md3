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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.data.ArtistData
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

/** Страница исполнителя: портрет в фигуре, альбомы лентой, популярные треки. */
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

    LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
        item(key = "header") {
            Column(Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, top = 4.dp)) {
                IconButton(onClick = onBack) { Symbol("arrow_back") }
                Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Cover(artist?.cover(400).orEmpty(), 196.dp, MaterialShapes.Cookie9Sided.toShape(), "person")
                    Spacer(Modifier.height(16.dp))
                    Text(
                        artist?.name.orEmpty(),
                        style = MaterialTheme.typography.displaySmall,
                        fontWeight = FontWeight.SemiBold,
                        textAlign = TextAlign.Center,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(Modifier.height(16.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = { player.play(tracks, 0, PlayContext.Artist) },
                            enabled = tracks.isNotEmpty(),
                            modifier = Modifier.weight(1f).height(56.dp),
                        ) {
                            Symbol("play_arrow", filled = true)
                            Spacer(Modifier.width(8.dp))
                            Text("Слушать")
                        }
                        FilledTonalButton(
                            onClick = { player.play(tracks, tracks.indices.random(), PlayContext.Artist, shuffle = true) },
                            enabled = tracks.isNotEmpty(),
                            modifier = Modifier.weight(1f).height(56.dp),
                        ) {
                            Symbol("shuffle")
                            Spacer(Modifier.width(8.dp))
                            Text("Перемешать")
                        }
                    }
                }
            }
        }
        if (error.isNotEmpty()) {
            item(key = "error") {
                Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                    TextButton(onClick = onRetry) { Text("Повторить") }
                }
            }
        }
        if (albums.isNotEmpty()) {
            item(key = "albums") {
                Column {
                    SectionTitle("Альбомы")
                    AlbumRow(albums, onOpenAlbum)
                }
            }
        }
        if (tracks.isNotEmpty()) item(key = "popular") { SectionTitle("Популярное") }
        itemsIndexed(tracks, key = { _, track -> track.id }) { index, track ->
            TrackRow(
                track = track,
                current = track.id == playerState.track?.id,
                onClick = { player.play(tracks, index, PlayContext.Artist) },
            )
        }
        if (loading) {
            item(key = "loading") {
                Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                    LoadingIndicator(Modifier.size(48.dp))
                }
            }
        }
    }
}
