package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.lyrics.LyricLine
import io.github.yaskorinov.yamusic.lyrics.Lyrics
import io.github.yaskorinov.yamusic.lyrics.LyricsState
import io.github.yaskorinov.yamusic.lyrics.LyricsStore
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import kotlinx.coroutines.delay

/**
 * Синхронный текст: спетые слова проявляются по мере звучания, текущая строка держится в верхней трети,
 * нажатие на строку перематывает к ней. Пока текст листают руками, автопрокрутка ждёт.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LyricsView(track: Track, store: LyricsStore, player: PlayerConnection, playing: Boolean, modifier: Modifier = Modifier) {
    val state by store.state.collectAsStateWithLifecycle()
    LaunchedEffect(track.id) { store.request(track) }

    Box(modifier, contentAlignment = Alignment.Center) {
        when (val lyrics = state) {
            is LyricsState.Found -> if (lyrics.trackId == track.id) SyncedLyrics(lyrics.lyrics, player, playing)
            is LyricsState.Missing -> if (lyrics.trackId == track.id) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Symbol("lyrics", size = 48.dp, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(
                        if (lyrics.failed) "Не удалось загрузить текст" else "Для этого трека текста нет",
                        Modifier.padding(top = 12.dp),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (lyrics.failed) TextButton(onClick = { store.request(track) }) { Text("Повторить") }
                }
            }
            else -> LoadingIndicator(Modifier.size(56.dp))
        }
    }
}

@Composable
private fun SyncedLyrics(lyrics: Lyrics, player: PlayerConnection, playing: Boolean) {
    val lines = lyrics.lines
    val listState = rememberLazyListState()

    // Позиция — раз в кадр, пока играет: её читают только текущая строка и расчёт номера строки
    var position by remember { mutableFloatStateOf(player.positionMs / 1000f) }
    LaunchedEffect(playing) {
        while (true) {
            position = player.positionMs / 1000f
            if (playing) withFrameNanos { } else delay(250)
        }
    }
    val active by remember(lines) {
        derivedStateOf { lines.indexOfLast { it.start <= position + LEAD }.coerceAtLeast(0) }
    }

    val dragged by listState.interactionSource.collectIsDraggedAsState()
    var holdUntil by remember { mutableLongStateOf(0L) }
    LaunchedEffect(dragged) { if (dragged) holdUntil = Long.MAX_VALUE else if (holdUntil != 0L) holdUntil = System.currentTimeMillis() + HOLD_MS }
    LaunchedEffect(active, holdUntil) {
        val wait = holdUntil - System.currentTimeMillis()
        if (wait > HOLD_MS) return@LaunchedEffect // текст держат пальцем
        if (wait > 0) delay(wait)
        val viewport = listState.layoutInfo.viewportSize.height
        listState.animateScrollToItem(active, scrollOffset = -(viewport * 0.3f).toInt())
    }

    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            // края списка тают: слой рисуется отдельно и умножается на вертикальную маску
            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
            .drawWithContent {
                drawContent()
                drawRect(
                    Brush.verticalGradient(0f to Color.Transparent, 0.12f to Color.Black, 0.86f to Color.Black, 1f to Color.Transparent),
                    blendMode = BlendMode.DstIn,
                )
            },
        contentPadding = PaddingValues(top = 48.dp, bottom = 220.dp),
    ) {
        itemsIndexed(lines) { index, line ->
            Line(
                line = line,
                isActive = index == active,
                isPast = index < active,
                position = { position },
                onClick = { player.seekTo((line.start * 1000).toLong()) },
            )
        }
        item {
            Text(
                "Текст: ${lyrics.source}" + if (lyrics.kind == "word") " · по словам" else "",
                Modifier.fillMaxWidth().padding(top = 24.dp),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
    }
}

@Composable
private fun Line(line: LyricLine, isActive: Boolean, isPast: Boolean, position: () -> Float, onClick: () -> Unit) {
    val color = MaterialTheme.colorScheme.onSurface
    val emphasis by animateFloatAsState(if (isActive) 1f else if (isPast) PAST else DIM, label = "line")
    val modifier = Modifier
        .fillMaxWidth()
        .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onClick)
        .padding(horizontal = 4.dp, vertical = 9.dp)
    val style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold)

    if (line.gap) {
        // Пауза: три точки загораются по очереди
        val lit by remember(line) {
            derivedStateOf { ((position() - line.start) / (line.end - line.start).coerceAtLeast(0.1f) * 3).toInt().coerceIn(0, 3) }
        }
        Text(
            buildAnnotatedString {
                repeat(3) { dot ->
                    withStyle(SpanStyle(color = color.copy(alpha = if (isActive && dot < lit) 1f else if (isActive) DIM else emphasis))) {
                        append(if (dot < 2) "•  " else "•")
                    }
                }
            },
            modifier,
            style = style,
        )
        return
    }
    if (!isActive) {
        Text(line.words.joinToString("") { it.text }, modifier, style = style, color = color.copy(alpha = emphasis))
        return
    }
    // Номер звучащего слова и его готовность (ступенями — чтобы не перерисовывать строку каждый кадр)
    val sung by remember(line) {
        derivedStateOf {
            val now = position()
            val index = line.words.indexOfLast { it.start <= now }
            if (index < 0) return@derivedStateOf 0f
            val word = line.words[index]
            val part = ((now - word.start) / (word.end - word.start).coerceAtLeast(0.05f)).coerceIn(0f, 1f)
            index + (part * STEPS).toInt() / STEPS.toFloat()
        }
    }
    Text(
        buildAnnotatedString {
            line.words.forEachIndexed { index, word ->
                val progress = (sung - index).coerceIn(0f, 1f)
                withStyle(SpanStyle(color = color.copy(alpha = DIM + (1f - DIM) * progress))) { append(word.text) }
            }
        },
        modifier,
        style = style,
    )
}

private const val LEAD = 0.15f      // строка становится текущей чуть раньше первого слова
private const val DIM = 0.34f       // ещё не спетое
private const val PAST = 0.5f       // уже спетые строки
private const val STEPS = 6
private const val HOLD_MS = 3000L   // сколько автопрокрутка ждёт после ручной
