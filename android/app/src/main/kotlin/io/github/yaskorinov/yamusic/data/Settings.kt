package io.github.yaskorinov.yamusic.data

import android.content.Context

/** Настройки приложения (обычный файл настроек; токена здесь нет — он в [TokenStore]). */
class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    /** Выбранные настройки волны: сиды через запятую. */
    var waveSeeds: String
        get() = prefs.getString("waveSeeds", "").orEmpty()
        set(value) = prefs.edit().putString("waveSeeds", value).apply()

    /**
     * Для проверок на настоящем аккаунте: не писать в историю и не слать обратную связь волне.
     * Включается только в отладочной сборке: `adb shell am start -n <пакет>/.MainActivity --ez noReport true`.
     */
    var noReport: Boolean
        get() = prefs.getBoolean("noReport", false)
        set(value) = prefs.edit().putBoolean("noReport", value).apply()
}
