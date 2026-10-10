package io.github.yaskorinov.yamusic.ui

import android.content.res.AssetManager
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
import androidx.compose.ui.unit.dp
import kotlinx.serialization.json.Json

/**
 * Значок Material Symbols Rounded по имени ('play_arrow', 'favorite'…). Шрифт и таблица имён —
 * те же урезанные файлы, что у десктопного клиента: новые значки добавляются там, в tools/subset_fonts.py.
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
    val style = TextStyle(
        color = tint,
        fontFamily = Symbols.family(assets, filled),
        fontSize = with(LocalDensity.current) { size.toSp() },
        platformStyle = PlatformTextStyle(includeFontPadding = false),
    )
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        BasicText(glyph, style = style, maxLines = 1, softWrap = false)
    }
}

private object Symbols {
    private var codes: Map<String, Int>? = null
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
