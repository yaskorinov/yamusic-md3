package io.github.yaskorinov.yamusic.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.toPath
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.center
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Matrix
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.graphics.shapes.Morph
import androidx.graphics.shapes.RoundedPolygon

/**
 * Фигура Material 3 Expressive, которая перетекает в новую при смене [polygon] и может вращаться.
 * [rotation] читается на этапе рисования: вращение не вызывает перекомпоновку.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MorphShape(polygon: RoundedPolygon, color: Color, modifier: Modifier = Modifier, rotation: () -> Float = { 0f }) {
    var from by remember { mutableStateOf(polygon) }
    var to by remember { mutableStateOf(polygon) }
    val progress = remember { Animatable(1f) }
    LaunchedEffect(polygon) {
        if (polygon !== to) {
            from = to
            to = polygon
            progress.snapTo(0f)
            progress.animateTo(1f, spring(dampingRatio = 0.75f, stiffness = 140f))
        }
    }
    val morph = remember(from, to) { Morph(from, to) }
    val path = remember { Path() }
    val scale = remember { Matrix() }
    Canvas(modifier) {
        // Фигуры нормализованы в единичный квадрат: растягиваем на размер элемента и центрируем
        morph.toPath(progress.value, path)
        scale.reset()
        scale.scale(size.width, size.height)
        path.transform(scale)
        path.translate(size.center - path.getBounds().center)
        rotate(rotation()) { drawPath(path, color) }
    }
}
