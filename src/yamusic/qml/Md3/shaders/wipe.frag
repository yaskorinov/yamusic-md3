#version 440
// Шторка смены темы: снимок окна со старыми цветами уезжает, открывая новые.
// Край мягкий (edge — доля ширины) и чуть наклонный; dir = +1 — слева направо, -1 — справа налево.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float progress;
    float dir;
    float edge;
    float skew;
    float shown;
};
layout(binding = 1) uniform sampler2D source;

void main() {
    float x = dir > 0.0 ? qt_TexCoord0.x : 1.0 - qt_TexCoord0.x;
    float t = x + skew * (qt_TexCoord0.y - 0.5);
    float front = progress * (1.0 + edge + skew) - edge - skew * 0.5;
    float a = smoothstep(front, front + edge, t) * shown * qt_Opacity;
    fragColor = texture(source, qt_TexCoord0) * a;
}
