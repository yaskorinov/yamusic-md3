package io.github.yaskorinov.yamusic.ui

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.yaskorinov.yamusic.data.AuthState
import io.github.yaskorinov.yamusic.data.Session
import kotlinx.coroutines.delay

/** Вход: код показываем здесь, вводит его пользователь сам — на странице Яндекса. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LoginScreen(state: AuthState, session: Session) {
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Column(
            Modifier.safeDrawingPadding().padding(horizontal = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Box(
                Modifier.size(132.dp).background(MaterialTheme.colorScheme.primary, MaterialShapes.Cookie9Sided.toShape()),
                contentAlignment = Alignment.Center,
            ) {
                Symbol("play_arrow", size = 64.dp, filled = true, tint = MaterialTheme.colorScheme.onPrimary)
            }
            Spacer(Modifier.height(28.dp))
            Text("YaMusic", style = MaterialTheme.typography.displaySmall, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.height(8.dp))

            when (state) {
                is AuthState.AwaitingUser -> CodeStep(state, session)
                is AuthState.SignedOut -> Welcome(state.error, "Войти через Яндекс", session::startDeviceLogin)
                is AuthState.Failed -> Welcome(state.error, "Повторить", session::retry)
                else -> {
                    Spacer(Modifier.height(32.dp))
                    LoadingIndicator(Modifier.size(64.dp))
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Welcome(error: String, action: String, onAction: () -> Unit) {
    Text(
        error.ifEmpty { "Клиент Яндекс Музыки в стиле Material 3 Expressive" },
        style = MaterialTheme.typography.bodyLarge,
        color = if (error.isEmpty()) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.error,
        textAlign = TextAlign.Center,
    )
    Spacer(Modifier.height(32.dp))
    val height = ButtonDefaults.MediumContainerHeight
    Button(
        onClick = onAction,
        shapes = ButtonDefaults.shapesFor(height),
        modifier = Modifier.fillMaxWidth().height(height),
        contentPadding = ButtonDefaults.contentPaddingFor(height),
    ) {
        Text(action, style = ButtonDefaults.textStyleFor(height))
    }
}

@Composable
private fun CodeStep(state: AuthState.AwaitingUser, session: Session) {
    val context = LocalContext.current
    val left by produceState(remaining(state.expiresAtMs), state.expiresAtMs) {
        while (value > 0) {
            delay(1000)
            value = remaining(state.expiresAtMs)
        }
    }
    Text(
        "Откройте ya.ru/device и введите код",
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )
    Spacer(Modifier.height(20.dp))
    Surface(color = MaterialTheme.colorScheme.secondaryContainer, shape = MaterialTheme.shapes.extraLarge) {
        Text(
            state.code,
            Modifier.padding(horizontal = 28.dp, vertical = 16.dp),
            style = MaterialTheme.typography.displayMedium,
            fontWeight = FontWeight.Bold,
            letterSpacing = 6.sp,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
        )
    }
    Spacer(Modifier.height(12.dp))
    Text(
        "Код действует ещё %d:%02d".format(left / 60, left % 60),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(28.dp))
    Button(
        onClick = {
            copy(context, state.code)
            open(context, state.url)
        },
        modifier = Modifier.fillMaxWidth().height(56.dp),
    ) {
        Symbol("open_in_new", size = 20.dp)
        Spacer(Modifier.size(8.dp))
        Text("Скопировать код и открыть страницу")
    }
    Spacer(Modifier.height(8.dp))
    FilledTonalButton(onClick = { copy(context, state.code) }, modifier = Modifier.fillMaxWidth().height(56.dp)) {
        Symbol("content_copy", size = 20.dp)
        Spacer(Modifier.size(8.dp))
        Text("Только скопировать код")
    }
    Spacer(Modifier.height(4.dp))
    TextButton(onClick = session::cancelLogin) { Text("Отмена") }
}

private fun remaining(expiresAtMs: Long): Long = ((expiresAtMs - System.currentTimeMillis()) / 1000).coerceAtLeast(0)

private fun copy(context: Context, text: String) {
    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Код входа", text))
}

private fun open(context: Context, url: String) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
    } catch (_: ActivityNotFoundException) {
        // браузера нет — код уже в буфере, страницу можно открыть на другом устройстве
    }
}
