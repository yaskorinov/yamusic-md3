package io.github.yaskorinov.yamusic.data

import android.util.Log
import androidx.media3.common.MediaItem
import io.github.yaskorinov.yamusic.api.YandexApi
import io.github.yaskorinov.yamusic.playback.PlaybackEvents
import io.github.yaskorinov.yamusic.playback.playContext
import io.github.yaskorinov.yamusic.playback.toTrack
import java.io.IOException
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Учёт прослушиваний (/play-audio): по нему Яндекс строит историю, рекомендации и «Мою волну».
 * Как официальные клиенты: отметка в начале трека (0 с) и в конце — сколько реально проиграно и где остановились.
 */
class PlayReporter(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    private val session: Session,
    private val settings: Settings,
) : PlaybackEvents {
    private var playId = ""

    override fun trackStarted(item: MediaItem) {
        playId = UUID.randomUUID().toString()
        send(item, 0.0, 0.0)
    }

    override fun trackEnded(item: MediaItem, playedSeconds: Double, endSeconds: Double, natural: Boolean) {
        if (playedSeconds >= 1) send(item, playedSeconds, endSeconds)
    }

    private fun send(item: MediaItem, played: Double, end: Double) {
        val uid = session.account?.uid ?: return
        val track = item.toTrack()
        val context = item.playContext()
        if (settings.noReport.value) {
            Log.w(TAG, "play-audio ${track.id}: %.1f с, конец %.1f с (не отправлено: noReport)".format(played, end))
            return
        }
        val id = playId
        scope.launch {
            try {
                api.playAudio(uid, track, context.from, context.playlistId, id, played, end)
            } catch (e: IOException) {
                Log.w(TAG, "учёт прослушиваний: ${e.message}")
            }
        }
    }

    private companion object {
        const val TAG = "YaMusic"
    }
}
