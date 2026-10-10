package io.github.yaskorinov.yamusic.data

import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.api.YandexApi
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Библиотека пользователя. Пока — «Мне нравится»; плейлисты придут следом. */
class Library(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    private val session: Session,
) {
    private val _liked = MutableStateFlow<List<Track>>(emptyList())
    val liked = _liked.asStateFlow()

    /** Идентификаторы без альбома: по ним строки и плеер узнают, стоит ли лайк. */
    private val _likedIds = MutableStateFlow<Set<String>>(emptySet())
    val likedIds = _likedIds.asStateFlow()

    private val _loading = MutableStateFlow(false)
    val loading = _loading.asStateFlow()

    private val _error = MutableStateFlow("")
    val error = _error.asStateFlow()

    private var job: Job? = null

    init {
        scope.launch {
            session.state.map { (it as? AuthState.SignedIn)?.account?.uid }.distinctUntilChanged().collect { uid ->
                if (uid == null) clear() else refresh()
            }
        }
    }

    fun refresh() {
        val uid = session.account?.uid ?: return
        job?.cancel()
        job = scope.launch {
            _loading.value = true
            _error.value = ""
            try {
                val ids = api.likedTrackIds(uid)
                _likedIds.value = ids.mapTo(HashSet()) { it.substringBefore(':') }
                // Порциями: первые строки видны сразу, даже если лайков тысячи
                var loaded = emptyList<Track>()
                for (chunk in ids.chunked(CHUNK)) {
                    loaded = (loaded + api.tracks(chunk)).distinctBy { it.id }
                    _liked.value = loaded
                }
                if (ids.isEmpty()) _liked.value = emptyList()
            } catch (e: IOException) {
                _error.value = "Не удалось загрузить библиотеку: ${e.message}"
            } finally {
                _loading.value = false
            }
        }
    }

    /** Лайк/снятие лайка. Интерфейс меняется сразу, при ошибке API — откат. */
    fun toggleLike(track: Track) {
        val uid = session.account?.uid ?: return
        val liked = track.id !in _likedIds.value
        apply(track, liked)
        scope.launch {
            try {
                api.setLiked(uid, track.fullId, liked)
            } catch (e: IOException) {
                apply(track, !liked)
                _error.value = "Не удалось ${if (liked) "поставить" else "снять"} лайк: ${e.message}"
            }
        }
    }

    private fun apply(track: Track, liked: Boolean) {
        _likedIds.update { if (liked) it + track.id else it - track.id }
        _liked.update { list ->
            when {
                !liked -> list.filterNot { it.id == track.id }
                list.any { it.id == track.id } -> list
                else -> listOf(track) + list
            }
        }
    }

    private fun clear() {
        job?.cancel()
        _liked.value = emptyList()
        _likedIds.value = emptySet()
        _loading.value = false
        _error.value = ""
    }

    private companion object {
        const val CHUNK = 200
    }
}
