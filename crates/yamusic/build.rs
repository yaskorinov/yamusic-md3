// Сборка: все файлы src/**/*.rs с #[cxx_qt::bridge] собираются автоматически —
// агентам «shell» и «bridges» не нужно править этот файл, чтобы добавить новый мост.
use std::path::{Path, PathBuf};

use cxx_qt_build::CxxQtBuilder;

fn collect_bridges(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_bridges(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if text.lines().any(|l| l.trim_start().starts_with("#[cxx_qt::bridge")) {
                out.push(path);
            }
        }
    }
}

fn main() {
    let mut bridges = Vec::new();
    collect_bridges(Path::new("src"), &mut bridges);
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-changed=cpp");

    // winframe.cpp собирается везде (вне Windows — заглушка), чтобы привязки в qt/ffi.rs не зависели от ОС
    let cpp = ["cpp/ym_shell.cpp", "cpp/register.cpp", "cpp/winframe.cpp"];

    // Без QmlModule: он требует, чтобы все мосты лежали в одном каталоге (QTBUG-93443);
    // типы YaMusic.Core регистрирует cpp/register.cpp.
    let builder = CxxQtBuilder::new()
        .qt_module("Gui")
        .qt_module("Qml")
        .qt_module("Quick")
        .qt_module("Network")
        .qt_module("Widgets")
        .include_dir("cpp")
        .files(bridges)
        .cpp_files(cpp);
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("cpp");
            if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
                cc.flag("/utf-8");
            }
        })
    };
    builder.build();

    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => {
            println!("cargo::rustc-link-lib=user32");
            println!("cargo::rustc-link-lib=dwmapi");
            println!("cargo::rustc-link-lib=shell32");
        }
        // dlsym (display.rs: QWindowSystemInterface::handlePrimaryScreenChanged)
        Ok("linux") => println!("cargo::rustc-link-lib=dl"),
        _ => {}
    }
}
