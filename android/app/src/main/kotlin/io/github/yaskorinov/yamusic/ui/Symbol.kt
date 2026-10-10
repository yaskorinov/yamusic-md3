package io.github.yaskorinov.yamusic.ui

import android.content.res.AssetManager
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.Typeface
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.serialization.json.Json

/**
 * Значок Material Symbols Rounded по имени ('play_arrow', 'favorite'…). Шрифт и таблица имён —
 * те же урезанные файлы, что у десктопного клиента: новые значки добавляются там, в tools/subset_fonts.py.
 * Как и на десктопе (Icon.qml), значок центрируется по фактическому контуру глифа, а не по его ячейке
 * шрифта: часть глифов (favorite, queue_music…) нарисована выше центра ячейки и в подложке смотрится смещённой.
 */
@Composable
fun Symbol(
    name: String,
    modifier: Modifier = Modifier,
    size: Dp = 24.dp,
    filled: Boolean = false,
    tint: Color = LocalContentColor.current,
) {
    val assets = LocalContext.current.assets
    val glyph = remember(name) { Symbols.glyph(assets, name) }
    val ink = remember(name, filled) { Symbols.inkShift(assets, glyph, filled) }
    val style = TextStyle(
        color = tint,
        fontFamily = Symbols.family(assets, filled),
        fontSize = with(LocalDensity.current) { size.toSp() },
        platformStyle = PlatformTextStyle(includeFontPadding = false),
    )
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        // Строка шрифта выше самого значка (1,2 кегля): без unbounded её прижимало к верху, и значок съезжал вниз
        BasicText(
            glyph,
            Modifier.wrapContentSize(unbounded = true).offset { IntOffset((ink.x * size.toPx()).roundToInt(), (ink.y * size.toPx()).roundToInt()) },
            style = style,
            maxLines = 1,
            softWrap = false,
        )
    }
}

private object Symbols {
    private var codes: Map<String, Int>? = null
    private val shifts = HashMap<String, Offset>()
    private val typefaces = arrayOfNulls<Typeface>(2)

    /** На сколько (в долях размера) сдвинуть глиф, чтобы центр его видимого контура встал в центр значка. */
    fun inkShift(assets: AssetManager, glyph: String, filled: Boolean): Offset {
        if (glyph.isEmpty()) return Offset.Zero
        return shifts.getOrPut("$glyph$filled") {
            val index = if (filled) 1 else 0
            val typeface = typefaces[index] ?: Typeface.Builder(assets, "MaterialSymbolsRounded.ttf")
                .setFontVariationSettings("'FILL' $index, 'wght' 400, 'GRAD' 0, 'opsz' 24")
                .build().also { typefaces[index] = it }
            val paint = Paint().apply {
                this.typeface = typeface
                textSize = MEASURE
            }
            val bounds = Rect()
            paint.getTextBounds(glyph, 0, glyph.length, bounds)
            if (bounds.isEmpty) return@getOrPut Offset.Zero
            // ячейка значка — квадрат над базовой линией: её центр на полкегля выше базовой линии
            val dx = (paint.measureText(glyph) / 2 - bounds.exactCenterX()) / MEASURE
            val dy = (-MEASURE / 2 - bounds.exactCenterY()) / MEASURE
            Offset(dx.coerceIn(-0.15f, 0.15f), dy.coerceIn(-0.15f, 0.15f))
        }
    }

    private const val MEASURE = 240f
    private val families = arrayOfNulls<FontFamily>(2)

    fun glyph(assets: AssetManager, name: String): String {
        val table = codes ?: assets.open("icons.json").use {
            Json.decodeFromString<Map<String, Int>>(it.readBytes().decodeToString())
        }.also { codes = it }
        return table[name]?.let { String(Character.toChars(it)) }.orEmpty()
    }

    @OptIn(ExperimentalTextApi::class)
    fun family(assets: AssetManager, filled: Boolean): FontFamily {
        val index = if (filled) 1 else 0
        return families[index] ?: FontFamily(
            Font(
                path = "MaterialSymbolsRounded.ttf",
                assetManager = assets,
                variationSettings = FontVariation.Settings(
                    FontVariation.Setting("FILL", if (filled) 1f else 0f),
                    FontVariation.Setting("wght", 400f),
                    FontVariation.Setting("GRAD", 0f),
                    FontVariation.Setting("opsz", 24f),
                ),
            ),
        ).also { families[index] = it }
    }
}
