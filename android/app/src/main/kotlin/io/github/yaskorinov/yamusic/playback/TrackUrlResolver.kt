package io.github.yaskorinov.yamusic.playback

import android.net.ConnectivityManager
import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.ResolvingDataSource
import io.github.yaskorinov.yamusic.api.ApiException
import io.github.yaskorinov.yamusic.api.StreamInfo
import io.github.yaskorinov.yamusic.api.YandexApi
import io.github.yaskorinov.yamusic.data.Downloads
import io.github.yaskorinov.yamusic.data.Settings
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.runBlocking

/**
 * Превращает «yamusic://track/<id>» в настоящую ссылку на файл (или в скачанный файл, если он есть). Вызывается в потоке загрузки плеера
 * при каждом открытии источника (в том числе при перемотке), поэтому ссылка недолго кэшируется.
 */
@OptIn(UnstableApi::class)
class TrackUrlResolver(
    private val api: YandexApi,
    private val connectivity: ConnectivityManager,
    private val settings: Settings,
    private val downloads: Downloads,
) : ResolvingDataSource.Resolver {
    private class Entry(val info: StreamInfo, val at: Long)

    private val cache = ConcurrentHashMap<String, Entry>()

    override fun resolveDataSpec(dataSpec: DataSpec): DataSpec {
        val id = trackIdOf(dataSpec.uri) ?: return dataSpec
        downloads.fileOf(id)?.let { return dataSpec.withUri(Uri.fromFile(it)) } // скачан — сеть не нужна
        val cached = cache[id]?.takeIf { System.currentTimeMillis() - it.at < TTL_MS }
        val info = cached?.info ?: runBlocking { fetch(id) }.also { cache[id] = Entry(it, System.currentTimeMillis()) }
        return dataSpec.withUri(Uri.parse(info.url))
    }

    /** Ссылка не сработала (протухла) — в следующий раз запросить заново. */
    fun forget(id: String) {
        cache.remove(id)
    }

    private suspend fun fetch(id: String): StreamInfo {
        val quality = when (val chosen = settings.quality.value) {
            // По лимитному соединению (мобильный интернет) — AAC, иначе без потерь
            "auto" -> if (connectivity.isActiveNetworkMetered) "hq" else "lossless"
            else -> chosen
        }
        return try {
            api.streamInfo(id, quality)
        } catch (e: ApiException) {
            if (quality == "hq" || e.unauthorized) throw e
            api.streamInfo(id, "hq") // такого качества нет или API изменился — откат на AAC/MP3
        }
    }

    private companion object {
        const val TTL_MS = 10 * 60 * 1000L
    }
}
