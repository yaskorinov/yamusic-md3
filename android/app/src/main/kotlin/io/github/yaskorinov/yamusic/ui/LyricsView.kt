package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.EaseInOut
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.lyrics.LyricLine
import io.github.yaskorinov.yamusic.lyrics.Lyrics
import io.github.yaskorinov.yamusic.lyrics.LyricsState
import io.github.yaskorinov.yamusic.lyrics.LyricsStore
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.sin
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest

/**
 * Синхронный текст играющего трека с заливкой по словам (перенос LyricsView.qml). Активная строка
 * держится в верхней трети; при смене строки текст едет каскадом: строки ниже трогаются чуть позже.
 * Неактивные строки приглушены и размыты, паузы — три фигуры. Нажатие на строку — перемотка к ней.
 * Пока текст листают руками, автопрокрутка ждёт.
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
                EmptyState(
                    icon = "title",
                    title = if (lyrics.failed) "Текст не загрузился" else "Текста нет",
                    text = if (lyrics.failed) "Источники не ответили" else "Ни один источник не знает текста этого трека",
                    shape = Shapes.Flower6,
                    actionText = if (lyrics.failed) "Повторить" else "",
                    onAction = { store.request(track) },
                )
            }
            else -> LoadingIndicator(Modifier.size(56.dp))
        }
    }
}

@Composable
private fun SyncedLyrics(lyrics: Lyrics, player: PlayerConnection, playing: Boolean) {
    val lines = lyrics.lines
    val listState = rememberLazyListState()

    // Позиция — раз в кадр, пока играет: её читают только заливаемые строки и расчёт номера строки.
    // Плеер время от времени поправляет свою оценку позиции, в том числе назад, — заливка от этого
    // дёргалась. Поэтому между кадрами время идёт по часам, мелкое расхождение подтягивается плавно
    // и без шагов назад, а перемотка (расхождение больше 0,3 с) применяется скачком — как на десктопе.
    var position by remember { mutableFloatStateOf(player.positionMs / 1000f) }
    LaunchedEffect(playing) {
        position = player.positionMs / 1000f
        if (!playing) {
            while (true) {
                delay(250)
                position = player.positionMs / 1000f
            }
        }
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            val predicted = position + (now - last) / 1e9f
            last = now
            val error = player.positionMs / 1000f - predicted
            position = if (abs(error) > 0.3f) predicted + error else max(position, predicted + error * 0.06f)
        }
    }
    val active by remember(lines) {
        derivedStateOf { lines.indexOfLast { it.start <= position + LEAD }.coerceAtLeast(0) }
    }
    // Место строки среди видимых: паузы в счёт не идут (они свёрнуты), иначе в тексте, где пауза после
    // каждой строки, размытие и приглушение нарастали бы вдвое быстрее. Строка k — 2k, пауза перед ней — 2k − 1.
    val places = remember(lines) {
        var texts = 0
        IntArray(lines.size) { if (lines[it].gap) 2 * texts - 1 else 2 * texts++ }
    }

    // Прокрутка: список сразу встаёт в цель, а каждая строка получает компенсирующий сдвиг,
    // который уходит в ноль со своей задержкой
    val cascade = remember { Cascade() }
    LaunchedEffect(cascade) { cascade.run() }
    val dragged by listState.interactionSource.collectIsDraggedAsState()
    var holdUntil by remember { mutableLongStateOf(0L) }
    LaunchedEffect(dragged) { if (dragged) holdUntil = Long.MAX_VALUE else if (holdUntil != 0L) holdUntil = System.currentTimeMillis() + HOLD_MS }
    LaunchedEffect(active, holdUntil) {
        val wait = holdUntil - System.currentTimeMillis()
        if (wait > HOLD_MS) return@LaunchedEffect // текст держат пальцем
        if (wait > 0) delay(wait)
        fun top() = listState.layoutInfo.visibleItemsInfo.firstOrNull { it.index == active }?.offset
        val before = top()
        listState.scrollToItem(active, scrollOffset = -(listState.layoutInfo.viewportSize.height * ANCHOR).toInt())
        val after = top()
        if (before != null && after != null && before != after) cascade.push((before - after).toFloat(), active)
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
                    Brush.verticalGradient(0f to Color.Transparent, 0.16f to Color.Black, 0.82f to Color.Black, 1f to Color.Transparent),
                    blendMode = BlendMode.DstIn,
                )
            },
        contentPadding = PaddingValues(top = 48.dp, bottom = 260.dp),
    ) {
        itemsIndexed(lines) { index, line ->
            // каскад: строки ниже активной трогаются позже
            val moved = Modifier.graphicsLayer { translationY = cascade.shift(index) }
            // строка заливается, пока активна, и доливается, даже если следующая уже началась
            val filling by remember(line, index) { derivedStateOf { index == active || (index == active - 1 && position < line.end) } }
            if (line.gap) {
                Gap(line, isActive = index == active, position = { position }, modifier = moved)
            } else {
                Line(
                    line = line,
                    distance = (abs(places[index] - places[active]) + 1) / 2,
                    filling = filling,
                    position = { position },
                    onClick = { player.seekTo((line.start * 1000).toLong()) },
                    modifier = moved,
                )
            }
        }
        item {
            Text(
                lyrics.source + if (lyrics.kind == "word") " · по словам" else " · по строкам",
                Modifier.fillMaxWidth().padding(top = 24.dp).alpha(0.6f),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.End,
            )
        }
    }
}

/**
 * Строка текста. Заливаемая — яркая слева от «фронта» и приглушённая справа, с мягкой границей: фронт
 * идёт по словам в такт. Остальные тем бледнее и размытее, чем дальше от активной.
 */
@Composable
private fun Line(line: LyricLine, distance: Int, filling: Boolean, position: () -> Float, onClick: () -> Unit, modifier: Modifier) {
    val color = MaterialTheme.colorScheme.onSurface
    val text = remember(line) { line.words.joinToString("") { it.text } }
    val fade by animateFloatAsState(
        if (filling) 1f else max(0.22f, DIM * (1f - 0.1f * max(0, distance - 1))), tween(380, easing = EaseOutCubic), label = "fade",
    )
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    // дальние строки размыты не сильнее шестой: раньше с восьмой размытие пропадало вовсе, и под размытыми
    // строками оказывались резкие
    val blur = if (filling) 0.dp else 6.dp * (0.12f + 0.11f * (distance - 1).coerceIn(0, 5))
    Text(
        text,
        modifier
            .fillMaxWidth()
            .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 8.dp)
            .then(if (blur > 0.dp) Modifier.blur(blur, BlurredEdgeTreatment.Unbounded) else Modifier)
            .graphicsLayer { alpha = fade }
            .drawWithContent {
                drawContent()
                val measured = layout
                if (!filling || measured == null) return@drawWithContent
                // Звучащее слово и его готовность → где фронт заливки
                val now = position()
                val feather = FEATHER.toPx()
                var offset = 0
                var front = Float.NaN
                var row = 0
                var done = Float.NaN // где фронт остановился на предыдущем слове
                var doneRow = -1     // и в какой строке набора
                for (word in line.words) {
                    if (word.start > now) break
                    val first = offset
                    val last = offset + word.text.trimEnd().length
                    offset += word.text.length
                    if (last <= first) continue
                    val part = ((now - word.start) / (word.end - word.start).coerceAtLeast(0.05f)).coerceIn(0f, 1f)
                    // Длинное слово («белым-белым-белым») может переноситься: фронт проходит его отрезок
                    // в каждой строке набора по очереди, время делится по ширине отрезков
                    val firstRow = measured.getLineForOffset(first)
                    val lastRow = measured.getLineForOffset(last - 1)
                    // Фронт мягкий, поэтому проходит отрезок с запасом на полуширину размытия: в конце слово
                    // залито целиком. Начинает он не левее места, где закончил предыдущее слово, — назад не идёт.
                    fun from(r: Int): Float {
                        val left = (if (r == firstRow) measured.getHorizontalPosition(first, true) else measured.getLineLeft(r)) - feather
                        return if (r == doneRow && !done.isNaN()) max(left, done) else left
                    }
                    fun to(r: Int): Float {
                        val right = if (r == lastRow) measured.getHorizontalPosition(last, true) else measured.getLineRight(r)
                        return max(right + feather, from(r))
                    }
                    var total = 0f
                    for (r in firstRow..lastRow) total += to(r) - from(r)
                    var left = total * part
                    for (r in firstRow..lastRow) {
                        val length = to(r) - from(r)
                        row = r
                        front = from(r) + minOf(left, length)
                        if (left <= length) break
                        left -= length
                    }
                    done = to(lastRow)
                    doneRow = lastRow
                }
                if (front.isNaN()) return@drawWithContent
                val top = measured.getLineTop(row)
                val bottom = measured.getLineBottom(row)
                // строки выше уже спеты целиком
                if (row > 0) clipRect(bottom = top) { drawText(measured, color) }
                drawContext.canvas.saveLayer(Rect(0f, top, size.width, bottom), Paint())
                clipRect(top = top, bottom = bottom) { drawText(measured, color) }
                drawRect(
                    Brush.horizontalGradient(
                        0f to Color.Black,
                        1f to Color.Transparent,
                        startX = front - feather,
                        endX = front + feather,
                    ),
                    topLeft = Offset(0f, top),
                    size = Size(size.width, bottom - top),
                    blendMode = BlendMode.DstIn,
                )
                drawContext.canvas.restore()
            },
        style = LINE_STYLE.merge(MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold, fontSize = 26.sp, lineHeight = 32.sp)),
        // у заливаемой строки приглушение неспетого — в самом цвете, поверх рисуется яркий слой
        color = if (filling) color.copy(alpha = DIM) else color,
        onTextLayout = { layout = it },
    )
}

/** Пауза: вместо точек — три маленькие фигуры, которые «наливаются» по очереди и медленно вращаются. */
@Composable
private fun Gap(line: LyricLine, isActive: Boolean, position: () -> Float, modifier: Modifier) {
    val color = MaterialTheme.colorScheme.onSurface
    AnimatedVisibility(isActive, modifier, enter = expandVertically(tween(SCROLL_MS)) + fadeIn(tween(300)), exit = shrinkVertically(tween(SCROLL_MS)) + fadeOut(tween(300))) {
        val progress = { ((position() - line.start) / (line.end - line.start).coerceAtLeast(0.1f)).coerceIn(0f, 1f) }
        val lit by remember(line) { derivedStateOf { (progress() * 3).toInt() } }
        Row(
            Modifier.padding(horizontal = 6.dp, vertical = 14.dp).graphicsLayer {
                val now = position()
                val breathe = 1f + 0.09f * sin(now * 2 * PI.toFloat() / 1.9f)
                val tail = ((line.end - now) / 0.45f).coerceIn(0f, 1f)
                val scale = breathe * (0.35f + 0.65f * (1f - (1f - tail) * (1f - tail) * (1f - tail)))
                scaleX = scale
                scaleY = scale
                transformOrigin = TransformOrigin(0f, 0.5f)
            },
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            GAP_SHAPES.forEachIndexed { index, shape ->
                MorphShape(
                    if (lit > index) Shapes.Cookie9 else shape,
                    color,
                    Modifier.size(16.dp).graphicsLayer { alpha = DIM + (1f - DIM) * (progress() * 3 - index).coerceIn(0f, 1f) },
                    rotation = { position() * (if (index % 2 == 1) -40f else 50f) + index * 30f },
                )
            }
        }
    }
}

/**
 * Каскадная прокрутка. Каждый сдвиг списка запоминается с моментом и номером активной строки, а строка
 * считает своё смещение из этих записей. Строка, которая появилась в списке уже после сдвига (въехала
 * снизу), получает то же смещение, что и соседи: раньше она вставала сразу на место, и соседняя строка,
 * ещё не доехавшая, рисовалась поверх неё.
 */
private class Cascade {
    private class Move(val at: Long, val dy: Float, val active: Int)

    private val moves = mutableStateListOf<Move>()
    private var now by mutableLongStateOf(System.nanoTime())

    fun push(dy: Float, active: Int) {
        moves += Move(System.nanoTime(), dy, active)
    }

    /** Смещение строки [index] в этот кадр: сумма ещё не отыгранных сдвигов. */
    fun shift(index: Int): Float {
        var sum = 0f
        for (move in moves) {
            val delay = (index - move.active + 1).coerceIn(0, 7) * STAGGER_MS
            val t = ((now - move.at) / 1e6f - delay) / SCROLL_MS
            sum += move.dy * (1f - EaseInOut.transform(t.coerceIn(0f, 1f)))
        }
        return sum
    }

    /** Ведёт время, пока есть неотыгранные сдвиги. */
    suspend fun run() {
        snapshotFlow { moves.isNotEmpty() }.collectLatest { busy ->
            while (busy && moves.isNotEmpty()) {
                now = withFrameNanos { it }
                moves.removeAll { (now - it.at) / 1e6f > SCROLL_MS + 7 * STAGGER_MS }
            }
        }
    }
}

private val GAP_SHAPES = listOf(Shapes.Cookie4, Shapes.Clover4, Shapes.Cookie6)
private val LINE_STYLE = TextStyle()
private val FEATHER = 6.dp          // полуширина мягкой границы заливки
private const val LEAD = 0.25f      // строка становится текущей (и начинает ехать) чуть раньше, чем её поют
private const val ANCHOR = 0.3f     // где держится активная строка, доля высоты
private const val DIM = 0.35f       // ещё не спетое
private const val SCROLL_MS = 700
private const val STAGGER_MS = 45L
private const val HOLD_MS = 3000L   // сколько автопрокрутка ждёт после ручной
