package io.github.yaskorinov.yamusic.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.gestures.detectTapGestures
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
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * «Моя волна» — одна сцена на всю страницу («орбита», по мотивам WavePage.qml). В центре — крупная фигура
 * с кнопкой, вокруг неё летают четыре сферы-группы настроек, у каждой своя фигура. Нажатие на сферу
 * разворачивает её: сфера распадается на варианты, они разлетаются по орбите, а центр уменьшается,
 * уступая им место. Выбранный вариант получает цвет и форму группы и сворачивает орбиту обратно; сфера
 * группы после этого показывает выбор. Повторное нажатие на выбранный вариант отпускает его («любое»).
 * Центр тоже отзывается: его форму задаёт настроение, цвет — характер.
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
    val scope = rememberCoroutineScope()
    val active = playerState.wave
    val playing = active && playerState.playing

    // Развёрнутая группа (-1 — все свёрнуты) и та, чьи варианты нарисованы: она остаётся и пока орбита сворачивается
    var expanded by rememberSaveable { mutableIntStateOf(-1) }
    var shown by remember { mutableIntStateOf(0) }
    val open = remember { Animatable(0f) }
    LaunchedEffect(expanded) {
        if (expanded >= 0) shown = expanded
        open.animateTo(if (expanded >= 0) 1f else 0f, tween(if (expanded >= 0) 640 else 460, easing = LinearEasing))
    }
    val unfolded by remember { derivedStateOf { open.value > 0f } }
    BackHandler(enabled = expanded >= 0) { expanded = -1 }

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
        BoxWithConstraints(
            Modifier.weight(1f).fillMaxWidth().pointerInput(expanded) {
                if (expanded >= 0) detectTapGestures { expanded = -1 } // нажатие мимо вариантов сворачивает орбиту
            },
        ) {
            val density = LocalDensity.current
            val width = constraints.maxWidth.toFloat()
            val height = constraints.maxHeight.toFloat()
            val sphere = (maxWidth * 0.23f).coerceIn(76.dp, 100.dp)
            val options = groups.getOrNull(shown)?.items.orEmpty()
            val scene = remember(groups.size, options.size, width, height, sphere) {
                with(density) { scene(groups.size, options.size, width, height, sphere.toPx(), (maxWidth * 0.21f).coerceIn(68.dp, 92.dp).toPx(), 10.dp.toPx()) }
            }
            val bubble = with(density) { scene.bubble.toDp() }
            val hero by animateDpAsState(
                with(density) { (if (expanded >= 0) scene.heroSmall else scene.heroBig).toDp() },
                tween(Motion.SpatialSlow, easing = Motion.outBack(1.1f)), label = "hero",
            )
            val time = rememberTime(true)
            val intro = remember { Animatable(0f) }
            LaunchedEffect(groups) {
                intro.snapTo(0f)
                intro.animateTo(1f, tween(900, easing = LinearEasing))
            }

            // Орбита — пунктир: большая, пока варианты развёрнуты, и та, по которой летают сферы
            Canvas(Modifier.fillMaxSize()) {
                val dots = Stroke(
                    1.5.dp.toPx(), cap = StrokeCap.Round,
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(1.5.dp.toPx(), 7.5.dp.toPx())),
                )
                val unfold = Motion.EmphasizedDecelerate.transform(open.value)
                val rx = scene.sphereRx + (scene.rx - scene.sphereRx) * unfold
                val ry = scene.sphereRy + (scene.ry - scene.sphereRy) * unfold
                drawOval(
                    colors.outlineVariant, Offset(center.x - rx, center.y - ry), Size(rx * 2, ry * 2),
                    alpha = min(1f, intro.value * 2), style = dots,
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

            // Сферы групп
            groups.forEachIndexed { index, group ->
                key(group.key) {
                    val style = groupStyle(group.key)
                    val chosen = group.items.firstOrNull { it.seed == selection[group.key] }
                    val source = remember { MutableInteractionSource() }
                    val pressed by source.collectIsPressedAsState()
                    val press by animateFloatAsState(if (pressed) 0.94f else 1f, tween(Motion.SpatialFast, easing = Motion.outBack()), label = "press")
                    val slot = scene.spheres[index]
                    Box(
                        Modifier
                            .size(sphere)
                            .graphicsLayer {
                                // вылет из центра при появлении — по очереди
                                val a = (intro.value * 1.6f - index.toFloat() / groups.size * 0.6f).coerceIn(0f, 1f)
                                val appear = Motion.outBack(1.2f).transform(a)
                                // сфера распадается: та, что нажали, чуть раздувается и тает, остальные уходят
                                val gone = (open.value / 0.45f).coerceIn(0f, 1f)
                                val t = time()
                                val phase = index * 1.7f
                                translationX = width / 2 + slot.x * appear + 6.dp.toPx() * sin(t * 0.45f + phase) - size.width / 2
                                translationY = height / 2 + slot.y * appear + 6.dp.toPx() * cos(t * 0.37f + phase * 1.3f) - size.height / 2
                                alpha = a * (1f - gone)
                                scaleX = (0.4f + 0.6f * a) * press * (if (index == shown) 1f + 0.25f * gone else 1f - 0.3f * gone)
                                scaleY = scaleX
                            }
                            .clickable(source, indication = null, enabled = expanded < 0) { expanded = index },
                        contentAlignment = Alignment.Center,
                    ) {
                        MorphShape(
                            style.shape,
                            if (chosen != null) style.color(colors) else colors.surfaceContainerHighest,
                            Modifier.fillMaxSize(),
                            rotation = { time() * (if (index % 2 == 0) 5f else -5f) },
                        )
                        val tint = if (chosen != null) style.content(colors) else colors.onSurface
                        Column(Modifier.fillMaxWidth(0.8f), horizontalAlignment = Alignment.CenterHorizontally) {
                            Text(
                                group.title,
                                style = if (chosen == null) MaterialTheme.typography.labelLarge else MaterialTheme.typography.labelSmall,
                                fontWeight = FontWeight.SemiBold,
                                color = if (chosen == null) tint else tint.copy(alpha = 0.75f),
                                maxLines = 1,
                                softWrap = false,
                                autoSize = TextAutoSize.StepBased(minFontSize = 8.sp, maxFontSize = 14.sp, stepSize = 0.5.sp),
                            )
                            if (chosen != null) {
                                Text(
                                    chosen.label,
                                    style = MaterialTheme.typography.labelLarge,
                                    fontWeight = FontWeight.Bold,
                                    color = tint,
                                    maxLines = 1,
                                    softWrap = false,
                                    autoSize = TextAutoSize.StepBased(minFontSize = 8.sp, maxFontSize = 14.sp, stepSize = 0.5.sp),
                                )
                            }
                        }
                    }
                }
            }

            // Варианты развёрнутой группы: разлетаются из её сферы по орбите
            if (unfolded || expanded >= 0) {
                val group = groups.getOrNull(shown)
                val style = groupStyle(group?.key.orEmpty())
                val from = scene.spheres.getOrElse(shown) { Offset.Zero }
                options.forEachIndexed { index, item ->
                    key(group?.key, item.seed) {
                        val selected = group != null && selection[group.key] == item.seed
                        val source = remember { MutableInteractionSource() }
                        val pressed by source.collectIsPressedAsState()
                        val press by animateFloatAsState(if (pressed) 0.94f else 1f, tween(Motion.SpatialFast, easing = Motion.outBack()), label = "press")
                        val slot = scene.options.getOrElse(index) { Offset.Zero }
                        Box(
                            Modifier
                                .size(bubble)
                                .zIndex(2f)
                                .graphicsLayer {
                                    // по очереди вдоль орбиты, с лёгким перелётом
                                    val a = (open.value * 1.5f - index.toFloat() / options.size * 0.5f).coerceIn(0f, 1f)
                                    val fly = Motion.outBack(1.3f).transform(a)
                                    val t = time()
                                    val phase = index * 1.7f
                                    translationX = width / 2 + from.x + (slot.x - from.x) * fly + 4.dp.toPx() * sin(t * 0.45f + phase) * a - size.width / 2
                                    translationY = height / 2 + from.y + (slot.y - from.y) * fly + 3.5.dp.toPx() * cos(t * 0.37f + phase * 1.3f) * a - size.height / 2
                                    alpha = (a * 2.5f).coerceIn(0f, 1f)
                                    scaleX = (0.3f + 0.7f * fly) * press
                                    scaleY = scaleX
                                }
                                .clickable(source, indication = null, enabled = expanded >= 0 && group != null) {
                                    wave.select(group!!.key, item.seed, active)
                                    // выбор виден мгновение — и орбита сворачивается
                                    scope.launch {
                                        delay(280)
                                        expanded = -1
                                    }
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            MorphShape(
                                if (selected) style.shape else Shapes.Circle,
                                if (selected) style.color(colors) else colors.surfaceContainerHighest,
                                Modifier.fillMaxSize(),
                                rotation = { if (selected) time() * 6f else 0f },
                            )
                            Text(
                                item.label,
                                Modifier.fillMaxWidth(0.82f),
                                style = MaterialTheme.typography.labelLarge,
                                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                color = if (selected) style.content(colors) else colors.onSurface,
                                textAlign = TextAlign.Center,
                                maxLines = 1,
                                softWrap = false,
                                autoSize = TextAutoSize.StepBased(minFontSize = 8.sp, maxFontSize = 14.sp, stepSize = 0.5.sp),
                            )
                        }
                    }
                }
            }
        }

        // Подпись под орбитой: какая группа развёрнута / что играет / подсказка
        val track = playerState.track
        Box(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 32.dp), contentAlignment = Alignment.Center) {
            AnimatedContent(
                when {
                    error.isNotEmpty() -> error
                    expanded >= 0 -> groups.getOrNull(expanded)?.title.orEmpty()
                    active && track != null -> "${track.title} — ${track.artists}"
                    else -> "Бесконечный поток под ваш вкус"
                },
                transitionSpec = { fadeIn(tween(Motion.EffectsDefault)) togetherWith fadeOut(tween(Motion.EffectsFast)) },
                label = "caption",
            ) { text ->
                Text(
                    text,
                    Modifier.fillMaxWidth(),
                    style = if (expanded >= 0 && error.isEmpty()) MaterialTheme.typography.titleMedium else MaterialTheme.typography.bodyLarge,
                    color = if (error.isEmpty()) colors.onSurfaceVariant else colors.error,
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
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

/**
 * Раскладка сцены, от центра. Свёрнутая: [spheres] — места сфер на своей орбите, [heroBig] — центр.
 * Развёрнутая: [options] — места вариантов на большой орбите ([rx], [ry]), [bubble] — их размер, [heroSmall] — центр.
 */
private class Scene(
    val sphereRx: Float, val sphereRy: Float, val spheres: List<Offset>, val heroBig: Float,
    val rx: Float, val ry: Float, val options: List<Offset>, val bubble: Float, val heroSmall: Float,
)

/**
 * Обе орбиты — окружности: больше не помещается в ширину экрана. Сферы стоят на своей по диагоналям — так
 * между ними остаётся место для крупного центра. Варианты — на окружности у самого края сцены; только если
 * их так много, что кружки стали бы мельче [SMALLEST] от желаемого, орбита вытягивается по вертикали.
 */
private fun scene(groups: Int, options: Int, width: Float, height: Float, sphere: Float, wanted: Float, gap: Float): Scene {
    val turn = 2 * PI.toFloat()
    val half = min(width, height) / 2
    val diagonal = sqrt(0.5f)
    val sphereR = min(half - gap / 4, (half - sphere / 2 - gap) / diagonal)
    val spheres = List(groups) { g ->
        val a = PI.toFloat() / 2 + turn * (g + 0.5f) / max(1, groups)
        Offset(sphereR * cos(a), sphereR * sin(a))
    }
    val heroBig = min(width * 0.64f, 2 * (sphereR - sphere / 2 - gap))

    var bubble = wanted
    val rx = half - bubble / 2 - gap
    var ry = rx
    val needed = max(1, options) * (wanted * SMALLEST + gap)
    if (turn * rx < needed) {
        // окружности не хватает: вытягиваем настолько, насколько нужно и насколько пускает высота
        val stretched = sqrt(max(rx * rx, 2 * (needed / turn) * (needed / turn) - rx * rx))
        ry = min(stretched, height / 2 - bubble / 2 - gap)
    }
    bubble = min(bubble, turn * sqrt((rx * rx + ry * ry) / 2) / max(1, options) - gap)
    val steps = 720
    val xs = FloatArray(steps + 1)
    val ys = FloatArray(steps + 1)
    val acc = FloatArray(steps + 1)
    for (k in 0..steps) {
        val a = PI.toFloat() / 2 + turn * k / steps
        xs[k] = rx * cos(a)
        ys[k] = ry * sin(a)
        if (k > 0) acc[k] = acc[k - 1] + hypot(xs[k] - xs[k - 1], ys[k] - ys[k - 1])
    }
    var k = 0
    val places = List(options) { i ->
        val target = acc[steps] * (i + 0.5f) / options
        while (k < steps - 1 && acc[k + 1] < target) k++
        val f = (target - acc[k]) / max(1e-6f, acc[k + 1] - acc[k])
        Offset(xs[k] + (xs[k + 1] - xs[k]) * f, ys[k] + (ys[k + 1] - ys[k]) * f)
    }
    return Scene(sphereR, sphereR, spheres, heroBig, rx, ry, places, bubble, heroSmall = 2 * (min(rx, ry) - bubble / 2 - gap * 1.5f))
}

private const val SMALLEST = 0.7f
