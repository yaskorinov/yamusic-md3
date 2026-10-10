package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.api.ArtistRef
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.data.Downloads
import io.github.yaskorinov.yamusic.data.Library
import io.github.yaskorinov.yamusic.data.TrackList
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

/**
 * Страница со списком треков (перенос TrackListPage.qml): шапка-«герой» прокручивается вместе со списком —
 * обложка или фигура со значком, надзаголовок, название, «N треков · длительность», Слушать / Перемешать.
 * За шапкой медленно плывут фигуры. Поиск по списку (кнопка в шапке): пока в поле есть текст, список
 * показывает найденное, и «Слушать», «Перемешать», нажатие на трек ставят в очередь именно найденное.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalLayoutApi::class)
@Composable
fun TrackListScreen(
    title: String,
    list: TrackList,
    overline: String = "",
    subtitle: String = "",
    heroImage: String = "",
    heroIcon: String = "queue_music",
    heroShape: PolarShape = Shapes.Cookie9,
    artists: List<ArtistRef> = emptyList(),
    onOpenArtist: (String) -> Unit = {},
    numbered: Boolean = false,
    emptyIcon: String = "music_note",
    emptyTitle: String = "Здесь пусто",
    emptyText: String = "",
    context: PlayContext,
    player: PlayerConnection,
    playerState: PlayerState,
    contentPadding: PaddingValues,
    onRetry: () -> Unit,
    onBack: () -> Unit,
    downloads: Downloads,
) {
    val all by list.tracks.collectAsStateWithLifecycle()
    val loading by list.loading.collectAsStateWithLifecycle()
    val error by list.error.collectAsStateWithLifecycle()
    var searchOpen by rememberSaveable { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    val tracks = remember(all, query) { filterTracks(all, query) }
    val filtering = query.isNotBlank()

    val listState = rememberLazyListState()
    val threshold = with(LocalDensity.current) { 48.dp.toPx() }
    val scrolled by remember { derivedStateOf { listState.firstVisibleItemIndex > 0 || listState.firstVisibleItemScrollOffset > threshold } }
    var headerHeight by remember { mutableFloatStateOf(0f) }
    Box(Modifier.fillMaxSize().padding(top = contentPadding.calculateTopPadding())) {
        HeroShapes(
            seed = title,
            top = 56.dp,
            headerHeight = { headerHeight },
            scroll = { if (listState.firstVisibleItemIndex == 0) listState.firstVisibleItemScrollOffset.toFloat() else Float.MAX_VALUE },
        )
        Column {
            TopBar(title, showTitle = scrolled, onBack = onBack)
            LazyColumn(
                Modifier.fillMaxSize(),
                state = listState,
                contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = contentPadding.calculateBottomPadding() + 16.dp),
            ) {
                item(key = "header") {
                    Column(Modifier.fillMaxWidth().onSizeChanged { headerHeight = it.height.toFloat() }.padding(start = 8.dp, end = 8.dp, top = 28.dp, bottom = 20.dp)) {
                        Row(verticalAlignment = Alignment.Bottom) {
                            HeroCover(heroImage, heroIcon, heroShape)
                            Spacer(Modifier.width(20.dp))
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                if (overline.isNotEmpty()) Overline(overline)
                                Text(
                                    title,
                                    style = MaterialTheme.typography.headlineLarge,
                                    fontWeight = FontWeight.Bold,
                                    maxLines = 3,
                                    overflow = TextOverflow.Ellipsis,
                                    autoSize = TextAutoSize.StepBased(minFontSize = 22.sp, maxFontSize = 34.sp, stepSize = 2.sp),
                                )
                                if (artists.isNotEmpty()) {
                                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                        artists.forEachIndexed { index, artist ->
                                            Text(
                                                artist.name + if (index < artists.lastIndex) "," else "",
                                                Modifier.clip(CircleShape).clickable { onOpenArtist(artist.id) },
                                                style = MaterialTheme.typography.titleMedium,
                                                fontWeight = FontWeight.SemiBold,
                                            )
                                        }
                                    }
                                }
                                Text(
                                    when {
                                        all.isEmpty() && loading -> "Загружаем…"
                                        else -> listOf(subtitle, tracksCount(all.size), totalDuration(all)).filter { it.isNotEmpty() }.joinToString("  ·  ")
                                    },
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                        if (all.isNotEmpty()) {
                            Spacer(Modifier.height(18.dp))
                            FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                val padding = PaddingValues(start = 12.dp, end = 16.dp)
                                Button(onClick = { player.play(tracks, 0, context) }, enabled = tracks.isNotEmpty(), contentPadding = padding) {
                                    Symbol("play_arrow", size = 18.dp, filled = true)
                                    Spacer(Modifier.width(6.dp))
                                    Text("Слушать")
                                }
                                FilledTonalButton(
                                    onClick = { player.play(tracks, tracks.indices.random(), context, shuffle = true) },
                                    enabled = tracks.isNotEmpty(),
                                    contentPadding = padding,
                                ) {
                                    Symbol("shuffle", size = 18.dp)
                                    Spacer(Modifier.width(6.dp))
                                    Text("Перемешать")
                                }
                                DownloadButton(all, downloads)
                                if (searchOpen) {
                                    FilledTonalIconButton(onClick = {
                                        searchOpen = false
                                        query = ""
                                    }) { Symbol("search") }
                                } else {
                                    IconButton(onClick = { searchOpen = true }) { Symbol("search") }
                                }
                            }
                        }
                        if (searchOpen) {
                            Spacer(Modifier.height(10.dp))
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                SearchField(query, { query = it }, "Найти в списке", Modifier.weight(1f), focus = true)
                                if (filtering) {
                                    Text(
                                        "${tracks.size} из ${all.size}",
                                        Modifier.padding(start = 12.dp),
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
                when {
                    error.isNotEmpty() -> item(key = "error") {
                        EmptyState("cloud_off", "Не удалось загрузить", Modifier.fillMaxWidth().padding(top = 24.dp), text = error, actionText = "Повторить", onAction = onRetry)
                    }
                    all.isEmpty() && !loading -> item(key = "empty") {
                        EmptyState(emptyIcon, emptyTitle, Modifier.fillMaxWidth().padding(top = 24.dp), text = emptyText, shape = heroShape)
                    }
                    filtering && tracks.isEmpty() -> item(key = "nothing") {
                        Text(
                            "По запросу «${query.trim()}» в этом списке ничего нет",
                            Modifier.fillMaxWidth().padding(24.dp),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
                itemsIndexed(tracks, key = { _, track -> track.id }) { index, track ->
                    TrackRow(
                        track = track,
                        current = track.id == playerState.track?.id,
                        onClick = { player.play(tracks, index, context) },
                        number = if (numbered) index + 1 else null,
                    )
                }
                if (loading) {
                    item(key = "loading") {
                        Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                            LoadingIndicator(Modifier.size(if (all.isEmpty()) 64.dp else 48.dp))
                        }
                    }
                }
            }
        }
    }
}

/** Обложка шапки или фигура со значком; за обложкой — медленно вращающаяся фигура. */
@Composable
private fun HeroCover(image: String, icon: String, shape: PolarShape) {
    val colors = MaterialTheme.colorScheme
    Box(Modifier.size(HERO_COVER), contentAlignment = Alignment.Center) {
        if (image.isNotEmpty()) {
            val time = rememberTime(true)
            MorphShape(Shapes.Cookie9, colors.surfaceContainerHighest, Modifier.requiredSize(HERO_COVER * 1.3f).graphicsLayer(), rotation = { time() * 6f })
            MorphImage(image, Shapes.SoftSquare, Modifier.fillMaxSize(), placeholder = icon, placeholderSize = 48.dp)
        } else {
            MorphShape(shape, colors.primaryContainer, Modifier.fillMaxSize()) {
                Symbol(icon, size = HERO_COVER * 0.38f, filled = true, tint = colors.onPrimaryContainer)
            }
        }
    }
}

/** Найденное в списке: каждое слово запроса должно встретиться в названии, исполнителях или альбоме. */
private fun filterTracks(tracks: List<Track>, query: String): List<Track> {
    val words = query.lowercase().replace('ё', 'е').split(' ').filter { it.isNotEmpty() }
    if (words.isEmpty()) return tracks
    return tracks.filter { track ->
        val text = "${track.title} ${track.version} ${track.artists} ${track.album}".lowercase().replace('ё', 'е')
        words.all { it in text }
    }
}

private fun totalDuration(tracks: List<Track>): String {
    val minutes = Math.round(tracks.sumOf { it.durationMs } / 60000.0)
    return when {
        minutes <= 0 -> ""
        minutes >= 60 -> "${minutes / 60} ч ${minutes % 60} мин"
        else -> "$minutes мин"
    }
}

/**
 * Скачать список для игры без сети. Три состояния: не скачан → скачивается (сколько готово; нажатие
 * отменяет остаток) → скачан (нажатие предлагает удалить файлы).
 */
@Composable
private fun FlowRowScope.DownloadButton(tracks: List<Track>, downloads: Downloads) {
    val done by downloads.done.collectAsStateWithLifecycle()
    val pending by downloads.pending.collectAsStateWithLifecycle()
    val status by downloads.status.collectAsStateWithLifecycle()
    val wanted = tracks.filter { it.available }
    val ready = wanted.count { it.id in done }
    val queued = pending.count { queuedTrack -> wanted.any { it.id == queuedTrack.id } }
    var confirmRemove by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme

    FilledTonalIconButton(
        onClick = {
            when {
                queued > 0 -> downloads.remove(pending.map { it.id }.filter { id -> wanted.any { it.id == id } })
                wanted.isNotEmpty() && ready == wanted.size -> confirmRemove = true
                else -> downloads.enqueue(wanted)
            }
        },
        enabled = wanted.isNotEmpty(),
    ) {
        when {
            queued > 0 -> Text("$ready/${wanted.size}", style = MaterialTheme.typography.labelSmall, maxLines = 1)
            wanted.isNotEmpty() && ready == wanted.size -> Symbol("download_done", tint = colors.primary)
            else -> Symbol("download")
        }
    }
    if (queued > 0 && status.isNotEmpty()) {
        // очередь стоит — объяснить почему (например, «Ждём Wi-Fi»)
        Text(status, Modifier.padding(start = 4.dp).align(Alignment.CenterVertically), style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant)
    }
    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text("Удалить скачанное?") },
            text = { Text("Файлы этих треков (${tracksCount(ready)}) будут удалены с телефона. Сами треки останутся в коллекции.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmRemove = false
                    downloads.remove(wanted.map { it.id })
                }) { Text("Удалить") }
            },
            dismissButton = { TextButton(onClick = { confirmRemove = false }) { Text("Отмена") } },
        )
    }
}

/** Идентификаторы скачанных треков: строки помечают их значком, не зная о хранилище. */
val LocalDownloaded = compositionLocalOf<Set<String>> { emptySet() }

/** Коллекция — строкам треков: залитое сердце у отмеченных, нажатие на него снимает отметку. */
val LocalLibrary = staticCompositionLocalOf<Library?> { null }

/**
 * Строка трека: номер · обложка в фигуре · название/исполнители · лайк · длительность. Играющий трек —
 * на подложке цвета темы, а его обложка морфится в «печеньку». [trailing] — что поставить в конец строки.
 */
@Composable
fun TrackRow(track: Track, current: Boolean, onClick: () -> Unit, number: Int? = null, trailing: (@Composable () -> Unit)? = null) {
    val colors = MaterialTheme.colorScheme
    val library = LocalLibrary.current
    val liked = library?.likedIds?.collectAsStateWithLifecycle()?.value?.contains(track.id) == true
    Row(
        Modifier
            .fillMaxWidth()
            .height(64.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(if (current) colors.primary.copy(alpha = 0.12f) else Color.Transparent)
            .clickable(enabled = track.available, onClick = onClick)
            .padding(start = 12.dp, end = if (trailing == null) 12.dp else 4.dp)
            .alpha(if (track.available) 1f else 0.38f),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (number != null) {
            Text(
                number.toString(),
                Modifier.width(24.dp).padding(end = 2.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.End,
                maxLines = 1,
            )
            Spacer(Modifier.width(10.dp))
        }
        MorphImage(track.cover(200), if (current) Shapes.Cookie9 else Shapes.SoftSquare, Modifier.size(48.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    buildAnnotatedString {
                        append(track.title)
                        if (track.version.isNotEmpty()) {
                            withStyle(SpanStyle(color = colors.onSurfaceVariant, fontWeight = FontWeight.Normal)) { append("  ${track.version}") }
                        }
                    },
                    Modifier.weight(1f, fill = false),
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.Medium,
                    color = if (current) colors.primary else colors.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (track.explicit) {
                    Spacer(Modifier.width(6.dp))
                    ExplicitBadge()
                }
            }
            Text(track.artists, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (track.id in LocalDownloaded.current) {
            Spacer(Modifier.width(6.dp))
            Symbol("download_done", size = 16.dp, tint = colors.primary)
        }
        if (liked) {
            Box(Modifier.size(32.dp).clip(CircleShape).clickable { library.toggleLike(track) }, contentAlignment = Alignment.Center) {
                Symbol("favorite", size = 20.dp, filled = true, tint = colors.primary)
            }
        }
        Text(
            formatTime(track.durationMs),
            Modifier.width(44.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant,
            textAlign = TextAlign.End,
        )
        trailing?.invoke()
    }
}

fun formatTime(ms: Long): String {
    val seconds = (ms / 1000).coerceAtLeast(0)
    return "%d:%02d".format(seconds / 60, seconds % 60)
}

fun tracksCount(n: Int): String {
    val word = when {
        n % 100 in 11..14 -> "треков"
        n % 10 == 1 -> "трек"
        n % 10 in 2..4 -> "трека"
        else -> "треков"
    }
    return "$n $word"
}

private val HERO_COVER = 128.dp
