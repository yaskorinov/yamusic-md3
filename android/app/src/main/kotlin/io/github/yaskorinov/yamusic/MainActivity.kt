package io.github.yaskorinov.yamusic

import android.content.Intent
import android.content.pm.ApplicationInfo
import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ShortNavigationBar
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.data.AuthState
import io.github.yaskorinov.yamusic.playback.PlayContext
import io.github.yaskorinov.yamusic.playback.PlayerState
import io.github.yaskorinov.yamusic.ui.ArtistScreen
import io.github.yaskorinov.yamusic.ui.CollectionScreen
import io.github.yaskorinov.yamusic.ui.DefaultSeed
import io.github.yaskorinov.yamusic.ui.LocalDownloaded
import io.github.yaskorinov.yamusic.ui.LoginScreen
import io.github.yaskorinov.yamusic.ui.NowPlaying
import io.github.yaskorinov.yamusic.ui.PlayerBar
import io.github.yaskorinov.yamusic.ui.SearchScreen
import io.github.yaskorinov.yamusic.ui.SettingsScreen
import io.github.yaskorinov.yamusic.ui.Symbol
import io.github.yaskorinov.yamusic.ui.TrackListScreen
import io.github.yaskorinov.yamusic.ui.WaveScreen
import io.github.yaskorinov.yamusic.ui.YaTheme
import io.github.yaskorinov.yamusic.ui.rememberCoverSeed

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
            val accentFromCover by app.settings.accentFromCover.flow.collectAsStateWithLifecycle()
            val dark = when (themeMode) {
                "dark" -> true
                "light" -> false
                else -> isSystemInDarkTheme()
            }
            // Значки системных панелей — под тему приложения, а не системы
            DisposableEffect(dark) {
                val style = if (dark) SystemBarStyle.dark(Color.TRANSPARENT) else SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
                enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
                onDispose { }
            }
            val coverSeed = rememberCoverSeed(playerState.track?.cover(200).orEmpty())
            YaTheme(seed = if (accentFromCover) coverSeed else DefaultSeed, dark = dark) {
                when (val state = auth) {
                    is AuthState.SignedIn -> Home(app, playerState)
                    else -> LoginScreen(state, app.session)
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

private enum class Tab(val title: String, val icon: String) {
    Wave("Волна", "graphic_eq"),
    Collection("Коллекция", "queue_music"),
    Search("Поиск", "search"),
}

// Страницы поверх корня вкладки; путь вкладки — они же через перевод строки (так он переживает поворот экрана)
private const val PAGE_LIKED = "liked"
private const val PAGE_SETTINGS = "settings"
private const val PAGE_DOWNLOADED = "downloaded"
private const val PAGE_PLAYLIST = "playlist:"
private const val PAGE_ALBUM = "album:"
private const val PAGE_ARTIST = "artist:"

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Home(app: App, playerState: PlayerState) {
    var expanded by rememberSaveable { mutableStateOf(false) }
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

    val downloadedIds by app.downloads.done.collectAsStateWithLifecycle()
    CompositionLocalProvider(LocalDownloaded provides downloadedIds.keys) {
    Box(Modifier.fillMaxSize()) {
        Scaffold(
            containerColor = MaterialTheme.colorScheme.surface,
            bottomBar = {
                Column {
                    AnimatedVisibility(hasTrack, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
                        PlayerBar(
                            state = playerState,
                            player = app.player,
                            onOpen = { expanded = true },
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                        )
                    }
                    ShortNavigationBar {
                        for (item in Tab.entries) {
                            ShortNavigationBarItem(
                                selected = tab == item,
                                onClick = {
                                    if (tab == item) setPath("") // повторное нажатие — к корню раздела
                                    tab = item
                                },
                                icon = { Symbol(item.icon, filled = tab == item) },
                                label = { Text(item.title) },
                            )
                        }
                    }
                }
            },
        ) { padding ->
            AnimatedContent(
                targetState = tab to path.substringAfterLast('\n'),
                transitionSpec = { fadeIn(tween(220, delayMillis = 60)) togetherWith fadeOut(tween(100)) },
                label = "page",
            ) { (shownTab, page) ->
                Page(app, playerState, shownTab, page, padding, push, pop)
            }
        }
        AnimatedVisibility(
            visible = hasTrack && expanded,
            enter = slideInVertically { it },
            exit = slideOutVertically { it },
        ) {
            NowPlaying(
                state = playerState,
                app = app,
                onClose = { expanded = false },
                onOpenPage = {
                    expanded = false
                    if (path.substringAfterLast('\n') != it) push(it)
                },
            )
        }
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
                title = album?.title.orEmpty(),
                subtitle = listOfNotNull(album?.artists, album?.year?.takeIf { it > 0 }?.toString())
                    .filter { it.isNotEmpty() }.joinToString(" · "),
                numbered = true,
                list = data.list,
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
            contentPadding = padding,
            onOpenSettings = { push(PAGE_SETTINGS) },
            onOpenLiked = { push(PAGE_LIKED) },
            downloadedCount = downloaded.size,
            onOpenDownloaded = { push(PAGE_DOWNLOADED) },
            onOpenPlaylist = { push(PAGE_PLAYLIST + it.id) },
        )
    }
}
