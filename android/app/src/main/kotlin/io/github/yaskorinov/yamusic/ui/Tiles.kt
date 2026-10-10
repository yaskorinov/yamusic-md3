package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.api.Artist

/** Обложка с заглушкой-значком: у части плейлистов и исполнителей картинки нет. */
@Composable
fun Cover(url: String, size: Dp, shape: Shape, placeholder: String, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Box(modifier.size(size).clip(shape).background(colors.surfaceContainerHigh), contentAlignment = Alignment.Center) {
        Symbol(placeholder, size = size * 0.42f, tint = colors.onSurfaceVariant)
        if (url.isNotEmpty()) {
            AsyncImage(model = url, contentDescription = null, modifier = Modifier.size(size), contentScale = ContentScale.Crop)
        }
    }
}

@Composable
fun SectionTitle(text: String) {
    Text(
        text,
        Modifier.padding(start = 20.dp, top = 16.dp, bottom = 8.dp),
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
fun AlbumRow(albums: List<Album>, onOpen: (Album) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = 12.dp)) {
        items(albums, key = { it.id }) { album ->
            Column(Modifier.clip(RoundedCornerShape(24.dp)).clickable { onOpen(album) }.padding(4.dp).width(ALBUM_SIDE)) {
                Cover(album.cover(300), ALBUM_SIDE, RoundedCornerShape(20.dp), "album")
                Spacer(Modifier.height(8.dp))
                Text(album.title, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOf(album.kind, album.year.takeIf { it > 0 }?.toString().orEmpty()).filter { it.isNotEmpty() }.joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
fun ArtistRow(artists: List<Artist>, onOpen: (Artist) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = 12.dp)) {
        items(artists, key = { it.id }) { artist ->
            Column(
                Modifier.clip(RoundedCornerShape(24.dp)).clickable { onOpen(artist) }.padding(4.dp).width(ARTIST_SIDE),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Cover(artist.cover(200), ARTIST_SIDE, CircleShape, "person")
                Spacer(Modifier.height(8.dp))
                Text(
                    artist.name,
                    style = MaterialTheme.typography.titleSmall,
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

private val ALBUM_SIDE = 148.dp
private val ARTIST_SIDE = 104.dp
