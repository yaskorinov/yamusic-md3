package io.github.yaskorinov.yamusic.lyrics

import android.content.Context
import android.util.Log
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.api.YandexApi
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient

sealed interface LyricsState {
    data object Idle : LyricsState
    data class Loading(val trackId: String) : LyricsState
    data class Found(val trackId: String, val lyrics: Lyrics) : LyricsState
    data class Missing(val trackId: String, val failed: Boolean) : LyricsState
}

/**
 * Синхронный текст играющего трека. Ищется, только когда его кто-то показывает ([request]);
 * результат кэшируется на диске (ненайденный — на 6 часов).
 */
class LyricsStore(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    http: OkHttpClient,
    context: Context,
) {
    private val _state = MutableStateFlow<LyricsState>(LyricsState.Idle)
    val state = _state.asStateFlow()

    private val dir = File(context.cacheDir, "lyrics").apply { mkdirs() }
    private val prefs = context.getSharedPreferences("lyrics", Context.MODE_PRIVATE)
    private val json = Json { ignoreUnknownKeys = true }
    private val sources = LyricsSources(
        // Источников шесть, и зависший не должен держать остальные: на запрос — не больше 8 секунд
        http.newBuilder().callTimeout(8, TimeUnit.SECONDS).build(),
        loadMxmToken = { if (prefs.contains("mxmExpires")) prefs.getString("mxmToken", null) to prefs.getLong("mxmExpires", 0) else null },
        saveMxmToken = { token, expires -> prefs.edit().putString("mxmToken", token).putLong("mxmExpires", expires).apply() },
    )
    private var job: Job? = null

    fun request(track: Track) {
        val current = _state.value
        val known = when (current) {
            is LyricsState.Loading -> current.trackId
            is LyricsState.Found -> current.trackId
            is LyricsState.Missing -> current.trackId.takeUnless { current.failed }
            LyricsState.Idle -> null
        }
        if (known == track.id) return
        job?.cancel()
        _state.value = LyricsState.Loading(track.id)
        job = scope.launch {
            delay(DEBOUNCE_MS) // пролистывание треков подряд не должно запускать поиск на каждый
            val result = withContext(Dispatchers.IO) { fetchCached(track) }
            _state.value = when {
                result.lyrics != null -> LyricsState.Found(track.id, result.lyrics)
                else -> LyricsState.Missing(track.id, failed = result.details.isNotEmpty())
            }
            Log.i(TAG, "текст ${track.id}: ${result.lyrics?.source ?: "—"} ${result.lyrics?.kind.orEmpty()}")
        }
    }

    private fun fetchCached(track: Track): LyricsResult {
        val file = File(dir, sha1("$CACHE_VERSION|${track.id}") + ".json")
        try {
            val cached = json.decodeFromString(Lyrics.serializer(), file.readText())
            if (cached.lines.isNotEmpty()) return LyricsResult(cached, emptyList())
            if (System.currentTimeMillis() - file.lastModified() < NOT_FOUND_TTL_MS) return LyricsResult(null, emptyList())
        } catch (_: Exception) {
            // кэша нет или он испорчен — ищем заново
        }
        val title = if (track.version.isEmpty()) track.title else "${track.title} (${track.version})"
        val found = sources.fetch(title, track.artists, track.album, track.durationMs / 1000f) {
            runBlocking { api.lyricsLrc(track.id) }
        }
        val lyrics = found.lyrics?.let { Lyrics(it.source, it.kind, addGaps(it.lines)) }
        // Источник с ошибкой (503, таймаут) мог знать текст — такое «не найдено» не кэшируем
        if (lyrics != null || found.details.isEmpty()) {
            val part = File(dir, file.name + ".part")
            part.writeText(json.encodeToString(Lyrics.serializer(), lyrics ?: Lyrics()))
            part.renameTo(file)
        }
        return LyricsResult(lyrics, found.details)
    }

    private fun sha1(text: String): String =
        MessageDigest.getInstance("SHA-1").digest(text.toByteArray()).joinToString("") { "%02x".format(it) }

    private companion object {
        const val TAG = "YaMusic"
        const val CACHE_VERSION = 1
        const val NOT_FOUND_TTL_MS = 6 * 3600_000L
        const val DEBOUNCE_MS = 350L
    }
}
