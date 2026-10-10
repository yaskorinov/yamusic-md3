package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.api.Artist
import io.github.yaskorinov.yamusic.data.Catalog
import io.github.yaskorinov.yamusic.data.SearchState
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

/** Поиск по мере набора: исполнители и альбомы — лентами, треки — списком. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SearchScreen(
    catalog: Catalog,
    player: PlayerConnection,
    playerState: PlayerState,
    contentPadding: PaddingValues,
    onOpenAlbum: (Album) -> Unit,
    onOpenArtist: (Artist) -> Unit,
) {
    val query by catalog.query.collectAsStateWithLifecycle()
    val state by catalog.search.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val focus = LocalFocusManager.current

    Column(Modifier.fillMaxSize().padding(contentPadding)) {
        TextField(
            value = query,
            onValueChange = catalog::search,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
            placeholder = { Text("Трек, альбом, исполнитель") },
            leadingIcon = { Symbol("search") },
            trailingIcon = {
                if (query.isNotEmpty()) IconButton(onClick = { catalog.search("") }) { Symbol("close") }
            },
            singleLine = true,
            shape = CircleShape,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = colors.surfaceContainerHigh,
                unfocusedContainerColor = colors.surfaceContainerHigh,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent,
            ),
        )
        when (val search = state) {
            is SearchState.Found -> {
                val result = search.result
                LazyColumn(Modifier.fillMaxSize()) {
                    if (result.artists.isNotEmpty()) {
                        item(key = "artists") {
                            Column {
                                SectionTitle("Исполнители")
                                ArtistRow(result.artists, onOpenArtist)
                            }
                        }
                    }
                    if (result.albums.isNotEmpty()) {
                        item(key = "albums") {
                            Column {
                                SectionTitle("Альбомы")
                                AlbumRow(result.albums, onOpenAlbum)
                            }
                        }
                    }
                    if (result.tracks.isNotEmpty()) {
                        item(key = "tracks") { SectionTitle("Треки") }
                        // Один трек может прийти дважды (разные издания) — ключ с номером строки
                        itemsIndexed(result.tracks, key = { index, track -> "$index:${track.id}" }) { index, track ->
                            TrackRow(
                                track = track,
                                current = track.id == playerState.track?.id,
                                onClick = {
                                    focus.clearFocus()
                                    player.play(result.tracks, index, PlayContext.Search)
                                },
                            )
                        }
                    }
                }
            }
            SearchState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                LoadingIndicator(Modifier.size(56.dp))
            }
            else -> Hint(
                icon = if (search is SearchState.Idle) "search" else "search_off",
                text = when (search) {
                    is SearchState.Failed -> search.error
                    SearchState.Empty -> "По запросу «${query.trim()}» ничего не нашлось"
                    else -> "Найдём трек, альбом или исполнителя"
                },
                error = search is SearchState.Failed,
            )
        }
    }
}

@Composable
private fun Hint(icon: String, text: String, error: Boolean) {
    val colors = MaterialTheme.colorScheme
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
    ) {
        Symbol(icon, size = 56.dp, tint = colors.onSurfaceVariant)
        Text(
            text,
            Modifier.padding(top = 12.dp),
            style = MaterialTheme.typography.bodyLarge,
            color = if (error) colors.error else colors.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}
