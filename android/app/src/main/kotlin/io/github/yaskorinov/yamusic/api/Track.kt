package io.github.yaskorinov.yamusic.api

/** Трек в том виде, в каком он нужен интерфейсу и плееру. */
data class Track(
    val id: String,
    val albumId: String = "",
    val title: String = "",
    val version: String = "",
    val artists: String = "",
    val album: String = "",
    /** 'avatars.yandex.net/get-music-content/…/%%' — размер подставляет [cover]. */
    val coverUri: String = "",
    val durationMs: Long = 0,
    val explicit: Boolean = false,
    val available: Boolean = true,
) {
    /** 'id:albumId' — так API лайков и волны однозначно находит трек. */
    val fullId: String get() = if (albumId.isEmpty()) id else "$id:$albumId"

    /** Сторона — из набора Яндекса: 50, 100, 200, 300, 400, 600, 800, 1000. */
    fun cover(side: Int = 200): String =
        if (coverUri.isEmpty()) "" else "https://" + coverUri.replace("%%", "${side}x$side")
}

fun TrackDto.toTrack(): Track {
    val album = albums.firstOrNull()
    return Track(
        id = id,
        albumId = album?.id.orEmpty(),
        title = title,
        version = version,
        artists = artists.map { it.name }.filter { it.isNotEmpty() }.joinToString(", "),
        album = album?.title.orEmpty(),
        coverUri = coverUri.ifEmpty { album?.coverUri.orEmpty() },
        durationMs = durationMs,
        explicit = contentWarning == "explicit",
        available = available,
    )
}

class StreamInfo(val url: String, val codec: String, val bitrate: Int)

data class Playlist(
    val uid: String,
    val kind: String,
    val title: String,
    val trackCount: Int,
    val coverUri: String,
    val owner: String,
) {
    /** 'uid:kind' — так плейлист называют учёт прослушиваний и ключи списков. */
    val id: String get() = "$uid:$kind"

    fun cover(side: Int = 200): String =
        if (coverUri.isEmpty()) "" else "https://" + coverUri.replace("%%", "${side}x$side")
}

fun PlaylistDto.toPlaylist(): Playlist = Playlist(
    uid = uid.ifEmpty { owner.uid },
    kind = kind,
    title = title,
    trackCount = trackCount,
    coverUri = cover.uri.ifEmpty { cover.itemsUri.firstOrNull().orEmpty() }.ifEmpty { ogImage },
    owner = owner.name.ifEmpty { owner.login },
)

/** Партия треков волны. */
class RotorBatch(val sessionId: String, val batchId: String, val tracks: List<Track>, val unknownSession: Boolean)

fun RotorDto.toBatch(): RotorBatch =
    RotorBatch(radioSessionId, batchId, sequence.mapNotNull { it.track?.toTrack() }, unknownSession)
