package io.github.yaskorinov.yamusic.api

private fun coverUrl(uri: String, side: Int): String = if (uri.isEmpty()) "" else "https://" + uri.replace("%%", "${side}x$side")

data class Album(
    val id: String,
    val title: String,
    val version: String,
    val artists: String,
    val year: Int,
    /** «Альбом», «Сингл», «EP», «Сборник»… */
    val kind: String,
    val coverUri: String,
    val artistRefs: List<ArtistRef> = emptyList(),
) {
    fun cover(side: Int = 300): String = coverUrl(coverUri, side)
}

data class Artist(val id: String, val name: String, val coverUri: String) {
    fun cover(side: Int = 300): String = coverUrl(coverUri, side)
}

class AlbumPage(val album: Album, val tracks: List<Track>)

class ArtistPage(val artist: Artist, val popular: List<Track>, val albums: List<Album>)

/** Лучший результат поиска: ровно одно из полей задано. */
class Best(val track: Track? = null, val album: Album? = null, val artist: Artist? = null)

class SearchResult(val tracks: List<Track>, val albums: List<Album>, val artists: List<Artist>, val best: Best? = null) {
    val empty: Boolean get() = tracks.isEmpty() && albums.isEmpty() && artists.isEmpty()
}

private val ALBUM_KINDS = mapOf("single" to "Сингл", "compilation" to "Сборник", "podcast" to "Подкаст", "audiobook" to "Аудиокнига")

fun AlbumDto.toAlbum(): Album {
    var kind = ALBUM_KINDS[type] ?: "Альбом"
    if (kind == "Альбом" && trackCount in 1..6) kind = if (trackCount > 1) "EP" else "Сингл"
    return Album(
        id = id,
        title = title,
        version = version,
        artists = artists.map { it.name }.filter { it.isNotEmpty() }.joinToString(", "),
        year = year,
        kind = kind,
        coverUri = coverUri.ifEmpty { ogImage },
        artistRefs = artists.filter { it.id.isNotEmpty() && it.name.isNotEmpty() }.map { ArtistRef(it.id, it.name) },
    )
}

fun ArtistDto.toArtist(): Artist = Artist(id, name, cover.uri.ifEmpty { ogImage })
