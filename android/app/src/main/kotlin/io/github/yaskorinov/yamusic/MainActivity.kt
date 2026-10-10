package io.github.yaskorinov.yamusic

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.data.AuthState
import io.github.yaskorinov.yamusic.playback.PlayerState
import io.github.yaskorinov.yamusic.ui.LikedScreen
import io.github.yaskorinov.yamusic.ui.LoginScreen
import io.github.yaskorinov.yamusic.ui.NowPlaying
import io.github.yaskorinov.yamusic.ui.PlayerBar
import io.github.yaskorinov.yamusic.ui.YaTheme
import io.github.yaskorinov.yamusic.ui.rememberCoverSeed

class MainActivity : ComponentActivity() {
    private val app get() = application as App

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
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

@Composable
private fun Home(app: App, playerState: PlayerState) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    val hasTrack = playerState.track != null
    val navigationBar = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()

    // Surface, а не просто фон: он задаёт цвет текста и значков по умолчанию
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Box(Modifier.fillMaxSize()) {
            LikedScreen(
                library = app.library,
                session = app.session,
                player = app.player,
                playerState = playerState,
                bottomPadding = navigationBar + if (hasTrack) BAR_SPACE else 0.dp,
            )
            AnimatedVisibility(
                visible = hasTrack && !expanded,
                modifier = Modifier.align(Alignment.BottomCenter),
                enter = slideInVertically { it } + fadeIn(),
                exit = slideOutVertically { it } + fadeOut(),
            ) {
                PlayerBar(
                    state = playerState,
                    player = app.player,
                    onOpen = { expanded = true },
                    modifier = Modifier.navigationBarsPadding().padding(horizontal = 12.dp, vertical = 8.dp),
                )
            }
            AnimatedVisibility(
                visible = hasTrack && expanded,
                enter = slideInVertically { it },
                exit = slideOutVertically { it },
            ) {
                NowPlaying(playerState, app.player, app.library, onClose = { expanded = false })
            }
        }
    }
    BackHandler(enabled = expanded) { expanded = false }
}

private val BAR_SPACE = 104.dp
