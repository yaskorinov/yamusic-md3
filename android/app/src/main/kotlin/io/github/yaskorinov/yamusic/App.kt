package io.github.yaskorinov.yamusic

import android.app.Application
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import coil3.request.crossfade
import io.github.yaskorinov.yamusic.api.YandexApi
import io.github.yaskorinov.yamusic.data.Library
import io.github.yaskorinov.yamusic.data.Session
import io.github.yaskorinov.yamusic.data.TokenStore
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import okhttp3.OkHttpClient

/** Общие для интерфейса и сервиса воспроизведения объекты — в одном экземпляре на процесс. */
class App : Application(), SingletonImageLoader.Factory {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    val http: OkHttpClient by lazy {
        OkHttpClient.Builder().connectTimeout(10, TimeUnit.SECONDS).readTimeout(20, TimeUnit.SECONDS).build()
    }
    val api by lazy { YandexApi(http) }
    val session by lazy { Session(scope, api, TokenStore(this)) }
    val library by lazy { Library(scope, api, session) }
    val player by lazy { PlayerConnection(this) }

    override fun onCreate() {
        super.onCreate()
        library // подписывается на вход
        session.start()
    }

    override fun newImageLoader(context: PlatformContext): ImageLoader =
        ImageLoader.Builder(context).crossfade(true).build()
}
