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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import io.github.yaskorinov.yamusic.api.Playlist
import io.github.yaskorinov.yamusic.data.Library
import io.github.yaskorinov.yamusic.data.Session

/** Коллекция: «Мне нравится» и плейлисты пользователя. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun CollectionScreen(
    library: Library,
    session: Session,
    contentPadding: PaddingValues,
    onOpenLiked: () -> Unit,
    onOpenPlaylist: (Playlist) -> Unit,
) {
    val playlists by library.playlists.collectAsStateWithLifecycle()
    val liked by library.liked.collectAsStateWithLifecycle()
    val notice by library.notice.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    var confirmSignOut by remember { mutableStateOf(false) }

    LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
        item(key = "header") {
            Row(Modifier.fillMaxWidth().padding(start = 20.dp, end = 12.dp, top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "Коллекция",
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.displaySmall,
                    fontWeight = FontWeight.SemiBold,
                )
                IconButton(onClick = { confirmSignOut = true }) { Symbol("logout") }
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
                AsyncImage(
                    model = playlist.cover(200),
                    contentDescription = null,
                    modifier = Modifier.size(60.dp).clip(RoundedCornerShape(16.dp)).background(colors.surfaceContainerHigh),
                    contentScale = ContentScale.Crop,
                )
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

    if (confirmSignOut) {
        AlertDialog(
            onDismissRequest = { confirmSignOut = false },
            title = { Text("Выйти из аккаунта?") },
            text = { Text("Токен будет удалён с этого телефона.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmSignOut = false
                    session.signOut()
                }) { Text("Выйти") }
            },
            dismissButton = { TextButton(onClick = { confirmSignOut = false }) { Text("Отмена") } },
        )
    }
}
