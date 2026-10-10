package io.github.yaskorinov.yamusic.playback

import android.os.SystemClock
import androidx.media3.common.MediaItem
import androidx.media3.common.Player

/** Кому интересны начало и конец каждого трека: учёт прослушиваний и волна. */
interface PlaybackEvents {
    fun trackStarted(item: MediaItem)

    /**
     * [playedSeconds] — сколько трек реально звучал, [endSeconds] — где остановились,
     * [natural] — доигран до конца (а не пропущен).
     */
    fun trackEnded(item: MediaItem, playedSeconds: Double, endSeconds: Double, natural: Boolean)
}

/** Следит за плеером в сервисе и сообщает о начале и конце каждого трека. */
class PlaybackTracker(val player: Player, private val events: List<PlaybackEvents>) : Player.Listener {
    private var current: MediaItem? = null
    private var playedMs = 0L
    private var playingSince = 0L   // 0 — сейчас не звучит
    private var endMs = -1L         // позиция, на которой трек покинули
    private var naturalNext = false // следующую смену трека считать доигрыванием

    /** Сколько секунд звучал текущий трек. */
    val playedSeconds: Double
        get() = (playedMs + if (playingSince > 0) SystemClock.elapsedRealtime() - playingSince else 0) / 1000.0

    init {
        player.addListener(this)
        player.currentMediaItem?.let(::begin)
    }

    /** Плавный переход сам переключает трек чуть раньше конца — это доигрывание, а не пропуск. */
    fun expectNaturalEnd() {
        naturalNext = true
    }

    /** Сервис закрывается: досчитать последний трек. */
    fun release() {
        player.removeListener(this)
        endMs = player.currentPosition
        finish(natural = false)
    }

    // Приходит раньше onMediaItemTransition — здесь узнаём, где закончился прежний трек
    override fun onPositionDiscontinuity(old: Player.PositionInfo, new: Player.PositionInfo, reason: Int) {
        if (old.mediaItem?.mediaId == current?.mediaId && (old.mediaItemIndex != new.mediaItemIndex || old.mediaItem != new.mediaItem)) {
            endMs = old.positionMs
        }
    }

    override fun onMediaItemTransition(item: MediaItem?, reason: Int) {
        val auto = reason == Player.MEDIA_ITEM_TRANSITION_REASON_AUTO || reason == Player.MEDIA_ITEM_TRANSITION_REASON_REPEAT
        if (naturalNext) current?.let { endMs = it.toTrack().durationMs }
        finish(natural = auto || naturalNext)
        naturalNext = false
        item?.let(::begin)
    }

    override fun onIsPlayingChanged(isPlaying: Boolean) {
        val now = SystemClock.elapsedRealtime()
        if (isPlaying) {
            playingSince = now
        } else if (playingSince > 0) {
            playedMs += now - playingSince
            playingSince = 0
        }
    }

    override fun onPlaybackStateChanged(state: Int) {
        if (state == Player.STATE_ENDED) {
            endMs = player.currentPosition
            finish(natural = true)
        } else if (state == Player.STATE_READY && current == null) {
            player.currentMediaItem?.let(::begin) // очередь доиграла, и её запустили снова
        }
    }

    private fun begin(item: MediaItem) {
        current = item
        playedMs = 0
        playingSince = if (player.isPlaying) SystemClock.elapsedRealtime() else 0
        endMs = -1
        events.forEach { it.trackStarted(item) }
    }

    private fun finish(natural: Boolean) {
        val item = current ?: return
        val played = playedSeconds
        current = null
        playedMs = 0
        val end = endMs.coerceAtLeast(0) / 1000.0
        val duration = item.toTrack().durationMs / 1000.0
        events.forEach { it.trackEnded(item, played, end, natural || (duration > 0 && end >= duration - 3)) }
    }
}
