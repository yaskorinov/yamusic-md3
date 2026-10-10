package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.IconButton
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.App
import io.github.yaskorinov.yamusic.data.Pref
import kotlin.math.roundToInt

private val QUALITIES = listOf("auto" to "Авто", "lossless" to "Без потерь", "hq" to "Высокое", "nq" to "Экономное")
private val QUALITY_HINTS = mapOf(
    "auto" to "FLAC по Wi-Fi, AAC по мобильной сети",
    "lossless" to "FLAC всегда — около 25 МБ на трек",
    "hq" to "AAC или MP3 высокого битрейта",
    "nq" to "Низкий битрейт — меньше всего трафика",
)
private val THEMES = listOf("system" to "Как в системе", "dark" to "Тёмная", "light" to "Светлая")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(app: App, contentPadding: PaddingValues, onBack: () -> Unit) {
    val settings = app.settings
    val account = app.session.account
    val colors = MaterialTheme.colorScheme
    var confirmSignOut by remember { mutableStateOf(false) }
    var confirmClear by remember { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(contentPadding)) {
        IconButton(onClick = onBack, modifier = Modifier.padding(start = 8.dp, top = 4.dp)) { Symbol("arrow_back") }
        Text(
            "Настройки",
            Modifier.padding(start = 20.dp, bottom = 8.dp),
            style = MaterialTheme.typography.displaySmall,
            fontWeight = FontWeight.SemiBold,
        )

        Card {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Symbol("account_circle", size = 40.dp, tint = colors.primary)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(account?.displayName.orEmpty(), style = MaterialTheme.typography.titleMedium)
                    Text(
                        listOf(account?.login.orEmpty(), if (account?.hasPlus == true) "Плюс" else "без Плюса")
                            .filter { it.isNotEmpty() }.joinToString(" · "),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                    )
                }
                FilledTonalButton(onClick = { confirmSignOut = true }) { Text("Выйти") }
            }
        }

        Section("Звук")
        Card {
            val quality by settings.quality.flow.collectAsStateWithLifecycle()
            Label("Качество", QUALITY_HINTS[quality].orEmpty())
            Choice(QUALITIES, settings.quality)
            Spacer(Modifier.height(16.dp))
            val crossfade by settings.crossfade.flow.collectAsStateWithLifecycle()
            Label("Плавный переход", if (crossfade == 0) "Выключен: треки идут встык" else "Треки перетекают друг в друга $crossfade с")
            val slider = remember { SliderState(value = crossfade.toFloat(), steps = 11, trackRange = 0f..12f) }
            slider.value = crossfade.toFloat()
            Slider(state = slider, onValueChange = { settings.crossfade.value = it.roundToInt() })
        }

        Section("Оформление")
        Card {
            Label("Тема")
            Choice(THEMES, settings.themeMode)
            Spacer(Modifier.height(8.dp))
            Toggle("Цвет из обложки", "Приложение перекрашивается под играющий трек", settings.accentFromCover)
            Toggle("Фон плеера из обложки", "Размытая обложка за полноэкранным плеером", settings.backdrop)
        }

        Section("Офлайн")
        Card {
            val done by app.downloads.done.collectAsStateWithLifecycle()
            val pending by app.downloads.pending.collectAsStateWithLifecycle()
            val status by app.downloads.status.collectAsStateWithLifecycle()
            val megabytes = done.values.sumOf { it.size } / (1024 * 1024)
            Label(
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

        Text(
            "YaMusic для Android · неофициальный клиент Яндекс Музыки",
            Modifier.fillMaxWidth().padding(24.dp),
            style = MaterialTheme.typography.bodySmall,
            color = colors.onSurfaceVariant,
        )
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

@Composable
fun Section(title: String) {
    Text(
        title,
        Modifier.padding(start = 24.dp, top = 20.dp, bottom = 8.dp),
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
fun Card(content: @Composable ColumnScope.() -> Unit) {
    Surface(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.padding(16.dp), content = content)
    }
}

@Composable
fun Label(title: String, hint: String = "") {
    Text(title, style = MaterialTheme.typography.titleMedium)
    if (hint.isNotEmpty()) {
        Text(hint, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun Choice(options: List<Pair<String, String>>, pref: Pref<String>) {
    val chosen by pref.flow.collectAsStateWithLifecycle()
    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for ((value, title) in options) {
            FilterChip(selected = chosen == value, onClick = { pref.value = value }, label = { Text(title) })
        }
    }
}

@Composable
fun Toggle(title: String, hint: String, pref: Pref<Boolean>) {
    val on by pref.flow.collectAsStateWithLifecycle()
    Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) { Label(title, hint) }
        Spacer(Modifier.width(12.dp))
        Switch(checked = on, onCheckedChange = { pref.value = it })
    }
}
