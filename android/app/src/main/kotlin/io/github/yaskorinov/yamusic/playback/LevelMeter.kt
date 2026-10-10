package io.github.yaskorinov.yamusic.playback

import android.os.Handler
import android.os.Looper
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Уровень звука играющего трека — по нему «дышит» обложка полноэкранного плеера.
 * Считается, только пока [wanted]: среднеквадратичная громкость кусков по ~25 мс, в той же шкале,
 * что у десктопного клиента (−42 дБ → 0, −6 дБ → 1).
 */
class LevelMeter {
    val level = MutableStateFlow(0f)

    @Volatile
    var wanted = false

    private val main = Handler(Looper.getMainLooper())

    /**
     * Декодированный звук (PCM 16 бит) из потока плеера. Звук уходит в динамик не сразу, а через
     * [aheadMs] — на столько же откладывается и уровень, иначе обложка опережала бы музыку.
     */
    fun push(buffer: ByteBuffer, sampleRate: Int, channels: Int, aheadMs: Long) {
        if (!wanted || sampleRate <= 0 || channels <= 0) return
        val samples = buffer.duplicate().order(ByteOrder.LITTLE_ENDIAN).asShortBuffer()
        val total = samples.remaining()
        val chunk = max(1, sampleRate / 40) * channels
        var start = 0
        while (start < total) {
            val end = min(total, start + chunk)
            var sum = 0.0
            var count = 0
            var i = start
            while (i < end) {
                val sample = samples.get(i) / 32768.0
                sum += sample * sample
                count++
                i += 4 // каждого четвёртого отсчёта для громкости достаточно
            }
            val db = 10 * log10(sum / max(1, count) + 1e-12)
            // сверху не обрезаем: у громко сведённой музыки уровень иначе упирался бы в потолок и удары пропадали
            val value = ((db + 42) / 36).coerceAtLeast(0.0).toFloat()
            val delay = aheadMs + start * 1000L / (sampleRate * channels)
            main.postDelayed({ level.value = value }, delay.coerceIn(0L, 3000L))
            start = end
        }
    }
}
