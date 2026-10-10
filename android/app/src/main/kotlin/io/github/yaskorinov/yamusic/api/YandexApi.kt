package io.github.yaskorinov.yamusic.api

import java.io.IOException
import java.security.SecureRandom
import java.util.Base64
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.random.Random
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import okhttp3.Call
import okhttp3.Callback
import okhttp3.FormBody
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response

/** Ошибка API: [status] — код HTTP, [code] — имя ошибки из ответа ('authorization_pending', 'session-expired'…). */
class ApiException(val status: Int, val code: String, message: String) : IOException(message) {
    val unauthorized: Boolean get() = status == 401 || status == 403
}

/**
 * Клиент API Яндекс Музыки.
 *
 * Яндекс отвечает 429 «Concurrency limit exceeded», если с одним токеном одновременно идёт слишком
 * много запросов, поэтому разом уходит не больше [MAX_CONCURRENT], а 429 и 5xx повторяются
 * с нарастающей паузой.
 */
class YandexApi(private val http: OkHttpClient) {
    @Volatile
    var token: String? = null

    private val gate = Semaphore(MAX_CONCURRENT)
    private val json = Json {
        ignoreUnknownKeys = true
        coerceInputValues = true
    }

    // --- вход: OAuth Device Flow (код на ya.ru/device) --------------------------

    suspend fun requestDeviceCode(deviceName: String): DeviceCode = oauth(
        "device/code",
        mapOf("client_id" to CLIENT_ID, "device_id" to randomId(), "device_name" to deviceName),
        DeviceCode.serializer(),
    )

    /** null — пользователь ещё не подтвердил вход. */
    suspend fun pollDeviceToken(deviceCode: String): String? = try {
        oauth(
            "token",
            mapOf(
                "grant_type" to "device_code",
                "code" to deviceCode,
                "client_id" to CLIENT_ID,
                "client_secret" to CLIENT_SECRET,
            ),
            OAuthToken.serializer(),
        ).accessToken
    } catch (e: ApiException) {
        if (e.code == "authorization_pending") null else throw e
    }

    // --- аккаунт и библиотека -----------------------------------------------------

    suspend fun accountStatus(): AccountStatusDto = call(AccountStatusDto.serializer(), request("/account/status"))

    /** Полные идентификаторы ('id:albumId') в порядке «новые сверху». */
    suspend fun likedTrackIds(uid: String): List<String> =
        call(LikesDto.serializer(), request("/users/$uid/likes/tracks")).library.tracks.map {
            if (it.albumId.isNullOrEmpty()) it.id else "${it.id}:${it.albumId}"
        }

    suspend fun tracks(ids: List<String>): List<Track> {
        if (ids.isEmpty()) return emptyList()
        val form = mapOf("track-ids" to ids.joinToString(","), "with-positions" to "True")
        return call(ListSerializer(TrackDto.serializer()), request("/tracks", form = form)).map { it.toTrack() }
    }

    suspend fun setLiked(uid: String, fullId: String, liked: Boolean) {
        val action = if (liked) "add-multiple" else "remove"
        call(JsonElement.serializer(), request("/users/$uid/likes/tracks/$action", form = mapOf("track-ids" to fullId)))
    }

    // --- воспроизведение ----------------------------------------------------------

    /**
     * Прямая ссылка на файл трека. С transport=raw FLAC приходит незашифрованным.
     * Подпись проверяется вместе с заголовком клиента, к которому привязан ключ.
     */
    suspend fun streamInfo(trackId: String, quality: String): StreamInfo {
        val ts = (System.currentTimeMillis() / 1000).toString()
        val mac = Mac.getInstance("HmacSHA256").apply { init(SecretKeySpec(SIGN_KEY.toByteArray(), "HmacSHA256")) }
        val message = ts + trackId + quality + CODECS.joinToString("") + TRANSPORT
        val sign = Base64.getEncoder().encodeToString(mac.doFinal(message.toByteArray())).dropLast(1)
        val query = mapOf(
            "ts" to ts,
            "trackId" to trackId,
            "quality" to quality,
            "codecs" to CODECS.joinToString(","),
            "transports" to TRANSPORT,
            "sign" to sign,
        )
        val info = call(FileInfoDto.serializer(), request("/get-file-info", query, client = SIGN_CLIENT)).downloadInfo
        val url = info.urls.firstOrNull() ?: info.url
        if (url.isEmpty()) throw ApiException(200, "no-url", "Яндекс не дал ссылку на трек")
        return StreamInfo(url, info.codec, info.bitrate)
    }

    // --- внутреннее -----------------------------------------------------------------

    private fun request(
        path: String,
        query: Map<String, String> = emptyMap(),
        form: Map<String, String>? = null,
        client: String = CLIENT,
    ): Request {
        val url = (BASE_URL + path).toHttpUrl().newBuilder()
        query.forEach { (key, value) -> url.addQueryParameter(key, value) }
        val request = Request.Builder().url(url.build())
            .header("User-Agent", USER_AGENT)
            .header("X-Yandex-Music-Client", client)
            .header("Accept-Language", "ru")
        token?.let { request.header("Authorization", "OAuth $it") }
        form?.let { request.post(formBody(it)) }
        return request.build()
    }

    private suspend fun <T> call(serializer: KSerializer<T>, request: Request): T = send(request) { body ->
        json.decodeFromString(Envelope.serializer(serializer), body).result
            ?: throw ApiException(200, "empty", "Пустой ответ Яндекс Музыки")
    }

    private suspend fun <T> oauth(path: String, form: Map<String, String>, serializer: KSerializer<T>): T {
        val request = Request.Builder().url("$OAUTH_URL/$path").header("User-Agent", USER_AGENT).post(formBody(form))
        return send(request.build()) { json.decodeFromString(serializer, it) }
    }

    private suspend fun <T> send(request: Request, parse: (String) -> T): T {
        var pause = 800L
        repeat(RETRIES + 1) { attempt ->
            val (status, body) = gate.withPermit { execute(request) }
            if (status in 200..299) return parse(body)
            val retryable = status == 429 || status in 500..504
            if (!retryable || attempt == RETRIES) throw failure(status, body)
            delay((pause * (1 + 0.25 * Random.nextDouble())).toLong())
            pause *= 2
        }
        error("unreachable")
    }

    private suspend fun execute(request: Request): Pair<Int, String> = suspendCancellableCoroutine { cont ->
        val call = http.newCall(request)
        cont.invokeOnCancellation { call.cancel() }
        call.enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) = cont.resumeWithException(e)

            override fun onResponse(call: Call, response: Response) {
                try {
                    cont.resume(response.use { it.code to it.body.string() })
                } catch (e: IOException) {
                    cont.resumeWithException(e)
                }
            }
        })
    }

    /** Имя и текст ошибки: у OAuth — строка в error, у API музыки — объект { name, message }. */
    private fun failure(status: Int, body: String): ApiException {
        val root = runCatching { json.parseToJsonElement(body).jsonObject }.getOrNull()
        val error = root?.get("error")
        val (code, text) = when (error) {
            is JsonPrimitive -> error.content to (root["error_description"] as? JsonPrimitive)?.content.orEmpty()
            is JsonObject -> (error["name"] as? JsonPrimitive)?.content.orEmpty() to
                (error["message"] as? JsonPrimitive)?.content.orEmpty()
            else -> "" to ""
        }
        return ApiException(status, code, text.ifEmpty { code }.ifEmpty { "HTTP $status" })
    }

    private fun formBody(fields: Map<String, String>): FormBody =
        FormBody.Builder().apply { fields.forEach { (key, value) -> add(key, value) } }.build()

    private fun randomId(): String {
        val alphabet = ('a'..'z') + ('A'..'Z') + ('0'..'9')
        val random = SecureRandom()
        return (1..10).map { alphabet[random.nextInt(alphabet.size)] }.joinToString("")
    }

    private companion object {
        const val BASE_URL = "https://api.music.yandex.net"
        const val OAUTH_URL = "https://oauth.yandex.ru"
        const val USER_AGENT = "Yandex-Music-API"
        const val CLIENT = "YandexMusicAndroid/24023621"

        // client_id и секрет Android-приложения Яндекс Музыки — те же, что у библиотеки yandex-music
        const val CLIENT_ID = "23cabbbdc6cd418abb4b39c32c41195d"
        const val CLIENT_SECRET = "53bc75238f0c4d08a118e51fe9203300"

        // Ключ подписи get-file-info привязан к заголовку десктопного клиента
        const val SIGN_KEY = "kzqU4XhfCaY6B6JTHODeq5"
        const val SIGN_CLIENT = "YandexMusicDesktopAppWindows/5.95.0"
        const val TRANSPORT = "raw"
        val CODECS = listOf("flac", "flac-mp4", "aac-mp4", "he-aac-mp4", "aac", "he-aac", "mp3")

        const val MAX_CONCURRENT = 4
        const val RETRIES = 5 // паузы 0,8 → 12,8 с: всего до ~25 с
    }
}
