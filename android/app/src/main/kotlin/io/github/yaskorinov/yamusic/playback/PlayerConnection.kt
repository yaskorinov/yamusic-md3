package io.github.yaskorinov.yamusic.playback

import android.content.ComponentName
import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.util.Util
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import io.github.yaskorinov.yamusic.api.Track
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

data class PlayerState(
    val track: Track? = null,
    /** Показывать «паузу»: плеер играет или вот-вот заиграет (буферизация не мигает кнопкой). */
    val playing: Boolean = false,
    val buffering: Boolean = false,
    val durationMs: Long = 0,
    val hasNext: Boolean = false,
    val hasPrevious: Boolean = false,
    val shuffle: Boolean = false,
    val repeat: Int = Player.REPEAT_MODE_OFF,
    /** Играет «Моя волна». */
    val wave: Boolean = false,
    /** Место текущего трека в очереди. */
    val index: Int = 0,
    /** Места следующего и предыдущего треков в очереди с учётом перемешивания и повтора; -1 — такого нет. */
    val nextIndex: Int = -1,
    val previousIndex: Int = -1,
    val error: String = "",
)

/** Связь интерфейса с плеером в [PlaybackService]: состояние — потоком, команды — методами. */
@OptIn(UnstableApi::class)
class PlayerConnection(private val context: Context) {
    private val _state = MutableStateFlow(PlayerState())
    val state = _state.asStateFlow()

    private var future: ListenableFuture<MediaController>? = null
    private var controller: MediaController? = null

    /** Очередь в порядке плейлиста (при перемешивании играет вразбивку, но список тот же). */
    private val _queue = MutableStateFlow<List<Track>>(emptyList())
    val queue = _queue.asStateFlow()

    private val listener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            publish(player)
            if (events.contains(Player.EVENT_TIMELINE_CHANGED)) publishQueue(player)
        }
    }

    /** Позиция меняется непрерывно, поэтому её не публикуют, а спрашивают. */
    val positionMs: Long get() = controller?.currentPosition ?: 0

    fun connect() {
        if (future != null) return
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val pending = MediaController.Builder(context, token).buildAsync()
        future = pending
        pending.addListener({
            if (future !== pending) return@addListener
            runCatching { pending.get() }.onSuccess {
                controller = it
                it.addListener(listener)
                publish(it)
                publishQueue(it)
            }
        }, context.mainExecutor)
    }

    fun disconnect() {
        controller?.removeListener(listener)
        controller = null
        future?.let(MediaController::releaseFuture)
        future = null
    }

    /** Играть список с трека [index]; недоступные треки в очередь не попадают. */
    fun play(tracks: List<Track>, index: Int, context: PlayContext, shuffle: Boolean = false) {
        val player = controller ?: return
        val start = tracks.getOrNull(index)
        val playable = tracks.filter { it.available }
        if (playable.isEmpty()) return
        val from = playable.indexOf(start).coerceAtLeast(0)
        player.shuffleModeEnabled = shuffle
        player.setMediaItems(playable.map { it.toMediaItem(context) }, from, 0L)
        player.prepare()
        player.play()
    }

    /** Перейти к треку очереди под номером [index]. */
    fun playAt(index: Int) {
        val player = controller ?: return
        player.seekTo(index, 0L)
        player.play()
    }

    fun removeAt(index: Int) = controller?.removeMediaItem(index)

    fun togglePlay() {
        val player = controller ?: return
        Util.handlePlayPauseButtonAction(player)
    }

    fun next() = controller?.seekToNextMediaItem()

    /** В начале трека — к предыдущему, иначе — в начало текущего. */
    fun previous() = controller?.seekToPrevious()

    /** К предыдущему треку, где бы ни стояла позиция текущего. */
    fun previousItem() = controller?.seekToPreviousMediaItem()

    fun seekTo(positionMs: Long) = controller?.seekTo(positionMs)

    fun toggleShuffle() {
        controller?.let { it.shuffleModeEnabled = !it.shuffleModeEnabled }
    }

    /** Повтор по кругу: выключен → всё → один трек. */
    fun cycleRepeat() {
        controller?.let {
            it.repeatMode = when (it.repeatMode) {
                Player.REPEAT_MODE_OFF -> Player.REPEAT_MODE_ALL
                Player.REPEAT_MODE_ALL -> Player.REPEAT_MODE_ONE
                else -> Player.REPEAT_MODE_OFF
            }
        }
    }

    private fun publishQueue(player: Player) {
        _queue.value = List(player.mediaItemCount) { player.getMediaItemAt(it).toTrack() }
    }

    private fun publish(player: Player) {
        val item = player.currentMediaItem
        val track = item?.toTrack()
        _state.value = PlayerState(
            track = track,
            playing = !Util.shouldShowPlayButton(player),
            buffering = player.playbackState == Player.STATE_BUFFERING,
            durationMs = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: track?.durationMs ?: 0,
            hasNext = player.hasNextMediaItem(),
            hasPrevious = player.hasPreviousMediaItem(),
            shuffle = player.shuffleModeEnabled,
            repeat = player.repeatMode,
            wave = item?.playContext()?.wave ?: false,
            index = player.currentMediaItemIndex,
            nextIndex = player.nextMediaItemIndex,
            previousIndex = player.previousMediaItemIndex,
            error = player.playerError?.let { it.cause?.message ?: it.message }.orEmpty(),
        )
    }
}
