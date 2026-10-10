package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.foundation.shape.GenericShape
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.sin

/**
 * Полярная фигура MD3 Expressive — та же математика, что в десктопном клиенте (qml/Md3/Shapes.js):
 * фигура задана радиусом r(θ), морфинг — линейная интерполяция радиусов двух фигур в одних и тех же
 * углах, поэтому любые две фигуры перетекают друг в друга без «перескоков» вершин.
 */
class PolarShape(radius: (Float) -> Float) {
    /** Радиусы в [SAMPLES] равномерных углах, нормированные так, чтобы фигура вписывалась в квадрат [-1, 1]. */
    val radii: FloatArray = FloatArray(SAMPLES).also { out ->
        var extent = 0f
        for (i in 0 until SAMPLES) {
            val theta = TAU * i / SAMPLES
            val r = radius(theta)
            out[i] = r
            extent = max(extent, max(abs(r * cos(theta)), abs(r * sin(theta))))
        }
        for (i in 0 until SAMPLES) out[i] /= extent
    }

    /** Неподвижная фигура — для обычной обрезки ([androidx.compose.ui.draw.clip]) и фона. */
    val shape: Shape by lazy { GenericShape { size, _ -> outline(radii, radii, 0f, size) } }
}

/** Пресеты — с теми же именами и числами, что в Shapes.js. */
object Shapes {
    val Circle = PolarShape { 1f }
    val Square = superellipse(5f)
    val SoftSquare = superellipse(3.2f)
    val Cookie4 = cookie(4, 0.12f)
    val Cookie6 = cookie(6, 0.1f)
    val Cookie7 = cookie(7, 0.09f)
    val Cookie9 = cookie(9, 0.075f)
    val Cookie12 = cookie(12, 0.055f)
    val Scallop = cookie(24, 0.035f)
    val Clover4 = flower(4, 0.32f, 0.7f)
    val Flower6 = flower(6, 0.28f, 0.8f)
    val Flower8 = flower(8, 0.22f, 0.9f)
    val Sunny = flower(8, 0.12f, 2.5f)
    val Blob = PolarShape { t -> 1f + 0.07f * cos(2 * t + 0.3f) + 0.06f * cos(3 * t + 1.7f) + 0.025f * cos(5 * t + 4.1f) }

    /** Суперэллипс: p≈5 — «скруглённый квадрат». */
    private fun superellipse(p: Float) = PolarShape { t -> (abs(cos(t)).pow(p) + abs(sin(t)).pow(p)).pow(-1 / p) }

    /** Синусоидальный зубчатый край — «печенька». */
    private fun cookie(n: Int, depth: Float) = PolarShape { t -> 1f - depth * (0.5f - 0.5f * cos(n * t)) }

    /** Лепестки: |cos|^p заостряет впадины. */
    private fun flower(n: Int, depth: Float, p: Float) = PolarShape { t -> 1f - depth + depth * abs(cos(n * t / 2)).pow(p) }
}

/**
 * Контур фигуры: морф [from] → [to] с прогрессом [t], поворот [rotation] (градусы) вокруг [center].
 * [pulse] (0..1) — «дыхание»: впадины углубляются, а сама фигура чуть расширяется (97 % → 100 %).
 */
fun Path.outline(
    from: FloatArray,
    to: FloatArray,
    t: Float,
    size: Size,
    rotation: Float = 0f,
    pulse: Float = 0f,
    center: Offset = Offset(size.width / 2, size.height / 2),
) {
    rewind()
    val rx = size.width / 2
    val ry = size.height / 2
    val turn = rotation * (PI.toFloat() / 180f)
    for (i in 0 until SAMPLES) {
        var r = from[i] + (to[i] - from[i]) * t
        if (pulse > 0f) r = (1f - (1f - r) * (1f + 2.2f * pulse)) * (0.97f + 0.03f * pulse)
        val theta = TAU * i / SAMPLES + turn
        val x = center.x + rx * r * cos(theta)
        val y = center.y + ry * r * sin(theta)
        if (i == 0) moveTo(x, y) else lineTo(x, y)
    }
    close()
}

/** Длительности и кривые MD3 Expressive — как в Theme.qml: пружины приближены кривыми с перелётом. */
object Motion {
    const val SpatialFast = 350
    const val SpatialDefault = 500
    const val SpatialSlow = 650
    const val EffectsFast = 150
    const val EffectsDefault = 200

    val Emphasized: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)
    val EmphasizedDecelerate: Easing = CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f)

    /** Кривая с перелётом (Easing.OutBack в Qt). */
    fun outBack(overshoot: Float = 1.25f) = Easing { x ->
        val d = x - 1f
        1f + (overshoot + 1f) * d * d * d + overshoot * d * d
    }
}

const val SAMPLES = 120
private const val TAU = (PI * 2).toFloat()
