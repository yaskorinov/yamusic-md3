package io.github.yaskorinov.yamusic.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import io.github.yaskorinov.yamusic.App
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.playback.PlayerState
import kotlinx.coroutines.launch

/** Открыть меню трека. Строки треков и пилюля мини-плеера зовут его по долгому нажатию. */
val LocalTrackMenu = staticCompositionLocalOf<(Track) -> Unit> { {} }

/** То, что меню просит у главного экрана: он владеет разделами и полноэкранным плеером. */
class TrackMenuActions(
    val openQueue: () -> Unit,
    val openSettings: () -> Unit,
    /** Страница вида `album:ID`, `album:ID#TRACK` или `artist:ID`. */
    val openPage: (String) -> Unit,
)

/**
 * Меню трека (перенос TrackMenu.qml): волна, повтор, очередь, плейлисты, ссылки, переходы. На телефоне
 * это нижний лист поверх всего приложения; подменю десктопа («Добавить в плейлист», «Поделиться») —
 * страницы того же листа. Пункты собраны в группы: у группы скруглены внешние углы, между пунктами —
 * узкий зазор, нажатый пункт скругляется сильнее.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrackMenuSheet(track: Track, app: App, state: PlayerState, actions: TrackMenuActions, onDismiss: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val library = app.library
    var page by rememberSaveable { mutableStateOf("") }
    val depth = if (page.isEmpty()) 0 else if (page == "new") 2 else 1

    // Лист сначала уезжает вниз и только потом выполняется действие: переход не дёргает экран под ним
    fun close(then: () -> Unit = {}) {
        scope.launch { sheet.hide() }.invokeOnCompletion {
            onDismiss()
            then()
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheet,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        containerColor = colors.surfaceContainerLow,
        contentColor = colors.onSurface,
    ) {
        // imePadding: на странице «Новый плейлист» лист поднимается над клавиатурой
        Column(Modifier.fillMaxWidth().imePadding().padding(horizontal = 16.dp).padding(bottom = 16.dp)) {
            // Шапка: чей это список действий. Лайк — здесь: в строках треков его можно только снять
            Row(Modifier.fillMaxWidth().padding(start = 4.dp, bottom = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                MorphImage(track.cover(200), Shapes.Cookie9, Modifier.size(56.dp))
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(track.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(track.artists, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                LikeButton(track, library)
            }

            val shift = with(LocalDensity.current) { 64.dp.roundToPx() }
            AnimatedContent(
                targetState = page to depth,
                transitionSpec = {
                    // shared axis X, как между страницами приложения: вглубь — справа, назад — слева
                    val way = if (targetState.second >= initialState.second) 1 else -1
                    val move = tween<IntOffset>(Motion.SpatialFast, easing = Motion.EmphasizedDecelerate)
                    (fadeIn(tween(220, delayMillis = 60)) + slideInHorizontally(move) { way * shift }) togetherWith
                        (fadeOut(tween(120)) + slideOutHorizontally(move) { -way * shift / 2 })
                },
                label = "menu",
            ) { (shown, _) ->
                Column(Modifier.fillMaxWidth().heightIn(max = 520.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    when (shown) {
                        "" -> Main(track, app, state, onPage = { page = it }, close = ::close, actions = actions)
                        "playlists" -> Playlists(track, app, onBack = { page = "" }, onNew = { page = "new" }, close = ::close)
                        "new" -> NewPlaylist(onBack = { page = "playlists" }) { title ->
                            library.createPlaylist(title, track)
                            close()
                        }
                        "share" -> Share(track, context, onBack = { page = "" }, close = ::close)
                        "artists" -> {
                            Back("Исполнители") { page = "" }
                            Group {
                                track.artistRefs.forEachIndexed { index, artist ->
                                    Item("person", artist.name, at(index, track.artistRefs.size)) { close { actions.openPage("artist:${artist.id}") } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ColumnScope.Main(
    track: Track,
    app: App,
    state: PlayerState,
    onPage: (String) -> Unit,
    close: (() -> Unit) -> Unit,
    actions: TrackMenuActions,
) {
    val seed = "track:${track.id}"
    val pins by app.library.pins.collectAsStateWithLifecycle()
    val pinned = seed in pins
    val current = state.track?.id == track.id
    val hasAlbum = track.albumId.isNotEmpty()

    Group {
        Item("push_pin", if (pinned) "Открепить Мою волну" else "Закрепить Мою волну", Place.First, chosen = pinned) {
            app.library.toggleWavePin(seed)
            close {}
        }
        Item("radio", "Моя волна по треку", Place.Last) {
            app.wave.playStation(seed)
            close {}
        }
    }
    Group {
        val repeating = state.repeat != Player.REPEAT_MODE_OFF
        if (current) {
            val one = state.repeat == Player.REPEAT_MODE_ONE
            // повтор переключается по кругу (выкл → все → один), поэтому лист остаётся открытым
            Item(if (one) "repeat_one" else "repeat", "Повторять", Place.First, chosen = repeating, note = if (one) "Один трек" else if (repeating) "Всю очередь" else "") {
                app.player.cycleRepeat()
            }
        }
        Item("queue_music", "Открыть очередь", if (current) Place.Middle else Place.First, enabled = state.track != null) { close(actions.openQueue) }
        Item("tune", "Настройки звука", Place.Last) { close(actions.openSettings) }
    }
    Group {
        Item("playlist_add", "Добавить в плейлист", Place.First, more = true) { onPage("playlists") }
        Item("share", "Поделиться", Place.Middle, more = true) { onPage("share") }
        Item("music_note", "Перейти к треку", Place.Middle, enabled = hasAlbum) { close { actions.openPage("album:${track.albumId}#${track.id}") } }
        Item("album", "Перейти к альбому", Place.Middle, enabled = hasAlbum) { close { actions.openPage("album:${track.albumId}") } }
        val artists = track.artistRefs
        Item("person", "Перейти к исполнителю", Place.Last, enabled = artists.isNotEmpty(), more = artists.size > 1) {
            if (artists.size == 1) close { actions.openPage("artist:${artists[0].id}") } else onPage("artists")
        }
    }
}

@Composable
private fun ColumnScope.Playlists(track: Track, app: App, onBack: () -> Unit, onNew: () -> Unit, close: (() -> Unit) -> Unit) {
    val all by app.library.playlists.collectAsStateWithLifecycle()
    val own = remember(all) { app.library.ownPlaylists(all) }
    Back("Добавить в плейлист", onBack)
    Group { Item("add", "Новый плейлист…", Place.Single, more = true, onClick = onNew) }
    if (own.isNotEmpty()) {
        Group {
            own.forEachIndexed { index, playlist ->
                Item("queue_music", playlist.title, at(index, own.size), note = tracksCount(playlist.trackCount)) {
                    app.library.addToPlaylist(track, playlist)
                    close {}
                }
            }
        }
    }
}

@Composable
private fun ColumnScope.NewPlaylist(onBack: () -> Unit, onCreate: (String) -> Unit) {
    var title by rememberSaveable { mutableStateOf("") }
    Back("Новый плейлист", onBack)
    SearchField(title, { title = it }, "Название", Modifier.fillMaxWidth(), focus = true, icon = "queue_music", onSearch = { if (title.isNotBlank()) onCreate(title) })
    Button(onClick = { onCreate(title) }, enabled = title.isNotBlank(), modifier = Modifier.fillMaxWidth().height(56.dp)) {
        Text("Создать и добавить трек", style = MaterialTheme.typography.labelLarge)
    }
}

@Composable
private fun ColumnScope.Share(track: Track, context: Context, onBack: () -> Unit, close: (() -> Unit) -> Unit) {
    val link = if (track.albumId.isEmpty()) "https://music.yandex.ru/track/${track.id}" else "https://music.yandex.ru/album/${track.albumId}/track/${track.id}"
    // на Android 13+ система сама показывает, что скопировано, — своего сообщения не нужно
    fun copy(text: String) {
        context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("YaMusic", text))
        close {}
    }
    Back("Поделиться", onBack)
    Group {
        Item("share", "Отправить ссылку на трек…", Place.First) {
            close {
                val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link)
                    .putExtra(Intent.EXTRA_TITLE, "${track.title} — ${track.artists}")
                context.startActivity(Intent.createChooser(send, null))
            }
        }
        Item("content_copy", "Скопировать ссылку на трек", if (track.artistRefs.isEmpty()) Place.Last else Place.Middle) { copy(link) }
        val artists = track.artistRefs
        artists.forEachIndexed { index, artist ->
            val text = if (artists.size == 1) "Скопировать ссылку на исполнителя" else "Ссылка на ${artist.name}"
            Item("person", text, if (index == artists.lastIndex) Place.Last else Place.Middle) { copy("https://music.yandex.ru/artist/${artist.id}") }
        }
    }
}

/** Заголовок страницы-подменю со стрелкой назад. */
@Composable
private fun Back(title: String, onBack: () -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack) { Symbol("arrow_back") }
        Text(title, Modifier.padding(start = 4.dp), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun Group(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp), content = content)
}

/** Место пункта в группе: от него зависит, какие углы скруглены сильно. */
private enum class Place { Single, First, Middle, Last }

private fun at(index: Int, count: Int) = when {
    count == 1 -> Place.Single
    index == 0 -> Place.First
    index == count - 1 -> Place.Last
    else -> Place.Middle
}

/**
 * Пункт меню. [chosen] — включённое состояние (закреплено, повтор): тональная подложка и залитый значок.
 * [more] — пункт ведёт на следующую страницу листа. Нажатый пункт скругляет все углы.
 */
@Composable
private fun Item(
    icon: String,
    text: String,
    place: Place,
    enabled: Boolean = true,
    chosen: Boolean = false,
    more: Boolean = false,
    note: String = "",
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val bounce = spring<Dp>(dampingRatio = 0.6f, stiffness = 700f)
    val outer = if (pressed) 28.dp else 20.dp
    val inner = if (pressed) 28.dp else 6.dp
    val top by animateDpAsState(if (place == Place.Single || place == Place.First) outer else inner, bounce, label = "top")
    val bottom by animateDpAsState(if (place == Place.Single || place == Place.Last) outer else inner, bounce, label = "bottom")
    val container by animateColorAsState(if (chosen) colors.secondaryContainer else colors.surfaceContainerHighest, tween(Motion.EffectsDefault), label = "container")
    val content = if (chosen) colors.onSecondaryContainer else colors.onSurface
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom))
            .background(container)
            .clickable(interaction, LocalIndication.current, enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else 0.38f)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CompositionLocalProvider(LocalContentColor provides content) {
            Symbol(icon, filled = chosen, tint = if (chosen) colors.onSecondaryContainer else colors.onSurfaceVariant)
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                Text(text, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (note.isNotEmpty()) {
                    Text(note, style = MaterialTheme.typography.bodySmall, color = if (chosen) content.copy(alpha = 0.8f) else colors.onSurfaceVariant, maxLines = 1)
                }
            }
            if (more) Box(Modifier.rotate(-90f)) { Symbol("keyboard_arrow_down", size = 20.dp, tint = colors.onSurfaceVariant) }
        }
    }
}
