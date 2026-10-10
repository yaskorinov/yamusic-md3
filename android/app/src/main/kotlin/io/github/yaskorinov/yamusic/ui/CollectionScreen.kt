package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Playlist
import io.github.yaskorinov.yamusic.data.Account
import io.github.yaskorinov.yamusic.data.Library

/**
 * Коллекция — то, что на десктопе живёт в боковой панели: карточка аккаунта с настройками,
 * «Мне нравится», скачанное и плейлисты пользователя под волнистым разделителем.
 */
@Composable
fun CollectionScreen(
    library: Library,
    account: Account?,
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
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = 8.dp, end = 8.dp,
            top = contentPadding.calculateTopPadding(), bottom = contentPadding.calculateBottomPadding() + 16.dp,
        ),
    ) {
        item(key = "header") {
            Text(
                "Коллекция",
                Modifier.padding(start = 8.dp, top = 20.dp, bottom = 16.dp),
                style = MaterialTheme.typography.displaySmall,
                fontWeight = FontWeight.SemiBold,
            )
        }
        item(key = "account") {
            Surface(
                onClick = onOpenSettings,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp).padding(bottom = 12.dp),
                shape = RoundedCornerShape(16.dp),
                color = colors.surfaceContainer,
            ) {
                Row(Modifier.height(64.dp).padding(start = 12.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(account?.displayName.orEmpty())
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(account?.displayName.orEmpty(), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(
                            if (account?.hasPlus == true) "Плюс" else "Без Плюса",
                            style = MaterialTheme.typography.bodySmall,
                            color = colors.onSurfaceVariant,
                        )
                    }
                    IconButton(onClick = onOpenSettings) { Symbol("settings", tint = colors.onSurfaceVariant) }
                }
            }
        }
        if (notice.isNotEmpty()) {
            item(key = "notice") {
                Text(notice, Modifier.padding(horizontal = 12.dp, vertical = 8.dp), style = MaterialTheme.typography.bodyMedium, color = colors.error)
            }
        }
        item(key = "liked") {
            Entry("Мне нравится", tracksCount(liked.size), onOpenLiked) {
                MorphShape(Shapes.Clover4, colors.primaryContainer, Modifier.size(48.dp)) {
                    Symbol("favorite", size = 22.dp, filled = true, tint = colors.onPrimaryContainer)
                }
            }
        }
        if (downloadedCount > 0) {
            item(key = "downloaded") {
                Entry("Скачанные", "${tracksCount(downloadedCount)} · играют без сети", onOpenDownloaded) {
                    MorphShape(Shapes.Cookie6, colors.secondaryContainer, Modifier.size(48.dp)) {
                        Symbol("download_done", size = 22.dp, tint = colors.onSecondaryContainer)
                    }
                }
            }
        }
        if (playlists.isNotEmpty()) {
            item(key = "playlists") {
                Row(
                    Modifier.fillMaxWidth().height(40.dp).padding(start = 12.dp, end = 12.dp, top = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("Плейлисты", style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant)
                    WavyDivider(Modifier.weight(1f))
                }
            }
        }
        items(playlists, key = { it.id }) { playlist ->
            Entry(playlist.title, tracksCount(playlist.trackCount), { onOpenPlaylist(playlist) }) {
                MorphImage(playlist.cover(200), Shapes.SoftSquare, Modifier.size(48.dp), placeholder = "queue_music")
            }
        }
    }
}

/** Строка раздела — той же геометрии, что строка трека. */
@Composable
private fun Entry(title: String, subtitle: String, onClick: () -> Unit, leading: @Composable () -> Unit) {
    Row(
        Modifier.fillMaxWidth().height(64.dp).clip(RoundedCornerShape(16.dp)).clickable(onClick = onClick).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        leading()
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
        }
    }
}
