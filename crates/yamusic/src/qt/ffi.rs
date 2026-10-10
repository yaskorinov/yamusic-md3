// Привязки к cpp/ym_shell.h и cpp/winframe.h. Владелец — агент «shell»; мосты пользуются обёртками из crate::qt.
#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        include!(<QtCore/QObject>);
        type QObject = cxx_qt::QObject;
    }

    #[namespace = "ym"]
    unsafe extern "C++" {
        include!("ym_shell.h");

        fn app_create(args: &[String]);
        fn app_exec() -> i32;
        fn app_destroy();
        fn app_quit();
        fn app_set_names(app: &QString, org: &QString, desktop: &QString);
        fn app_set_quit_on_last_window_closed(value: bool);
        fn app_set_window_icon(path: &QString);
        fn app_add_font(path: &QString) -> i32;

        fn copy_to_clipboard(text: &QString);
        unsafe fn set_cpp_ownership(object: *mut QObject);
        fn system_color_scheme_dark() -> bool;
        fn watch_color_scheme();

        fn engine_add_import_path(engine: Pin<&mut QQmlApplicationEngine>, path: &QString);
        fn engine_set_context_bool(engine: Pin<&mut QQmlApplicationEngine>, name: &QString, value: bool);
        fn engine_touch_singletons(engine: Pin<&mut QQmlApplicationEngine>, names: &QStringList);
        fn engine_has_root(engine: &QQmlApplicationEngine) -> bool;
        fn engine_root_object(engine: &QQmlApplicationEngine) -> *mut QObject;
        fn engine_install_caching_nam(engine: Pin<&mut QQmlApplicationEngine>, dir: &QString, max_bytes: i64);

        unsafe fn object_has_property(object: *mut QObject, name: &QString) -> bool;
        unsafe fn set_main_window(window: *mut QObject);
        fn post_raise_window();
        fn post_quit();
        unsafe fn schedule_screenshot(window: *mut QObject, path: &QString, delay_ms: i32);

        fn prefer_fastest_screen() -> QString;
        fn icon_measure(text: &QString, size: i32) -> QRectF;

        include!("winframe.h");
        unsafe fn winframe_install(window: *mut QObject, grip: i32) -> bool;

        include!("register.h");
        fn register_qml_types();
    }

    #[namespace = "ym"]
    extern "Rust" {
        /// QStyleHints::colorSchemeChanged (cpp/ym_shell.cpp, watch_color_scheme)
        fn system_dark_changed(dark: bool);
    }
}

fn system_dark_changed(dark: bool) {
    super::set_system_dark(dark);
}
