//! Минимальная обёртка libmpv через libloading (без python-mpv).
//!
//! Поиск библиотеки: PlayerConfig.libmpv_path / $YAMUSIC_LIBMPV → каталог exe → родители exe (до 3 уровней:
//! target/debug → корень репозитория, где лежит libmpv-2.dll) → текущий каталог → системный поиск по имени.
//! Имена: Windows "libmpv-2.dll", "mpv-2.dll"; Linux "libmpv.so.2", "libmpv.so"; macOS "libmpv.2.dylib", "libmpv.dylib".
//!
//! Нужные символы: mpv_create, mpv_initialize, mpv_set_option_string, mpv_set_property, mpv_set_property_string,
//! mpv_get_property, mpv_command, mpv_observe_property, mpv_wait_event, mpv_wakeup, mpv_terminate_destroy,
//! mpv_free, mpv_free_node_contents, mpv_error_string.
//!
//! Опции экземпляра (как _make_mpv): vid=no, ytdl=no, audio-display=no, input-default-bindings=no,
//! input-vo-keyboard=no, config=no, load-scripts=no, osc=no, gapless-audio=weak, prefetch-playlist=yes,
//! cache=yes, demuxer-max-bytes=32MiB, demuxer-max-back-bytes=8MiB, keep-open=no, idle=yes,
//! audio-client-name=yamusic, af=LEVEL_FILTER, msg-level=all=error,ffmpeg=fatal, ao=$YAMUSIC_AO (если задан).
//! Наблюдаемые свойства: pause, idle-active, paused-for-cache, duration, playlist-pos (флаги/double/int64),
//! af-metadata/lvl (MPV_FORMAT_NODE, ключ "lavfi.astats.Overall.RMS_level"); событие end-file (reason: 0 EOF, 4 ERROR).
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{LEVEL_FILTER, PlayerError};

#[cfg(target_os = "windows")]
const LIB_NAMES: &[&str] = &["libmpv-2.dll", "mpv-2.dll"];
#[cfg(target_os = "macos")]
const LIB_NAMES: &[&str] = &["libmpv.2.dylib", "libmpv.dylib"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const LIB_NAMES: &[&str] = &["libmpv.so.2", "libmpv.so"];

// mpv_format
const FORMAT_NONE: c_int = 0;
const FORMAT_STRING: c_int = 1;
const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;
const FORMAT_NODE: c_int = 6;
const FORMAT_NODE_MAP: c_int = 8;

// mpv_event_id
const EVENT_NONE: c_int = 0;
const EVENT_SHUTDOWN: c_int = 1;
const EVENT_END_FILE: c_int = 7;
const EVENT_PROPERTY_CHANGE: c_int = 22;

#[repr(C)]
struct RawEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct RawEventProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct RawEventEndFile {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
union RawNodeValue {
    string: *mut c_char,
    flag: c_int,
    int64: i64,
    double: f64,
    list: *mut RawNodeList,
}

#[repr(C)]
struct RawNode {
    u: RawNodeValue,
    format: c_int,
}

#[repr(C)]
struct RawNodeList {
    num: c_int,
    values: *mut RawNode,
    keys: *mut *mut c_char,
}

type Handle = *mut c_void;

/// Загруженная библиотека (указатели на функции).
pub struct MpvLib {
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    set_property_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    get_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    observe_property: unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int,
    wait_event: unsafe extern "C" fn(Handle, f64) -> *mut RawEvent,
    wakeup: unsafe extern "C" fn(Handle),
    terminate_destroy: unsafe extern "C" fn(Handle),
    _free: unsafe extern "C" fn(*mut c_void),
    _free_node_contents: unsafe extern "C" fn(*mut RawNode),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    /// откуда загружена (для логов)
    pub path: PathBuf,
    _lib: libloading::Library,
}

impl MpvLib {
    fn open(path: &Path) -> Result<Self, String> {
        // SAFETY: загрузка libmpv; её инициализаторы не делают ничего опасного
        let lib = unsafe { libloading::Library::new(path.as_os_str()) }.map_err(|e| e.to_string())?;
        macro_rules! sym {
            ($name:literal) => {
                // SAFETY: сигнатуры совпадают с client.h libmpv; указатель живёт, пока жива `lib`
                *unsafe { lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| format!("{}: {e}", $name))?
            };
        }
        Ok(Self {
            create: sym!("mpv_create"),
            initialize: sym!("mpv_initialize"),
            set_option_string: sym!("mpv_set_option_string"),
            set_property: sym!("mpv_set_property"),
            set_property_string: sym!("mpv_set_property_string"),
            get_property: sym!("mpv_get_property"),
            command: sym!("mpv_command"),
            observe_property: sym!("mpv_observe_property"),
            wait_event: sym!("mpv_wait_event"),
            wakeup: sym!("mpv_wakeup"),
            terminate_destroy: sym!("mpv_terminate_destroy"),
            _free: sym!("mpv_free"),
            _free_node_contents: sym!("mpv_free_node_contents"),
            error_string: sym!("mpv_error_string"),
            path: path.to_path_buf(),
            _lib: lib,
        })
    }

    fn error_text(&self, code: c_int) -> String {
        // SAFETY: mpv_error_string возвращает статическую строку
        let ptr = unsafe { (self.error_string)(code) };
        if ptr.is_null() {
            return format!("ошибка {code}");
        }
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    }
}

/// Найти и загрузить libmpv.
pub fn load_library(explicit: Option<&Path>) -> Result<Arc<MpvLib>, PlayerError> {
    let mut errors = Vec::new();
    for path in candidate_paths(explicit) {
        // относительное имя без каталога — системный поиск; путь с каталогом — только если файл есть
        let bare = path.parent().is_none_or(|p| p.as_os_str().is_empty());
        if !bare && !path.is_file() {
            continue;
        }
        match MpvLib::open(&path) {
            Ok(lib) => {
                tracing::debug!(path = %path.display(), "libmpv загружена");
                return Ok(Arc::new(lib));
            }
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    let tried = if errors.is_empty() { LIB_NAMES.join(", ") } else { errors.join("; ") };
    Err(PlayerError::LibraryNotFound(tried))
}

/// Кандидаты путей в порядке поиска (для сообщения об ошибке и тестов).
pub fn candidate_paths(explicit: Option<&Path>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !paths.contains(&p) {
            paths.push(p);
        }
    };
    if let Some(p) = explicit {
        push(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("YAMUSIC_LIBMPV").filter(|v| !v.is_empty()) {
        push(PathBuf::from(p));
    }
    let mut dirs = Vec::new();
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        dirs.extend(exe_dir.ancestors().take(4).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    for dir in &dirs {
        for name in LIB_NAMES {
            push(dir.join(name));
        }
    }
    for name in LIB_NAMES {
        push(PathBuf::from(name));
    }
    paths
}

/// Значение наблюдаемого свойства.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    None,
    Flag(bool),
    Int(i64),
    Double(f64),
    Str(String),
    /// MPV_FORMAT_NODE_MAP, сведённая к строкам (af-metadata)
    Map(Vec<(String, String)>),
}

impl PropertyValue {
    pub fn as_flag(&self) -> Option<bool> {
        match self {
            Self::Flag(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Double(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn map_get(&self, key: &str) -> Option<&str> {
        match self {
            Self::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MpvEvent {
    PropertyChange { name: String, value: PropertyValue },
    /// reason: 0 — EOF, 2 — STOP, 3 — QUIT, 4 — ERROR, 5 — REDIRECT
    EndFile { reason: i32 },
    Shutdown,
}

/// Формат наблюдения по имени свойства (см. шапку).
fn observe_format(name: &str) -> c_int {
    match name {
        "pause" | "idle-active" | "paused-for-cache" | "eof-reached" | "seeking" => FORMAT_FLAG,
        "duration" | "time-pos" | "volume" => FORMAT_DOUBLE,
        "playlist-pos" | "playlist-count" => FORMAT_INT64,
        n if n.starts_with("af-metadata") => FORMAT_NODE,
        _ => FORMAT_STRING,
    }
}

fn cstring(s: &str) -> Result<CString, PlayerError> {
    CString::new(s).map_err(|_| PlayerError::Mpv(format!("строка с нулевым байтом: {s:?}")))
}

unsafe fn c_text(ptr: *const c_char) -> String {
    if ptr.is_null() { String::new() } else { unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned() }
}

unsafe fn node_text(node: &RawNode) -> String {
    unsafe {
        match node.format {
            FORMAT_STRING => c_text(node.u.string),
            FORMAT_FLAG => if node.u.flag != 0 { "yes" } else { "no" }.to_owned(),
            FORMAT_INT64 => node.u.int64.to_string(),
            FORMAT_DOUBLE => node.u.double.to_string(),
            _ => String::new(),
        }
    }
}

unsafe fn property_value(format: c_int, data: *mut c_void) -> PropertyValue {
    if data.is_null() {
        return PropertyValue::None;
    }
    unsafe {
        match format {
            FORMAT_FLAG => PropertyValue::Flag(*(data as *const c_int) != 0),
            FORMAT_INT64 => PropertyValue::Int(*(data as *const i64)),
            FORMAT_DOUBLE => PropertyValue::Double(*(data as *const f64)),
            FORMAT_STRING => PropertyValue::Str(c_text(*(data as *const *const c_char))),
            FORMAT_NODE => {
                let node = &*(data as *const RawNode);
                if node.format == FORMAT_NODE_MAP {
                    let list = node.u.list;
                    if list.is_null() {
                        return PropertyValue::Map(Vec::new());
                    }
                    let list = &*list;
                    let n = list.num.max(0) as usize;
                    let entries = (0..n)
                        .map(|k| (c_text(*list.keys.add(k)), node_text(&*list.values.add(k))))
                        .collect();
                    PropertyValue::Map(entries)
                } else if node.format == FORMAT_NONE {
                    PropertyValue::None
                } else {
                    PropertyValue::Str(node_text(node))
                }
            }
            _ => PropertyValue::None,
        }
    }
}

/// Экземпляр mpv. Send + Sync: libmpv потокобезопасна; события читаются отдельным потоком через wait_event.
pub struct Mpv {
    lib: Arc<MpvLib>,
    handle: *mut std::ffi::c_void,
    closing: AtomicBool,
}

unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Mpv {
    /// Создать и инициализировать с опциями из шапки модуля.
    pub fn new(lib: Arc<MpvLib>, audio_output: Option<&str>) -> Result<Self, PlayerError> {
        // SAFETY: mpv_create без аргументов; NULL — нехватка памяти или неверная локаль
        let handle = unsafe { (lib.create)() };
        if handle.is_null() {
            return Err(PlayerError::Mpv("mpv_create вернул NULL".into()));
        }
        let mpv = Self { lib, handle, closing: AtomicBool::new(false) };
        let mut options: Vec<(&str, &str)> = vec![
            ("vid", "no"),
            ("ytdl", "no"),
            ("audio-display", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            // без пользовательского конфига и встроенных Lua-скриптов (OSC, консоль, статистика) — минус память
            ("config", "no"),
            ("load-scripts", "no"),
            ("osc", "no"),
            ("gapless-audio", "weak"),
            ("prefetch-playlist", "yes"),
            ("cache", "yes"),
            ("demuxer-max-bytes", "32MiB"),
            ("demuxer-max-back-bytes", "8MiB"),
            ("keep-open", "no"),
            ("idle", "yes"),
            ("audio-client-name", "yamusic"),
            ("af", LEVEL_FILTER),
            // astats печатает сводку при выходе
            ("msg-level", "all=error,ffmpeg=fatal"),
        ];
        if let Some(ao) = audio_output.filter(|ao| !ao.is_empty()) {
            options.push(("ao", ao));
        }
        for (name, value) in options {
            if let Err(e) = mpv.set_option_string(name, value) {
                tracing::debug!("mpv: опция {name}={value}: {e}");
            }
        }
        // SAFETY: handle создан выше и ещё не инициализирован
        let code = unsafe { (mpv.lib.initialize)(mpv.handle) };
        mpv.check(code, "mpv_initialize")?;
        Ok(mpv)
    }

    fn check(&self, code: c_int, what: &str) -> Result<(), PlayerError> {
        if code >= 0 { Ok(()) } else { Err(PlayerError::Mpv(format!("{what}: {}", self.lib.error_text(code)))) }
    }

    fn set_option_string(&self, name: &str, value: &str) -> Result<(), PlayerError> {
        let (n, v) = (cstring(name)?, cstring(value)?);
        let code = unsafe { (self.lib.set_option_string)(self.handle, n.as_ptr(), v.as_ptr()) };
        self.check(code, name)
    }

    pub fn set_property_string(&self, name: &str, value: &str) -> Result<(), PlayerError> {
        let (n, v) = (cstring(name)?, cstring(value)?);
        let code = unsafe { (self.lib.set_property_string)(self.handle, n.as_ptr(), v.as_ptr()) };
        self.check(code, name)
    }

    pub fn set_property_f64(&self, name: &str, value: f64) -> Result<(), PlayerError> {
        let n = cstring(name)?;
        let mut v = value;
        let code = unsafe { (self.lib.set_property)(self.handle, n.as_ptr(), FORMAT_DOUBLE, &mut v as *mut f64 as *mut c_void) };
        self.check(code, name)
    }

    pub fn set_property_flag(&self, name: &str, value: bool) -> Result<(), PlayerError> {
        let n = cstring(name)?;
        let mut v: c_int = value.into();
        let code = unsafe { (self.lib.set_property)(self.handle, n.as_ptr(), FORMAT_FLAG, &mut v as *mut c_int as *mut c_void) };
        self.check(code, name)
    }

    pub fn get_property_f64(&self, name: &str) -> Option<f64> {
        let n = cstring(name).ok()?;
        let mut v = 0f64;
        let code = unsafe { (self.lib.get_property)(self.handle, n.as_ptr(), FORMAT_DOUBLE, &mut v as *mut f64 as *mut c_void) };
        (code >= 0).then_some(v)
    }

    pub fn get_property_i64(&self, name: &str) -> Option<i64> {
        let n = cstring(name).ok()?;
        let mut v = 0i64;
        let code = unsafe { (self.lib.get_property)(self.handle, n.as_ptr(), FORMAT_INT64, &mut v as *mut i64 as *mut c_void) };
        (code >= 0).then_some(v)
    }

    /// mpv_command: ["loadfile", url, "replace"|"append", …], ["playlist-remove", "1"], ["stop"], ["seek", "12.3", "absolute"], ["cycle", "pause"]
    pub fn command(&self, args: &[&str]) -> Result<(), PlayerError> {
        let owned = args.iter().map(|a| cstring(a)).collect::<Result<Vec<_>, _>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: массив строк, завершённый NULL; строки живут до конца вызова
        let code = unsafe { (self.lib.command)(self.handle, ptrs.as_mut_ptr()) };
        self.check(code, args.first().copied().unwrap_or("command"))
    }

    /// loadfile с опцией start=<секунды> (восстановленная позиция).
    pub fn loadfile(&self, url: &str, mode: &str, start_seconds: Option<f64>) -> Result<(), PlayerError> {
        let Some(start) = start_seconds else {
            return self.command(&["loadfile", url, mode]);
        };
        let options = format!("start={start:.2}");
        // mpv ≥ 0.38: loadfile <url> <flags> <index> <options>; раньше — без index
        self.command(&["loadfile", url, mode, "-1", &options])
            .or_else(|_| self.command(&["loadfile", url, mode, &options]))
    }

    pub fn observe(&self, id: u64, name: &str) -> Result<(), PlayerError> {
        let n = cstring(name)?;
        let code = unsafe { (self.lib.observe_property)(self.handle, id, n.as_ptr(), observe_format(name)) };
        self.check(code, name)
    }

    /// Блокирующее ожидание события (timeout < 0 — бесконечно). None — таймаут/пустое событие.
    pub fn wait_event(&self, timeout: f64) -> Option<MpvEvent> {
        // SAFETY: wait_event зовёт только поток событий этого экземпляра; событие живёт до следующего вызова
        let event = unsafe { (self.lib.wait_event)(self.handle, timeout) };
        if event.is_null() {
            return None;
        }
        let event = unsafe { &*event };
        match event.event_id {
            EVENT_NONE => None,
            EVENT_SHUTDOWN => Some(MpvEvent::Shutdown),
            EVENT_END_FILE if !event.data.is_null() => {
                let data = unsafe { &*(event.data as *const RawEventEndFile) };
                let _ = data.error;
                Some(MpvEvent::EndFile { reason: data.reason })
            }
            EVENT_PROPERTY_CHANGE if !event.data.is_null() => {
                let prop = unsafe { &*(event.data as *const RawEventProperty) };
                let name = unsafe { c_text(prop.name) };
                let value = unsafe { property_value(prop.format, prop.data) };
                Some(MpvEvent::PropertyChange { name, value })
            }
            _ => {
                let _ = (event.error, event.reply_userdata);
                None
            }
        }
    }

    pub fn wakeup(&self) {
        unsafe { (self.lib.wakeup)(self.handle) }
    }

    /// Пометить экземпляр закрывающимся и разбудить поток событий.
    pub fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        let _ = self.command(&["quit"]);
        self.wakeup();
    }

    pub fn is_closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: последний владелец; поток событий к этому моменту завершился (держит Arc)
        unsafe { (self.lib.terminate_destroy)(self.handle) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_order() {
        let explicit = PathBuf::from("/opt/custom/libmpv-test");
        let paths = candidate_paths(Some(&explicit));
        assert_eq!(paths[0], explicit);
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        let first_name = LIB_NAMES[0];
        let exe_pos = paths.iter().position(|p| *p == exe_dir.join(first_name)).unwrap();
        let parent_pos = paths.iter().position(|p| *p == exe_dir.parent().unwrap().join(first_name)).unwrap();
        assert!(exe_pos < parent_pos);
        // последние — голые имена для системного поиска
        let tail: Vec<_> = paths[paths.len() - LIB_NAMES.len()..].to_vec();
        assert_eq!(tail, LIB_NAMES.iter().map(PathBuf::from).collect::<Vec<_>>());
        let mut dedup = paths.clone();
        dedup.dedup();
        assert_eq!(dedup.len(), paths.len());
    }

    #[test]
    fn missing_library_error() {
        // явный несуществующий путь пропускается, ошибка не паникует
        let r = MpvLib::open(Path::new("definitely-not-a-libmpv-xyz.dll"));
        assert!(r.is_err());
    }

    #[test]
    fn formats() {
        assert_eq!(observe_format("pause"), FORMAT_FLAG);
        assert_eq!(observe_format("duration"), FORMAT_DOUBLE);
        assert_eq!(observe_format("playlist-pos"), FORMAT_INT64);
        assert_eq!(observe_format("af-metadata/lvl"), FORMAT_NODE);
    }

    #[test]
    fn property_map_lookup() {
        let v = PropertyValue::Map(vec![("lavfi.astats.Overall.RMS_level".into(), "-20.5".into())]);
        assert_eq!(v.map_get("lavfi.astats.Overall.RMS_level"), Some("-20.5"));
        assert_eq!(v.map_get("x"), None);
        assert_eq!(PropertyValue::Int(3).as_f64(), Some(3.0));
    }
}
