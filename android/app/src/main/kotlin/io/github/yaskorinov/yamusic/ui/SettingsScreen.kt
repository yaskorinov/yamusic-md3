package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.App
import io.github.yaskorinov.yamusic.data.Pref
import kotlin.math.roundToInt

private val THEMES = listOf("system", "light", "dark")
private val THEME_SEGMENTS = listOf(Segment("Системная", "contrast"), Segment("Светлая", "light_mode"), Segment("Тёмная", "dark_mode"))

private val QUALITIES = listOf("auto", "lossless", "hq", "nq")
private val QUALITY_SEGMENTS = listOf(Segment("Авто"), Segment("Без потерь"), Segment("Высокое"), Segment("Обычное"))
private val QUALITY_HINTS = mapOf(
    "auto" to "FLAC по Wi-Fi, AAC по мобильной сети",
    "lossless" to "FLAC, если трек есть без потерь; иначе — AAC. Около 25 МБ на трек",
    "hq" to "AAC или MP3 высокого битрейта",
    "nq" to "Низкий битрейт — экономит трафик",
)

private val ACCENTS = listOf("#FFCC00", "#FF6D00", "#E8175D", "#9C4DFF", "#6750A4", "#00A3FF", "#00BFA5", "#1DB954")

/** Способ → название и пояснение — те же, что в настройках десктопного клиента. */
private val TRANSITIONS = listOf(
    Triple("wipe", "Шторка", "Новые цвета и фон въезжают сбоку: слева направо — следующий трек, справа налево — предыдущий"),
    Triple("ripple", "Волна от обложки", "Новые цвета и фон расходятся от обложки фигурой с волнистым краем"),
    Triple("dissolve", "Растворение", "Старые цвета и фон спокойно тают, проявляя новые"),
    Triple("liquid", "Перетекание", "Старая картинка растекается и тает неровной волной в сторону перехода"),
)
private val BACKDROPS = listOf(
    Triple("gauss", "Мягкое", "Ровное размытие без пятен и полос: обложка плавно растворяется в цвете"),
    Triple("glass", "Матовое стекло", "Обложка угадывается, как за матовым стеклом; поверх — мелкое зерно"),
    Triple("palette", "Градиент", "Без обложки: плавный градиент из цветов темы"),
    Triple("blobs", "Пятна", "Крупные цветовые пятна"),
)

/** Настройки (перенос SettingsPage.qml): группы-карточки с заголовком и значком, строки «подпись — контрол». */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun SettingsScreen(app: App, contentPadding: PaddingValues, onBack: () -> Unit) {
    val settings = app.settings
    val account = app.session.account
    val colors = MaterialTheme.colorScheme
    var confirmSignOut by remember { mutableStateOf(false) }
    var confirmClear by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize().padding(top = contentPadding.calculateTopPadding())) {
        TopBar("Настройки", onBack = onBack)
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)
                .padding(bottom = contentPadding.calculateBottomPadding() + 16.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            SettingsCard("Аккаунт", "person") {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Avatar(account?.displayName.orEmpty(), size = 48.dp)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(account?.displayName.orEmpty(), style = MaterialTheme.typography.bodyLarge)
                        Text(
                            listOf(account?.login.orEmpty(), if (account?.hasPlus == true) "Плюс" else "без Плюса")
                                .filter { it.isNotEmpty() }.joinToString(" · "),
                            style = MaterialTheme.typography.bodySmall,
                            color = colors.onSurfaceVariant,
                        )
                    }
                    FilledTonalButton(onClick = { confirmSignOut = true }) { Text("Выйти") }
                }
            }

            SettingsCard("Внешний вид", "palette") {
                val theme by settings.themeMode.flow.collectAsStateWithLifecycle()
                SettingRow("Тема", stacked = true) {
                    Segments(THEME_SEGMENTS, THEMES.indexOf(theme), { settings.themeMode.value = THEMES[it] }, compact = true)
                }
                val fromCover by settings.accentFromCover.flow.collectAsStateWithLifecycle()
                Toggle("Цвет из обложки", "Приложение перекрашивается под обложку играющего трека", settings.accentFromCover)
                val seed by settings.customSeed.flow.collectAsStateWithLifecycle()
                SettingRow("Акцентный цвет", if (fromCover) "Когда ничего не играет" else "Основной цвет приложения", stacked = true) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        for (accent in ACCENTS) {
                            val current = seed.equals(accent, ignoreCase = true)
                            MorphShape(
                                if (current) Shapes.Cookie6 else Shapes.Circle,
                                Color(android.graphics.Color.parseColor(accent)),
                                Modifier.size(32.dp).clickable(remember { MutableInteractionSource() }, indication = null) { settings.customSeed.value = accent },
                            ) {
                                if (current) MorphShape(Shapes.Circle, Color.White, Modifier.size(12.dp))
                            }
                        }
                    }
                }
                Choice("Цветовая схема", SCHEME_VARIANTS.map { Triple(it.first, it.second, "") }, settings.schemeVariant)
                Choice("Смена цветов и фона", TRANSITIONS, settings.trackTransition)
                val backdrop by settings.backdropMode.flow.collectAsStateWithLifecycle()
                Choice("Фон из обложки", BACKDROPS, settings.backdropMode)
                val blur by settings.nowPlayingBlur.flow.collectAsStateWithLifecycle()
                SettingRow(
                    "Размытие фона плеера",
                    if (backdrop == "palette") "Градиент от обложки не зависит"
                    else "${(blur * 100).roundToInt()} % · 0 — чёткая обложка, 100 — только цвет",
                    stacked = true,
                    modifier = Modifier.alpha(if (backdrop == "palette") 0.5f else 1f),
                ) {
                    Steps(blur, 0f..1f, 19, enabled = backdrop != "palette") { settings.nowPlayingBlur.value = (it * 20).roundToInt() / 20f }
                }
                val drift by settings.nowPlayingDrift.flow.collectAsStateWithLifecycle()
                SettingRow(
                    "Плавание размытия",
                    if (drift > 0) "${(drift * 100).roundToInt()} % · цвета фона медленно переливаются, пока играет музыка"
                    else "Выключено — размытие неподвижно",
                    stacked = true,
                ) {
                    Steps(drift, 0f..1f, 19) { settings.nowPlayingDrift.value = (it * 20).roundToInt() / 20f }
                }
                Toggle("Рамка обложки вращается", "Пока играет музыка, фигурная рамка обложки медленно крутится; сама картинка стоит", settings.coverSpin)
                Toggle("Обложка дышит в такт", "В полноэкранном плеере обложка слегка пульсирует по громкости музыки", settings.coverPulse)
                Toggle("Атмосферный фон", "Размытая обложка играющего трека за всем приложением — слабым цветным свечением", settings.ambientBackground)
            }

            SettingsCard("Воспроизведение", "graphic_eq") {
                val quality by settings.quality.flow.collectAsStateWithLifecycle()
                SettingRow("Качество звука", QUALITY_HINTS[quality].orEmpty(), stacked = true) {
                    Segments(QUALITY_SEGMENTS, QUALITIES.indexOf(quality), { settings.quality.value = QUALITIES[it] }, compact = true)
                }
                val crossfade by settings.crossfade.flow.collectAsStateWithLifecycle()
                SettingRow(
                    "Плавный переход между треками",
                    if (crossfade > 0) "Следующий трек начинается за $crossfade с до конца текущего и плавно сменяет его"
                    else "Выключен — треки идут друг за другом без пауз",
                    stacked = true,
                ) {
                    Steps(crossfade.toFloat(), 0f..12f, 11) { settings.crossfade.value = it.roundToInt() }
                }
            }

            SettingsCard("Офлайн", "download") {
                val done by app.downloads.done.collectAsStateWithLifecycle()
                val pending by app.downloads.pending.collectAsStateWithLifecycle()
                val status by app.downloads.status.collectAsStateWithLifecycle()
                val megabytes = done.values.sumOf { it.size } / (1024 * 1024)
                SettingRow(
                    "Скачано: ${tracksCount(done.size)}" + if (megabytes > 0) " · $megabytes МБ" else "",
                    when {
                        pending.isEmpty() -> "Скачанные треки играют без сети. Кнопка скачивания — в шапке плейлиста или альбома"
                        status.isNotEmpty() -> "В очереди ещё ${pending.size}. $status"
                        else -> "Скачивается, в очереди ещё ${pending.size}"
                    },
                )
                Toggle("Только по Wi-Fi", "Не тратить мобильный трафик на скачивание", settings.downloadOnWifiOnly)
                if (done.isNotEmpty() || pending.isNotEmpty()) {
                    TextButton(onClick = { confirmClear = true }) { Text("Удалить всё скачанное") }
                }
            }

            SettingsCard("О программе", "info") {
                SettingRow(
                    "YaMusic для Android",
                    "Неофициальный клиент Яндекс Музыки в стиле Material 3 Expressive. Шрифты: Google Sans (OFL), Material Symbols (Apache 2.0)",
                )
            }
        }
    }
    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("Удалить всё скачанное?") },
            text = { Text("Файлы треков будут удалены с телефона, очередь скачивания — очищена.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmClear = false
                    app.downloads.clear()
                }) { Text("Удалить") }
            },
            dismissButton = { TextButton(onClick = { confirmClear = false }) { Text("Отмена") } },
        )
    }
    if (confirmSignOut) {
        AlertDialog(
            onDismissRequest = { confirmSignOut = false },
            title = { Text("Выйти из аккаунта?") },
            text = { Text("Токен будет удалён с этого телефона.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmSignOut = false
                    app.session.signOut()
                }) { Text("Выйти") }
            },
            dismissButton = { TextButton(onClick = { confirmSignOut = false }) { Text("Отмена") } },
        )
    }
}

/** Группа настроек: заголовок секции со значком + карточка. */
@Composable
private fun SettingsCard(title: String, icon: String, rows: @Composable ColumnScope.() -> Unit) {
    val colors = MaterialTheme.colorScheme
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(Modifier.padding(start = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Symbol(icon, size = 20.dp, tint = colors.primary)
            Text(title, style = MaterialTheme.typography.titleMedium, color = colors.primary)
        }
        Surface(Modifier.fillMaxWidth(), shape = RoundedCornerShape(28.dp), color = colors.surfaceContainerHigh) {
            Column(Modifier.padding(horizontal = 20.dp, vertical = 20.dp), verticalArrangement = Arrangement.spacedBy(20.dp), content = rows)
        }
    }
}

/** Строка настройки: заголовок и пояснение слева, контрол справа; широкий контрол ([stacked]) — под подписью. */
@Composable
private fun SettingRow(
    title: String,
    description: String = "",
    stacked: Boolean = false,
    modifier: Modifier = Modifier,
    control: @Composable () -> Unit = {},
) {
    val label = @Composable {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (description.isNotEmpty()) {
                Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
    if (stacked) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            label()
            control()
        }
    } else {
        Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) { label() }
            control()
        }
    }
}

@Composable
private fun Toggle(title: String, description: String, pref: Pref<Boolean>) {
    val on by pref.flow.collectAsStateWithLifecycle()
    SettingRow(title, description) { Switch(checked = on, onCheckedChange = { pref.value = it }) }
}

/** Выбор одного из вариантов чипами; под ними — пояснение к выбранному. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Choice(title: String, options: List<Triple<String, String, String>>, pref: Pref<String>) {
    val chosen by pref.flow.collectAsStateWithLifecycle()
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for ((value, name) in options) {
                FilterChip(
                    selected = chosen == value,
                    onClick = { pref.value = value },
                    label = { Text(name) },
                    leadingIcon = if (chosen == value) ({ Symbol("check", size = 18.dp) }) else null,
                )
            }
        }
        val hint = options.firstOrNull { it.first == chosen }?.third.orEmpty()
        if (hint.isNotEmpty()) Text(hint, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Ползунок с шагами. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Steps(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int, enabled: Boolean = true, onChange: (Float) -> Unit) {
    val slider = remember { SliderState(value = value, steps = steps, trackRange = range) }
    slider.value = value
    Slider(state = slider, onValueChange = onChange, enabled = enabled)
}
