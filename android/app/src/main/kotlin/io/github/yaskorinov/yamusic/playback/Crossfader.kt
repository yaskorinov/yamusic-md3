package io.github.yaskorinov.yamusic.playback

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.MediaSource
import io.github.yaskorinov.yamusic.data.Settings
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * Плавный переход между треками. За N секунд до конца «хвост» трека подхватывает второй плеер и
 * затихает, а основной сразу переходит к следующему треку и набирает громкость (равномощная кривая:
 * сумма мощностей постоянна, провала громкости посередине нет).
 *
 * Основной плеер остаётся единственным хозяином очереди и сеанса: второй только доигрывает хвост.
 * Если хвост не успел подготовиться, перехода нет — треки идут встык, как при выключенной настройке.
 */
@OptIn(UnstableApi::class)
class Crossfader(
    context: Context,
    private val main: ExoPlayer,
    mediaSources: MediaSource.Factory,
    audio: AudioAttributes,
    private val settings: Settings,
    private val tracker: () -> PlaybackTracker?,
) : Player.Listener {
    private val handler = Handler(Looper.getMainLooper())

    // Фокус звука и «выдернули наушники» ведёт основной плеер; второй только повторяет за ним
    private val tail: ExoPlayer = ExoPlayer.Builder(context)
        .setMediaSourceFactory(mediaSources)
        .setAudioAttributes(audio, /* handleAudioFocus = */ false)
        .build()

    private var preparedId: String? = null  // чей хвост готов во втором плеере
    private var fading = false
    private var switched = false            // основной плеер уже перешёл к следующему треку
    private var fadeMs = 0L
    private var elapsedMs = 0L              // сколько длится переход (паузы не считаются)
    private var lastTick = 0L
    private var ownSeek = false

    private val tick = object : Runnable {
        override fun run() {
            step()
            if (main.isPlaying || fading) handler.postDelayed(this, TICK_MS)
        }
    }

    init {
        main.addListener(this)
    }

    fun release() {
        handler.removeCallbacks(tick)
        main.removeListener(this)
        tail.release()
    }

    override fun onIsPlayingChanged(isPlaying: Boolean) {
        handler.removeCallbacks(tick)
        lastTick = SystemClock.elapsedRealtime()
        if (fading) tail.playWhenReady = isPlaying
        if (isPlaying || fading) handler.post(tick)
    }

    // Перемотка или смена трека не нами — переход отменяется
    override fun onPositionDiscontinuity(old: Player.PositionInfo, new: Player.PositionInfo, reason: Int) {
        if (!ownSeek && (reason == Player.DISCONTINUITY_REASON_SEEK || reason == Player.DISCONTINUITY_REASON_REMOVE)) cancel()
    }

    override fun onMediaItemTransition(item: MediaItem?, reason: Int) {
        if (!ownSeek) cancel()
    }

    private fun step() {
        val now = SystemClock.elapsedRealtime()
        val delta = (now - lastTick).coerceIn(0, 500)
        lastTick = now
        if (fading) {
            if (main.isPlaying) elapsedMs += delta
            advance()
            return
        }
        val seconds = settings.crossfade.value
        val item = main.currentMediaItem ?: return
        val duration = main.duration
        if (seconds <= 0 || duration == C.TIME_UNSET || !main.hasNextMediaItem() || main.repeatMode == Player.REPEAT_MODE_ONE) return
        val fade = seconds * 1000L
        if (duration < fade * 2 + MIN_BODY_MS) return // короткий трек: переход съел бы его целиком
        val remaining = duration - main.currentPosition
        if (remaining <= fade + PREPARE_AHEAD_MS && preparedId != item.mediaId) {
            preparedId = item.mediaId
            tail.volume = 0f
            tail.playWhenReady = false
            tail.setMediaItem(item, duration - fade)
            tail.prepare()
        }
        if (remaining <= fade && remaining > fade / 2 && preparedId == item.mediaId && tail.playbackState == Player.STATE_READY) {
            fading = true
            switched = false
            fadeMs = remaining
            elapsedMs = 0
            tail.play()
        }
    }

    private fun advance() {
        if (!switched) {
            // Шов: хвост и основной плеер звучат вместе долю секунды — так не слышно, что они чуть разошлись
            val seam = (elapsedMs.toFloat() / SEAM_MS).coerceIn(0f, 1f)
            tail.volume = seam
            main.volume = 1f - seam
            if (seam < 1f) return
            switched = true
            tracker()?.expectNaturalEnd()
            ownSeek = true
            main.seekToNextMediaItem()
            ownSeek = false
        }
        val progress = ((elapsedMs - SEAM_MS).toFloat() / (fadeMs - SEAM_MS).coerceAtLeast(1)).coerceIn(0f, 1f)
        main.volume = sin(progress * PI / 2).toFloat()
        tail.volume = cos(progress * PI / 2).toFloat()
        if (progress >= 1f || tail.playbackState == Player.STATE_ENDED) finish()
    }

    private fun cancel() {
        if (fading || preparedId != null) finish()
    }

    private fun finish() {
        fading = false
        preparedId = null
        tail.stop()
        tail.clearMediaItems()
        main.volume = 1f
    }

    private companion object {
        const val TICK_MS = 40L
        const val SEAM_MS = 140L
        const val PREPARE_AHEAD_MS = 8000L
        const val MIN_BODY_MS = 10_000L
    }
}
