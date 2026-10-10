package io.github.yaskorinov.yamusic.playback

import android.net.Uri
import android.os.Bundle
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import io.github.yaskorinov.yamusic.api.ArtistRef
import io.github.yaskorinov.yamusic.api.Track

// Плеер получает не ссылку на файл, а «yamusic://track/<id>»: настоящая ссылка живёт недолго,
// поэтому её запрашивает TrackUrlResolver прямо перед загрузкой.
private const val SCHEME = "yamusic"

fun trackUri(id: String): Uri = Uri.Builder().scheme(SCHEME).authority("track").appendPath(id).build()

/** Идентификатор трека из «yamusic://track/<id>», для остальных адресов — null. */
fun trackIdOf(uri: Uri): String? = if (uri.scheme == SCHEME) uri.lastPathSegment else null

/** Откуда играет трек: это нужно учёту прослушиваний ([from], [playlistId]) и волне. */
data class PlayContext(val from: String, val playlistId: String = "", val wave: Boolean = false) {
    companion object {
        // Метки источника — как у десктопного клиента: с ними учёт прослушиваний проверен
        val Liked = PlayContext("desktop_win-own_tracks-track-default")
        val Wave = PlayContext("desktop_win-radio-user-onyourwave-default", wave = true)

        val Search = PlayContext("desktop_win-search-track-default")
        val Album = PlayContext("desktop_win-album-track-default")
        val Artist = PlayContext("desktop_win-artist-track-default")

        fun playlist(id: String) = PlayContext("desktop_win-playlist-track-default", playlistId = id)
    }
}

fun MediaItem.playContext(): PlayContext {
    val extras = mediaMetadata.extras
    return PlayContext(
        from = extras?.getString("from") ?: PlayContext.Liked.from,
        playlistId = extras?.getString("playlistId").orEmpty(),
        wave = extras?.getBoolean("wave") ?: false,
    )
}

fun Track.toMediaItem(context: PlayContext): MediaItem {
    val extras = Bundle().apply {
        putString("from", context.from)
        putString("playlistId", context.playlistId)
        putBoolean("wave", context.wave)
        putString("albumId", albumId)
        putString("version", version)
        putString("coverUri", coverUri)
        putLong("durationMs", durationMs)
        putBoolean("explicit", explicit)
        putStringArrayList("artistIds", ArrayList(artistRefs.map { it.id }))
        putStringArrayList("artistNames", ArrayList(artistRefs.map { it.name }))
    }
    val metadata = MediaMetadata.Builder()
        .setTitle(title)
        .setArtist(artists)
        .setAlbumTitle(album)
        .setArtworkUri(cover(400).takeIf { it.isNotEmpty() }?.let(Uri::parse))
        .setExtras(extras)
        .build()
    return MediaItem.Builder().setMediaId(id).setUri(trackUri(id)).setMediaMetadata(metadata).build()
}

/** Обратно в трек: интерфейс мог перезапуститься, а очередь осталась в сервисе. */
fun MediaItem.toTrack(): Track {
    val extras = mediaMetadata.extras
    return Track(
        id = mediaId,
        albumId = extras?.getString("albumId").orEmpty(),
        title = mediaMetadata.title?.toString().orEmpty(),
        version = extras?.getString("version").orEmpty(),
        artists = mediaMetadata.artist?.toString().orEmpty(),
        album = mediaMetadata.albumTitle?.toString().orEmpty(),
        coverUri = extras?.getString("coverUri").orEmpty(),
        durationMs = extras?.getLong("durationMs") ?: 0,
        explicit = extras?.getBoolean("explicit") ?: false,
        artistRefs = extras?.getStringArrayList("artistIds").orEmpty()
            .zip(extras?.getStringArrayList("artistNames").orEmpty()) { id, name -> ArtistRef(id, name) },
    )
}
