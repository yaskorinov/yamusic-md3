package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.sin

/**
 * Фон из обложки. Размытие считается не на весь экран, а на крошечной копии обложки ([SIDE]) —
 * её слой затем растягивается видеокартой, так что цена не зависит от размера экрана.
 * Поверх — вуаль цвета темы, чтобы текст оставался читаемым. [animate] — фон медленно «плавает».
 */
@Composable
fun CoverBackdrop(url: String, animate: Boolean, modifier: Modifier = Modifier) {
    val veil = MaterialTheme.colorScheme.surface
    Box(modifier.clipToBounds().background(veil)) {
        // Новая обложка проявляется поверх старой — без провала в цвет темы
        Crossfade(url, animationSpec = tween(700), label = "backdrop") { shown ->
            if (shown.isNotEmpty()) BlurredCover(shown, animate)
        }
        Box(Modifier.fillMaxSize().background(veil.copy(alpha = VEIL)))
    }
}

@Composable
private fun BlurredCover(url: String, animate: Boolean) {
    var time by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(animate) {
        if (!animate) return@LaunchedEffect
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            time += (now - last) / 1e9f
            last = now
        }
    }
    val context = LocalContext.current
    val blur = with(LocalDensity.current) { (SIDE * 0.16f).toPx() }
    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        // С запасом: при повороте и смещении края копии не должны показаться
        val zoom = max(maxWidth / SIDE, maxHeight / SIDE) * 1.5f
        AsyncImage(
            model = remember(url) { ImageRequest.Builder(context).data(url).size(96).build() },
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = Modifier.requiredSize(SIDE).graphicsLayer {
                scaleX = zoom
                scaleY = zoom
                // время читается здесь, на этапе рисования: движение не вызывает перекомпоновку
                rotationZ = sin(time * 0.11f) * 14f
                translationX = cos(time * 0.07f) * size.width * 1.2f
                translationY = sin(time * 0.05f) * size.height * 1.2f
                renderEffect = BlurEffect(blur, blur, TileMode.Clamp)
            },
        )
    }
}

private val SIDE = 64.dp
private const val VEIL = 0.5f
