package io.github.yaskorinov.yamusic.ui

import android.graphics.Bitmap
import android.graphics.BitmapShader
import android.graphics.RuntimeShader
import android.graphics.Shader
import androidx.compose.foundation.layout.Spacer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.graphicsLayer
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt

/**
 * Размытая обложка играющего трека во весь элемент: фон полноэкранного плеера и атмосферная подложка
 * приложения (перенос CoverBackdrop.qml). Размытие — увеличение крошечной копии обложки шейдером:
 * ни свёртки, ни полноэкранной текстуры; сила размытия [blur] (0..1) — размер копии.
 * [mode] — характер размытия:
 *   gauss    — ровное, как гауссово: гладкие переходы без сетки и полос;
 *   glass    — матовое стекло: обложка угадывается, поверх — мелкое зерно;
 *   palette  — без обложки: плавный градиент из цветов темы (они и так взяты из обложки);
 *   blobs    — крупные цветовые пятна (линейная интерполяция).
 * [flow] (0..1) — «плавание»: картинка на месте, переливается только размытие (пока [running]).
 * Обложка берётся из темы и меняется вместе с цветами — под той же шторкой.
 */
@Composable
fun CoverBackdrop(mode: String, blur: Float, flow: Float, running: Boolean, modifier: Modifier = Modifier, alpha: Float = 1f) {
    val theme = LocalTheme.current
    val art = theme.art
    val scheme = theme.scheme
    val palette = mode == "palette"
    val strength = if (palette) 1f else blur.coerceIn(0f, 1f)
    val side = when {
        palette -> 0
        mode == "blobs" -> (12 + 788 * (1 - strength).pow(3)).roundToInt()
        else -> {
            val least = if (mode == "glass") 14f else 6f
            (least * (800 / least).pow((1 - strength).pow(1.5f))).roundToInt()
        }
    }
    val texture = remember(art, side, if (palette) scheme else null) {
        when {
            palette -> Bitmap.createBitmap(scheme.palette, 4, 3, Bitmap.Config.ARGB_8888)
            else -> art?.pixels?.boxScaled(side)
        }
    } ?: return
    val source = remember(texture) {
        BitmapShader(texture, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP).apply { filterMode = BitmapShader.FILTER_MODE_LINEAR }
    }
    val shader = remember { RuntimeShader(BACKDROP_AGSL) }
    val flowing = flow > 0f && strength > 0.2f // чёткую обложку «переливать» нечего
    val time = rememberTime(running && flowing)
    val speed = 0.15f + 0.85f * min(1f, flow)
    Spacer(
        modifier.graphicsLayer { this.alpha = alpha }.drawBehind {
            shader.setInputShader("source", source)
            shader.setFloatUniform("time", time() * speed)
            shader.setFloatUniform("amp", if (flowing) 0.08f * strength else 0f)
            shader.setFloatUniform("inset", if (flowing) 0.06f else 0f)
            shader.setFloatUniform("cubic", if (mode == "blobs") 0f else 1f)
            shader.setFloatUniform("saturation", if (palette) 1f else 1.1f + 0.4f * (1 - strength))
            shader.setFloatUniform("grain", if (mode == "glass") 0.05f else 0f)
            shader.setFloatUniform("texSize", texture.width.toFloat(), texture.height.toFloat())
            // видимая доля обложки по осям: обрезка под пропорции элемента
            when {
                palette -> shader.setFloatUniform("crop", 1f, 1f)
                size.width >= size.height -> shader.setFloatUniform("crop", 1f, size.height / max(1f, size.width))
                else -> shader.setFloatUniform("crop", size.width / max(1f, size.height), 1f)
            }
            shader.setFloatUniform("size", size.width, size.height)
            drawRect(ShaderBrush(shader))
        },
    )
}

/** Уменьшенная копия со стороной [side]: каждый пиксель — среднее своего блока (без ряби, в отличие от билинейного сжатия). */
fun Bitmap.boxScaled(side: Int): Bitmap {
    if (side >= width || side >= height) return this
    val from = IntArray(width * height)
    getPixels(from, 0, width, 0, 0, width, height)
    val out = IntArray(side * side)
    for (y in 0 until side) {
        val top = y * height / side
        val bottom = max(top + 1, (y + 1) * height / side)
        for (x in 0 until side) {
            val left = x * width / side
            val right = max(left + 1, (x + 1) * width / side)
            var r = 0
            var g = 0
            var b = 0
            for (v in top until bottom) {
                for (u in left until right) {
                    val pixel = from[v * width + u]
                    r += pixel shr 16 and 0xFF
                    g += pixel shr 8 and 0xFF
                    b += pixel and 0xFF
                }
            }
            val n = (bottom - top) * (right - left)
            out[y * side + x] = (0xFF shl 24) or (r / n shl 16) or (g / n shl 8) or (b / n)
        }
    }
    return Bitmap.createBitmap(out, side, side, Bitmap.Config.ARGB_8888)
}

/**
 * Перенос qml/app/shaders/backdrop.frag. Кубический B-сплайн почти совпадает с гауссом и даёт гладкий
 * результат без сетки, линейная интерполяция (cubic = 0) — «пятна». «Плавание»: плавное поле смещений
 * (сумма синусов) двигает точку выборки. Треугольный шум — зерно стекла и заодно дизеринг: поверх фона
 * лежит вуаль, она ослабляет шум втрое, поэтому 3/255 — плавные тёмные переходы не распадаются на полосы.
 */
private const val BACKDROP_AGSL = """
uniform shader source;
uniform float2 size;
uniform float time;
uniform float amp;
uniform float inset;
uniform float cubic;
uniform float saturation;
uniform float grain;
uniform float2 texSize;
uniform float2 crop;

float3 texel(float2 p) {
    return source.eval(p * texSize).rgb;
}

// Четыре линейные выборки вместо шестнадцати: пары соседних весов сплайна сливаются в одну
float3 bspline(float2 uv) {
    float2 st = uv * texSize - 0.5;
    float2 i = floor(st);
    float2 f = st - i;
    float2 f2 = f * f;
    float2 f3 = f2 * f;
    float2 w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
    float2 w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
    float2 w3 = f3 / 6.0;
    float2 g0 = w0 + w1;
    float2 g1 = 1.0 - g0;
    float2 p0 = (i + w1 / g0 - 0.5) / texSize;
    float2 p1 = (i + w3 / g1 + 1.5) / texSize;
    return g0.y * (g0.x * texel(p0) + g1.x * texel(float2(p1.x, p0.y)))
         + g1.y * (g0.x * texel(float2(p0.x, p1.y)) + g1.x * texel(p1));
}

float hash(float2 p) {
    float3 q = fract(float3(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

half4 main(float2 xy) {
    float2 uv = xy / size;
    float t = time;
    float2 off = float2(
        sin(uv.y * 4.1 + t * 0.83) + 0.6 * sin(uv.x * 6.3 - t * 0.57 + 1.7) + 0.35 * sin((uv.x + uv.y) * 9.0 + t * 1.1),
        cos(uv.x * 3.7 - t * 0.71) + 0.6 * sin(uv.y * 5.9 + t * 0.49 + 0.6) + 0.35 * cos((uv.x - uv.y) * 8.0 - t * 0.9)
    ) / 1.95;
    float2 p = inset + uv * (1.0 - 2.0 * inset) + amp * off;
    p = 0.5 + (p - 0.5) * crop;

    float3 c = cubic > 0.5 ? bspline(p) : texel(p);
    c = mix(float3(dot(c, float3(0.2126, 0.7152, 0.0722))), c, saturation);
    float n = hash(xy) + hash(xy + 71.7) - 1.0;
    c += n * (grain + 3.0 / 255.0);
    return half4(half3(clamp(c, 0.0, 1.0)), 1.0);
}
"""
