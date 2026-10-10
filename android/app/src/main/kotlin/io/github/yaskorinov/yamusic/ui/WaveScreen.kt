package io.github.yaskorinov.yamusic.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yaskorinov.yamusic.data.Wave
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState

/**
 * «Моя волна»: в центре — фигура с кнопкой (форму задаёт настроение, цвет — характер), ниже — настройки.
 * Фигуры вращаются, только пока волна звучит.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun WaveScreen(wave: Wave, player: PlayerConnection, playerState: PlayerState, contentPadding: PaddingValues) {
    val groups by wave.groups.collectAsStateWithLifecycle()
    val selection by wave.selection.collectAsStateWithLifecycle()
    val loading by wave.loading.collectAsStateWithLifecycle()
    val error by wave.error.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val active = playerState.wave
    val playing = active && playerState.playing

    val outer = when (selection["moodEnergy"]?.substringAfter(':')) {
        "active" -> MaterialShapes.Sunny
        "fun" -> MaterialShapes.Flower
        "calm" -> MaterialShapes.Clover8Leaf
        "sad" -> MaterialShapes.Puffy
        else -> MaterialShapes.Cookie12Sided
    }
    val (container, accent, onAccent) = when (selection["diversity"]?.substringAfter(':')) {
        "discover" -> Triple(colors.tertiaryContainer, colors.tertiary, colors.onTertiary)
        "popular" -> Triple(colors.secondaryContainer, colors.secondary, colors.onSecondary)
        else -> Triple(colors.primaryContainer, colors.primary, colors.onPrimary)
    }

    var rotation by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(playing) {
        if (!playing) return@LaunchedEffect
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            rotation = (rotation + (now - last) / 1e9f * DEGREES_PER_SECOND) % 360f
            last = now
        }
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(contentPadding),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            "Моя волна",
            Modifier.fillMaxWidth().padding(start = 20.dp, top = 12.dp, end = 20.dp),
            style = MaterialTheme.typography.displaySmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(20.dp))
        Box(
            Modifier
                .fillMaxWidth(0.74f)
                .aspectRatio(1f)
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {
                    if (active) player.togglePlay() else wave.start()
                },
            contentAlignment = Alignment.Center,
        ) {
            MorphShape(outer, container, Modifier.fillMaxSize(), rotation = { rotation })
            MorphShape(
                if (playing) MaterialShapes.Cookie6Sided else MaterialShapes.Clover4Leaf,
                accent.copy(alpha = 0.22f),
                Modifier.fillMaxSize(0.7f),
                rotation = { -rotation * 0.6f },
            )
            Box(Modifier.fillMaxSize(0.36f), contentAlignment = Alignment.Center) {
                MorphShape(if (playing) MaterialShapes.Cookie9Sided else MaterialShapes.Circle, accent, Modifier.fillMaxSize())
                if (loading) {
                    LoadingIndicator(Modifier.fillMaxSize(0.6f), color = onAccent)
                } else {
                    Symbol(if (playing) "pause" else "play_arrow", size = 44.dp, filled = true, tint = onAccent)
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        val track = playerState.track
        Text(
            when {
                error.isNotEmpty() -> error
                active && track != null -> "${track.title} — ${track.artists}"
                else -> "Музыка под ваш вкус и настроение"
            },
            Modifier.padding(horizontal = 24.dp),
            style = MaterialTheme.typography.bodyLarge,
            color = if (error.isEmpty()) colors.onSurfaceVariant else colors.error,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(20.dp))

        for (group in groups) {
            val (chip, onChip) = when (group.key) {
                "diversity" -> colors.tertiary to colors.onTertiary
                "moodEnergy" -> colors.secondary to colors.onSecondary
                "language" -> colors.inverseSurface to colors.inverseOnSurface
                else -> colors.primary to colors.onPrimary
            }
            Text(
                group.title,
                Modifier.fillMaxWidth().padding(start = 24.dp, top = 8.dp),
                style = MaterialTheme.typography.titleSmall,
                color = colors.onSurfaceVariant,
            )
            Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 20.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                for (item in group.items) {
                    FilterChip(
                        selected = selection[group.key] == item.seed,
                        onClick = { wave.select(group.key, item.seed, active) },
                        label = { Text(item.label) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = chip,
                            selectedLabelColor = onChip,
                        ),
                    )
                }
            }
        }
        Spacer(Modifier.height(16.dp))
    }
}

private const val DEGREES_PER_SECOND = 10f
