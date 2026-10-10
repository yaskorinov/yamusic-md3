package io.github.yaskorinov.yamusic.playback

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
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
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin

/**
 * Плавный переход между треками. За N секунд до конца «хвост» трека подхватывает второй плеер и
 * затихает, а основной сразу переходит к следующему треку и набирает громкость (равномощная кривая:
 * сумма мощностей постоянна, провала громкости посередине нет).
 *
 * Основной плеер остаётся единственным хозяином очереди и сеанса: второй только доигрывает хвост.
 * Если хвост не успел подготовиться, перехода нет — треки идут встык, как при выключенной настройке.
 *
 * Хвост запускается без звука за несколько секунд до перехода и подстраивается под основной плеер
 * (чуть быстрее или медленнее, пока позиции не сойдутся). Если запускать его прямо в момент перехода,
 * звук появляется с задержкой запуска и с отставшего места: слышны провал и повтор.
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
    private var syncing = false             // хвост уже играет без звука и подстраивается под основной плеер
    private var syncSince = 0L              // когда он заиграл
    private var offsetSum = 0L              // расхождение позиций (хвост − основной), мс: сумма и число замеров
    private var offsetCount = 0
    private var speed = 1f
    private var fadeTarget = 0L             // длина перехода для подготовленного трека
    private var fading = false
    private var switched = false            // основной плеер уже перешёл к следующему треку
    private var fadeMs = 0L
    private var elapsedMs = 0L              // сколько длится переход (паузы не считаются)
    private var lastTick = 0L
    private var ownSeek = false

    private val tick = object : Runnable {
        override fun run() {
            step()
            if (main.isPlaying || fading) handler.postDelayed(this, if (syncing) SYNC_TICK_MS else TICK_MS)
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
        if (fading || syncing) tail.playWhenReady = isPlaying
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
        if (seconds <= 0 || duration == C.TIME_UNSET || !main.hasNextMediaItem() || main.repeatMode == Player.REPEAT_MODE_ONE) {
            cancel() // переход выключили или следующего трека не стало, пока хвост готовился
            return
        }
        val fade = seconds * 1000L
        if (duration < fade * 2 + MIN_BODY_MS) return // короткий трек: переход съел бы его целиком
        val remaining = duration - main.currentPosition
        if (remaining <= fade + PREROLL_MS + PREPARE_AHEAD_MS && preparedId != item.mediaId) {
            preparedId = item.mediaId
            syncing = false
            fadeTarget = fade
            setSpeed(1f)
            tail.volume = 0f
            tail.playWhenReady = false
            tail.setMediaItem(item, duration - fade - PREROLL_MS)
            tail.prepare()
        }
        if (preparedId != item.mediaId) return
        // Хвост заиграл без звука: до перехода ещё есть время сойтись с основным плеером
        if (!syncing && tail.playbackState == Player.STATE_READY && remaining <= fade + PREROLL_MS + START_LAG_MS && remaining > fade + MIN_SYNC_MS) {
            syncing = true
            syncSince = 0
            offsetSum = 0
            offsetCount = 0
            // запуск занимает время — хвост ставится чуть вперёд; подготовка могла и опоздать
            val target = main.currentPosition + START_LAG_MS
            if (abs(target - tail.currentPosition) > 40) tail.seekTo(target)
            tail.play()
        }
        if (syncing) sync(now, remaining)
        if (syncing && remaining <= fade && remaining > fade / 2 && tail.isPlaying) {
            syncing = false
            fading = true
            switched = false
            fadeMs = remaining
            elapsedMs = 0
            setSpeed(1f)
            Log.i(TAG, "плавный переход: хвост расходится с основным плеером на ${tail.currentPosition - main.currentPosition} мс")
        }
    }

    /** Подстройка хвоста под основной плеер: по среднему расхождению позиций — чуть быстрее или медленнее. */
    private fun sync(now: Long, remaining: Long) {
        if (!tail.isPlaying || !main.isPlaying) {
            syncSince = 0
            return
        }
        if (syncSince == 0L) syncSince = now
        if (now - syncSince < SETTLE_MS) return // сразу после запуска позиция ещё неточная
        offsetSum += tail.currentPosition - main.currentPosition
        if (++offsetCount < SYNC_SAMPLES) return
        val offset = offsetSum.toFloat() / offsetCount
        offsetSum = 0
        offsetCount = 0
        // перед самым переходом скорость уже не трогаем: её смена доходит до динамика не сразу
        val hold = remaining <= fadeTarget + HOLD_MS
        if (abs(offset) > RESEEK_MS && !hold) {
            setSpeed(1f)
            tail.seekTo(main.currentPosition + START_LAG_MS)
            syncSince = 0
            return
        }
        setSpeed(if (hold || abs(offset) <= SYNCED_MS) 1f else (1f - offset / PULL_MS).coerceIn(0.9f, 1.1f))
    }

    private fun setSpeed(wanted: Float) {
        if (abs(wanted - speed) < 0.004f) return
        speed = wanted
        tail.setPlaybackSpeed(wanted)
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
        if (fading || syncing || preparedId != null) finish()
    }

    private fun finish() {
        fading = false
        syncing = false
        setSpeed(1f)
        preparedId = null
        tail.stop()
        tail.clearMediaItems()
        main.volume = 1f
    }

    private companion object {
        const val TAG = "YaMusic"
        const val TICK_MS = 40L
        const val SYNC_TICK_MS = 20L
        const val SEAM_MS = 140L
        const val PREPARE_AHEAD_MS = 8000L
        const val PREROLL_MS = 5000L    // за сколько до перехода хвост начинает играть без звука
        const val MIN_SYNC_MS = 1500L   // меньше этого на подстройку не хватит — тогда перехода нет
        const val START_LAG_MS = 120L   // примерная задержка запуска воспроизведения
        const val SETTLE_MS = 500L
        const val SYNC_SAMPLES = 8
        const val SYNCED_MS = 5f        // расхождение, которое уже не слышно
        const val PULL_MS = 700f        // расхождение выбирается примерно за это время
        const val RESEEK_MS = 400f      // так далеко скоростью не догнать — перемотка
        const val HOLD_MS = 700L
        const val MIN_BODY_MS = 10_000L
    }
}
