#pragma once

namespace ym {

// Создать синглтоны и зарегистрировать типы YaMusic.Core 1.0. Вызывать после QApplication и
// crate::context::init(), до создания QQmlApplicationEngine.
void register_qml_types();

} // namespace ym
