#version 440
// «Плавание» размытия: картинка стоит на месте, а её размытые цветовые пятна мягко переливаются.
// Плавное поле смещений (сумма синусов разных частот) двигает выборку текстуры; на размытом
// изображении это выглядит как медленно текущие пятна. Края не видны: выборка чуть «приближена».
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float time;
    float amp;      // сила смещения, доля размера
};
layout(binding = 1) uniform sampler2D source;

void main() {
    vec2 uv = qt_TexCoord0;
    float t = time;
    vec2 off = vec2(
        sin(uv.y * 4.1 + t * 0.83) + 0.6 * sin(uv.x * 6.3 - t * 0.57 + 1.7) + 0.35 * sin((uv.x + uv.y) * 9.0 + t * 1.1),
        cos(uv.x * 3.7 - t * 0.71) + 0.6 * sin(uv.y * 5.9 + t * 0.49 + 0.6) + 0.35 * cos((uv.x - uv.y) * 8.0 - t * 0.9)
    ) / 1.95;
    vec2 p = 0.06 + uv * 0.88 + amp * off;
    fragColor = texture(source, p) * qt_Opacity;
}
