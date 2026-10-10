package io.github.yaskorinov.yamusic.data

import io.github.yaskorinov.yamusic.api.ApiException
import io.github.yaskorinov.yamusic.api.Playlist
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.api.YandexApi
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Список треков, который заполняется асинхронно. */
class TrackList {
    val tracks = MutableStateFlow<List<Track>>(emptyList())
    val loading = MutableStateFlow(false)
    val error = MutableStateFlow("")
}

@Serializable
private class LibrarySnapshot(
    val liked: List<Track> = emptyList(),
    val playlists: List<Playlist> = emptyList(),
    val playlistTracks: Map<String, List<Track>> = emptyMap(),
)

/**
 * Библиотека пользователя: «Мне нравится», плейлисты и их треки. Копия лежит на диске ([dir]):
 * с неё списки показываются сразу при запуске и остаются доступны без сети.
 */
class Library(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    private val session: Session,
    private val dir: File,
) {
    private val json = Json { ignoreUnknownKeys = true }
    private val disk = Dispatchers.IO.limitedParallelism(1)
    private var refreshFailed = false

    /** «Мне нравится» — такой же список, как у плейлистов. */
    val likedList = TrackList()
    private val _liked get() = likedList.tracks
    val liked = _liked.asStateFlow()

    /** Идентификаторы без альбома: по ним строки и плеер узнают, стоит ли лайк. */
    private val _likedIds = MutableStateFlow<Set<String>>(emptySet())
    val likedIds = _likedIds.asStateFlow()

    /** Сбой, не привязанный к одному списку (плейлисты, лайк) — показывает экран коллекции. */
    val notice = MutableStateFlow("")

    private val _loading get() = likedList.loading
    private val _error get() = likedList.error

    private val _playlists = MutableStateFlow<List<Playlist>>(emptyList())
    val playlists = _playlists.asStateFlow()

    /** Подсказка волне: (like | unlike | dislike, трек). */
    var onFeedback: ((String, Track) -> Unit)? = null

    private val playlistTracks = HashMap<String, TrackList>()
    private val loadedPlaylists = HashSet<String>() // обновлены с сервера в этом запуске
    private var job: Job? = null
    private var playlistsJob: Job? = null

    init {
        scope.launch {
            session.state.map { (it as? AuthState.SignedIn)?.account?.uid }.distinctUntilChanged().collect { uid ->
                if (uid == null) {
                    clear()
                } else {
                    restore(uid)
                    refresh()
                }
            }
        }
    }

    fun refresh() {
        val uid = session.account?.uid ?: return
        refreshFailed = false
        notice.value = ""
        playlistsJob?.cancel()
        playlistsJob = scope.launch {
            try {
                _playlists.value = api.playlists(uid)
                save(uid)
            } catch (e: IOException) {
                offline(e, "плейлисты")
            }
        }
        job?.cancel()
        job = scope.launch {
            _loading.value = true
            _error.value = ""
            try {
                val ids = api.likedTrackIds(uid)
                _likedIds.value = ids.mapTo(HashSet()) { it.substringBefore(':') }
                // Порциями: первые строки видны сразу, даже если лайков тысячи. Если на экране уже
                // сохранённая копия, её не трогаем, пока не загрузится всё: иначе список мигнул бы
                val progressive = _liked.value.isEmpty()
                var loaded = emptyList<Track>()
                for (chunk in ids.chunked(CHUNK)) {
                    loaded = (loaded + api.tracks(chunk)).distinctBy { it.id }
                    if (progressive) _liked.value = loaded
                }
                _liked.value = loaded
                save(uid)
            } catch (e: IOException) {
                if (_liked.value.isEmpty()) _error.value = "Не удалось загрузить библиотеку: ${e.message}" else offline(e, "«Мне нравится»")
            } finally {
                _loading.value = false
            }
        }
    }

    /** Сеть вернулась: если прошлое обновление не удалось, повторить. */
    fun retryIfFailed() {
        if (refreshFailed) refresh()
    }

    private fun offline(e: IOException, what: String) {
        refreshFailed = true
        notice.value = if (e is ApiException) "Не удалось обновить $what: ${e.message}" else "Нет сети — показана сохранённая копия"
    }

    private suspend fun restore(uid: String) {
        val snapshot = withContext(disk) {
            runCatching { json.decodeFromString(LibrarySnapshot.serializer(), File(dir, "library-$uid.json").readText()) }.getOrNull()
        } ?: return
        _liked.value = snapshot.liked
        _likedIds.value = snapshot.liked.mapTo(HashSet()) { it.id }
        _playlists.value = snapshot.playlists
        for ((id, tracks) in snapshot.playlistTracks) playlistTracks.getOrPut(id) { TrackList() }.tracks.value = tracks
    }

    private fun save(uid: String) {
        val snapshot = LibrarySnapshot(
            liked = _liked.value,
            playlists = _playlists.value,
            playlistTracks = playlistTracks.mapValues { it.value.tracks.value }.filterValues { it.isNotEmpty() },
        )
        scope.launch(disk) {
            val file = File(dir, "library-$uid.json")
            val part = File(file.path + ".part")
            part.writeText(json.encodeToString(LibrarySnapshot.serializer(), snapshot))
            part.renameTo(file)
        }
    }

    /** Треки плейлиста: загружаются при первом обращении, [reload] — заново. */
    fun tracksOf(playlist: Playlist, reload: Boolean = false): TrackList {
        val known = playlistTracks[playlist.id]
        if (known != null && loadedPlaylists.contains(playlist.id) && !reload) return known
        val list = known ?: TrackList().also { playlistTracks[playlist.id] = it }
        loadedPlaylists += playlist.id
        list.loading.value = true
        list.error.value = ""
        scope.launch {
            try {
                list.tracks.value = api.playlistTracks(playlist.uid, playlist.kind).distinctBy { it.id }
                session.account?.uid?.let(::save)
            } catch (e: IOException) {
                loadedPlaylists -= playlist.id // в следующий раз попробуем снова
                // без сети остаётся сохранённая копия, если она есть
                if (list.tracks.value.isEmpty()) list.error.value = "Не удалось загрузить плейлист: ${e.message}"
            } finally {
                list.loading.value = false
            }
        }
        return list
    }

    /** Лайк/снятие лайка. Интерфейс меняется сразу, при ошибке API — откат. */
    fun toggleLike(track: Track) {
        val uid = session.account?.uid ?: return
        val liked = track.id !in _likedIds.value
        apply(track, liked)
        onFeedback?.invoke(if (liked) "like" else "unlike", track)
        scope.launch {
            try {
                api.setLiked(uid, track.fullId, liked)
                save(uid)
            } catch (e: IOException) {
                apply(track, !liked)
                notice.value = "Не удалось ${if (liked) "поставить" else "снять"} лайк: ${e.message}"
            }
        }
    }

    /** «Не рекомендовать»: трек пропадает из рекомендаций (и из «Мне нравится», если был там). */
    fun dislike(track: Track) {
        val uid = session.account?.uid ?: return
        if (track.id in _likedIds.value) apply(track, false)
        onFeedback?.invoke("dislike", track)
        scope.launch {
            try {
                api.setDisliked(uid, track.fullId)
            } catch (e: IOException) {
                notice.value = "Не удалось отметить «Не рекомендовать»: ${e.message}"
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
        playlistsJob?.cancel()
        _playlists.value = emptyList()
        playlistTracks.clear()
        loadedPlaylists.clear()
        _liked.value = emptyList()
        _likedIds.value = emptySet()
        _loading.value = false
        _error.value = ""
        notice.value = ""
    }

    private companion object {
        const val CHUNK = 200
    }
}
