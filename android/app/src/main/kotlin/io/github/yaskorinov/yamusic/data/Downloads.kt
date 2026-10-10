package io.github.yaskorinov.yamusic.data

import android.content.Context
import android.net.ConnectivityManager
import android.util.Log
import io.github.yaskorinov.yamusic.api.ApiException
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.api.YandexApi
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request

@Serializable
class Downloaded(val track: Track, val file: String, val codec: String = "", val bitrate: Int = 0, val size: Long = 0)

@Serializable
private class DownloadIndex(val done: List<Downloaded> = emptyList(), val pending: List<Track> = emptyList())

/**
 * Треки для игры без сети: файл целиком лежит в хранилище приложения, плеер берёт его вместо ссылки.
 * Очередь скачивания переживает перезапуск; качаем по одному, пока жив процесс приложения.
 */
class Downloads(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    private val http: OkHttpClient,
    private val settings: Settings,
    context: Context,
) {
    private val dir = File(context.filesDir, "tracks").apply { mkdirs() }
    private val indexFile = File(context.filesDir, "downloads.json")
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private val json = Json { ignoreUnknownKeys = true }
    private val disk = Dispatchers.IO.limitedParallelism(1) // записи индекса — строго по очереди

    /** Скачанные треки по идентификатору. */
    private val _done = MutableStateFlow<Map<String, Downloaded>>(emptyMap())
    val done = _done.asStateFlow()

    private val _pending = MutableStateFlow<List<Track>>(emptyList())
    val pending = _pending.asStateFlow()

    /** Почему очередь стоит («Ждём Wi-Fi», «Нет сети», ошибка) или пусто. */
    private val _status = MutableStateFlow("")
    val status = _status.asStateFlow()

    /** Скачанное как обычный список треков — для страницы «Скачанные». */
    val list = TrackList()

    private var job: Job? = null

    init {
        try {
            val index = json.decodeFromString(DownloadIndex.serializer(), indexFile.readText())
            publish(index.done.filter { File(dir, it.file).exists() }.associateBy { it.track.id })
            _pending.value = index.pending
        } catch (_: Exception) {
            // индекса ещё нет или он испорчен — начинаем с пустого
        }
        kick()
    }

    /** Файл трека или null. Вызывается из потока загрузки плеера. */
    fun fileOf(id: String): File? = _done.value[id]?.let { File(dir, it.file) }?.takeIf { it.exists() }

    fun enqueue(tracks: List<Track>) {
        val known = _done.value.keys + _pending.value.map { it.id }
        val fresh = tracks.filter { it.available && it.id !in known }.distinctBy { it.id }
        if (fresh.isEmpty()) return
        _pending.value = _pending.value + fresh
        save()
        kick()
    }

    fun remove(ids: Collection<String>) {
        val gone = ids.toSet()
        _pending.value = _pending.value.filterNot { it.id in gone }
        val removed = _done.value.filterKeys { it in gone }
        publish(_done.value - gone)
        save()
        scope.launch(Dispatchers.IO) { removed.values.forEach { File(dir, it.file).delete() } }
    }

    fun clear() = remove(_done.value.keys + _pending.value.map { it.id })

    /** Продолжить очередь: при запуске, новом задании и появлении сети. */
    fun kick() {
        if (job?.isActive == true || _pending.value.isEmpty()) return
        job = scope.launch {
            while (true) {
                val track = _pending.value.firstOrNull() ?: break
                if (connectivity.activeNetwork == null) {
                    _status.value = "Нет сети"
                    return@launch
                }
                if (settings.downloadOnWifiOnly.value && connectivity.isActiveNetworkMetered) {
                    _status.value = "Ждём Wi-Fi"
                    return@launch
                }
                _status.value = ""
                try {
                    val entry = withContext(Dispatchers.IO) { download(track) }
                    // пока качали, задание могли отменить
                    if (_pending.value.any { it.id == track.id }) publish(_done.value + (track.id to entry))
                    else File(dir, entry.file).delete()
                } catch (e: ApiException) {
                    if (e.unauthorized || e.status == 429 || e.status >= 500) {
                        _status.value = "Скачивание остановлено: ${e.message}"
                        return@launch
                    }
                    Log.w(TAG, "трек ${track.id} не скачать: ${e.message}") // недоступен — пропускаем
                } catch (e: IOException) {
                    _status.value = "Скачивание прервано: ${e.message}"
                    return@launch
                }
                _pending.value = _pending.value.filterNot { it.id == track.id }
                save()
            }
            _status.value = ""
        }
    }

    private suspend fun download(track: Track): Downloaded {
        val quality = when (val chosen = settings.quality.value) {
            "auto" -> if (connectivity.isActiveNetworkMetered) "hq" else "lossless"
            else -> chosen
        }
        val info = try {
            api.streamInfo(track.id, quality)
        } catch (e: ApiException) {
            if (quality == "hq" || e.unauthorized) throw e
            api.streamInfo(track.id, "hq")
        }
        val name = "${track.id}.${extension(info.codec)}"
        val part = File(dir, "$name.part")
        http.newCall(Request.Builder().url(info.url).build()).execute().use { response ->
            if (!response.isSuccessful) throw IOException("HTTP ${response.code}")
            part.outputStream().use { out ->
                val source = response.body.byteStream()
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    scope.coroutineContext.ensureActive()
                    val read = source.read(buffer)
                    if (read < 0) break
                    out.write(buffer, 0, read)
                }
            }
        }
        val file = File(dir, name)
        if (!part.renameTo(file)) throw IOException("не удалось сохранить файл")
        return Downloaded(track, name, info.codec, info.bitrate, file.length())
    }

    private fun publish(done: Map<String, Downloaded>) {
        _done.value = done
        list.tracks.value = done.values.map { it.track }.reversed()
    }

    private fun save() {
        val index = DownloadIndex(_done.value.values.toList(), _pending.value)
        scope.launch(disk) {
            val part = File(indexFile.path + ".part")
            part.writeText(json.encodeToString(DownloadIndex.serializer(), index))
            part.renameTo(indexFile)
        }
    }

    private fun extension(codec: String): String = when {
        codec.endsWith("-mp4") -> "m4a"
        codec == "flac" -> "flac"
        codec == "mp3" -> "mp3"
        else -> "aac"
    }

    private companion object {
        const val TAG = "YaMusic"
    }
}
