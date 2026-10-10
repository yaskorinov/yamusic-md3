package io.github.yaskorinov.yamusic.lyrics

import java.io.IOException
import java.security.MessageDigest
import java.text.Normalizer
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import okhttp3.FormBody
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request

// Источники синхронного текста — перенос lyrics_sources.py десктопного клиента (а тот — из плагина
// Word Lyrics того же автора). Цепочка: NetEase YRC (по словам) → Musixmatch richsync (по словам) →
// LRCLIB (по строкам) → Musixmatch subtitles → NetEase LRC → Яндекс LRC. У построчных текстов время
// слов интерполируется по длине. Всё синхронное — вызывается из фонового потока.

@Serializable
class LyricWord(val text: String, val start: Float, val end: Float)

/** Строка текста или пауза «• • •» ([gap]) между строками. Время — секунды от начала трека. */
@Serializable
class LyricLine(
    val start: Float,
    val end: Float,
    val words: List<LyricWord> = emptyList(),
    val gap: Boolean = false,
)

/** [kind]: word — известно время каждого слова, line — только строк (слова интерполированы). */
@Serializable
class Lyrics(val source: String = "", val kind: String = "", val lines: List<LyricLine> = emptyList())

/** [details] — ошибки источников: с ними «не найдено» ненадёжно и в кэш не идёт. */
class LyricsResult(val lyrics: Lyrics?, val details: List<String>)

class LyricsSources(
    private val http: OkHttpClient,
    /** Токен Musixmatch живёт несколько часов — хранит его вызывающий. */
    private val loadMxmToken: () -> Pair<String?, Long>?,
    private val saveMxmToken: (String?, Long) -> Unit,
) {
    private val json = Json { ignoreUnknownKeys = true }

    fun fetch(title: String, artist: String, album: String, duration: Float, yandexLrc: () -> String?): LyricsResult {
        val errors = ArrayList<String>()
        fun <T> attempt(name: String, block: () -> T?): T? = try {
            block()
        } catch (e: Exception) {
            errors += "$name: ${e.message}"
            null
        }

        // 1. NetEase YRC (по словам)
        val neSong = attempt("netease-search") { neSearch(title, artist, duration) }
        var neData: JsonObject? = null
        if (neSong != null) {
            neData = attempt("netease-lyric") { neLyric(neSong) }
            val yrc = neData.obj("yrc").text("lyric")
            if (yrc.isNotEmpty()) {
                val lines = parseYrc(yrc)
                if (lines.isNotEmpty() && !isPlaceholder(lines.map { line -> line.words.joinToString("") { it.text } })) {
                    return LyricsResult(Lyrics("NetEase", "word", lines), errors)
                }
            }
        }

        // 2. Musixmatch richsync (по словам) — заодно даёт построчный запасной вариант
        var mxmLines: List<LyricLine>? = null
        val token = attempt("musixmatch-token") { mxmToken() }
        if (token != null) {
            val found = attempt("musixmatch") { mxmFetch(title, artist, duration, token) }
            if (found != null && found.first == "word") return LyricsResult(Lyrics("Musixmatch", "word", found.second), errors)
            mxmLines = found?.second
        }

        // 3. LRCLIB (по строкам)
        val lrc = attempt("lrclib") { lrclibFetch(title, artist, album, duration) }
        if (lrc != null) {
            val lines = buildFromTimedLines(linesFromLrc(lrc), duration)
            if (lines.isNotEmpty()) return LyricsResult(Lyrics("LRCLIB", "line", lines), errors)
        }

        // 4. Musixmatch subtitles
        if (!mxmLines.isNullOrEmpty()) return LyricsResult(Lyrics("Musixmatch", "line", mxmLines), errors)

        // 5. NetEase LRC
        val neLrc = neData.obj("lrc").text("lyric")
        if (neLrc.isNotEmpty()) {
            val pairs = linesFromLrc(neLrc)
            val lines = if (isPlaceholder(pairs.map { it.second })) emptyList() else buildFromTimedLines(pairs, duration)
            if (lines.isNotEmpty()) return LyricsResult(Lyrics("NetEase", "line", lines), errors)
        }

        // 6. Яндекс — последним: синхронизация у него грубее, но русскую музыку он знает почти всю
        val yandex = attempt("yandex", yandexLrc)
        if (yandex != null) {
            val lines = buildFromTimedLines(linesFromLrc(yandex), duration)
            if (lines.isNotEmpty()) return LyricsResult(Lyrics("Яндекс", "line", lines), errors)
        }
        return LyricsResult(null, errors)
    }

    // ── HTTP ─────────────────────────────────────────────────────────────────

    private class HttpStatus(val code: Int) : IOException("HTTP $code")

    private fun get(url: String, headers: Map<String, String> = emptyMap(), form: Map<String, String>? = null): String {
        var last: IOException? = null
        repeat(RETRIES + 1) { attempt ->
            val request = Request.Builder().url(url).header("User-Agent", UA)
            headers.forEach { (key, value) -> request.header(key, value) }
            form?.let { fields -> request.post(FormBody.Builder().apply { fields.forEach { (k, v) -> add(k, v) } }.build()) }
            try {
                http.newCall(request.build()).execute().use { response ->
                    if (response.isSuccessful) return response.body.string()
                    if (response.code < 500) throw HttpStatus(response.code)
                    last = HttpStatus(response.code)
                }
            } catch (e: HttpStatus) {
                throw e
            } catch (e: IOException) {
                last = e
            }
            if (attempt < RETRIES) Thread.sleep(700L * (attempt + 1))
        }
        throw last ?: IOException("нет ответа")
    }

    private fun getJson(url: String, headers: Map<String, String> = emptyMap(), form: Map<String, String>? = null): JsonElement =
        json.parseToJsonElement(get(url, headers, form))

    private fun url(base: String, params: Map<String, String>): String =
        base.toHttpUrl().newBuilder().apply { params.forEach { (key, value) -> addQueryParameter(key, value) } }.build().toString()

    // ── NetEase ──────────────────────────────────────────────────────────────

    /** Идентификатор песни на NetEase или null. */
    private fun neSearch(title: String, artist: String, duration: Float): String? {
        val query = "${cleanTitle(title)} ${primaryArtist(artist)}"
        val found = getJson(
            url("https://music.163.com/api/cloudsearch/pc", mapOf("s" to query, "type" to "1", "limit" to "15", "offset" to "0")),
            mapOf("Referer" to "https://music.163.com"),
        )
        var best: String? = null
        var bestScore = -1f
        for (song in found.obj("result").list("songs")) {
            val name = song.text("name")
            val artists = song.list("ar").map { it.text("name") }
            val length = song.number("dt") / 1000f
            if (!similar(cleanTitle(name), cleanTitle(title))) continue
            val artistOk = artists.any { similar(it, primaryArtist(artist)) } || similar(artists.joinToString(" "), artist)
            val diff = if (duration > 0) abs(length - duration) else 0f
            if (duration > 0 && diff > 6) continue
            // «Song (Remix)», когда играет «Song»: другой тайминг
            if ((versionTags(name) - versionTags(title)).isNotEmpty()) continue
            if (!artistOk && duration <= 0) continue
            val score = (if (artistOk) 10 else 0) + (if (norm(name) == norm(title)) 5 else 0) + max(0f, 5 - diff)
            if (score > bestScore) {
                best = song.text("id")
                bestScore = score
            }
        }
        return best
    }

    private fun neLyric(songId: String): JsonObject {
        val path = "/api/song/lyric/v1"
        val params = buildJsonObject {
            put("id", songId)
            put("cp", "false")
            for (key in listOf("lv", "kv", "tv", "rv", "yv", "ytv", "yrv")) put(key, "0")
            put("csrf_token", "")
        }.toString()
        val digest = md5("nobody${path}use${params}md5forencrypt")
        val data = "$path-36cd479b6b5-$params-36cd479b6b5-$digest".toByteArray()
        val cipher = Cipher.getInstance("AES/ECB/PKCS5Padding")
        cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(NE_KEY.toByteArray(), "AES"))
        val encrypted = cipher.doFinal(data).joinToString("") { "%02X".format(it) }
        return getJson(
            "https://interface3.music.163.com/eapi/song/lyric/v1",
            mapOf("Referer" to "https://music.163.com", "Cookie" to "os=pc; appver=2.10.13"),
            form = mapOf("params" to encrypted),
        ) as? JsonObject ?: JsonObject(emptyMap())
    }

    private fun parseYrc(yrc: String): List<LyricLine> {
        val lines = ArrayList<LyricLine>()
        for (raw in yrc.lines()) {
            val match = YRC_LINE.matchEntire(raw.trim()) ?: continue // строки-титры в JSON
            val start = match.groupValues[1].toLong() / 1000f
            val length = match.groupValues[2].toLong() / 1000f
            val syllables = YRC_WORD.findAll(match.groupValues[3]).map {
                val from = it.groupValues[1].toLong()
                Syllable(it.groupValues[3], from / 1000f, (from + it.groupValues[2].toLong()) / 1000f)
            }.toList()
            val text = syllables.joinToString("") { it.text }.trim()
            if (text.isEmpty() || META_LINE.containsMatchIn(text)) continue
            val words = groupSyllables(syllables)
            if (words.isEmpty()) continue
            lines += LyricLine(start, max(start + length, words.last().end), words)
        }
        return lines
    }

    // ── Musixmatch ───────────────────────────────────────────────────────────

    private fun mxmToken(): String? {
        val now = System.currentTimeMillis()
        loadMxmToken()?.let { (token, expires) -> if (expires > now) return token } // null — заблокированы, повтор позже
        val token = try {
            getJson("${MXM_BASE}token.get?app_id=web-desktop-app-v1.0&format=json", MXM_HEADERS)
                .obj("message").obj("body").text("user_token").takeIf { it.isNotEmpty() && it.any { c -> c != '0' } }
        } catch (e: IOException) {
            null
        }
        saveMxmToken(token, now + if (token != null) 6 * 3600_000L else 3600_000L)
        return token
    }

    private fun mxmCall(method: String, token: String, params: Map<String, String>): JsonObject? {
        val all = params + mapOf("app_id" to "web-desktop-app-v1.0", "format" to "json", "usertoken" to token)
        return getJson(url(MXM_BASE + method, all), MXM_HEADERS).obj("message")
    }

    /** ("word" | "line", строки) или null. */
    private fun mxmFetch(title: String, artist: String, duration: Float, token: String): Pair<String, List<LyricLine>>? {
        val message = mxmCall(
            "macro.subtitles.get", token,
            mapOf(
                "q_track" to cleanTitle(title),
                "q_artist" to artist,
                "q_duration" to if (duration > 0) duration.toInt().toString() else "",
                "namespace" to "lyrics_richsynched",
                "subtitle_format" to "mxm",
                "optional_calls" to "track.richsync",
            ),
        )
        val calls = message.obj("body").obj("macro_calls")
        val track = calls.obj("matcher.track.get").obj("message").obj("body").obj("track") ?: return null
        if (track.flag("has_richsync")) {
            val rich = mxmCall(
                "track.richsync.get", token,
                mapOf("commontrack_id" to track.text("commontrack_id"), "f_richsync_length" to track.text("track_length")),
            )
            val body = rich.obj("body").obj("richsync").text("richsync_body")
            if (body.isNotEmpty()) {
                val lines = ArrayList<LyricLine>()
                for (line in json.parseToJsonElement(body) as? JsonArray ?: JsonArray(emptyList())) {
                    val start = line.number("ts")
                    val end = line.number("te")
                    val chunks = line.list("l")
                    val syllables = ArrayList<Syllable>()
                    chunks.forEachIndexed { index, chunk ->
                        val text = chunk.text("c")
                        val from = start + chunk.number("o")
                        val to = if (index + 1 < chunks.size) start + chunks[index + 1].number("o") else end
                        if (text.isBlank() && syllables.isNotEmpty()) {
                            val last = syllables.removeAt(syllables.lastIndex)
                            syllables += Syllable(last.text + text, last.start, last.end)
                        } else if (text.isNotEmpty()) {
                            syllables += Syllable(text, from, to)
                        }
                    }
                    val words = groupSyllables(syllables)
                    if (words.isNotEmpty()) lines += LyricLine(start, end, words)
                }
                if (lines.isNotEmpty()) return "word" to lines
            }
        }
        val subtitles = calls.obj("track.subtitles.get").obj("message").obj("body").list("subtitle_list")
        val body = subtitles.firstOrNull().obj("subtitle").text("subtitle_body")
        if (body.isNotEmpty()) {
            val pairs = (json.parseToJsonElement(body) as? JsonArray).orEmpty().map { it.obj("time").number("total") to it.text("text") }
            return "line" to buildFromTimedLines(pairs, duration)
        }
        return null
    }

    // ── LRCLIB ───────────────────────────────────────────────────────────────

    private fun lrclibFetch(title: String, artist: String, album: String, duration: Float): String? {
        val headers = mapOf("Lrclib-Client" to "yamusic")
        val params = buildMap {
            put("track_name", title)
            put("artist_name", artist)
            if (album.isNotEmpty()) put("album_name", album)
            if (duration > 0) put("duration", duration.roundToInt().toString())
        }
        try {
            val synced = getJson(url("https://lrclib.net/api/get", params), headers).text("syncedLyrics")
            if (synced.isNotEmpty()) return synced
        } catch (e: HttpStatus) {
            if (e.code != 404) throw e
        }
        val query = mapOf("track_name" to cleanTitle(title), "artist_name" to primaryArtist(artist))
        for (item in getJson(url("https://lrclib.net/api/search", query), headers) as? JsonArray ?: JsonArray(emptyList())) {
            val synced = item.text("syncedLyrics")
            if (synced.isEmpty()) continue
            if (duration > 0 && abs(item.number("duration") - duration) > 5) continue
            return synced
        }
        return null
    }

    private companion object {
        const val UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36"
        const val RETRIES = 2
        const val NE_KEY = "e82ckenh8dichen8"
        const val MXM_BASE = "https://apic-desktop.musixmatch.com/ws/1.1/"
        val MXM_HEADERS = mapOf("authority" to "apic-desktop.musixmatch.com", "cookie" to "x-mxm-token-guid=")
        val YRC_LINE = Regex("""^\[(\d+),(\d+)\](.*)$""")
        val YRC_WORD = Regex("""\((\d+),(\d+),\d+\)([^(]*)""")
    }
}

// ── разбор и сопоставление ─────────────────────────────────────────────────────

private class Syllable(val text: String, val start: Float, val end: Float)

private const val GAP_MIN = 4.5f // секунд тишины, после которых показывается пауза «• • •»

private val PAREN = Regex("""\s*[(\[{（【].*?[)\]}）】]""")
private val FEAT = Regex("""\s+(feat\.?|ft\.?|featuring|при уч\.?)\s+.*$""", RegexOption.IGNORE_CASE)
private val DASH_SUFFIX =
    Regex("""\s+[-–—]\s+(.*(remaster|version|edit|live|mix|mono|stereo|ремастер|версия).*)$""", RegexOption.IGNORE_CASE)
private val ARTIST_SPLIT = Regex("""\s*(?:,|&|/|;| feat\.? | ft\.? | x | и )\s*""", RegexOption.IGNORE_CASE)
private val NON_WORD = Regex("""[^\p{L}\p{N}]+""") // явный класс: \W вне Android не знает кириллицу
private val COMBINING = Regex("""\p{M}+""")

private val META_LINE = Regex(
    """^\s*(作词|作曲|编曲|制作人|制作|混音|母带|和声|吉他|贝斯|鼓|录音|监制|出品|发行|词|曲|""" +
        """lyrics|lyricist|composer|composed|producer|produced|arranger|written|mixed|mastered|vocals?)\s*[:：]""",
    RegexOption.IGNORE_CASE,
)

// NetEase вместо «текста нет» отдаёт заглушку-строку: «纯音乐，请欣赏» (инструментал, приятного
// прослушивания), «暂无歌词» (текста пока нет), «此歌曲为没有填词的纯音乐» и т. п.
private val PLACEHOLDER = Regex(
    "纯音乐|純音樂|暂无歌词|暫無歌詞|没有填词|沒有填詞|无歌词|無歌詞|请欣赏|請欣賞|歌词贡献者|翻译贡献者|暂时没有歌词|此歌曲为",
)
private val CJK_END = Regex("""[぀-ヿ㐀-鿿가-힯]$""")
private val LRC_TAG = Regex("""\[(\d+):(\d+(?:[.:]\d+)?)\]""")
private val LRC_WORD_TAG = Regex("""<\d+:\d+(?:[.:]\d+)?>""")
private val TOKEN = Regex("""\S+\s*""")

private val VERSION_WORDS = listOf(
    "remix", "live", "acoustic", "instrumental", "karaoke", "cover", "sped up", "slowed", "nightcore", "demo",
    "edit", "version", "mix", "ремикс", "лайв", "акустика", "минус", "伴奏",
)

private fun cleanTitle(title: String): String =
    title.replace(DASH_SUFFIX, "").replace(PAREN, "").replace(FEAT, "").trim().ifEmpty { title }

private fun primaryArtist(artist: String): String = artist.split(ARTIST_SPLIT).first().trim().ifEmpty { artist }

private fun norm(text: String): String =
    Normalizer.normalize(text.lowercase(), Normalizer.Form.NFKD).replace(COMBINING, "").replace('ё', 'е').replace(NON_WORD, "")

private fun similar(a: String, b: String): Boolean {
    val x = norm(a)
    val y = norm(b)
    return x.isNotEmpty() && y.isNotEmpty() && (x == y || x in y || y in x)
}

private fun versionTags(text: String): Set<String> = text.lowercase().let { lower -> VERSION_WORDS.filterTo(HashSet()) { it in lower } }

/** Строки — не текст песни, а заглушка «текста нет»: тогда источник пропускается. */
internal fun isPlaceholder(texts: List<String>): Boolean {
    val lines = texts.map { it.trim() }.filter { it.isNotEmpty() }
    if (lines.isEmpty()) return true
    val marked = lines.count { PLACEHOLDER.containsMatchIn(it) }
    // заглушка обычно одна-две строки; в настоящем китайском тексте такие слова — редкость
    if (marked > 0) return lines.size <= 4 || marked * 2 >= lines.size
    // одна-две строки целиком из иероглифов — тоже заглушка, а не песня
    return lines.size <= 2 && lines.all { line -> line.all { !it.isLetter() || it in '\u4e00'..'\u9fff' || it in '\u3400'..'\u4dbf' } }
}

/** Слоги → слова: слово кончается там, где слог кончается пробелом, и на каждом иероглифе. */
private fun groupSyllables(syllables: List<Syllable>): List<LyricWord> {
    val words = ArrayList<LyricWord>()
    val current = ArrayList<Syllable>()
    fun flush() {
        if (current.isEmpty()) return
        words += LyricWord(current.joinToString("") { it.text }, current.first().start, current.last().end)
        current.clear()
    }
    for (syllable in syllables) {
        if (syllable.text.isEmpty()) continue
        current += syllable
        if (syllable.text.last().isWhitespace() || CJK_END.containsMatchIn(syllable.text)) flush()
    }
    flush()
    return words
}

private fun interpolateWords(text: String, start: Float, end: Float): List<LyricWord> {
    val tokens = TOKEN.findAll(text).map { it.value }.toList()
    if (tokens.isEmpty()) return emptyList()
    val weights = tokens.map { it.trim().length + 1.5f }
    val total = weights.sum()
    var at = start
    return tokens.mapIndexed { index, token ->
        val length = (end - start) * weights[index] / total
        LyricWord(token, at, at + length).also { at += length }
    }
}

/** [mm:ss.xx] LRC → пары (время, текст) по возрастанию времени. */
internal fun linesFromLrc(lrc: String): List<Pair<Float, String>> {
    val out = ArrayList<Pair<Float, String>>()
    for (raw in lrc.lines()) {
        val stamps = LRC_TAG.findAll(raw).toList()
        if (stamps.isEmpty()) continue
        val text = raw.replace(LRC_TAG, "").replace(LRC_WORD_TAG, "").trim()
        for (stamp in stamps) {
            val seconds = stamp.groupValues[2].replace(':', '.').toFloatOrNull() ?: continue
            out += (stamp.groupValues[1].toInt() * 60 + seconds) to text
        }
    }
    return out.sortedBy { it.first }
}

/** Построчный текст → строки с интерполированным временем слов. */
internal fun buildFromTimedLines(pairs: List<Pair<Float, String>>, duration: Float): List<LyricLine> {
    val kept = pairs.filterNot { META_LINE.containsMatchIn(it.second) }
    val lines = ArrayList<LyricLine>()
    kept.forEachIndexed { index, (start, text) ->
        if (text.isEmpty()) return@forEachIndexed
        val next = kept.getOrNull(index + 1)?.first ?: if (duration > start) duration else start + 6
        val sung = (text.length * 0.085f + 0.4f).coerceIn(1.2f, 9f)
        var end = min(next - 0.05f, start + sung)
        if (end <= start) end = start + 0.5f
        lines += LyricLine(start, end, interpolateWords(text, start, end))
    }
    return lines
}

/** Вставить паузы «• • •» там, где долго не поют. */
fun addGaps(lines: List<LyricLine>): List<LyricLine> {
    val out = ArrayList<LyricLine>()
    var previousEnd = 0f
    for (line in lines) {
        if (line.start - previousEnd >= GAP_MIN) {
            out += LyricLine(previousEnd + if (previousEnd > 0) 0.3f else 0f, line.start - 0.15f, gap = true)
        }
        out += line
        previousEnd = max(previousEnd, line.end)
    }
    return out
}

private fun md5(text: String): String =
    MessageDigest.getInstance("MD5").digest(text.toByteArray()).joinToString("") { "%02x".format(it) }

// ── JSON без схемы: чужие API меняются, читаем терпимо ─────────────────────────

private fun JsonElement?.obj(key: String): JsonObject? = (this as? JsonObject)?.get(key) as? JsonObject
private fun JsonElement?.list(key: String): List<JsonElement> = ((this as? JsonObject)?.get(key) as? JsonArray).orEmpty()
private fun JsonElement?.text(key: String): String = ((this as? JsonObject)?.get(key) as? JsonPrimitive)?.content.orEmpty()
    .let { if (it == "null") "" else it }
private fun JsonElement?.number(key: String): Float = text(key).toFloatOrNull() ?: 0f
private fun JsonElement?.flag(key: String): Boolean = text(key).let { it == "true" || it == "1" }
