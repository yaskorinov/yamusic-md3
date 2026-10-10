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
