package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.WaveGroup
import io.github.yaskorinov.yamusic.data.Wave
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * «Моя волна» — одна сцена на всю страницу («орбита», перенос WavePage.qml). В центре — фигура с кнопкой,
 * вокруг неё по эллипсу плавают настройки волны: кружки, собранные в дуги по группам. Выбранная настройка
 * притягивается к центру, прилипает к фигуре и получает цвет и форму своей группы; повторное нажатие
 * отпускает её обратно («любое»). Центр тоже отзывается: его форму задаёт настроение, цвет — характер.
 * Движение — только сдвиги и повороты слоёв, перекомпоновки при этом нет.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun WaveScreen(wave: Wave, player: PlayerConnection, playerState: PlayerState, contentPadding: PaddingValues) {
    val groups by wave.groups.collectAsStateWithLifecycle()
    val selection by wave.selection.collectAsStateWithLifecycle()
    val loading by wave.loading.collectAsStateWithLifecycle()
    val error by wave.error.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val active = playerState.wave
    val playing = active && playerState.playing
    val flat = remember(groups) { flatten(groups) }

    // Центр отзывается на выбор
    val heroShape = when (selection["moodEnergy"]?.substringAfterLast(':')) {
        "active" -> Shapes.Sunny
        "fun" -> Shapes.Flower8
        "calm" -> Shapes.Scallop
        "sad" -> Shapes.Blob
        else -> Shapes.Cookie12
    }
    val (heroColor, heroAccent) = when (selection["diversity"]?.substringAfterLast(':')) {
        "discover" -> colors.tertiaryContainer to colors.tertiary
        "popular" -> colors.secondaryContainer to colors.secondary
        else -> colors.primaryContainer to colors.primary
    }

    Column(Modifier.fillMaxSize().padding(contentPadding)) {
        TopBar("Моя волна")
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
            val density = LocalDensity.current
            val width = constraints.maxWidth.toFloat()
            val height = constraints.maxHeight.toFloat()
            val bubble = with(density) { (maxWidth * 0.17f).coerceIn(60.dp, 84.dp) }
            val orbit = remember(flat, width, height, bubble) {
                with(density) { orbit(flat, groups.size, width, height, bubble.toPx(), 8.dp.toPx()) }
            }
            val hero = with(density) { orbit.hero.toDp().coerceIn(104.dp, 240.dp) }
            val time = rememberTime(true)
            val intro = remember { Animatable(0f) }
            LaunchedEffect(groups) {
                intro.snapTo(0f)
                intro.animateTo(1f, tween(1100, easing = LinearEasing))
            }

            // Орбита — пунктир
            Canvas(Modifier.fillMaxSize()) {
                drawOval(
                    colors.outlineVariant,
                    topLeft = Offset(center.x - orbit.rx, center.y - orbit.ry),
                    size = Size(orbit.rx * 2, orbit.ry * 2),
                    alpha = min(1f, intro.value * 2),
                    style = Stroke(
                        1.5.dp.toPx(), cap = StrokeCap.Round,
                        pathEffect = PathEffect.dashPathEffect(floatArrayOf(1.5.dp.toPx(), 7.5.dp.toPx())),
                    ),
                )
            }

            // Центр: фигуры и кнопка
            Box(Modifier.align(Alignment.Center).size(hero).zIndex(1f), contentAlignment = Alignment.Center) {
                val outer = rememberSpin(playing, speed = 8f)
                val inner = rememberSpin(playing, speed = 12f)
                MorphShape(heroShape, heroColor, Modifier.fillMaxSize().graphicsLayer(), duration = Motion.SpatialSlow, rotation = outer)
                MorphShape(
                    if (playing) Shapes.Cookie6 else Shapes.Clover4, heroAccent.copy(alpha = 0.22f),
                    Modifier.fillMaxSize(0.7f).graphicsLayer(), duration = Motion.SpatialSlow, rotation = { -inner() },
                )
                PlayButton(
                    playing = playing,
                    onClick = { if (active) player.togglePlay() else wave.start() },
                    size = (hero * 0.42f).coerceAtLeast(56.dp),
                    playingShape = Shapes.Cookie9,
                    pausedShape = Shapes.Cookie6,
                    spin = false,
                )
                if (loading || (active && playerState.buffering)) {
                    LoadingIndicator(Modifier.size(hero * 0.52f).alpha(0.6f), color = colors.primary)
                }
            }

            // Настройки на орбите
            val reach = with(density) { hero.toPx() / 2 }
            flat.forEachIndexed { index, entry ->
                key(entry.key, entry.seed) {
                    val selected = !entry.caption && selection[entry.key] == entry.seed
                    // 0 — на орбите, 1 — прилип к центру
                    val docked by animateFloatAsState(if (selected) 1f else 0f, tween(620, easing = Motion.outBack(1.3f)), label = "dock")
                    val source = remember { MutableInteractionSource() }
                    val pressed by source.collectIsPressedAsState()
                    val press by animateFloatAsState(if (pressed) 0.94f else 1f, tween(Motion.SpatialFast, easing = Motion.outBack()), label = "press")
                    val slot = orbit.at[index]
                    val dock = orbit.dock[entry.group]
                    val style = groupStyle(entry.key)
                    Box(
                        Modifier
                            .then(if (entry.caption) Modifier.height(bubble) else Modifier.size(bubble))
                            .zIndex(if (selected) 2f else if (entry.caption) 0.5f else 0f)
                            .graphicsLayer {
                                // вылет из центра при появлении: по очереди вдоль орбиты
                                val a = (intro.value * 1.9f - index.toFloat() / flat.size * 0.9f).coerceIn(0f, 1f)
                                val appear = 1f - (1f - a).pow(3)
                                // лёгкое покачивание на месте; у центра затихает
                                val phase = index * 1.7f
                                val t = time()
                                val bobX = 4.dp.toPx() * sin(t * 0.45f + phase)
                                val bobY = 3.5.dp.toPx() * cos(t * 0.37f + phase * 1.3f)
                                translationX = width / 2 + (slot.x * appear + bobX) * (1 - docked) + dock.x * reach * docked - size.width / 2
                                translationY = height / 2 + (slot.y * appear + bobY) * (1 - docked) + dock.y * reach * docked - size.height / 2
                                alpha = appear
                                scaleX = (0.4f + 0.6f * appear) * press
                                scaleY = scaleX
                            }
                            .clickable(source, indication = null, enabled = !entry.caption) { wave.select(entry.key, entry.seed, active) },
                        contentAlignment = Alignment.Center,
                    ) {
                        if (entry.caption) {
                            Text(entry.label, style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.SemiBold, color = colors.onSurfaceVariant)
                        } else {
                            MorphShape(
                                if (selected) style.shape else Shapes.Circle,
                                if (selected) style.color(colors) else colors.surfaceContainerHighest,
                                Modifier.fillMaxSize(),
                                rotation = { if (selected) time() * 6f else 0f },
                            )
                            Text(
                                entry.label,
                                Modifier.fillMaxWidth(0.84f),
                                style = MaterialTheme.typography.labelMedium,
                                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                color = if (selected) style.content(colors) else colors.onSurface,
                                textAlign = TextAlign.Center,
                                maxLines = 1,
                                softWrap = false,
                                autoSize = TextAutoSize.StepBased(minFontSize = 8.sp, maxFontSize = 13.sp, stepSize = 0.5.sp),
                            )
                        }
                    }
                }
            }
        }

        // Подпись под орбитой: что играет / подсказка
        val track = playerState.track
        Box(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 32.dp), contentAlignment = Alignment.Center) {
            Text(
                when {
                    error.isNotEmpty() -> error
                    active && track != null -> "${track.title} — ${track.artists}"
                    else -> "Бесконечный поток под ваш вкус"
                },
                style = MaterialTheme.typography.bodyLarge,
                color = if (error.isEmpty()) colors.onSurfaceVariant else colors.error,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** То, что летает: заголовки групп тоже стоят на орбите. */
private class Entry(val caption: Boolean, val group: Int, val key: String, val label: String, val seed: String)

private fun flatten(groups: List<WaveGroup>): List<Entry> = buildList {
    groups.forEachIndexed { index, group ->
        add(Entry(true, index, group.key, group.title, ""))
        for (item in group.items) add(Entry(false, index, group.key, item.label, item.seed))
    }
}

/** Форма и цвет выбранной настройки — свои у каждой группы. */
private class GroupStyle(
    val shape: PolarShape,
    val color: (androidx.compose.material3.ColorScheme) -> androidx.compose.ui.graphics.Color,
    val content: (androidx.compose.material3.ColorScheme) -> androidx.compose.ui.graphics.Color,
)

private val GROUP_STYLES = mapOf(
    "context" to GroupStyle(Shapes.Cookie9, { it.primary }, { it.onPrimary }),
    "diversity" to GroupStyle(Shapes.Cookie4, { it.tertiary }, { it.onTertiary }),
    "moodEnergy" to GroupStyle(Shapes.Flower8, { it.secondary }, { it.onSecondary }),
    "language" to GroupStyle(Shapes.Cookie6, { it.inverseSurface }, { it.inverseOnSurface }),
)

private fun groupStyle(key: String) = GROUP_STYLES[key] ?: GROUP_STYLES.getValue("context")

/** Раскладка сцены: полуоси орбиты, места настроек (от центра), направления «причала» групп и размер центра. */
private class Orbit(val rx: Float, val ry: Float, val at: List<Offset>, val dock: List<Offset>, val hero: Float)

/**
 * Места на орбите — равные шаги по длине дуги эллипса, от низа по часовой стрелке. Соседи стоят по разные
 * стороны от орбиты (сдвиг [delta] по нормали): на узком экране кружки не мельчают, а расходятся на два
 * кольца. Заголовки групп остаются на самой орбите. На телефоне эллипс вытянут по вертикали и у боков
 * центра места нет, поэтому выбранные настройки причаливают к фигуре по диагоналям — у каждой группы
 * своя, в том же порядке, что и дуги на орбите.
 */
private fun orbit(flat: List<Entry>, groups: Int, width: Float, height: Float, bubble: Float, gap: Float): Orbit {
    val n = flat.size
    val outerX = width / 2 - bubble / 2 - gap
    val outerY = height / 2 - bubble / 2 - gap
    var delta = bubble * 0.3f
    var rx = outerX
    var ry = outerY
    repeat(4) {
        rx = outerX - delta
        ry = outerY - delta
        val spacing = 2 * PI.toFloat() * sqrt((rx * rx + ry * ry) / 2) / max(1, n)
        // с запасом: на крутых концах эллипса внутреннее кольцо короче самой орбиты
        delta = max(gap, sqrt(max(0f, (bubble + gap).pow(2) - spacing * spacing)) / 2 * 1.15f)
    }
    val steps = 720
    val xs = FloatArray(steps + 1)
    val ys = FloatArray(steps + 1)
    val acc = FloatArray(steps + 1)
    for (k in 0..steps) {
        val a = PI.toFloat() / 2 + 2 * PI.toFloat() * k / steps
        xs[k] = rx * cos(a)
        ys[k] = ry * sin(a)
        if (k > 0) acc[k] = acc[k - 1] + hypot(xs[k] - xs[k - 1], ys[k] - ys[k - 1])
    }
    var k = 0
    val at = List(n) { i ->
        val target = acc[steps] * (i + 0.5f) / n
        while (k < steps - 1 && acc[k + 1] < target) k++
        val f = (target - acc[k]) / max(1e-6f, acc[k + 1] - acc[k])
        val a = PI.toFloat() / 2 + 2 * PI.toFloat() * (k + f) / steps
        // нормаль к эллипсу в этой точке
        val nx = cos(a) / rx
        val ny = sin(a) / ry
        val side = (if (flat[i].caption) 0f else if (i % 2 == 1) delta else -delta) / hypot(nx, ny)
        Offset(xs[k] + (xs[k + 1] - xs[k]) * f + nx * side, ys[k] + (ys[k + 1] - ys[k]) * f + ny * side)
    }
    val dock = List(groups) { g ->
        val a = PI.toFloat() / 2 + 2 * PI.toFloat() * (g + 0.5f) / groups
        Offset(cos(a), sin(a))
    }
    return Orbit(rx, ry, at, dock, hero = 2 * (min(rx, ry) - delta - bubble / 2 - gap))
}
