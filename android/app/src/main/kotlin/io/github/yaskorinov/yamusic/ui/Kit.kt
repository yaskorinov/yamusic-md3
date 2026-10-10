package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.EaseInOutCubic
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInParent
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.data.Library
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.min
import kotlin.math.sin
import kotlin.random.Random
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Play/Pause в «печеньке» MD3 Expressive. Во время воспроизведения фигура медленно вращается,
 * при нажатии сжимается в более круглую, при смене состояния морфится.
 */
@Composable
fun PlayButton(
    playing: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 64.dp,
    container: Color = MaterialTheme.colorScheme.primary,
    content: Color = MaterialTheme.colorScheme.onPrimary,
    playingShape: PolarShape = Shapes.Cookie9,
    pausedShape: PolarShape = Shapes.Cookie4,
    spin: Boolean = true,
    enabled: Boolean = true,
) {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed) 0.88f else 1f, tween(Motion.SpatialFast, easing = Motion.outBack()), label = "press")
    val rotation = rememberSpin(spin && playing, speed = 30f)
    Box(
        modifier.size(size).clickable(source, indication = null, enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        MorphShape(
            shape = if (pressed) Shapes.Circle else if (playing) playingShape else pausedShape,
            color = container,
            modifier = Modifier.fillMaxSize().graphicsLayer {
                scaleX = scale
                scaleY = scale
            },
            rotation = rotation,
        )
        Symbol(if (playing) "pause" else "play_arrow", size = size * 0.45f, filled = true, tint = content)
    }
}

/**
 * Линейный прогресс MD3 Expressive: проигранная часть — бегущая волна, остаток — прямой трек с зазором
 * и stop-точкой. На паузе ([wavy] = false) волна плавно выпрямляется. С [onSeek] превращается в перемотку:
 * пока палец ведёт, [onDrag] сообщает значение под ним (null — отпустили).
 */
@Composable
fun WavyProgress(
    value: () -> Float,
    wavy: Boolean,
    modifier: Modifier = Modifier,
    thickness: Dp = 4.dp,
    amplitude: Dp = 3.dp,
    wavelength: Dp = 40.dp,
    onDrag: (Float?) -> Unit = {},
    onSeek: ((Float) -> Unit)? = null,
) {
    val colors = MaterialTheme.colorScheme
    val amp by animateDpAsState(if (wavy) amplitude else 0.dp, tween(Motion.SpatialDefault, easing = Motion.Emphasized), label = "amp")
    val time = rememberTime(wavy)
    var drag by remember { mutableStateOf<Float?>(null) }
    val path = remember { Path() }
    // Показанное значение догоняет настоящее плавно: перемотка и смена трека — не скачком.
    // Обычный ход трека меньше порога и идёт без запаздывания.
    val shownValue = remember { mutableFloatStateOf(value()) }
    val target by rememberUpdatedState(value)
    val moving by rememberUpdatedState(wavy)
    LaunchedEffect(Unit) {
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            val dt = (now - last) / 1e9f
            last = now
            val wanted = drag ?: target()
            val gap = wanted - shownValue.floatValue
            if (drag != null || abs(gap) < 0.0006f) {
                shownValue.floatValue = wanted
                if (!moving && drag == null) { // стоит на месте — не будить кадры зря
                    delay(120)
                    last = withFrameNanos { it }
                }
            } else {
                shownValue.floatValue += gap * (1f - exp(-dt / 0.11f))
            }
        }
    }
    Canvas(
        modifier.graphicsLayer().pointerInput(onSeek) {
            if (onSeek == null) return@pointerInput
            val cap = thickness.toPx() / 2
            fun at(x: Float) = ((x - cap) / (size.width - 2 * cap)).coerceIn(0f, 1f)
            awaitEachGesture {
                val down = awaitFirstDown()
                var last = at(down.position.x)
                while (true) {
                    drag = last
                    onDrag(last)
                    val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: break
                    if (!change.pressed) break
                    last = at(change.position.x)
                    change.consume()
                }
                onSeek(last)
                drag = null
                onDrag(null)
            }
        },
    ) {
        val stroke = thickness.toPx()
        val cap = stroke / 2
        val cy = size.height / 2
        val x0 = cap
        val x1 = size.width - cap
        val shown = (drag ?: shownValue.floatValue).coerceIn(0f, 1f)
        val activeEnd = x0 + (x1 - x0) * shown
        val gap = 4.dp.toPx()
        val trackStart = min(x1, activeEnd + gap + stroke)
        if (trackStart < x1) drawLine(colors.secondaryContainer, Offset(trackStart, cy), Offset(x1, cy), stroke, StrokeCap.Round)
        if (activeEnd < x1 - gap - stroke) drawCircle(colors.primary, cap, Offset(x1, cy))
        if (shown > 0f) {
            val a = amp.toPx()
            val k = 2 * PI.toFloat() / wavelength.toPx()
            val phase = time() * 2 * PI.toFloat() // один период волны в секунду
            path.rewind()
            var x = x0
            path.moveTo(x, cy + a * sin(k * x - phase))
            while (x < activeEnd) {
                x = min(activeEnd, x + 1.5.dp.toPx())
                path.lineTo(x, cy + a * sin(k * x - phase))
            }
            drawPath(path, colors.primary, style = Stroke(stroke, cap = StrokeCap.Round, join = StrokeJoin.Round))
        }
    }
}

class Segment(val text: String = "", val icon: String = "")

/**
 * Связанная группа кнопок MD3 Expressive: выбранный сегмент и внешние углы — полностью круглые,
 * внутренние — малое скругление. [selected] = -1 — ничего не выбрано.
 */
@Composable
fun Segments(
    items: List<Segment>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    height: Dp = 40.dp,
    compact: Boolean = false,
    segmentColor: Color = MaterialTheme.colorScheme.surfaceContainerHighest,
) {
    val colors = MaterialTheme.colorScheme
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        items.forEachIndexed { index, item ->
            val chosen = index == selected
            val source = remember { MutableInteractionSource() }
            val pressed by source.collectIsPressedAsState()
            val inner = if (pressed) 4.dp else 8.dp
            val spec = tween<Dp>(Motion.SpatialFast, easing = Motion.outBack())
            val left by animateDpAsState(if (chosen || index == 0) height / 2 else inner, spec, label = "left")
            val right by animateDpAsState(if (chosen || index == items.lastIndex) height / 2 else inner, spec, label = "right")
            val container by animateColorAsState(if (chosen) colors.secondaryContainer else segmentColor, tween(Motion.EffectsDefault), label = "segment")
            val tint = if (chosen) colors.onSecondaryContainer else colors.onSurfaceVariant
            Row(
                Modifier
                    .height(height)
                    .clip(RoundedCornerShape(left.coerceAtLeast(0.dp), right.coerceAtLeast(0.dp), right.coerceAtLeast(0.dp), left.coerceAtLeast(0.dp)))
                    .background(container)
                    .clickable(source, indication = null) { onSelect(index) }
                    .padding(horizontal = if (compact) 10.dp else 16.dp),
                horizontalArrangement = Arrangement.spacedBy(if (compact) 4.dp else 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (item.icon.isNotEmpty()) Symbol(item.icon, size = 18.dp, filled = chosen, tint = tint)
                if (item.text.isNotEmpty()) Text(item.text, style = MaterialTheme.typography.labelLarge, color = tint, maxLines = 1)
            }
        }
    }
}

/**
 * Пустое состояние: значок в перетекающих фигурах, заголовок, пояснение, необязательная кнопка.
 * Фигуры медленно перетекают одна в другую: морф раз в несколько секунд, между морфами экран
 * не перерисовывается. За основной фигурой — бледная побольше, со сдвигом по фазе.
 */
@Composable
fun EmptyState(
    icon: String,
    title: String,
    modifier: Modifier = Modifier,
    text: String = "",
    shape: PolarShape = Shapes.Cookie9,
    actionText: String = "",
    onAction: () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    val cycle = remember(shape) {
        listOf(shape, Shapes.Cookie12, Shapes.Flower8, Shapes.Clover4, Shapes.Sunny, Shapes.Cookie6, Shapes.Flower6, Shapes.Cookie9)
    }
    var step by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(2800)
            step++
        }
    }
    val back by animateFloatAsState(-step * 25f, tween(Motion.SpatialSlow * 2, easing = EaseInOutCubic), label = "back")
    val front by animateFloatAsState(step * 40f, tween(Motion.SpatialSlow * 8 / 5, easing = EaseInOutCubic), label = "front")
    Column(modifier.padding(horizontal = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.size(156.dp), contentAlignment = Alignment.Center) {
            MorphShape(
                cycle[(step + 3) % cycle.size], colors.primaryContainer.copy(alpha = 0.35f), Modifier.fillMaxSize(),
                duration = Motion.SpatialSlow * 2, overshoot = 1.05f, rotation = { back },
            )
            MorphShape(
                cycle[step % cycle.size], colors.secondaryContainer, Modifier.size(120.dp),
                duration = Motion.SpatialSlow * 8 / 5, overshoot = 1.15f, rotation = { front },
            )
            Symbol(icon, size = 46.dp, filled = true, tint = colors.onSecondaryContainer)
        }
        Spacer(Modifier.height(16.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
        if (text.isNotEmpty()) {
            Spacer(Modifier.height(12.dp))
            Text(text, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, textAlign = TextAlign.Center)
        }
        if (actionText.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            FilledTonalButton(onClick = onAction) { Text(actionText) }
        }
    }
}

/** Заголовок секции страницы с необязательной кнопкой справа («Все», «Свернуть»). */
@Composable
fun SectionHeader(text: String, modifier: Modifier = Modifier, actionText: String = "", onAction: () -> Unit = {}) {
    Row(modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 20.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text, Modifier.weight(1f), style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold)
        if (actionText.isNotEmpty()) TextButton(onClick = onAction) { Text(actionText) }
    }
}

/** Надзаголовок шапки: «ПЛЕЙЛИСТ», «ИСПОЛНИТЕЛЬ». */
@Composable
fun Overline(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, letterSpacing = 1.2.sp, maxLines = 1)
}

/** Отметка «E» у треков и альбомов с ненормативной лексикой. */
@Composable
fun ExplicitBadge() {
    val colors = MaterialTheme.colorScheme
    Box(Modifier.size(16.dp).background(colors.surfaceContainerHighest, RoundedCornerShape(4.dp)), contentAlignment = Alignment.Center) {
        Text("E", style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant)
    }
}

/** Верхняя панель страницы: «назад» и заголовок, который проявляется, когда шапка уехала из виду. */
@Composable
fun TopBar(title: String, showTitle: Boolean = true, onBack: (() -> Unit)? = null, actions: @Composable () -> Unit = {}) {
    val shown by animateFloatAsState(if (showTitle) 1f else 0f, tween(Motion.EffectsDefault), label = "title")
    Row(Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        if (onBack != null) IconButton(onClick = onBack) { Symbol("arrow_back") } else Spacer(Modifier.width(12.dp))
        Text(
            title,
            Modifier.weight(1f).padding(start = 4.dp).graphicsLayer { alpha = shown },
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        actions()
    }
}

/** Заголовок раздела верхнего уровня: один кегль на «Моей волне», в «Коллекции» и в «Настройках». */
@Composable
fun PageTitle(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier.padding(top = 20.dp, bottom = 16.dp),
        style = MaterialTheme.typography.displaySmall,
        fontWeight = FontWeight.SemiBold,
        maxLines = 1,
    )
}

/**
 * Пункт навигации: пилюля; у выбранного — значок в квадратике цвета primary. В нижней панели
 * подпись стоит рядом со значком, как в боковой панели десктопного клиента.
 */
@Composable
fun NavItem(icon: String, text: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val container by animateColorAsState(if (selected) colors.secondaryContainer else Color.Transparent, tween(Motion.EffectsDefault), label = "nav")
    val square by animateFloatAsState(if (selected) 1f else 0.4f, tween(Motion.SpatialFast, easing = Motion.outBack(2f)), label = "square")
    val squareAlpha by animateFloatAsState(if (selected) 1f else 0f, tween(Motion.EffectsFast), label = "squareAlpha")
    Row(
        modifier.height(48.dp).clip(CircleShape).background(container).clickable(onClick = onClick).padding(start = 8.dp, end = 14.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) {
            Box(
                Modifier.fillMaxSize().graphicsLayer {
                    scaleX = square
                    scaleY = square
                    alpha = squareAlpha
                }.background(colors.primary, RoundedCornerShape(8.dp)),
            )
            Symbol(icon, size = 20.dp, filled = selected, tint = if (selected) colors.onPrimary else colors.onSurfaceVariant)
        }
        Spacer(Modifier.width(8.dp))
        Text(
            text,
            style = MaterialTheme.typography.labelLarge,
            color = if (selected) colors.onSecondaryContainer else colors.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Аватар-«печенька» с инициалами. */
@Composable
fun Avatar(name: String, size: Dp = 40.dp) {
    val colors = MaterialTheme.colorScheme
    val initials = name.split(Regex("\\s+")).filter { it.isNotEmpty() }.take(2).joinToString("") { it.take(1).uppercase() }
    MorphShape(Shapes.Cookie9, colors.tertiaryContainer, Modifier.size(size)) {
        if (initials.isEmpty()) {
            Symbol("person", size = size * 0.55f, filled = true, tint = colors.onTertiaryContainer)
        } else {
            Text(
                initials,
                style = if (size >= 64.dp) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
                color = colors.onTertiaryContainer,
            )
        }
    }
}

/** Волнистый разделитель — как у заголовков секций боковой панели. */
@Composable
fun WavyDivider(modifier: Modifier = Modifier) {
    val color = MaterialTheme.colorScheme.outlineVariant
    val path = remember { Path() }
    Canvas(modifier.height(8.dp)) {
        val k = 2 * PI.toFloat() / 14.dp.toPx()
        val cy = size.height / 2
        path.rewind()
        var x = 0f
        path.moveTo(x, cy)
        while (x < size.width) {
            x = min(size.width, x + 1.5.dp.toPx())
            path.lineTo(x, cy + 2.dp.toPx() * sin(k * x))
        }
        drawPath(path, color, style = Stroke(1.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

/** «Мне нравится» для трека; при лайке — короткий «удар сердца». */
@Composable
fun LikeButton(track: Track?, library: Library, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    val colors = MaterialTheme.colorScheme
    val likedIds by library.likedIds.collectAsStateWithLifecycle()
    val liked = track != null && track.id in likedIds
    val scale = remember { Animatable(1f) }
    var seen by remember(track?.id) { mutableStateOf(liked) }
    LaunchedEffect(liked) {
        if (liked && !seen) {
            scale.animateTo(1.22f, tween(110, easing = EaseOutCubic))
            scale.animateTo(1f, tween(260, easing = Motion.outBack(2.5f)))
        }
        seen = liked
    }
    IconButton(onClick = { track?.let(library::toggleLike) }, enabled = track != null, modifier = modifier) {
        Symbol(
            "favorite",
            Modifier.graphicsLayer {
                scaleX = scale.value
                scaleY = scale.value
            },
            size = size,
            filled = liked,
            tint = if (liked) colors.primary else colors.onSurfaceVariant,
        )
    }
}

/**
 * Плавающие фигуры за шапкой страницы (перенос HeroShapes.qml и FloatingShapes.qml): несколько крупных
 * фигур медленно плывут и вращаются. Раскладка детерминирована от [seed] — у каждого плейлиста своя.
 * Слой лежит под списком и едет вместе с шапкой по [scroll]; сверху (под верхней панелью) и снизу
 * фигуры растворяются — видимой рамки нет. Стоит, когда шапка уехала из виду.
 * [top] — высота верхней панели над шапкой, [headerHeight] — высота самой шапки, пиксели.
 */
@Composable
fun HeroShapes(seed: String, top: Dp, headerHeight: () -> Float, scroll: () -> Float, modifier: Modifier = Modifier, opacity: Float = 0.75f) {
    val color = MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = opacity)
    val specs = remember(seed) { floatingSpecs(seed) }
    var visible by remember { mutableStateOf(true) }
    val time = rememberTime(visible)
    val path = remember { Path() }
    Spacer(
        modifier.fillMaxSize().graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }.drawBehind {
            val above = top.toPx()
            val below = 140.dp.toPx()
            val height = headerHeight()
            val offset = -scroll()
            visible = -offset < height + below
            if (!visible || height <= 0f) return@drawBehind
            val t = time()
            for (spec in specs) {
                val side = spec.size * height
                val center = Offset(
                    spec.x * size.width + spec.ax.dp.toPx() * sin(t * spec.sx * TAU + spec.phase),
                    offset + above + spec.y * height + spec.ay.dp.toPx() * cos(t * spec.sy * TAU + spec.phase),
                )
                path.outline(spec.shape.radii, spec.shape.radii, 0f, Size(side, side), spec.r0 + t * spec.rs, center = center)
                drawPath(path, color)
            }
            val total = above + height + below
            drawRect(
                Brush.verticalGradient(
                    0f to Color.Transparent,
                    above * 1.6f / total to Color.Black,
                    1f - below * 1.4f / total to Color.Black,
                    1f to Color.Transparent,
                    startY = offset,
                    endY = offset + total,
                ),
                blendMode = BlendMode.DstIn,
            )
        },
    )
}

private class FloatingSpec(
    val shape: PolarShape, val x: Float, val y: Float, val size: Float,
    val ax: Float, val ay: Float, val sx: Float, val sy: Float, val phase: Float, val r0: Float, val rs: Float,
)

private fun floatingSpecs(seed: String): List<FloatingSpec> {
    val random = Random(seed.hashCode())
    val shapes = listOf(Shapes.Cookie9, Shapes.Flower8, Shapes.Clover4, Shapes.Cookie6, Shapes.Sunny, Shapes.Flower6, Shapes.Cookie12, Shapes.Scallop)
    return List(FLOATING_COUNT) { i ->
        FloatingSpec(
            shape = shapes[random.nextInt(shapes.size)],
            // по ширине — равномерно по полосам, чтобы фигуры не слипались
            x = (i + 0.15f + 0.7f * random.nextFloat()) / FLOATING_COUNT,
            y = 0.15f + 0.7f * random.nextFloat(),
            size = 0.45f + 0.5f * random.nextFloat(), // доля высоты шапки
            ax = 18 + 30 * random.nextFloat(), ay = 10 + 22 * random.nextFloat(),
            sx = 0.05f + 0.08f * random.nextFloat(), sy = 0.04f + 0.07f * random.nextFloat(),
            phase = random.nextFloat() * TAU,
            r0 = random.nextFloat() * 360f, rs = (if (random.nextBoolean()) -1 else 1) * (3 + 5 * random.nextFloat()),
        )
    }
}

// На узком экране шести фигур десктопа слишком много — они сливаются в одно пятно
private const val FLOATING_COUNT = 4
private const val TAU = (2 * PI).toFloat()

/** Поле поиска-пилюля. [focus] — сразу поставить курсор (поле только что открыли). */
@Composable
fun SearchField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier,
    focus: Boolean = false,
    onSearch: () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    val requester = remember { androidx.compose.ui.focus.FocusRequester() }
    if (focus) LaunchedEffect(Unit) { requester.requestFocus() }
    androidx.compose.material3.TextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier.focusRequester(requester),
        placeholder = { Text(placeholder, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        leadingIcon = { Symbol("search") },
        trailingIcon = { if (value.isNotEmpty()) IconButton(onClick = { onValueChange("") }) { Symbol("close") } },
        singleLine = true,
        shape = CircleShape,
        keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Search),
        keyboardActions = androidx.compose.foundation.text.KeyboardActions(onSearch = { onSearch() }),
        colors = androidx.compose.material3.TextFieldDefaults.colors(
            focusedContainerColor = colors.surfaceContainerHigh,
            unfocusedContainerColor = colors.surfaceContainerHigh,
            focusedIndicatorColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
        ),
    )
}

/** Круглая кнопка нижней панели: поиск слева, аккаунт справа. */
@Composable
fun NavCircle(selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val container by animateColorAsState(if (selected) colors.secondaryContainer else colors.surfaceContainerHigh, tween(Motion.EffectsDefault), label = "circle")
    val scale by animateFloatAsState(if (selected) 1f else 0.92f, tween(Motion.SpatialFast, easing = Motion.outBack(2f)), label = "circleScale")
    // у выбранного — ещё и кольцо: аватар закрывает почти всю подложку
    val ring by animateColorAsState(if (selected) colors.primary else colors.primary.copy(alpha = 0f), tween(Motion.EffectsDefault), label = "ring")
    Box(
        modifier.size(48.dp).graphicsLayer {
            scaleX = scale
            scaleY = scale
        }.clip(CircleShape).background(container).border(2.dp, ring, CircleShape).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { content() }
}

/**
 * Выбор одного из вариантов одной сплошной кнопкой: все варианты лежат на общей подложке, а подсветка
 * перетекает от прежнего к новому — передний край уходит первым, задний догоняет, так что пятно
 * по дороге вытягивается. Варианты переносятся на новые строки, подсветка перетекает и между строками.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun FlowChoice(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val bounds = remember { androidx.compose.runtime.mutableStateMapOf<String, androidx.compose.ui.geometry.Rect>() }
    val left = remember { Animatable(0f) }
    val top = remember { Animatable(0f) }
    val right = remember { Animatable(0f) }
    val bottom = remember { Animatable(0f) }
    var placed by remember { mutableStateOf(false) }
    val goal = bounds[selected]
    LaunchedEffect(goal) {
        goal ?: return@LaunchedEffect
        if (!placed) {
            left.snapTo(goal.left)
            top.snapTo(goal.top)
            right.snapTo(goal.right)
            bottom.snapTo(goal.bottom)
            placed = true
            return@LaunchedEffect
        }
        val lead = androidx.compose.animation.core.spring<Float>(dampingRatio = 0.78f, stiffness = 520f)
        val trail = androidx.compose.animation.core.spring<Float>(dampingRatio = 0.86f, stiffness = 150f)
        val toRight = goal.center.x >= (left.value + right.value) / 2
        val down = goal.center.y >= (top.value + bottom.value) / 2
        kotlinx.coroutines.coroutineScope {
            launch { left.animateTo(goal.left, if (toRight) trail else lead) }
            launch { right.animateTo(goal.right, if (toRight) lead else trail) }
            launch { top.animateTo(goal.top, if (down) trail else lead) }
            launch { bottom.animateTo(goal.bottom, if (down) lead else trail) }
        }
    }
    androidx.compose.foundation.layout.FlowRow(
        modifier
            .background(colors.surfaceContainerHighest, RoundedCornerShape(24.dp))
            .padding(4.dp)
            .drawBehind {
                if (!placed) return@drawBehind
                val size = Size(right.value - left.value, bottom.value - top.value)
                drawRoundRect(
                    colors.secondaryContainer, Offset(left.value, top.value), size,
                    androidx.compose.ui.geometry.CornerRadius(minOf(size.height / 2, 20.dp.toPx())),
                )
            },
    ) {
        for ((value, title) in options) {
            val chosen = value == selected
            val tint by animateColorAsState(if (chosen) colors.onSecondaryContainer else colors.onSurfaceVariant, tween(Motion.EffectsDefault), label = "choice")
            Box(
                Modifier
                    .onGloballyPositioned { bounds[value] = it.boundsInParent() }
                    .height(40.dp)
                    .clip(CircleShape)
                    .clickable(remember { MutableInteractionSource() }, indication = null) { onSelect(value) }
                    .padding(horizontal = 14.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(title, style = MaterialTheme.typography.labelLarge, fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Medium, color = tint, maxLines = 1)
            }
        }
    }
}
