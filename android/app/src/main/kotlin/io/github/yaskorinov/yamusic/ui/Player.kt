package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import coil3.compose.AsyncImage
import io.github.yaskorinov.yamusic.data.Library
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Позиция играющего трека, мс: плеер её не присылает, поэтому опрашиваем, пока экран виден. */
@Composable
private fun rememberPosition(player: PlayerConnection, state: PlayerState): State<Long> =
    produceState(player.positionMs, state.track?.id, state.playing) {
        while (true) {
            value = player.positionMs
            delay(if (state.playing) 200 else 1000)
        }
    }

/** Мини-плеер над списком: обложка, название, пауза и «дальше»; нажатие раскрывает полный плеер. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PlayerBar(state: PlayerState, player: PlayerConnection, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val track = state.track ?: return
    val colors = MaterialTheme.colorScheme
    val position by rememberPosition(player, state)
    Surface(
        onClick = onOpen,
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        color = colors.surfaceContainerHigh,
        shadowElevation = 6.dp,
    ) {
        Column {
            Row(Modifier.padding(start = 10.dp, end = 6.dp, top = 10.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                AsyncImage(
                    model = track.cover(200),
                    contentDescription = null,
                    modifier = Modifier.size(48.dp).clip(RoundedCornerShape(14.dp)).background(colors.surfaceContainerHighest),
                    contentScale = ContentScale.Crop,
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(track.title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        state.error.ifEmpty { track.artists },
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (state.error.isEmpty()) colors.onSurfaceVariant else colors.error,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                FilledIconButton(onClick = player::togglePlay) {
                    Symbol(if (state.playing) "pause" else "play_arrow", filled = true)
                }
                IconButton(onClick = { player.next() }, enabled = state.hasNext) { Symbol("skip_next", filled = true) }
            }
            LinearWavyProgressIndicator(
                progress = { fraction(position, state.durationMs) },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 18.dp).padding(bottom = 8.dp),
                amplitude = { if (state.playing) 1f else 0f },
            )
        }
    }
}

/** Полноэкранный плеер. */
@Composable
fun NowPlaying(state: PlayerState, player: PlayerConnection, library: Library, onClose: () -> Unit) {
    val track = state.track ?: return
    val colors = MaterialTheme.colorScheme
    val likedIds by library.likedIds.collectAsStateWithLifecycle()
    val liked = track.id in likedIds
    Surface(Modifier.fillMaxSize(), color = colors.surfaceContainerLow) {
        Column(Modifier.safeDrawingPadding().padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose) { Symbol("keyboard_arrow_down", size = 28.dp) }
                Text(
                    track.album,
                    Modifier.weight(1f).padding(horizontal = 8.dp),
                    style = MaterialTheme.typography.labelLarge,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.weight(1f))
            AsyncImage(
                model = track.cover(1000),
                contentDescription = null,
                modifier = Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(36.dp)).background(colors.surfaceContainerHighest),
                contentScale = ContentScale.Crop,
            )
            Spacer(Modifier.weight(1f))
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        track.title,
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        state.error.ifEmpty { track.artists },
                        style = MaterialTheme.typography.titleMedium,
                        color = if (state.error.isEmpty()) colors.onSurfaceVariant else colors.error,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                IconButton(onClick = {
                    library.dislike(track)
                    player.next()
                }) {
                    Symbol("thumb_down", tint = colors.onSurfaceVariant)
                }
                IconButton(onClick = { library.toggleLike(track) }) {
                    Symbol("favorite", size = 28.dp, filled = liked, tint = if (liked) colors.primary else colors.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(12.dp))
            Seek(state, player)
            Spacer(Modifier.height(8.dp))
            Controls(state, player)
            Spacer(Modifier.height(28.dp))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Seek(state: PlayerState, player: PlayerConnection) {
    val scope = rememberCoroutineScope()
    val position by rememberPosition(player, state)
    var drag by remember { mutableStateOf<Float?>(null) }
    val shown = drag ?: fraction(position, state.durationMs)
    val slider = remember { SliderState() }
    slider.value = shown
    Slider(
        state = slider,
        onValueChange = { drag = it },
        onValueChangeFinished = {
            drag?.let { player.seekTo((it * state.durationMs).toLong()) }
            // Отпускаем ползунок чуть позже: опрос позиции ещё не успел увидеть перемотку
            scope.launch {
                delay(300)
                drag = null
            }
        },
        modifier = Modifier.fillMaxWidth(),
    )
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        val style = MaterialTheme.typography.labelMedium
        val color = MaterialTheme.colorScheme.onSurfaceVariant
        Text(formatTime((shown * state.durationMs).toLong()), style = style, color = color)
        Text(formatTime(state.durationMs), style = style, color = color)
    }
}

@Composable
private fun Controls(state: PlayerState, player: PlayerConnection) {
    val colors = MaterialTheme.colorScheme
    // Кнопка «дышит» формой: круг на паузе, скруглённый квадрат во время игры
    val corner by animateDpAsState(if (state.playing) 28.dp else 44.dp, label = "play")
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = player::toggleShuffle) {
            Symbol("shuffle", tint = if (state.shuffle) colors.primary else colors.onSurfaceVariant)
        }
        IconButton(onClick = { player.previous() }, modifier = Modifier.size(64.dp)) {
            Symbol("skip_previous", size = 40.dp, filled = true)
        }
        FilledIconButton(onClick = player::togglePlay, modifier = Modifier.size(88.dp), shape = RoundedCornerShape(corner)) {
            Symbol(if (state.playing) "pause" else "play_arrow", size = 44.dp, filled = true)
        }
        IconButton(onClick = { player.next() }, enabled = state.hasNext, modifier = Modifier.size(64.dp)) {
            Symbol("skip_next", size = 40.dp, filled = true)
        }
        IconButton(onClick = player::cycleRepeat) {
            Symbol(
                if (state.repeat == Player.REPEAT_MODE_ONE) "repeat_one" else "repeat",
                tint = if (state.repeat == Player.REPEAT_MODE_OFF) colors.onSurfaceVariant else colors.primary,
            )
        }
    }
}

private fun fraction(positionMs: Long, durationMs: Long): Float =
    if (durationMs <= 0) 0f else (positionMs.toFloat() / durationMs).coerceIn(0f, 1f)
