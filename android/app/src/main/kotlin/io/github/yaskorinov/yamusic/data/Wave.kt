package io.github.yaskorinov.yamusic.data

import android.util.Log
import androidx.media3.common.MediaItem
import io.github.yaskorinov.yamusic.api.FALLBACK_WAVE_GROUPS
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.api.WAVE_SEED
import io.github.yaskorinov.yamusic.api.WaveGroup
import io.github.yaskorinov.yamusic.api.YandexApi
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlaybackEvents
import io.github.yaskorinov.yamusic.playback.PlaybackTracker
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.playContext
import io.github.yaskorinov.yamusic.playback.toMediaItem
import io.github.yaskorinov.yamusic.playback.toTrack
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * «Моя волна»: сессия rotor, настройки (занятие, характер, настроение, язык), обратная связь.
 *
 * Сессия создаётся по сидам: `user:onyourwave` (или сид занятия) + сиды настроек. Треки приходят
 * партиями; следующая запрашивается, когда впереди остаётся мало. Смена настроек во время игры
 * пересоздаёт сессию и заменяет всё после текущего трека.
 */
class Wave(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    session: Session,
    private val settings: Settings,
    private val player: PlayerConnection,
    /** Плеер в сервисе; null, пока сервис не запущен. */
    private val tracker: () -> PlaybackTracker?,
) : PlaybackEvents {
    private val _groups = MutableStateFlow(FALLBACK_WAVE_GROUPS)
    val groups = _groups.asStateFlow()

    /** Ключ группы → выбранный сид. */
    private val _selection = MutableStateFlow(restoreSelection(FALLBACK_WAVE_GROUPS))
    val selection = _selection.asStateFlow()

    private val _loading = MutableStateFlow(false)
    val loading = _loading.asStateFlow()

    private val _error = MutableStateFlow("")
    val error = _error.asStateFlow()

    private var sessionId = ""
    private var batchId = ""
    private val recent = ArrayList<String>()   // 'id:albumId' полученных треков
    private var fetching = false
    private var generation = 0                 // смена сессии отменяет ответы старой
    private var job: Job? = null

    init {
        scope.launch {
            session.state.map { (it as? AuthState.SignedIn)?.account?.uid }.distinctUntilChanged().collect { uid ->
                if (uid != null) loadSettings()
            }
        }
    }

    // --- управление -----------------------------------------------------------------

    /** Запустить волну с нынешними настройками. */
    fun start() = newSession(replace = false)

    /** Выбрать вариант настройки; повторный выбор того же — снять («любое»). [active] — волна сейчас играет. */
    fun select(key: String, seed: String, active: Boolean) {
        val selection = _selection.value.toMutableMap()
        if (selection[key] == seed) selection.remove(key) else selection[key] = seed
        _selection.value = selection
        settings.waveSeeds.value = selection.values.joinToString(",")
        if (active) newSession(replace = true)
    }

    private fun seeds(): List<String> {
        val selection = _selection.value
        return listOf(selection["context"] ?: WAVE_SEED) + selection.filterKeys { it != "context" }.values
    }

    private fun newSession(replace: Boolean) {
        val gen = ++generation
        job?.cancel()
        job = scope.launch {
            _loading.value = true
            _error.value = ""
            try {
                val engine = tracker()?.player
                val current = engine?.currentMediaItem?.takeIf { replace }?.toTrack()
                val batch = api.rotorSessionNew(seeds(), listOfNotNull(current?.fullId))
                if (gen != generation) return@launch
                if (batch.tracks.isEmpty()) {
                    _error.value = "Волна не прислала треков — попробуйте другие настройки"
                    return@launch
                }
                sessionId = batch.sessionId
                batchId = batch.batchId
                fetching = false
                val tracks = take(batch.tracks)
                if (replace && engine != null && engine.currentMediaItem?.playContext()?.wave == true) {
                    engine.removeMediaItems(engine.currentMediaItemIndex + 1, engine.mediaItemCount)
                    engine.addMediaItems(tracks.map { it.toMediaItem(PlayContext.Wave) })
                } else {
                    player.play(tracks, 0, PlayContext.Wave)
                }
                feedback("radioStarted", from = PlayContext.Wave.from)
            } catch (e: IOException) {
                if (gen == generation) _error.value = "Не удалось запустить волну: ${e.message}"
            } finally {
                if (gen == generation) _loading.value = false
            }
        }
    }

    /** Впереди мало треков — дозапросить партию и добавить в конец очереди. */
    private fun feed() {
        val engine = tracker()?.player ?: return
        val item = engine.currentMediaItem ?: return
        if (!item.playContext().wave || fetching || sessionId.isEmpty()) return
        if (engine.mediaItemCount - engine.currentMediaItemIndex - 1 >= MIN_AHEAD) return
        fetching = true
        val gen = generation
        scope.launch {
            try {
                var batch = api.rotorSessionTracks(sessionId, recent.toList())
                if (gen != generation) return@launch
                if (batch.unknownSession || batch.tracks.isEmpty()) {
                    // Сессия протухла (долгая пауза) — новая с теми же настройками, в конец очереди
                    batch = api.rotorSessionNew(seeds(), recent.toList())
                    if (gen != generation) return@launch
                    sessionId = batch.sessionId
                }
                if (batch.batchId.isNotEmpty()) batchId = batch.batchId
                val now = tracker()?.player ?: return@launch
                if (now.currentMediaItem?.playContext()?.wave == true) {
                    now.addMediaItems(take(batch.tracks).map { it.toMediaItem(PlayContext.Wave) })
                }
            } catch (e: IOException) {
                Log.w(TAG, "волна: следующая партия не загрузилась: ${e.message}")
            } finally {
                if (gen == generation) fetching = false
            }
        }
    }

    private fun take(tracks: List<Track>): List<Track> {
        recent += tracks.map { it.fullId }
        while (recent.size > RECENT) recent.removeAt(0)
        return tracks
    }

    // --- обратная связь -------------------------------------------------------------

    override fun trackStarted(item: MediaItem) {
        if (!item.playContext().wave) return
        feedback("trackStarted", item.toTrack().fullId)
        feed()
    }

    override fun trackEnded(item: MediaItem, playedSeconds: Double, endSeconds: Double, natural: Boolean) {
        if (!item.playContext().wave) return
        feedback(if (natural) "trackFinished" else "skip", item.toTrack().fullId, playedSeconds)
    }

    /** Лайк/дизлайк во время волны — подсказка рекомендациям (коллекцию меняет Library). */
    fun liked(kind: String, track: Track) {
        val tracker = tracker() ?: return
        val current = tracker.player.currentMediaItem ?: return
        if (!current.playContext().wave) return
        val played = tracker.playedSeconds.takeIf { kind == "dislike" && current.mediaId == track.id }
        feedback(kind, track.fullId, played)
    }

    private fun feedback(type: String, trackId: String? = null, played: Double? = null, from: String? = null) {
        val session = sessionId
        if (session.isEmpty()) return
        if (settings.noReport.value) {
            Log.w(TAG, "волна: $type ${trackId.orEmpty()} (не отправлено: noReport)")
            return
        }
        val batch = batchId
        scope.launch {
            try {
                api.rotorFeedback(session, type, batch, trackId, played, from)
            } catch (e: IOException) {
                Log.w(TAG, "волна: обратная связь не отправилась: ${e.message}")
            }
        }
    }

    // --- служебное ------------------------------------------------------------------

    private suspend fun loadSettings() {
        try {
            val groups = api.waveSettings()
            _groups.value = groups
            _selection.value = restoreSelection(groups)
        } catch (e: IOException) {
            Log.w(TAG, "волна: настройки не загрузились: ${e.message}")
        }
    }

    private fun restoreSelection(groups: List<WaveGroup>): Map<String, String> {
        val seeds = settings.waveSeeds.value.split(',').filter { it.isNotEmpty() }
        return buildMap {
            for (seed in seeds) groups.firstOrNull { group -> group.items.any { it.seed == seed } }?.let { put(it.key, seed) }
        }
    }

    private companion object {
        const val TAG = "YaMusic"
        const val RECENT = 60     // сколько последних треков передавать в queue, чтобы волна их не повторяла
        const val MIN_AHEAD = 3
    }
}
