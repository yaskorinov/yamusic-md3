package io.github.yaskorinov.yamusic.ui

import android.content.Context
import android.content.res.AssetManager
import android.graphics.Bitmap
import android.graphics.BitmapShader
import android.graphics.RuntimeShader
import android.graphics.Shader
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.allowHardware
import coil3.size.Size as CoilSize
import coil3.toBitmap
import com.materialkolor.hct.Hct
import com.materialkolor.quantize.QuantizerCelebi
import com.materialkolor.scheme.DynamicScheme
import com.materialkolor.scheme.SchemeContent
import com.materialkolor.scheme.SchemeExpressive
import com.materialkolor.scheme.SchemeFidelity
import com.materialkolor.scheme.SchemeMonochrome
import com.materialkolor.scheme.SchemeNeutral
import com.materialkolor.scheme.SchemeTonalSpot
import com.materialkolor.scheme.SchemeVibrant
import com.materialkolor.score.Score
import io.github.yaskorinov.yamusic.data.Settings
import kotlin.random.Random
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Жёлтый Яндекса — цвет приложения по умолчанию. */
const val DEFAULT_SEED = "#FFCC00"

/** Варианты цветовой схемы — как в десктопном клиенте. */
val SCHEME_VARIANTS = listOf(
    "content" to "По обложке", "tonalSpot" to "Спокойная", "vibrant" to "Яркая", "expressive" to "Выразительная",
    "fidelity" to "Точная", "neutral" to "Нейтральная", "monochrome" to "Монохром",
)

/** Обложка играющего трека: исходная копия для фона и её «фирменный» цвет для темы. */
class CoverArt(val url: String, val pixels: Bitmap, val seed: Int)

/** Цвета приложения: схема Material и то, чего в ней нет, — признак тёмной темы и палитра для фона-градиента. */
class Scheme(val colors: ColorScheme, val dark: Boolean, val palette: IntArray)

/**
 * Применённая тема. Цвета и обложка-фон меняются одновременно: под «шторкой» — см. [YaTheme].
 * [miniCover] и [bigCover] — центры обложек мини-плеера и полноэкранного плеера: оттуда расходится «волна».
 */
@Stable
class ThemeState internal constructor(scheme: Scheme, art: CoverArt?) {
    var scheme by mutableStateOf(scheme)
        internal set
    var art by mutableStateOf(art)
        internal set
    var miniCover: Offset? = null
    var bigCover: Offset? = null
    val dark get() = scheme.dark
}

val LocalTheme = staticCompositionLocalOf<ThemeState> { error("Нет темы") }

/**
 * Тема приложения. Схема строится из цвета обложки играющего трека ([coverUrl]) или из акцентного цвета
 * настроек. Смена схемы — через снимок, как в десктопном клиенте (ThemeWipe): снимок экрана со старыми
 * цветами кладётся поверх, тема переключается мгновенно под ним, и снимок уходит — шторкой в сторону
 * [direction], волной от обложки, растворением или перетеканием. Все цвета пересчитываются один раз,
 * а не каждый кадр, и фон из обложки меняется в тот же момент.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun YaTheme(coverUrl: String, dark: Boolean, settings: Settings, direction: Int, content: @Composable () -> Unit) {
    val context = LocalContext.current.applicationContext
    val typography = remember(context) { Typography().withFamily(googleSans(context.assets)) }
    val accentFromCover by settings.accentFromCover.flow.collectAsStateWithLifecycle()
    val customSeed by settings.customSeed.flow.collectAsStateWithLifecycle()
    val variant by settings.schemeVariant.flow.collectAsStateWithLifecycle()
    val transition by settings.trackTransition.flow.collectAsStateWithLifecycle()

    val art by produceState(lastArt?.takeIf { it.url == coverUrl }, coverUrl) {
        value = if (coverUrl.isEmpty()) null else (loadCoverArt(context, coverUrl) ?: value)
        lastArt = value
    }
    val seed = art?.seed?.takeIf { accentFromCover } ?: parseSeed(customSeed)
    val scheme = remember(seed, dark, variant) { schemeFor(seed, dark, variant) }
    val state = remember { ThemeState(scheme, art) }

    val layer = rememberGraphicsLayer()
    var snapshot by remember { mutableStateOf<ImageBitmap?>(null) }
    var wipe by remember { mutableStateOf(Wipe()) }
    var progress by remember { mutableFloatStateOf(1f) }
    LaunchedEffect(scheme, art) {
        if (state.scheme === scheme && state.art === art) return@LaunchedEffect
        // Сначала снимок со старыми цветами, затем — в одном кадре — новая тема под ним
        val shot = if (layer.size.width > 0) runCatching { layer.toImageBitmap() }.getOrNull() else null
        val origin = (state.bigCover ?: state.miniCover)?.let { Offset(it.x / layer.size.width, it.y / layer.size.height) }
        wipe = Wipe(WIPE_MODES.indexOf(transition).coerceAtLeast(0), direction, origin ?: Offset(0.5f, 0.5f), Random.nextFloat() * 50f)
        snapshot = shot
        progress = 0f
        state.scheme = scheme
        state.art = art
        if (shot == null) return@LaunchedEffect
        try {
            animate(0f, 1f, animationSpec = tween(wipe.duration, easing = wipe.easing)) { value, _ -> progress = value }
        } finally {
            if (snapshot === shot) snapshot = null
        }
    }

    MaterialExpressiveTheme(colorScheme = state.scheme.colors, motionScheme = MotionScheme.expressive(), typography = typography) {
        CompositionLocalProvider(LocalTheme provides state) {
            Box(
                Modifier.fillMaxSize().drawWithContent {
                    layer.record { this@drawWithContent.drawContent() }
                    drawLayer(layer)
                    snapshot?.let { wipe.draw(this, it, progress) }
                },
            ) { content() }
        }
    }
}

/** Последняя разобранная обложка: пересоздание окна (поворот экрана) не начинает с цвета по умолчанию. */
private var lastArt: CoverArt? = null

private val WIPE_MODES = listOf("wipe", "ripple", "dissolve", "liquid")

/** Уход снимка со старой темой: способ [mode] (номер в [WIPE_MODES]), направление, центр волны, зерно шума. */
private class Wipe(val mode: Int = 0, val direction: Int = 1, val origin: Offset = Offset(0.5f, 0.5f), val seed: Float = 0f) {
    val duration = intArrayOf(650, 900, 1000, 1200)[mode]
    // шторка и волна — быстро стартуют и долго тормозят; таяние — ровно
    val easing: Easing = if (mode >= 2) Easing { (1f - kotlin.math.cos(Math.PI.toFloat() * it)) / 2f } else Motion.Emphasized

    fun draw(scope: DrawScope, shot: ImageBitmap, progress: Float) = with(scope) {
        val shader = wipeShader
        shader.setInputShader("source", BitmapShader(shot.asAndroidBitmap(), Shader.TileMode.CLAMP, Shader.TileMode.CLAMP))
        shader.setFloatUniform("size", size.width, size.height)
        shader.setFloatUniform("progress", progress)
        shader.setFloatUniform("dir", if (direction < 0) -1f else 1f)
        shader.setFloatUniform("mode", mode.toFloat())
        shader.setFloatUniform("seed", seed)
        shader.setFloatUniform("origin", origin.x, origin.y)
        drawRect(ShaderBrush(shader))
    }
}

private val wipeShader by lazy { RuntimeShader(WIPE_AGSL) }

/**
 * Перенос qml/Md3/shaders/wipe.frag. Четыре способа (mode):
 *   0 — шторка: мягкий, чуть наклонный край; dir = +1 — слева направо, -1 — справа налево;
 *   1 — волна: новое расходится от точки origin фигурой с волнистым краем (край вращается по dir);
 *   2 — растворение: снимок равномерно тает;
 *   3 — перетекание: снимок тает по плавному шуму и при этом «плывёт» — волна шума идёт по dir.
 */
private const val WIPE_AGSL = """
uniform shader source;
uniform float2 size;
uniform float progress;
uniform float dir;
uniform float mode;
uniform float seed;
uniform float2 origin;

float hash(float2 p) {
    float3 q = fract(float3(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

float noise(float2 p) {
    float2 i = floor(p);
    float2 f = fract(p);
    float2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(mix(hash(i), hash(i + float2(1.0, 0.0)), u.x),
               mix(hash(i + float2(0.0, 1.0)), hash(i + float2(1.0, 1.0)), u.x), u.y);
}

float fbm(float2 p) {
    return (noise(p) + 0.5 * noise(p * 2.03 + 11.7) + 0.25 * noise(p * 4.01 + 5.3)) / 1.75;
}

half4 main(float2 xy) {
    float2 uv = xy / size;
    float aspect = size.x / size.y;
    float2 at = uv;
    float a = 1.0;
    if (mode < 0.5) {
        float edge = 0.22;
        float skew = 0.12;
        float x = dir > 0.0 ? uv.x : 1.0 - uv.x;
        float t = x + skew * (uv.y - 0.5);
        float front = progress * (1.0 + edge + skew) - edge - skew * 0.5;
        a = smoothstep(front, front + edge, t);
    } else if (mode < 1.5) {
        float2 k = float2(aspect, 1.0);
        float2 d = (uv - origin) * k;
        float reach = length(max(origin, 1.0 - origin) * k);
        float lobes = 1.0 + 0.07 * cos(9.0 * atan(d.y, d.x) + dir * (seed + progress * 2.6));
        float soft = 0.05;
        float front = progress * (reach * 1.08 + soft);
        a = smoothstep(front - soft, front, length(d) / lobes);
    } else if (mode < 2.5) {
        a = 1.0 - progress;
    } else {
        float2 q = uv * float2(aspect, 1.0) + seed;
        float x = dir > 0.0 ? uv.x : 1.0 - uv.x;
        float v = mix(fbm(q * 2.4), x, 0.4);
        float soft = 0.22;
        float front = progress * (1.0 + soft) - soft;
        a = smoothstep(front, front + soft, v);
        at += (float2(noise(q * 3.1 + 7.3), noise(q * 3.1 + 19.1)) - 0.5) * 0.05 * progress;
    }
    return source.eval(at * size) * a;
}
"""

private fun parseSeed(text: String): Int =
    runCatching { android.graphics.Color.parseColor(text) }.getOrDefault(android.graphics.Color.parseColor(DEFAULT_SEED))

/** Обложка и её доминантный цвет по алгоритму Material (Celebi + Score) — как в десктопном клиенте. */
private suspend fun loadCoverArt(context: Context, url: String): CoverArt? = withContext(Dispatchers.Default) {
    val request = ImageRequest.Builder(context).data(url).size(CoilSize.ORIGINAL).allowHardware(false).build()
    val bitmap = SingletonImageLoader.get(context).execute(request).image?.toBitmap() ?: return@withContext null
    val small = bitmap.boxScaled(SEED_SIDE)
    val pixels = IntArray(small.width * small.height)
    small.getPixels(pixels, 0, small.width, 0, 0, small.width, small.height)
    val seed = Score.score(QuantizerCelebi.quantize(pixels, 128)).firstOrNull() ?: return@withContext null
    CoverArt(url, bitmap, seed)
}

private fun schemeFor(seed: Int, dark: Boolean, variant: String): Scheme {
    val hct = Hct.fromInt(seed)
    val s: DynamicScheme = when (variant) {
        "tonalSpot" -> SchemeTonalSpot(hct, dark, 0.0)
        "vibrant" -> SchemeVibrant(hct, dark, 0.0)
        "expressive" -> SchemeExpressive(hct, dark, 0.0)
        "fidelity" -> SchemeFidelity(hct, dark, 0.0)
        "neutral" -> SchemeNeutral(hct, dark, 0.0)
        "monochrome" -> SchemeMonochrome(hct, dark, 0.0)
        else -> SchemeContent(hct, dark, 0.0)
    }
    // Сетка 4×3 для фона-градиента — те же роли и порядок, что в CoverBackdrop.qml
    val palette = intArrayOf(
        s.primaryPaletteKeyColor, s.secondaryContainer, s.tertiaryContainer, s.tertiaryPaletteKeyColor,
        s.primaryContainer, s.primaryPaletteKeyColor, s.secondaryPaletteKeyColor, s.tertiaryContainer,
        s.tertiaryPaletteKeyColor, s.primaryContainer, s.primaryPaletteKeyColor, s.secondaryContainer,
    )
    return Scheme(s.toColorScheme(), dark, palette)
}

private fun DynamicScheme.toColorScheme(): ColorScheme {
    val s = this
    // Заданы все роли, так что исходная схема (светлая или тёмная) значения не имеет
    return darkColorScheme(
        primary = Color(s.primary),
        onPrimary = Color(s.onPrimary),
        primaryContainer = Color(s.primaryContainer),
        onPrimaryContainer = Color(s.onPrimaryContainer),
        inversePrimary = Color(s.inversePrimary),
        secondary = Color(s.secondary),
        onSecondary = Color(s.onSecondary),
        secondaryContainer = Color(s.secondaryContainer),
        onSecondaryContainer = Color(s.onSecondaryContainer),
        tertiary = Color(s.tertiary),
        onTertiary = Color(s.onTertiary),
        tertiaryContainer = Color(s.tertiaryContainer),
        onTertiaryContainer = Color(s.onTertiaryContainer),
        background = Color(s.background),
        onBackground = Color(s.onBackground),
        surface = Color(s.surface),
        onSurface = Color(s.onSurface),
        surfaceVariant = Color(s.surfaceVariant),
        onSurfaceVariant = Color(s.onSurfaceVariant),
        surfaceTint = Color(s.surfaceTint),
        inverseSurface = Color(s.inverseSurface),
        inverseOnSurface = Color(s.inverseOnSurface),
        error = Color(s.error),
        onError = Color(s.onError),
        errorContainer = Color(s.errorContainer),
        onErrorContainer = Color(s.onErrorContainer),
        outline = Color(s.outline),
        outlineVariant = Color(s.outlineVariant),
        scrim = Color(s.scrim),
        surfaceBright = Color(s.surfaceBright),
        surfaceContainer = Color(s.surfaceContainer),
        surfaceContainerHigh = Color(s.surfaceContainerHigh),
        surfaceContainerHighest = Color(s.surfaceContainerHighest),
        surfaceContainerLow = Color(s.surfaceContainerLow),
        surfaceContainerLowest = Color(s.surfaceContainerLowest),
        surfaceDim = Color(s.surfaceDim),
        primaryFixed = Color(s.primaryFixed),
        primaryFixedDim = Color(s.primaryFixedDim),
        onPrimaryFixed = Color(s.onPrimaryFixed),
        onPrimaryFixedVariant = Color(s.onPrimaryFixedVariant),
        secondaryFixed = Color(s.secondaryFixed),
        secondaryFixedDim = Color(s.secondaryFixedDim),
        onSecondaryFixed = Color(s.onSecondaryFixed),
        onSecondaryFixedVariant = Color(s.onSecondaryFixedVariant),
        tertiaryFixed = Color(s.tertiaryFixed),
        tertiaryFixedDim = Color(s.tertiaryFixedDim),
        onTertiaryFixed = Color(s.onTertiaryFixed),
        onTertiaryFixedVariant = Color(s.onTertiaryFixedVariant),
    )
}

/** Вариативный Google Sans из набора десктопного клиента: wght 400..700, opsz 18. */
@OptIn(ExperimentalTextApi::class)
private fun googleSans(assets: AssetManager): FontFamily = FontFamily(
    listOf(400, 500, 600, 700).map { weight ->
        Font(
            path = "GoogleSans.ttf",
            assetManager = assets,
            weight = FontWeight(weight),
            style = FontStyle.Normal,
            variationSettings = FontVariation.Settings(FontVariation.weight(weight), FontVariation.Setting("opsz", 18f)),
        )
    },
)

private fun Typography.withFamily(family: FontFamily): Typography = copy(
    displayLarge = displayLarge.copy(fontFamily = family),
    displayMedium = displayMedium.copy(fontFamily = family),
    displaySmall = displaySmall.copy(fontFamily = family),
    headlineLarge = headlineLarge.copy(fontFamily = family),
    headlineMedium = headlineMedium.copy(fontFamily = family),
    headlineSmall = headlineSmall.copy(fontFamily = family),
    titleLarge = titleLarge.copy(fontFamily = family),
    titleMedium = titleMedium.copy(fontFamily = family),
    titleSmall = titleSmall.copy(fontFamily = family),
    bodyLarge = bodyLarge.copy(fontFamily = family),
    bodyMedium = bodyMedium.copy(fontFamily = family),
    bodySmall = bodySmall.copy(fontFamily = family),
    labelLarge = labelLarge.copy(fontFamily = family),
    labelMedium = labelMedium.copy(fontFamily = family),
    labelSmall = labelSmall.copy(fontFamily = family),
)

private const val SEED_SIDE = 64
