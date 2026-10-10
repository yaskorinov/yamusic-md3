package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.api.Artist

/**
 * Плитка альбома или исполнителя: обложка в фигуре MD3E, название, подпись. Под пальцем фигура морфится
 * (на десктопе — при наведении), у исполнителей — круг.
 */
@Composable
fun MediaTile(
    cover: String,
    title: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    subtitle: String = "",
    round: Boolean = false,
    placeholder: String = "album",
) {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val align = if (round) Alignment.CenterHorizontally else Alignment.Start
    Column(modifier.clickable(source, indication = null, onClick = onClick), horizontalAlignment = align) {
        MorphImage(
            cover,
            when {
                pressed -> if (round) Shapes.Cookie12 else Shapes.Cookie9
                else -> if (round) Shapes.Circle else Shapes.SoftSquare
            },
            Modifier.fillMaxWidth().aspectRatio(1f),
            placeholder = placeholder,
            placeholderSize = 48.dp,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            title,
            style = MaterialTheme.typography.titleSmall,
            textAlign = if (round) TextAlign.Center else TextAlign.Start,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (subtitle.isNotEmpty()) {
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** Ряд плиток сетки: одинаковая ширина, недостающие места остаются пустыми. */
@Composable
fun <T> TileRow(items: List<T>, columns: Int = TILE_COLUMNS, tile: @Composable (T, Modifier) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        for (item in items) tile(item, Modifier.weight(1f))
        repeat(columns - items.size) { Spacer(Modifier.weight(1f).width(0.dp)) }
    }
}

@Composable
fun AlbumTile(album: Album, modifier: Modifier, artist: Boolean = false, onOpen: (Album) -> Unit) {
    val year = album.year.takeIf { it > 0 }?.toString().orEmpty()
    val subtitle = if (artist) listOf(album.artists, year) else listOf(year, album.kind)
    MediaTile(album.cover(400), album.title, { onOpen(album) }, modifier, subtitle = subtitle.filter { it.isNotEmpty() }.joinToString(" · "))
}

@Composable
fun ArtistTile(artist: Artist, modifier: Modifier, onOpen: (Artist) -> Unit) {
    MediaTile(artist.cover(400), artist.name, { onOpen(artist) }, modifier, round = true, placeholder = "person")
}

const val TILE_COLUMNS = 2
