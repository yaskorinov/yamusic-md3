package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import coil3.size.Size as CoilSize
import kotlin.math.roundToInt

/**
 * Состояние морфа: текущая форма — смесь [from] и [to]. Новый морф начинается с того, что видно сейчас,
 * даже если предыдущий ещё не закончился.
 */
@Stable
class MorphState(initial: PolarShape) {
    private var from by mutableStateOf(initial.radii)
    private var to by mutableStateOf(initial.radii)
    private val progress = Animatable(1f)

    suspend fun morphTo(next: PolarShape, duration: Int = Motion.SpatialDefault, overshoot: Float = 1.4f) {
        if (next.radii === to) return
        val t = progress.value
        from = if (t >= 1f) to else FloatArray(SAMPLES) { from[it] + (to[it] - from[it]) * t }
        to = next.radii
        progress.snapTo(0f)
        progress.animateTo(1f, tween(duration, easing = Motion.outBack(overshoot)))
    }

    /** Сменить фигуру сразу, без морфа. */
    suspend fun jumpTo(next: PolarShape) {
        from = next.radii
        to = next.radii
        progress.snapTo(1f)
    }

    fun outline(path: Path, size: Size, rotation: Float = 0f, pulse: Float = 0f) =
        path.outline(from, to, progress.value, size, rotation, pulse)
}

@Composable
fun rememberMorph(shape: PolarShape, duration: Int = Motion.SpatialDefault, overshoot: Float = 1.4f): MorphState {
    val state = remember { MorphState(shape) }
    LaunchedEffect(shape) { state.morphTo(shape, duration, overshoot) }
    return state
}

/**
 * Фигура MD3 Expressive: при смене [shape] плавно перетекает в новую. [rotation] (градусы) и [pulse]
 * читаются на этапе рисования — движение не вызывает перекомпоновку. Содержимое (значок) — по центру.
 */
@Composable
fun MorphShape(
    shape: PolarShape,
    color: Color,
    modifier: Modifier = Modifier,
    duration: Int = Motion.SpatialDefault,
    overshoot: Float = 1.4f,
    rotation: () -> Float = { 0f },
    content: @Composable BoxScope.() -> Unit = {},
) {
    val morph = rememberMorph(shape, duration, overshoot)
    val path = remember { Path() }
    Box(
        modifier.drawBehind {
            morph.outline(path, size, rotation())
            drawPath(path, color)
        },
        contentAlignment = Alignment.Center,
        content = content,
    )
}

/**
 * Картинка (обложка), обрезанная по фигуре с морфингом. Вращается только рамка ([rotation]),
 * картинка внутри стоит; [pulse] — «дыхание» рамки. Без картинки — значок [placeholder] на подложке.
 */
@Composable
fun MorphImage(
    url: String,
    shape: PolarShape,
    modifier: Modifier = Modifier,
    placeholder: String = "music_note",
    placeholderSize: Dp = 24.dp,
    duration: Int = Motion.SpatialDefault,
    morph: MorphState = rememberMorph(shape, duration),
    rotation: () -> Float = { 0f },
    pulse: () -> Float = { 0f },
) {
    val colors = MaterialTheme.colorScheme
    val path = remember { Path() }
    Box(
        modifier
            .drawWithContent {
                morph.outline(path, size, rotation(), pulse())
                clipPath(path) { this@drawWithContent.drawContent() }
            }
            .background(colors.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) {
        if (url.isEmpty()) {
            Symbol(placeholder, size = placeholderSize, tint = colors.onSurfaceVariant)
        } else {
            AsyncImage(coverRequest(url), contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        }
    }
}

/**
 * Запрос обложки в исходном размере (он уже задан в адресе): у показа и у предзагрузки один и тот же
 * ключ кэша, поэтому заранее загруженная картинка появляется сразу, в том же кадре.
 */
@Composable
fun coverRequest(url: String): ImageRequest {
    val context = LocalContext.current
    return remember(url) { ImageRequest.Builder(context).data(url).size(CoilSize.ORIGINAL).build() }
}

/**
 * Медленное вращение фигуры-рамки, пока [spinning]. Остановившись, рамка доворачивается до ближайшей
 * четверти оборота — квадрат не должен встать криво.
 */
@Composable
fun rememberSpin(spinning: Boolean, speed: Float = 8f): () -> Float {
    val angle = remember { Animatable(0f) }
    LaunchedEffect(spinning, speed) {
        if (spinning) {
            var last = withFrameNanos { it }
            while (true) {
                val now = withFrameNanos { it }
                angle.snapTo((angle.value + (now - last) / 1e9f * speed) % 360f)
                last = now
            }
        } else {
            val rest = (angle.value / 90f).roundToInt() * 90f
            if (rest != angle.value) angle.animateTo(rest, tween(Motion.SpatialSlow, easing = Motion.EmphasizedDecelerate))
        }
    }
    return remember { { angle.value } }
}

/** Время в секундах, которое идёт, пока [running]: общий счётчик для плавающих и вращающихся фигур. */
@Composable
fun rememberTime(running: Boolean): () -> Float {
    val time = remember { androidx.compose.runtime.mutableFloatStateOf(0f) }
    LaunchedEffect(running) {
        if (!running) return@LaunchedEffect
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            time.floatValue += (now - last) / 1e9f
            last = now
        }
    }
    return remember { { time.floatValue } }
}
