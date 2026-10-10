package io.github.yaskorinov.yamusic.ui

import android.content.Context
import android.content.res.AssetManager
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.allowHardware
import coil3.toBitmap
import com.materialkolor.hct.Hct
import com.materialkolor.quantize.QuantizerCelebi
import com.materialkolor.scheme.SchemeContent
import com.materialkolor.score.Score
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Жёлтый Яндекса — цвет приложения, пока ничего не играет. */
val DefaultSeed = Color(0xFFFFCC00)

/**
 * Тема приложения: схема Material строится из цвета [seed] (доминантный цвет обложки играющего трека)
 * и плавно перетекает в новую при его смене.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun YaTheme(seed: Color, dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val assets = LocalContext.current.assets
    val typography = remember(assets) { Typography().withFamily(googleSans(assets)) }
    val scheme = remember(seed, dark) { schemeFor(seed.toArgb(), dark) }
    MaterialExpressiveTheme(
        colorScheme = scheme.animated(),
        motionScheme = MotionScheme.expressive(),
        typography = typography,
        content = content,
    )
}

/** Цвет темы для обложки по адресу [url]; пока обложка не разобрана — прежний цвет. */
@Composable
fun rememberCoverSeed(url: String): Color {
    val context = LocalContext.current.applicationContext
    var last by rememberSaveable { mutableStateOf(DefaultSeed.toArgb()) }
    val seed by produceState(Color(last), url) {
        val found = if (url.isEmpty()) DefaultSeed else coverSeed(context, url)
        if (found != null) {
            value = found
            last = found.toArgb()
        }
    }
    return seed
}

/** Доминантный «фирменный» цвет картинки по алгоритму Material (Celebi + Score) — как в десктопном клиенте. */
private suspend fun coverSeed(context: Context, url: String): Color? = withContext(Dispatchers.Default) {
    val request = ImageRequest.Builder(context).data(url).size(SEED_SIDE).allowHardware(false).build()
    val bitmap = SingletonImageLoader.get(context).execute(request).image?.toBitmap() ?: return@withContext null
    val pixels = IntArray(bitmap.width * bitmap.height)
    bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
    Score.score(QuantizerCelebi.quantize(pixels, 128)).firstOrNull()?.let { Color(it) }
}

private fun schemeFor(seed: Int, dark: Boolean): ColorScheme {
    val s = SchemeContent(Hct.fromInt(seed), dark, 0.0)
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

/** Та же схема, но каждый цвет плавно догоняет новое значение. */
@Composable
private fun ColorScheme.animated(): ColorScheme {
    @Composable
    fun Color.a(): Color = animateColorAsState(this, tween(COLOR_MS), label = "scheme").value
    return copy(
        primary = primary.a(),
        onPrimary = onPrimary.a(),
        primaryContainer = primaryContainer.a(),
        onPrimaryContainer = onPrimaryContainer.a(),
        inversePrimary = inversePrimary.a(),
        secondary = secondary.a(),
        onSecondary = onSecondary.a(),
        secondaryContainer = secondaryContainer.a(),
        onSecondaryContainer = onSecondaryContainer.a(),
        tertiary = tertiary.a(),
        onTertiary = onTertiary.a(),
        tertiaryContainer = tertiaryContainer.a(),
        onTertiaryContainer = onTertiaryContainer.a(),
        background = background.a(),
        onBackground = onBackground.a(),
        surface = surface.a(),
        onSurface = onSurface.a(),
        surfaceVariant = surfaceVariant.a(),
        onSurfaceVariant = onSurfaceVariant.a(),
        surfaceTint = surfaceTint.a(),
        inverseSurface = inverseSurface.a(),
        inverseOnSurface = inverseOnSurface.a(),
        outline = outline.a(),
        outlineVariant = outlineVariant.a(),
        surfaceBright = surfaceBright.a(),
        surfaceContainer = surfaceContainer.a(),
        surfaceContainerHigh = surfaceContainerHigh.a(),
        surfaceContainerHighest = surfaceContainerHighest.a(),
        surfaceContainerLow = surfaceContainerLow.a(),
        surfaceContainerLowest = surfaceContainerLowest.a(),
        surfaceDim = surfaceDim.a(),
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
private const val COLOR_MS = 600
