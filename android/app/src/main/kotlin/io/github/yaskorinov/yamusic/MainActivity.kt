package io.github.yaskorinov.yamusic

import android.content.Intent
import android.content.pm.ApplicationInfo
import android.os.Bundle
import androidx.activity.ComponentActivity
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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ShortNavigationBar
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import io.github.yaskorinov.yamusic.ui.CollectionScreen
import io.github.yaskorinov.yamusic.ui.LoginScreen
import io.github.yaskorinov.yamusic.ui.NowPlaying
import io.github.yaskorinov.yamusic.ui.PlayerBar
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
            YaTheme(seed = rememberCoverSeed(playerState.track?.cover(200).orEmpty())) {
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
            app.settings.noReport = intent.getBooleanExtra("noReport", false)
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
}

/** Страница внутри коллекции: её корень, «Мне нравится» или плейлист 'uid:kind'. */
private const val PAGE_ROOT = ""
private const val PAGE_LIKED = "liked"

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Home(app: App, playerState: PlayerState) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    var tab by rememberSaveable { mutableStateOf(Tab.Wave) }
    var page by rememberSaveable { mutableStateOf(PAGE_ROOT) }
    val hasTrack = playerState.track != null
    val playlists by app.library.playlists.collectAsStateWithLifecycle()

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
                                    if (tab == item) page = PAGE_ROOT // повторное нажатие — к корню раздела
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
                targetState = tab to page,
                transitionSpec = { fadeIn(tween(220, delayMillis = 60)) togetherWith fadeOut(tween(100)) },
                label = "page",
            ) { (shownTab, shownPage) ->
                val playlist = playlists.firstOrNull { it.id == shownPage }
                when {
                    shownTab == Tab.Wave -> WaveScreen(app.wave, app.player, playerState, padding)
                    shownPage == PAGE_LIKED -> TrackListScreen(
                        title = "Мне нравится",
                        list = app.library.likedList,
                        context = PlayContext.Liked,
                        player = app.player,
                        playerState = playerState,
                        contentPadding = padding,
                        onRetry = app.library::refresh,
                        onBack = { page = PAGE_ROOT },
                    )
                    playlist != null -> TrackListScreen(
                        title = playlist.title,
                        list = remember(playlist.id) { app.library.tracksOf(playlist) },
                        context = PlayContext.playlist(playlist.id),
                        player = app.player,
                        playerState = playerState,
                        contentPadding = padding,
                        onRetry = { app.library.tracksOf(playlist, reload = true) },
                        onBack = { page = PAGE_ROOT },
                    )
                    else -> CollectionScreen(
                        library = app.library,
                        session = app.session,
                        contentPadding = padding,
                        onOpenLiked = { page = PAGE_LIKED },
                        onOpenPlaylist = { page = it.id },
                    )
                }
            }
        }
        AnimatedVisibility(
            visible = hasTrack && expanded,
            enter = slideInVertically { it },
            exit = slideOutVertically { it },
        ) {
            NowPlaying(playerState, app.player, app.library, onClose = { expanded = false })
        }
    }
    BackHandler(enabled = expanded || (tab == Tab.Collection && page != PAGE_ROOT)) {
        if (expanded) expanded = false else page = PAGE_ROOT
    }
}
