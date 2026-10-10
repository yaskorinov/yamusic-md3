package io.github.yaskorinov.yamusic.data

import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Одна настройка: значение можно читать, менять и наблюдать. */
class Pref<T>(
    private val prefs: SharedPreferences,
    private val key: String,
    default: T,
    read: SharedPreferences.(String, T) -> T,
    private val write: SharedPreferences.Editor.(String, T) -> Unit,
) {
    private val state = MutableStateFlow(prefs.read(key, default))
    val flow: StateFlow<T> = state.asStateFlow()

    var value: T
        get() = state.value
        set(value) {
            state.value = value
            prefs.edit().apply { write(key, value) }.apply()
        }
}

/** Настройки приложения (обычный файл настроек; токена здесь нет — он в [TokenStore]). */
class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    private fun string(key: String, default: String) =
        Pref(prefs, key, default, { k, d -> getString(k, d) ?: d }, { k, v -> putString(k, v) })

    private fun bool(key: String, default: Boolean) =
        Pref(prefs, key, default, { k, d -> getBoolean(k, d) }, { k, v -> putBoolean(k, v) })

    private fun float(key: String, default: Float) =
        Pref(prefs, key, default, { k, d -> getFloat(k, d) }, { k, v -> putFloat(k, v) })

    private fun int(key: String, default: Int) =
        Pref(prefs, key, default, { k, d -> getInt(k, d) }, { k, v -> putInt(k, v) })

    /** auto — без потерь по Wi-Fi и AAC по мобильной сети; lossless | hq | nq — всегда так. */
    val quality = string("quality", "auto")

    // --- оформление: те же настройки и значения по умолчанию, что в десктопном клиенте ---

    /** system | dark | light */
    val themeMode = string("themeMode", "dark")

    /** Цвета приложения — из обложки играющего трека. */
    val accentFromCover = bool("accentFromCover", true)

    /** Акцентный цвет (#RRGGBB): когда ничего не играет или цвет из обложки выключен. */
    val customSeed = string("customSeed", "#FFCC00")

    /** Вариант цветовой схемы: content | tonalSpot | vibrant | expressive | fidelity | neutral | monochrome. */
    val schemeVariant = string("schemeVariant", "content")

    /** Смена цветов и фона при смене трека: wipe | ripple | dissolve | liquid. */
    val trackTransition = string("trackTransition", "wipe")

    /** Характер размытия фона из обложки: gauss | glass | palette | blobs. */
    val backdropMode = string("backdropMode", "gauss")

    /** Размытие фона полноэкранного плеера, 0..1. */
    val nowPlayingBlur = float("nowPlayingBlur", 1f)

    /** «Плавание» размытия: скорость перелива 0..1, 0 — неподвижно. */
    val nowPlayingDrift = float("nowPlayingDrift", 0.5f)

    /** Размытая обложка играющего трека за всем приложением. */
    val ambientBackground = bool("ambientBackground", true)

    /** Рамка обложки медленно вращается, пока играет музыка. */
    val coverSpin = bool("coverSpin", true)

    /** Обложка в полноэкранном плеере «дышит» в такт. */
    val coverPulse = bool("coverPulse", true)

    /** Плавный переход между треками, секунды; 0 — встык, без пауз. */
    val crossfade = int("crossfade", 0)

    /** Скачивать треки только по безлимитной сети (Wi-Fi). */
    val downloadOnWifiOnly = bool("downloadOnWifiOnly", true)

    /** Выбранные настройки волны: сиды через запятую. */
    val waveSeeds = string("waveSeeds", "")

    /**
     * Для проверок на настоящем аккаунте: не писать в историю и не слать обратную связь волне.
     * Включается только в отладочной сборке: `adb shell am start -n <пакет>/.MainActivity --ez noReport true`.
     */
    val noReport = bool("noReport", false)
}
