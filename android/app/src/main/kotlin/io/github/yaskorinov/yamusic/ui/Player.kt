package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.EaseInCubic
import androidx.compose.animation.core.EaseInQuad
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.EaseOutQuad
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.size.Size as CoilSize
import io.github.yaskorinov.yamusic.App
import io.github.yaskorinov.yamusic.api.Track
import io.github.yaskorinov.yamusic.playback.PlayerConnection
import io.github.yaskorinov.yamusic.playback.PlayerState
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** Позиция играющего трека, мс — для подписей времени: плеер её не присылает, поэтому опрашиваем. */
@Composable
private fun rememberPosition(player: PlayerConnection, state: PlayerState): State<Long> =
    produceState(player.positionMs, state.track?.id, state.playing) {
        while (true) {
            value = player.positionMs
            delay(if (state.playing) 200 else 300)
        }
    }

/**
 * Плавающий мини-плеер-пилюля: обложка в фигуре (рамка вращается, пока играет), название, волнистый
 * прогресс, лайк и управление. Нажатие раскрывает полный плеер — [onOpen] получает центр обложки,
 * из которого вырастает «окно». Пилюлю можно смахнуть: влево — следующий трек, вправо — предыдущий;
 * она уезжает за пальцем, а следом въезжает пилюля соседнего трека.
 */
@Composable
fun MiniPlayer(state: PlayerState, app: App, onOpen: (Offset) -> Unit, modifier: Modifier = Modifier) {
    val track = state.track ?: return
    val theme = LocalTheme.current
    val player = app.player
    val queue by player.queue.collectAsStateWithLifecycle()
    val next = queue.getOrNull(state.nextIndex)
    val previous = queue.getOrNull(state.previousIndex)
    val scope = rememberCoroutineScope()
    var offset by remember { mutableFloatStateOf(0f) }
    // Куда тянут: -1 — к следующему (пилюля едет влево), 1 — к предыдущему
    val pull by remember { derivedStateOf { if (offset < 0f) -1 else if (offset > 0f) 1 else 0 } }
    // Трек, на который только что перелистнули: плеер сообщит о смене чуть позже, а пилюля уже на месте
    var held by remember { mutableStateOf<Track?>(null) }
    LaunchedEffect(held, track.id) {
        if (held == null) return@LaunchedEffect
        if (held?.id != track.id) delay(1500) // плеер так и не переключился — показываем, что есть
        held = null
    }
    val shown = held?.takeIf { it.id != track.id } ?: track
    var width by remember { mutableIntStateOf(0) }
    val density = LocalDensity.current
    val gap = with(density) { 12.dp.toPx() }
    val flick = with(density) { 200.dp.toPx() } // бросок: хватает короткого быстрого движения
    var cover by remember { mutableStateOf(Offset.Zero) }
    // Доводка после отпускания и перелистывание, которое она завершит: трек и направление
    var settle by remember { mutableStateOf<Job?>(null) }
    var turning by remember { mutableStateOf<Pair<Track, Boolean>?>(null) }
    // Сам жест: идёт ли он и сколько пилюля прошла под пальцем
    var dragging by remember { mutableStateOf(false) }
    var travel by remember { mutableFloatStateOf(0f) }
    // Соседняя пилюля становится текущей: подменяем их в одном кадре и только тогда переключаем трек.
    // [at] — где она в этот миг стоит (0 — доехала до места)
    fun turn(at: Float) {
        val (to, forward) = turning ?: return
        turning = null
        held = to
        offset = at
        if (forward) player.next() else player.previousItem()
    }
    Box(
        modifier.fillMaxWidth().height(72.dp).onSizeChanged { width = it.width }.draggable(
            state = rememberDraggableState { delta ->
                if (!dragging) {
                    // новое касание не ждёт конца доводки: перелистывание завершается сразу, а пилюля остаётся под пальцем.
                    // Делается здесь, а не в onDragStarted: тот срабатывает на кадр позже, и начало движения терялось бы
                    dragging = true
                    travel = 0f
                    settle?.cancel()
                    settle = null
                    turning?.let { (_, forward) -> turn(offset + if (forward) width + gap else -(width + gap)) }
                }
                // тянуть некуда — пилюля поддаётся, но туго
                val stuck = (offset + delta < 0f && next == null) || (offset + delta > 0f && previous == null)
                offset += if (stuck) delta * 0.2f else delta
                travel += delta
            },
            orientation = Orientation.Horizontal,
            onDragStopped = { velocity ->
                dragging = false
                val step = width + gap
                // судим по самому жесту, а не по месту пилюли: её могли подхватить посреди доводки
                val moved = travel
                val forward = moved < 0f
                val along = if (forward) -velocity else velocity // скорость в сторону перелистывания
                val far = along > -flick && (abs(moved) > step * 0.2f || along > flick)
                // сосед — из самого плеера: после только что завершённого перелистывания состояние ещё старое
                val to = if (far) player.neighbour(forward) else null
                turning = to?.let { it to forward }
                settle = scope.launch {
                    animate(offset, if (to == null) 0f else if (forward) -step else step, velocity, spring(dampingRatio = 0.9f, stiffness = 420f)) { value, _ ->
                        offset = value
                    }
                    turn(0f)
                    settle = null
                }
            },
        ),
    ) {
        Pill(
            track = shown,
            state = state,
            app = app,
            current = true,
            onOpen = { onOpen(cover) },
            modifier = Modifier.graphicsLayer { translationX = offset },
            coverModifier = Modifier.onGloballyPositioned {
                cover = it.boundsInRoot().center
                theme.miniCover = it
            },
        )
        val neighbour = if (pull < 0) next else if (pull > 0) previous else null
        if (neighbour != null) {
            Pill(
                track = neighbour,
                state = state,
                app = app,
                current = false,
                onOpen = null,
                modifier = Modifier.graphicsLayer { translationX = offset - pull * (width + gap) },
            )
        }
    }
}

/**
 * Сама пилюля. [current] — она стоит на месте играющего трека: вращается рамка, показаны ошибка и загрузка;
 * иначе это сосед по очереди. Прогресс идёт, только когда в ней и правда играющий трек.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Pill(
    track: Track,
    state: PlayerState,
    app: App,
    current: Boolean,
    onOpen: (() -> Unit)?,
    modifier: Modifier,
    coverModifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val player = app.player
    val coverSpin by app.settings.coverSpin.flow.collectAsStateWithLifecycle()
    val spin = rememberSpin(current && coverSpin && state.playing)
    val live = track.id == state.track?.id
    Surface(
        onClick = { onOpen?.invoke() },
        enabled = onOpen != null,
        modifier = modifier.fillMaxWidth().height(72.dp),
        shape = CircleShape,
        color = colors.surfaceContainerHighest,
        shadowElevation = 3.dp,
    ) {
        Row(Modifier.padding(start = 12.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            MorphImage(
                track.cover(200),
                if (state.playing) Shapes.Cookie12 else Shapes.SoftSquare,
                Modifier.size(48.dp).graphicsLayer().then(coverModifier),
                rotation = spin,
            )
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                val error = state.error.takeIf { live }.orEmpty()
                Text(track.title, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    error.ifEmpty { track.artists },
                    style = MaterialTheme.typography.bodySmall,
                    color = if (error.isEmpty()) colors.onSurfaceVariant else colors.error,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                WavyProgress(
                    value = { if (live) fraction(player.positionMs, state.durationMs) else 0f },
                    wavy = live && state.playing,
                    modifier = Modifier.fillMaxWidth().height(14.dp),
                    thickness = 3.dp,
                    amplitude = 2.dp,
                    wavelength = 28.dp,
                )
            }
            LikeButton(track, app.library)
            Box(Modifier.size(52.dp), contentAlignment = Alignment.Center) {
                PlayButton(state.playing, player::togglePlay, size = 52.dp, spin = current)
                if (current && state.buffering) LoadingIndicator(Modifier.requiredSize(60.dp).alpha(0.5f), color = colors.primary)
            }
            IconButton(onClick = { player.next() }, enabled = state.hasNext) { Symbol("skip_next", filled = true) }
        }
    }
}

/**
 * Полноэкранный плеер. Раскрывается через «окно» — случайную фигуру MD3 Expressive: фаза 1 — фигура
 * быстро вращается и растёт из маленькой (на обложке мини-плеера, [origin]) до средней, уходя к центру;
 * фаза 2 — растёт на весь экран, пока весь экран не окажется внутри неё. Закрытие — тот же путь обратно.
 * [tab] — что показано вместо обложки: "" — обложка, lyrics — текст, queue — очередь; выбор хранит вызывающий,
 * так что открытый текст остаётся открытым и после сворачивания плеера.
 * [onOpenPage] — перейти на страницу исполнителя или альбома (плеер при этом сворачивается).
 */
@Composable
fun NowPlaying(
    open: Boolean,
    origin: Offset,
    state: PlayerState,
    app: App,
    direction: Int,
    tab: String,
    onTab: (String) -> Unit,
    onClose: () -> Unit,
    onOpenPage: (String) -> Unit,
) {
    val reveal = remember { Animatable(0f) }
    var window by remember { mutableStateOf(Shapes.Cookie9) }
    LaunchedEffect(open) {
        if (open && reveal.value == 0f) window = WINDOW_SHAPES.random()
        // кривые — у каждой фазы свои, см. ниже
        reveal.animateTo(if (open) 1f else 0f, tween(if (open) 820 else 620, easing = LinearEasing))
    }
    val visible by remember { derivedStateOf { reveal.value > 0f } }
    val track = state.track
    if (track == null || !(visible || open)) return

    val colors = MaterialTheme.colorScheme
    val theme = LocalTheme.current
    val path = remember { Path() }
    Box(
        Modifier
            .fillMaxSize()
            // в самом конце сворачивания содержимое гаснет — без «обрубка» в крошечной фигуре
            .graphicsLayer { alpha = min(1f, reveal.value / 0.12f) }
            .drawWithContent {
                val r = reveal.value
                if (r >= 1f) return@drawWithContent drawContent()
                val e1 = inOut(min(1f, r / SPLIT))
                val e2 = inOut(max(0f, (r - SPLIT) / (1 - SPLIT)))
                val small = 36.dp.toPx()
                val medium = 0.42f * size.minDimension
                // во впадинах фигура уже описанного круга — берём с запасом, чтобы углы экрана оказались внутри
                val full = 2 * 1.6f * hypot(size.width / 2, size.height / 2)
                val d = if (r > SPLIT) medium + (full - medium) * e2 else small + (medium - small) * e1
                val at = Offset(origin.x + (size.width / 2 - origin.x) * e1, origin.y + (size.height / 2 - origin.y) * e1)
                path.outline(window.radii, window.radii, 0f, Size(d, d), 240 * e1 + 50 * e2, center = at)
                clipPath(path) { this@drawWithContent.drawContent() }
            }
            .pointerInput(Unit) { detectTapGestures { } } // не пропускать нажатия к приложению под плеером
            .background(colors.surface),
    ) {
        val settings = app.settings
        val mode by settings.backdropMode.flow.collectAsStateWithLifecycle()
        val blur by settings.nowPlayingBlur.flow.collectAsStateWithLifecycle()
        val drift by settings.nowPlayingDrift.flow.collectAsStateWithLifecycle()
        // Фон: размытая обложка + вуаль цвета темы
        CoverBackdrop(mode, blur, drift, running = open && state.playing, modifier = Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(colors.surface.copy(alpha = if (theme.dark) 0.62f else 0.55f)))

        val shift = with(LocalDensity.current) { 80.dp.roundToPx() }
        Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            // Свернуть + переключатель текст/очередь: повторное нажатие возвращает обложку
            Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                FilledTonalIconButton(onClick = onClose) { Symbol("keyboard_arrow_down") }
                Spacer(Modifier.weight(1f))
                Segments(
                    listOf(Segment(icon = "title"), Segment(icon = "queue_music")),
                    selected = TABS.indexOf(tab),
                    onSelect = { onTab(if (tab == TABS[it]) "" else TABS[it]) },
                )
            }
            // Смена вкладки — shared axis X: текст слева, очередь справа
            AnimatedContent(
                targetState = tab,
                modifier = Modifier.weight(1f).fillMaxWidth(),
                transitionSpec = {
                    val spec = tween<androidx.compose.ui.unit.IntOffset>(460, easing = Motion.EmphasizedDecelerate)
                    (fadeIn(tween(460, easing = Motion.EmphasizedDecelerate)) + slideInHorizontally(spec) { shift * side(targetState) }) togetherWith
                        (fadeOut(tween(Motion.EffectsDefault)) + slideOutHorizontally(spec) { shift * side(initialState) })
                },
                label = "tab",
            ) { shown ->
                when (shown) {
                    "" -> CoverPane(track, state, app, open, direction, onOpenPage)
                    else -> Column {
                        Row(Modifier.fillMaxWidth().padding(top = 12.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            MorphImage(
                                track.cover(200), if (state.playing) Shapes.Cookie12 else Shapes.SoftSquare,
                                Modifier.size(56.dp).onGloballyPositioned { theme.smallCover = it }, // отсюда расходится «волна» смены темы
                            )
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(track.title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                ArtistLink(track, state.error, MaterialTheme.typography.bodyMedium, onOpenPage)
                            }
                        }
                        if (shown == "lyrics") {
                            LyricsView(track, app.lyrics, app.player, state.playing, Modifier.fillMaxSize())
                        } else {
                            QueuePane(app.player, state)
                        }
                    }
                }
            }
            Seek(state, app.player)
            Controls(state, app.player)
            // Не рекомендовать · лайк
            Row(Modifier.padding(top = 4.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(40.dp)) {
                IconButton(onClick = {
                    app.library.dislike(track)
                    app.player.next()
                }) { Symbol("thumb_down", tint = colors.onSurfaceVariant) }
                LikeButton(track, app.library)
            }
        }
    }
}

/** Обложка и подписи. Подписи при смене трека проявляются заново, чуть снизу. */
@Composable
private fun CoverPane(track: Track, state: PlayerState, app: App, open: Boolean, direction: Int, onOpenPage: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val theme = LocalTheme.current
    val coverSpin by app.settings.coverSpin.flow.collectAsStateWithLifecycle()
    val coverPulse by app.settings.coverPulse.flow.collectAsStateWithLifecycle()

    // «Дыхание» в такт: уровень звука от плеера; доля над скользящим средним — удар
    val beat = remember { Animatable(0f) }
    val listening = coverPulse && open && state.playing
    LaunchedEffect(listening) {
        if (!listening) return@LaunchedEffect beat.snapTo(0f)
        var average = 0f
        app.levels.wanted = true
        try {
            app.levels.level.collectLatest { level ->
                average += (level - average) * 0.06f
                beat.animateTo(((level - average) * 4f).coerceIn(0f, 1f), tween(110, easing = EaseOutQuad))
            }
        } finally {
            app.levels.wanted = false
        }
    }

    val info = remember { Animatable(1f) }
    var shownId by remember { mutableStateOf(track.id) }
    LaunchedEffect(track.id) {
        if (shownId == track.id) return@LaunchedEffect
        shownId = track.id
        info.snapTo(0f)
        info.animateTo(1f, tween(520, easing = Motion.EmphasizedDecelerate))
    }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        // Подписи занимают ~130 dp — обложка берёт оставшееся
        val side = minOf(maxWidth, maxHeight - 150.dp, 440.dp).coerceAtLeast(160.dp)
        Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            NowPlayingCover(
                url = track.cover(800),
                restShape = if (state.playing) Shapes.Cookie12 else Shapes.Square,
                direction = direction,
                pulse = { beat.value },
                spinning = coverSpin && state.playing && open,
                modifier = Modifier.size(side).onGloballyPositioned { theme.bigCover = it },
            )
            Spacer(Modifier.height(20.dp))
            Text(
                track.title,
                Modifier.graphicsLayer {
                    alpha = info.value
                    translationY = (1 - info.value) * 14.dp.toPx()
                },
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            // чуть позже названия
            Column(
                Modifier.graphicsLayer {
                    alpha = max(0f, info.value * 1.3f - 0.3f)
                    translationY = (1 - info.value) * 18.dp.toPx()
                },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                ArtistLink(track, state.error, MaterialTheme.typography.titleMedium, onOpenPage)
                if (track.album.isNotEmpty()) {
                    Text(
                        track.album,
                        Modifier.clip(CircleShape).clickable(enabled = track.albumId.isNotEmpty()) { onOpenPage("album:${track.albumId}") }.padding(horizontal = 8.dp, vertical = 2.dp),
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

/**
 * Обложка полноэкранного плеера. Смена трека — морф через фигуру: старая обложка, вращаясь, сжимается
 * в маленькую «печеньку» и тает; новая вырастает из неё, морфясь из цветка в свою фигуру (с лёгким
 * перелётом). Две обложки — двойной буфер: новая показывается, когда загрузилась.
 * [pulse] — «дыхание» фигуры в такт, [spinning] — медленное вращение рамки (обложка внутри неподвижна),
 * [direction] — +1 вперёд по очереди, -1 назад: куда вращаться.
 */
@Composable
private fun NowPlayingCover(url: String, restShape: PolarShape, direction: Int, pulse: () -> Float, spinning: Boolean, modifier: Modifier) {
    val context = LocalContext.current.applicationContext
    val layers = remember { arrayOf(CoverLayer(restShape), CoverLayer(restShape)) }
    var front by remember { mutableIntStateOf(0) }
    val spin = rememberSpin(spinning)
    LaunchedEffect(restShape, front) { layers[front].morph.morphTo(restShape, Motion.SpatialSlow) }
    LaunchedEffect(url) {
        val old = layers[front]
        val new = layers[1 - front]
        if (old.url.isEmpty()) { // первая обложка — без анимации
            old.url = url
            return@LaunchedEffect
        }
        if (old.url == url) return@LaunchedEffect
        withTimeoutOrNull(1500) {
            SingletonImageLoader.get(context).execute(ImageRequest.Builder(context).data(url).size(CoilSize.ORIGINAL).build())
        }
        new.url = url
        new.morph.jumpTo(Shapes.Flower8)
        new.scale.snapTo(0.2f)
        new.rotation.snapTo(120f * direction)
        new.alpha.snapTo(0f)
        front = 1 - front // отсюда новая морфится в свою фигуру: цветок → restShape
        coroutineScope {
            launch { old.morph.morphTo(Shapes.Cookie4, Motion.SpatialSlow) }
            launch { old.scale.animateTo(0.15f, tween(380, easing = EaseInCubic)) }
            launch { old.rotation.animateTo(-90f * direction, tween(380, easing = EaseInCubic)) }
            launch { old.alpha.animateTo(0f, tween(380, easing = EaseInQuad)) }
            delay(140)
            launch { new.scale.animateTo(1f, tween(680, easing = Motion.outBack(1.25f))) }
            launch { new.rotation.animateTo(0f, tween(680, easing = EaseOutCubic)) }
            launch { new.alpha.animateTo(1f, tween(220, easing = EaseOutQuad)) }
        }
    }
    Box(modifier) {
        layers.forEachIndexed { index, layer ->
            if (layer.url.isNotEmpty()) {
                MorphImage(
                    layer.url,
                    restShape,
                    Modifier.fillMaxSize().zIndex(if (index == front) 1f else 0f).graphicsLayer {
                        scaleX = layer.scale.value
                        scaleY = layer.scale.value
                        rotationZ = layer.rotation.value
                        alpha = layer.alpha.value
                    },
                    placeholderSize = 72.dp,
                    morph = layer.morph,
                    rotation = if (index == front) spin else NoMotion,
                    pulse = if (index == front) pulse else NoMotion,
                )
            }
        }
    }
}

private class CoverLayer(shape: PolarShape) {
    var url by mutableStateOf("")
    val morph = MorphState(shape)
    val scale = Animatable(1f)
    val rotation = Animatable(0f)
    val alpha = Animatable(1f)
}

/** Исполнители; нажатие открывает страницу (нескольких — через меню). Вместо них — ошибка, если она есть. */
@Composable
private fun ArtistLink(track: Track, error: String, style: TextStyle, onOpenPage: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    Box {
        Text(
            error.ifEmpty { track.artists },
            Modifier.clip(CircleShape).clickable(enabled = error.isEmpty() && track.artistRefs.isNotEmpty()) {
                if (track.artistRefs.size == 1) onOpenPage("artist:${track.artistRefs[0].id}") else menu = true
            },
            style = style,
            color = if (error.isEmpty()) colors.onSurfaceVariant else colors.error,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            for (artist in track.artistRefs) {
                DropdownMenuItem(
                    text = { Text(artist.name) },
                    leadingIcon = { Symbol("person") },
                    onClick = {
                        menu = false
                        onOpenPage("artist:${artist.id}")
                    },
                )
            }
        }
    }
}

/** Очередь: нажатие — перейти к треку, крестик — убрать из очереди. При открытии текущий трек — в поле зрения. */
@Composable
private fun QueuePane(player: PlayerConnection, state: PlayerState) {
    val queue by player.queue.collectAsStateWithLifecycle()
    val listState = rememberLazyListState(initialFirstVisibleItemIndex = (state.index - 1).coerceAtLeast(0))
    LazyColumn(Modifier.fillMaxSize(), state = listState) {
        item(key = "header") {
            Box(Modifier.height(56.dp).padding(start = 4.dp), contentAlignment = Alignment.CenterStart) {
                Text("Очередь · ${queue.size}", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            }
        }
        // Один и тот же трек может стоять в очереди дважды — ключом служит место
        itemsIndexed(queue) { index, track ->
            TrackRow(
                track = track,
                current = index == state.index,
                onClick = { player.playAt(index) },
                number = index + 1,
                modifier = Modifier.animateItem(),
                trailing = {
                    // У текущего трека крестика нет, но место под него остаётся: длительности стоят в один столбец
                    Box(Modifier.size(36.dp).clip(CircleShape).clickable(enabled = index != state.index) { player.removeAt(index) }, contentAlignment = Alignment.Center) {
                        if (index != state.index) Symbol("close", size = 18.dp, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                },
            )
        }
    }
}

@Composable
private fun Seek(state: PlayerState, player: PlayerConnection) {
    val position by rememberPosition(player, state)
    var drag by remember { mutableStateOf<Float?>(null) }
    val style = MaterialTheme.typography.labelMedium
    val color = MaterialTheme.colorScheme.onSurfaceVariant
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = drag?.let { (it * state.durationMs).toLong() } ?: position
        Text(formatTime(shown), style = style, color = color)
        WavyProgress(
            value = { fraction(player.positionMs, state.durationMs) },
            wavy = state.playing,
            modifier = Modifier.weight(1f).height(36.dp),
            thickness = 6.dp,
            amplitude = 4.dp,
            wavelength = 48.dp,
            onDrag = { drag = it },
            onSeek = { player.seekTo((it * state.durationMs).toLong()) },
        )
        Text("−" + formatTime(state.durationMs - shown), style = style, color = color)
    }
}

@Composable
private fun Controls(state: PlayerState, player: PlayerConnection) {
    val colors = MaterialTheme.colorScheme
    Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Toggle("shuffle", checked = state.shuffle, enabled = !state.wave, onClick = player::toggleShuffle)
        IconButton(onClick = { player.previous() }) { Symbol("skip_previous", size = 32.dp, filled = true) }
        PlayButton(state.playing, player::togglePlay, size = 96.dp, container = colors.primaryContainer, content = colors.onPrimaryContainer)
        IconButton(onClick = { player.next() }, enabled = state.hasNext) { Symbol("skip_next", size = 32.dp, filled = true) }
        Toggle(
            if (state.repeat == Player.REPEAT_MODE_ONE) "repeat_one" else "repeat",
            checked = state.repeat != Player.REPEAT_MODE_OFF,
            onClick = player::cycleRepeat,
        )
    }
}

/** Кнопка-переключатель: включённая — тональная. */
@Composable
private fun Toggle(icon: String, checked: Boolean, onClick: () -> Unit, enabled: Boolean = true) {
    if (checked) {
        FilledTonalIconButton(onClick = onClick, enabled = enabled) { Symbol(icon) }
    } else {
        IconButton(onClick = onClick, enabled = enabled) { Symbol(icon, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}

private fun fraction(positionMs: Long, durationMs: Long): Float =
    if (durationMs <= 0) 0f else (positionMs.toFloat() / durationMs).coerceIn(0f, 1f)

private fun inOut(x: Float): Float = if (x < 0.5f) 4 * x * x * x else 1 - (-2 * x + 2).let { it * it * it } / 2

private fun side(tab: String) = when (tab) {
    "lyrics" -> -1
    "queue" -> 1
    else -> 0
}

private val NoMotion: () -> Float = { 0f }
private val TABS = listOf("lyrics", "queue")
private val WINDOW_SHAPES = listOf(
    Shapes.Cookie9, Shapes.Cookie12, Shapes.Cookie7, Shapes.Flower8, Shapes.Flower6, Shapes.Clover4, Shapes.Sunny, Shapes.Cookie6,
)
private const val SPLIT = 0.45f
