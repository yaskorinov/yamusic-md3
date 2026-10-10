package io.github.yaskorinov.yamusic

import android.content.Intent
import android.content.pm.ApplicationInfo
import android.graphics.Color as AndroidColor
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.data.AuthState
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerState
import io.github.yaskorinov.yamusic.ui.ArtistScreen
import io.github.yaskorinov.yamusic.ui.CollectionScreen
import io.github.yaskorinov.yamusic.ui.CoverBackdrop
import io.github.yaskorinov.yamusic.ui.LocalDownloaded
import io.github.yaskorinov.yamusic.ui.LocalLibrary
import io.github.yaskorinov.yamusic.ui.LocalTheme
import io.github.yaskorinov.yamusic.ui.LoginScreen
import io.github.yaskorinov.yamusic.ui.MiniPlayer
import io.github.yaskorinov.yamusic.ui.NavItem
import io.github.yaskorinov.yamusic.ui.NowPlaying
import io.github.yaskorinov.yamusic.ui.SearchScreen
import io.github.yaskorinov.yamusic.ui.SettingsScreen
import io.github.yaskorinov.yamusic.ui.Shapes
import io.github.yaskorinov.yamusic.ui.TrackListScreen
import io.github.yaskorinov.yamusic.ui.WaveScreen
import io.github.yaskorinov.yamusic.ui.YaTheme

class MainActivity : ComponentActivity() {
    private val app get() = application as App

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        applyDebugExtras(intent)
        setContent {
            val auth by app.session.state.collectAsStateWithLifecycle()
            val playerState by app.player.state.collectAsStateWithLifecycle()
            val themeMode by app.settings.themeMode.flow.collectAsStateWithLifecycle()
            val dark = when (themeMode) {
                "dark" -> true
                "light" -> false
                else -> isSystemInDarkTheme()
            }
            // Значки системных панелей — под тему приложения, а не системы
            DisposableEffect(dark) {
                val style = if (dark) SystemBarStyle.dark(AndroidColor.TRANSPARENT) else SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
                enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
                onDispose { }
            }
            // Куда сменился трек: вперёд по очереди — шторка и обложка идут в одну сторону, назад — в другую
            val turn = remember { Turn(playerState.index) }
            val direction = turn.update(playerState.index)
            YaTheme(playerState.track?.cover(400).orEmpty(), dark, app.settings, direction) {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface)) {
                    CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
                        when (val state = auth) {
                            is AuthState.SignedIn -> Home(app, playerState, direction)
                            else -> LoginScreen(state, app.session)
                        }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        applyDebugExtras(intent)
    }

    /** Только отладочная сборка: `--ez noReport true` выключает учёт прослушиваний и обратную связь волне. */
    private fun applyDebugExtras(intent: Intent?) {
        val debuggable = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        if (debuggable && intent?.hasExtra("noReport") == true) {
            app.settings.noReport.value = intent.getBooleanExtra("noReport", false)
        }
    }

    // Связь с сервисом держим, только пока окно видно: без неё и без музыки сервис сам завершится
    override fun onStart() {
        super.onStart()
        app.player.connect()
    }

    override fun onStop() {
        app.player.disconnect()
        super.onStop()
    }
}

/** Направление последней смены трека: +1 — вперёд по очереди, -1 — назад. */
private class Turn(private var index: Int) {
    private var direction = 1

    fun update(now: Int): Int {
        if (now != index) direction = if (now < index) -1 else 1
        index = now
        return direction
    }
}

// Разделы — те же, что в боковой панели десктопного клиента, и в том же порядке
private enum class Tab(val title: String, val icon: String) {
    Search("Поиск", "search"),
    Wave("Моя волна", "graphic_eq"),
    Collection("Коллекция", "favorite"),
}

// Страницы поверх корня вкладки; путь вкладки — они же через перевод строки (так он переживает поворот экрана)
private const val PAGE_LIKED = "liked"
private const val PAGE_SETTINGS = "settings"
private const val PAGE_DOWNLOADED = "downloaded"
private const val PAGE_PLAYLIST = "playlist:"
private const val PAGE_ALBUM = "album:"
private const val PAGE_ARTIST = "artist:"

/**
 * Главный экран: страница раздела, над ней плавает мини-плеер-пилюля, внизу — разделы. За всем этим —
 * атмосферный фон: размытая обложка играющего трека слабым цветным свечением.
 */
@Composable
private fun Home(app: App, playerState: PlayerState, direction: Int) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    var origin by remember { mutableStateOf(Offset.Zero) }
    var tab by rememberSaveable { mutableStateOf(Tab.Wave) }
    var wavePath by rememberSaveable { mutableStateOf("") }
    var collectionPath by rememberSaveable { mutableStateOf("") }
    var searchPath by rememberSaveable { mutableStateOf("") }
    val hasTrack = playerState.track != null

    val path = when (tab) {
        Tab.Collection -> collectionPath
        Tab.Search -> searchPath
        Tab.Wave -> wavePath
    }
    val setPath: (String) -> Unit = {
        when (tab) {
            Tab.Collection -> collectionPath = it
            Tab.Search -> searchPath = it
            Tab.Wave -> wavePath = it
        }
    }
    val push: (String) -> Unit = { setPath(if (path.isEmpty()) it else "$path\n$it") }
    val pop: () -> Unit = { setPath(path.substringBeforeLast('\n', "")) }

    val theme = LocalTheme.current
    val settings = app.settings
    val ambient by settings.ambientBackground.flow.collectAsStateWithLifecycle()
    val mode by settings.backdropMode.flow.collectAsStateWithLifecycle()
    val drift by settings.nowPlayingDrift.flow.collectAsStateWithLifecycle()
    val downloadedIds by app.downloads.done.collectAsStateWithLifecycle()
    CompositionLocalProvider(LocalDownloaded provides downloadedIds.keys, LocalLibrary provides app.library) {
        Box(Modifier.fillMaxSize()) {
            if (ambient && hasTrack) {
                CoverBackdrop(
                    // зерно «стекла» под всем интерфейсом — лишнее
                    mode = if (mode == "glass") "gauss" else mode,
                    blur = 1f,
                    flow = drift,
                    running = playerState.playing && !expanded,
                    modifier = Modifier.fillMaxSize(),
                    alpha = if (theme.dark) 0.3f else 0.35f,
                )
            }
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    val padding = PaddingValues(
                        top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding(),
                        bottom = if (hasTrack) 88.dp else 0.dp, // под плавающий мини-плеер
                    )
                    AnimatedContent(
                        targetState = tab to path.substringAfterLast('\n'),
                        transitionSpec = { fadeIn(tween(220, delayMillis = 60)) togetherWith fadeOut(tween(100)) },
                        label = "page",
                    ) { (shownTab, page) ->
                        Page(app, playerState, shownTab, page, padding, push, pop)
                    }
                    androidx.compose.animation.AnimatedVisibility(
                        visible = hasTrack,
                        modifier = Modifier.align(Alignment.BottomCenter),
                        enter = slideInVertically { it } + fadeIn(),
                        exit = slideOutVertically { it } + fadeOut(),
                    ) {
                        MiniPlayer(
                            state = playerState,
                            app = app,
                            onOpen = {
                                origin = it
                                expanded = true
                            },
                            modifier = Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp),
                        )
                    }
                }
                Row(
                    Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 12.dp).padding(top = 4.dp, bottom = 8.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    for (item in Tab.entries) {
                        NavItem(
                            icon = item.icon,
                            text = item.title,
                            selected = tab == item,
                            onClick = {
                                if (tab == item) setPath("") // повторное нажатие — к корню раздела
                                tab = item
                            },
                        )
                    }
                }
            }
            NowPlaying(
                open = hasTrack && expanded,
                origin = origin,
                state = playerState,
                app = app,
                direction = direction,
                onClose = { expanded = false },
                onOpenPage = {
                    expanded = false
                    if (path.substringAfterLast('\n') != it) push(it)
                },
            )
        }
    }
    BackHandler(enabled = expanded || path.isNotEmpty()) {
        if (expanded) expanded = false else pop()
    }
}

/** Страница [page] вкладки [tab]; пустая — корень вкладки. */
@Composable
private fun Page(
    app: App,
    playerState: PlayerState,
    tab: Tab,
    page: String,
    padding: PaddingValues,
    push: (String) -> Unit,
    pop: () -> Unit,
) {
    val playlists by app.library.playlists.collectAsStateWithLifecycle()
    val downloaded by app.downloads.done.collectAsStateWithLifecycle()
    val playlist = playlists.firstOrNull { PAGE_PLAYLIST + it.id == page }
    when {
        page == PAGE_LIKED -> TrackListScreen(
            title = "Мне нравится",
            list = app.library.likedList,
            overline = "КОЛЛЕКЦИЯ",
            heroIcon = "favorite",
            heroShape = Shapes.Clover4,
            emptyIcon = "favorite",
            emptyTitle = "Пока пусто",
            emptyText = "Отмечайте треки сердечком — они появятся здесь",
            context = PlayContext.Liked,
            player = app.player,
            playerState = playerState,
            contentPadding = padding,
            onRetry = app.library::refresh,
            onBack = pop,
            downloads = app.downloads,
        )
        page == PAGE_DOWNLOADED -> TrackListScreen(
            title = "Скачанные",
            list = app.downloads.list,
            overline = "КОЛЛЕКЦИЯ",
            heroIcon = "download_done",
            heroShape = Shapes.Cookie6,
            emptyIcon = "download",
            emptyTitle = "Ничего не скачано",
            emptyText = "Кнопка скачивания — в шапке плейлиста или альбома",
            context = PlayContext.Liked,
            player = app.player,
            playerState = playerState,
            contentPadding = padding,
            onRetry = {},
            onBack = pop,
            downloads = app.downloads,
        )
        playlist != null -> TrackListScreen(
            title = playlist.title,
            list = remember(playlist.id) { app.library.tracksOf(playlist) },
            overline = "ПЛЕЙЛИСТ",
            heroImage = playlist.cover(400),
            emptyIcon = "queue_music",
            emptyTitle = "В плейлисте нет треков",
            context = PlayContext.playlist(playlist.id),
            player = app.player,
            playerState = playerState,
            contentPadding = padding,
            onRetry = { app.library.tracksOf(playlist, reload = true) },
            onBack = pop,
            downloads = app.downloads,
        )
        page == PAGE_SETTINGS -> SettingsScreen(app, padding, onBack = pop)
        page.startsWith(PAGE_ALBUM) -> {
            val id = page.removePrefix(PAGE_ALBUM)
            val data = remember(id) { app.catalog.album(id) }
            val album by data.info.collectAsStateWithLifecycle()
            TrackListScreen(
                title = album?.let { it.title + if (it.version.isNotEmpty()) " (${it.version})" else "" }.orEmpty(),
                list = data.list,
                overline = (album?.kind ?: "Альбом").uppercase(),
                subtitle = album?.year?.takeIf { it > 0 }?.toString().orEmpty(),
                heroImage = album?.cover(400).orEmpty(),
                heroIcon = "album",
                artists = album?.artistRefs.orEmpty(),
                onOpenArtist = { push(PAGE_ARTIST + it) },
                numbered = true,
                emptyIcon = "album",
                emptyTitle = "В альбоме нет треков",
                context = PlayContext.Album,
                player = app.player,
                playerState = playerState,
                contentPadding = padding,
                onRetry = { app.catalog.album(id, reload = true) },
                onBack = pop,
                downloads = app.downloads,
            )
        }
        page.startsWith(PAGE_ARTIST) -> {
            val id = page.removePrefix(PAGE_ARTIST)
            ArtistScreen(
                data = remember(id) { app.catalog.artist(id) },
                player = app.player,
                playerState = playerState,
                contentPadding = padding,
                onRetry = { app.catalog.artist(id, reload = true) },
                onBack = pop,
                onOpenAlbum = { push(PAGE_ALBUM + it.id) },
            )
        }
        tab == Tab.Wave -> WaveScreen(app.wave, app.player, playerState, padding)
        tab == Tab.Search -> SearchScreen(
            catalog = app.catalog,
            player = app.player,
            playerState = playerState,
            contentPadding = padding,
            onOpenAlbum = { push(PAGE_ALBUM + it.id) },
            onOpenArtist = { push(PAGE_ARTIST + it.id) },
        )
        else -> CollectionScreen(
            library = app.library,
            account = app.session.account,
            contentPadding = padding,
            onOpenSettings = { push(PAGE_SETTINGS) },
            onOpenLiked = { push(PAGE_LIKED) },
            downloadedCount = downloaded.size,
            onOpenDownloaded = { push(PAGE_DOWNLOADED) },
            onOpenPlaylist = { push(PAGE_PLAYLIST + it.id) },
        )
    }
}
