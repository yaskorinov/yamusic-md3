package io.github.yaskorinov.yamusic.api

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.jsonPrimitive

/** Идентификаторы приходят то числом, то строкой (у загруженных пользователем треков — UUID). */
object IdSerializer : KSerializer<String> {
    override val descriptor = PrimitiveSerialDescriptor("Id", PrimitiveKind.STRING)

    override fun deserialize(decoder: Decoder): String =
        (decoder as JsonDecoder).decodeJsonElement().jsonPrimitive.content

    override fun serialize(encoder: Encoder, value: String) = encoder.encodeString(value)
}

@Serializable
class Envelope<T>(val result: T? = null)

// --- вход -------------------------------------------------------------------

@Serializable
class DeviceCode(
    @SerialName("device_code") val deviceCode: String,
    @SerialName("user_code") val userCode: String,
    @SerialName("verification_url") val verificationUrl: String = "https://ya.ru/device",
    val interval: Int = 5,
    @SerialName("expires_in") val expiresIn: Int = 300,
)

@Serializable
class OAuthToken(@SerialName("access_token") val accessToken: String)

@Serializable
class AccountStatusDto(val account: AccountDto = AccountDto(), val plus: PlusDto = PlusDto())

@Serializable
class AccountDto(
    @Serializable(with = IdSerializer::class) val uid: String = "",
    val login: String = "",
    val displayName: String = "",
    val fullName: String = "",
)

@Serializable
class PlusDto(val hasPlus: Boolean = false)

// --- библиотека -------------------------------------------------------------

@Serializable
class LikesDto(val library: LikedLibraryDto = LikedLibraryDto())

@Serializable
class LikedLibraryDto(val tracks: List<TrackIdDto> = emptyList())

@Serializable
class TrackIdDto(
    @Serializable(with = IdSerializer::class) val id: String,
    @Serializable(with = IdSerializer::class) val albumId: String? = null,
)

@Serializable
class TrackDto(
    @Serializable(with = IdSerializer::class) val id: String,
    val title: String = "",
    val version: String = "",
    val artists: List<NamedDto> = emptyList(),
    val albums: List<AlbumRefDto> = emptyList(),
    val coverUri: String = "",
    val durationMs: Long = 0,
    val contentWarning: String = "",
    val available: Boolean = true,
)

@Serializable
class NamedDto(val name: String = "")

@Serializable
class AlbumRefDto(
    @Serializable(with = IdSerializer::class) val id: String = "",
    val title: String = "",
    val coverUri: String = "",
)

// --- воспроизведение --------------------------------------------------------

@Serializable
class FileInfoDto(val downloadInfo: DownloadInfoDto)

@Serializable
class DownloadInfoDto(
    val codec: String = "",
    val bitrate: Int = 0,
    val urls: List<String> = emptyList(),
    val url: String = "",
)

// --- плейлисты ---------------------------------------------------------------

@Serializable
class PlaylistDto(
    @Serializable(with = IdSerializer::class) val uid: String = "",
    @Serializable(with = IdSerializer::class) val kind: String = "",
    val title: String = "",
    val trackCount: Int = 0,
    val cover: PlaylistCoverDto = PlaylistCoverDto(),
    val ogImage: String = "",
    val owner: OwnerDto = OwnerDto(),
    val tracks: List<PlaylistItemDto> = emptyList(),
)

@Serializable
class PlaylistCoverDto(val uri: String = "", val itemsUri: List<String> = emptyList())

@Serializable
class OwnerDto(@Serializable(with = IdSerializer::class) val uid: String = "", val name: String = "", val login: String = "")

@Serializable
class PlaylistItemDto(@Serializable(with = IdSerializer::class) val id: String, val track: TrackDto? = null)

// --- волна -------------------------------------------------------------------

@Serializable
class RotorDto(
    val radioSessionId: String = "",
    val batchId: String = "",
    val sequence: List<RotorItemDto> = emptyList(),
    val unknownSession: Boolean = false,
)

@Serializable
class RotorItemDto(val track: TrackDto? = null)
