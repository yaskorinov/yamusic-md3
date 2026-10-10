package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import io.github.yaskorinov.yamusic.data.Downloads
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.data.TrackList
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

/** Список треков (плейлист или «Мне нравится»): шапка с кнопками и строки. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun TrackListScreen(
    title: String,
    list: TrackList,
    subtitle: String = "",
    numbered: Boolean = false,
    context: PlayContext,
    player: PlayerConnection,
    playerState: PlayerState,
    contentPadding: PaddingValues,
    onRetry: () -> Unit,
    onBack: () -> Unit,
    downloads: Downloads,
) {
    val tracks by list.tracks.collectAsStateWithLifecycle()
    val loading by list.loading.collectAsStateWithLifecycle()
    val error by list.error.collectAsStateWithLifecycle()

    LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
        item(key = "header") {
            Header(
                title = title,
                subtitle = subtitle,
                count = tracks.size,
                loading = loading,
                onBack = onBack,
                onPlay = { player.play(tracks, 0, context) },
                onShuffle = { if (tracks.isNotEmpty()) player.play(tracks, tracks.indices.random(), context, shuffle = true) },
                download = { DownloadButton(tracks, downloads) },
            )
        }
        if (error.isNotEmpty()) {
            item(key = "error") {
                Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                    TextButton(onClick = onRetry) { Text("Повторить") }
                }
            }
        }
        itemsIndexed(tracks, key = { _, track -> track.id }) { index, track ->
            TrackRow(
                track = track,
                current = track.id == playerState.track?.id,
                onClick = { player.play(tracks, index, context) },
                number = if (numbered) index + 1 else null,
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

@Composable
private fun Header(
    title: String,
    subtitle: String,
    count: Int,
    loading: Boolean,
    onBack: () -> Unit,
    onPlay: () -> Unit,
    onShuffle: () -> Unit,
    download: @Composable () -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(start = 8.dp, end = 20.dp, top = 4.dp, bottom = 12.dp)) {
        IconButton(onClick = onBack) { Symbol("arrow_back") }
        Column(Modifier.padding(start = 12.dp)) {
            Text(
                title,
                style = MaterialTheme.typography.displaySmall,
                fontWeight = FontWeight.SemiBold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                when {
                    count == 0 && loading -> "Загружаем…"
                    subtitle.isEmpty() -> tracksCount(count)
                    else -> "$subtitle · ${tracksCount(count)}"
                },
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = onPlay, enabled = count > 0, modifier = Modifier.weight(1f).height(56.dp)) {
                    Symbol("play_arrow", filled = true)
                    Spacer(Modifier.width(8.dp))
                    Text("Слушать")
                }
                // «Перемешать» и «Скачать» — значками: втроём с подписями кнопки не помещаются
                FilledTonalIconButton(onClick = onShuffle, enabled = count > 0, modifier = Modifier.size(56.dp)) {
                    Symbol("shuffle")
                }
                download()
            }
        }
    }
}

/**
 * Скачать список для игры без сети. Три состояния: не скачан → скачивается (сколько готово; нажатие
 * отменяет остаток) → скачан (нажатие предлагает удалить файлы).
 */
@Composable
private fun DownloadButton(tracks: List<Track>, downloads: Downloads) {
    val done by downloads.done.collectAsStateWithLifecycle()
    val pending by downloads.pending.collectAsStateWithLifecycle()
    val status by downloads.status.collectAsStateWithLifecycle()
    val wanted = tracks.filter { it.available }
    val ready = wanted.count { it.id in done }
    val queued = pending.count { queuedTrack -> wanted.any { it.id == queuedTrack.id } }
    var confirmRemove by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme

    FilledTonalIconButton(
        onClick = {
            when {
                queued > 0 -> downloads.remove(pending.map { it.id }.filter { id -> wanted.any { it.id == id } })
                wanted.isNotEmpty() && ready == wanted.size -> confirmRemove = true
                else -> downloads.enqueue(wanted)
            }
        },
        enabled = wanted.isNotEmpty(),
        modifier = Modifier.size(56.dp),
    ) {
        when {
            queued > 0 -> Text("$ready/${wanted.size}", style = MaterialTheme.typography.labelSmall, maxLines = 1)
            wanted.isNotEmpty() && ready == wanted.size -> Symbol("download_done", tint = colors.primary)
            else -> Symbol("download")
        }
    }
    if (queued > 0 && status.isNotEmpty()) {
        // очередь стоит — объяснить почему (например, «Ждём Wi-Fi»)
        Text(status, Modifier.padding(start = 4.dp), style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant)
    }
    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text("Удалить скачанное?") },
            text = { Text("Файлы этих треков (${tracksCount(ready)}) будут удалены с телефона. Сами треки останутся в коллекции.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmRemove = false
                    downloads.remove(wanted.map { it.id })
                }) { Text("Удалить") }
            },
            dismissButton = { TextButton(onClick = { confirmRemove = false }) { Text("Отмена") } },
        )
    }
}

/** Идентификаторы скачанных треков: строки помечают их значком, не зная о хранилище. */
val LocalDownloaded = compositionLocalOf<Set<String>> { emptySet() }

@Composable
fun TrackRow(track: Track, current: Boolean, onClick: () -> Unit, number: Int? = null) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(enabled = track.available, onClick = onClick)
            .alpha(if (track.available) 1f else 0.38f)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (number != null) {
            // В альбоме обложка у всех одна — вместо неё номер трека
            Box(Modifier.size(width = 36.dp, height = 52.dp), contentAlignment = Alignment.Center) {
                if (current) {
                    Symbol("graphic_eq", tint = colors.primary)
                } else {
                    Text(number.toString(), style = MaterialTheme.typography.titleMedium, color = colors.onSurfaceVariant)
                }
            }
        } else {
            AsyncImage(
                model = track.cover(200),
                contentDescription = null,
                modifier = Modifier.size(52.dp).clip(RoundedCornerShape(12.dp)).background(colors.surfaceContainerHigh),
                contentScale = ContentScale.Crop,
            )
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(
                buildAnnotatedString {
                    append(track.title)
                    if (track.version.isNotEmpty()) {
                        withStyle(SpanStyle(color = colors.onSurfaceVariant)) { append("  ${track.version}") }
                    }
                },
                style = MaterialTheme.typography.titleMedium,
                color = if (current) colors.primary else colors.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (track.explicit) {
                    Symbol("explicit", size = 16.dp, tint = colors.onSurfaceVariant)
                    Spacer(Modifier.width(4.dp))
                }
                Text(
                    track.artists,
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        if (track.id in LocalDownloaded.current) {
            Symbol("download_done", size = 16.dp, tint = colors.primary)
            Spacer(Modifier.width(6.dp))
        }
        Text(formatTime(track.durationMs), style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
    }
}

fun formatTime(ms: Long): String {
    val seconds = (ms / 1000).coerceAtLeast(0)
    return "%d:%02d".format(seconds / 60, seconds % 60)
}

fun tracksCount(n: Int): String {
    val word = when {
        n % 100 in 11..14 -> "треков"
        n % 10 == 1 -> "трек"
        n % 10 in 2..4 -> "трека"
        else -> "треков"
    }
    return "$n $word"
}
