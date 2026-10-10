package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Playlist
import io.github.yaskorinov.yamusic.data.Library

/** Коллекция: «Мне нравится» и плейлисты пользователя. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun CollectionScreen(
    library: Library,
    contentPadding: PaddingValues,
    onOpenSettings: () -> Unit,
    onOpenLiked: () -> Unit,
    downloadedCount: Int,
    onOpenDownloaded: () -> Unit,
    onOpenPlaylist: (Playlist) -> Unit,
) {
    val playlists by library.playlists.collectAsStateWithLifecycle()
    val liked by library.liked.collectAsStateWithLifecycle()
    val notice by library.notice.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme

    LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
        item(key = "header") {
            Row(Modifier.fillMaxWidth().padding(start = 20.dp, end = 12.dp, top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "Коллекция",
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.displaySmall,
                    fontWeight = FontWeight.SemiBold,
                )
                IconButton(onClick = onOpenSettings) { Symbol("settings") }
            }
        }
        if (notice.isNotEmpty()) {
            item(key = "notice") {
                Text(
                    notice,
                    Modifier.padding(horizontal = 20.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.error,
                )
            }
        }
        item(key = "liked") {
            Surface(
                onClick = onOpenLiked,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 16.dp),
                shape = RoundedCornerShape(32.dp),
                color = colors.primaryContainer,
            ) {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        Modifier.size(72.dp).background(colors.primary, MaterialShapes.Cookie9Sided.toShape()),
                        contentAlignment = Alignment.Center,
                    ) {
                        Symbol("favorite", size = 32.dp, filled = true, tint = colors.onPrimary)
                    }
                    Spacer(Modifier.width(16.dp))
                    Column {
                        Text("Мне нравится", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
                        Text(tracksCount(liked.size), style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }
        }
        if (downloadedCount > 0) {
            item(key = "downloaded") {
                Row(
                    Modifier.fillMaxWidth().clickable(onClick = onOpenDownloaded).padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(
                        Modifier.size(60.dp).background(colors.secondaryContainer, RoundedCornerShape(16.dp)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Symbol("download_done", size = 28.dp, tint = colors.onSecondaryContainer)
                    }
                    Spacer(Modifier.width(14.dp))
                    Column(Modifier.weight(1f)) {
                        Text("Скачанные", style = MaterialTheme.typography.titleMedium)
                        Text(
                            "${tracksCount(downloadedCount)} · играют без сети",
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
            }
        }
        if (playlists.isNotEmpty()) {
            item(key = "playlists") {
                Text(
                    "Плейлисты",
                    Modifier.padding(start = 20.dp, top = 4.dp, bottom = 4.dp),
                    style = MaterialTheme.typography.titleMedium,
                    color = colors.onSurfaceVariant,
                )
            }
        }
        items(playlists, key = { it.id }) { playlist ->
            Row(
                Modifier.fillMaxWidth().clickable { onOpenPlaylist(playlist) }.padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Cover(playlist.cover(200), 60.dp, RoundedCornerShape(16.dp), "queue_music")
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f)) {
                    Text(playlist.title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        tracksCount(playlist.trackCount),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                    )
                }
            }
        }
    }

}
