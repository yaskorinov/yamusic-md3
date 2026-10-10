package io.github.yaskorinov.yamusic.data

import io.github.yaskorinov.yamusic.api.Album
import io.github.yaskorinov.yamusic.api.Artist
import io.github.yaskorinov.yamusic.api.SearchResult
import io.github.yaskorinov.yamusic.api.YandexApi
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface SearchState {
    data object Idle : SearchState
    data object Loading : SearchState
    data object Empty : SearchState
    data class Failed(val error: String) : SearchState
    data class Found(val result: SearchResult) : SearchState
}

class AlbumData {
    val info = MutableStateFlow<Album?>(null)
    val list = TrackList()
}

class ArtistData {
    val info = MutableStateFlow<Artist?>(null)
    val albums = MutableStateFlow<List<Album>>(emptyList())
    val popular = TrackList()
}

/** Каталог: поиск, страницы альбомов и исполнителей. */
class Catalog(private val scope: CoroutineScope, private val api: YandexApi) {
    private val _query = MutableStateFlow("")
    val query = _query.asStateFlow()

    private val _search = MutableStateFlow<SearchState>(SearchState.Idle)
    val search = _search.asStateFlow()

    private val albums = HashMap<String, AlbumData>()
    private val artists = HashMap<String, ArtistData>()
    private var job: Job? = null

    /** Искать по мере набора: запрос уходит, когда пользователь на миг остановился. */
    fun search(text: String) {
        _query.value = text
        job?.cancel()
        val query = text.trim()
        if (query.isEmpty()) {
            _search.value = SearchState.Idle
            return
        }
        job = scope.launch {
            delay(TYPING_PAUSE_MS)
            _search.value = SearchState.Loading
            _search.value = try {
                val result = api.search(query)
                if (result.empty) SearchState.Empty else SearchState.Found(result)
            } catch (e: IOException) {
                SearchState.Failed("Поиск не удался: ${e.message}")
            }
        }
    }

    fun album(id: String, reload: Boolean = false): AlbumData {
        val known = albums[id]
        if (known != null && !reload) return known
        val data = known ?: AlbumData().also { albums[id] = it }
        load(data.list) {
            val page = api.album(id)
            data.info.value = page.album
            data.list.tracks.value = page.tracks.distinctBy { it.id }
        }
        return data
    }

    fun artist(id: String, reload: Boolean = false): ArtistData {
        val known = artists[id]
        if (known != null && !reload) return known
        val data = known ?: ArtistData().also { artists[id] = it }
        load(data.popular) {
            val page = api.artist(id)
            data.info.value = page.artist
            data.albums.value = page.albums
            data.popular.tracks.value = page.popular.distinctBy { it.id }
        }
        return data
    }

    private fun load(list: TrackList, block: suspend () -> Unit) {
        list.loading.value = true
        list.error.value = ""
        scope.launch {
            try {
                block()
            } catch (e: IOException) {
                list.error.value = "Не удалось загрузить: ${e.message}"
            } finally {
                list.loading.value = false
            }
        }
    }

    private companion object {
        const val TYPING_PAUSE_MS = 350L
    }
}
