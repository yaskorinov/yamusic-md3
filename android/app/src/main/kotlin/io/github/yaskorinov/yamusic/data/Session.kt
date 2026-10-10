package io.github.yaskorinov.yamusic.data

import android.os.Build
import io.github.yaskorinov.yamusic.api.ApiException
import io.github.yaskorinov.yamusic.api.YandexApi
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class Account(val uid: String, val login: String, val displayName: String, val hasPlus: Boolean)

sealed interface AuthState {
    data object Checking : AuthState
    data class SignedOut(val error: String = "") : AuthState
    data object RequestingCode : AuthState
    data class AwaitingUser(val code: String, val url: String, val expiresAtMs: Long) : AuthState
    data object SigningIn : AuthState
    data class SignedIn(val account: Account) : AuthState

    /** Сбой, после которого имеет смысл «Повторить» (нет сети, Яндекс не отвечает). */
    data class Failed(val error: String) : AuthState
}

/** Вход в Яндекс: OAuth Device Flow — код вводит сам пользователь на ya.ru/device. */
class Session(
    private val scope: CoroutineScope,
    private val api: YandexApi,
    private val store: TokenStore,
) {
    private val _state = MutableStateFlow<AuthState>(AuthState.Checking)
    val state = _state.asStateFlow()

    val account: Account? get() = (_state.value as? AuthState.SignedIn)?.account

    private var job: Job? = null

    /**
     * Восстановить сессию при запуске. Если аккаунт уже известен, приложение открывается сразу,
     * а токен проверяется следом: без сети остаёмся в аккаунте (скачанное играет и так).
     */
    fun start() = attempt {
        _state.value = AuthState.Checking
        val token = withContext(Dispatchers.IO) { store.load() }
        if (token == null) {
            _state.value = AuthState.SignedOut()
            return@attempt
        }
        val known = withContext(Dispatchers.IO) { store.account }?.let {
            runCatching { Json.decodeFromString(Account.serializer(), it) }.getOrNull()
        }
        if (known == null) {
            signIn(token, save = false)
            return@attempt
        }
        api.token = token
        _state.value = AuthState.SignedIn(known)
        try {
            signIn(token, save = false, quiet = true)
        } catch (e: ApiException) {
            if (e.unauthorized) throw e // токен отозван — выходим, как обычно
        } catch (e: IOException) {
            // нет сети — работаем с тем, что сохранено
        }
    }

    fun retry() = start()

    fun startDeviceLogin() = attempt {
        _state.value = AuthState.RequestingCode
        val code = api.requestDeviceCode("YaMusic (${Build.MODEL})")
        val expiresAt = System.currentTimeMillis() + code.expiresIn * 1000L
        _state.value = AuthState.AwaitingUser(code.userCode, code.verificationUrl, expiresAt)
        val interval = maxOf(code.interval, 2) * 1000L
        while (System.currentTimeMillis() < expiresAt) {
            delay(interval)
            val token = api.pollDeviceToken(code.deviceCode) ?: continue
            signIn(token, save = true)
            return@attempt
        }
        _state.value = AuthState.SignedOut("Время действия кода истекло")
    }

    fun cancelLogin() {
        job?.cancel()
        if (_state.value is AuthState.RequestingCode || _state.value is AuthState.AwaitingUser) {
            _state.value = AuthState.SignedOut()
        }
    }

    fun signOut() {
        job?.cancel()
        api.token = null
        scope.launch(Dispatchers.IO) { store.clear() }
        _state.value = AuthState.SignedOut()
    }

    /** [quiet] — не показывать «входим»: приложение уже открыто по сохранённому аккаунту. */
    private suspend fun signIn(token: String, save: Boolean, quiet: Boolean = false) {
        if (!quiet) _state.value = AuthState.SigningIn
        api.token = token
        val status = api.accountStatus()
        val info = status.account
        val account = Account(
            uid = info.uid,
            login = info.login,
            displayName = info.displayName.ifEmpty { info.fullName }.ifEmpty { info.login },
            hasPlus = status.plus.hasPlus,
        )
        withContext(Dispatchers.IO) {
            if (save) store.save(token)
            store.account = Json.encodeToString(Account.serializer(), account)
        }
        _state.value = AuthState.SignedIn(account)
    }

    /** Одна операция входа за раз; её ошибки превращаются в состояние. */
    private fun attempt(block: suspend () -> Unit) {
        job?.cancel()
        job = scope.launch {
            try {
                block()
            } catch (e: ApiException) {
                _state.value = when {
                    e.unauthorized -> {
                        api.token = null
                        withContext(Dispatchers.IO) { store.clear() }
                        AuthState.SignedOut("Токен недействителен или отозван — войдите заново")
                    }
                    e.code == "expired_token" -> AuthState.SignedOut("Время действия кода истекло")
                    e.status == 400 -> AuthState.SignedOut("Вход отклонён: ${e.message}")
                    else -> AuthState.Failed("Ошибка Яндекс Музыки: ${e.message}")
                }
            } catch (e: IOException) {
                _state.value = AuthState.Failed("Нет связи с Яндексом. Проверьте подключение к интернету")
            }
        }
    }
}
