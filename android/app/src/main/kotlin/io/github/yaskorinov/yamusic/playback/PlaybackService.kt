package io.github.yaskorinov.yamusic.playback

import android.app.PendingIntent
import android.content.Intent
import android.net.ConnectivityManager
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import io.github.yaskorinov.yamusic.App
import io.github.yaskorinov.yamusic.MainActivity

/**
 * Воспроизведение живёт в сервисе: музыка играет при погашенном экране и закрытом окне, а система
 * получает уведомление с кнопками, обложку на экране блокировки и управление с гарнитуры.
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private lateinit var resolver: TrackUrlResolver

    override fun onCreate() {
        super.onCreate()
        val app = application as App
        resolver = TrackUrlResolver(app.api, getSystemService(ConnectivityManager::class.java))
        val dataSource = ResolvingDataSource.Factory(OkHttpDataSource.Factory(app.http), resolver)
        val audio = AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build()
        val player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource))
            .setAudioAttributes(audio, /* handleAudioFocus = */ true)
            .setHandleAudioBecomingNoisy(true) // выдернули наушники — пауза
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        player.addListener(Recovery(player))

        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, player).setCallback(Callback()).setSessionActivity(open).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** Приложение смахнули из недавних: играющая музыка продолжается, иначе сервис не нужен. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }

    private inner class Callback : MediaSession.Callback {
        /** Адрес источника по пути от интерфейса к сервису теряется — восстанавливаем по идентификатору. */
        override fun onAddMediaItems(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            mediaItems: MutableList<MediaItem>,
        ): ListenableFuture<MutableList<MediaItem>> =
            Futures.immediateFuture(mediaItems.mapTo(ArrayList()) { it.buildUpon().setUri(trackUri(it.mediaId)).build() })
    }

    /** Трек не открылся: сначала ещё раз с новой ссылкой, затем — следующий (но не бесконечно). */
    private inner class Recovery(private val player: ExoPlayer) : Player.Listener {
        private var retried: String? = null
        private var failures = 0

        override fun onPlayerError(error: PlaybackException) {
            val id = player.currentMediaItem?.mediaId ?: return
            resolver.forget(id)
            if (retried != id) {
                retried = id
                player.prepare()
            } else if (player.hasNextMediaItem() && ++failures <= MAX_SKIPS) {
                player.seekToNextMediaItem()
                player.prepare()
            }
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (isPlaying) failures = 0
        }
    }

    private companion object {
        const val MAX_SKIPS = 3
    }
}
