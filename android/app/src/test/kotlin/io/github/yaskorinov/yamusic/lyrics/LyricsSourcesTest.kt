package io.github.yaskorinov.yamusic.lyrics

import okhttp3.OkHttpClient
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

class LyricsSourcesTest {
    @Test
    fun lrcIsParsedAndSorted() {
        val pairs = linesFromLrc("[00:12.50]вторая строка\n[00:05.00][01:10.00]первая и повтор\n[ar:кто-то]\nбез метки")
        assertEquals(listOf(5f, 12.5f, 70f), pairs.map { it.first })
        assertEquals("первая и повтор", pairs.first().second)
    }

    @Test
    fun wordsAreInterpolatedInsideTheLine() {
        val lines = buildFromTimedLines(listOf(10f to "раз два три", 14f to "", 20f to "конец"), duration = 30f)
        assertEquals(2, lines.size) // пустая строка — не текст
        val first = lines[0]
        assertEquals(listOf("раз ", "два ", "три"), first.words.map { it.text })
        assertEquals(10f, first.words.first().start, 0.001f)
        assertTrue(first.end <= 13.95f + 0.001f)
        assertTrue(first.words.zipWithNext().all { (a, b) -> a.end <= b.start + 0.001f })
    }

    @Test
    fun creditsAreNotLyrics() {
        val lines = buildFromTimedLines(listOf(0f to "作词 : 某人", 1f to "Lyrics: someone", 5f to "настоящая строка"), duration = 10f)
        assertEquals(1, lines.size)
    }

    @Test
    fun neteaseStubsAreRecognised() {
        assertTrue(isPlaceholder(listOf("纯音乐，请欣赏")))
        assertTrue(isPlaceholder(listOf("暂无歌词")))
        assertTrue(isPlaceholder(emptyList()))
        assertFalse(isPlaceholder(listOf("Белый снег, серый лёд", "На растрескавшейся земле")))
        assertFalse(isPlaceholder(listOf("Hello darkness, my old friend")))
    }

    @Test
    fun gapsAppearWhereNobodySings() {
        val lines = addGaps(listOf(LyricLine(8f, 10f), LyricLine(11f, 12f), LyricLine(20f, 22f)))
        assertEquals(listOf(true, false, false, true, false), lines.map { it.gap })
    }

    /** Живая проверка источников: `YAMUSIC_LYRICS_LIVE=1 ./gradlew :app:testDebugUnitTest`. */
    @Test
    fun liveSourcesFindAKnownSong() {
        assumeTrue(System.getenv("YAMUSIC_LYRICS_LIVE") == "1")
        var token: Pair<String?, Long>? = null
        val sources = LyricsSources(OkHttpClient(), { token }, { value, expires -> token = value to expires })
        for ((title, artist, duration) in listOf(Triple("Creep", "Radiohead", 238f), Triple("Группа крови", "Кино", 286f))) {
            val result = sources.fetch(title, artist, "", duration) { null }
            println("$artist — $title: ${result.lyrics?.source} ${result.lyrics?.kind} строк ${result.lyrics?.lines?.size} ошибки ${result.details}")
            println("   " + result.lyrics?.lines?.take(3)?.joinToString(" | ") { line -> line.words.joinToString("") { it.text } })
            assertNotNull(result.lyrics)
        }
    }
}
