//! Minimal COM server. Explorer only collects the selected paths; conversion runs
//! in the application process. All returned strings use the COM task allocator.
#![cfg(windows)]
#![allow(non_snake_case)]

use std::{
    ffi::{c_void, OsString},
    fs::OpenOptions,
    io::Write,
    os::windows::{ffi::OsStringExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
    ptr,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

type HResult = i32;
const S_OK: HResult = 0;
const S_FALSE: HResult = 1;
const E_NOINTERFACE: HResult = 0x80004002u32 as i32;
const E_POINTER: HResult = 0x80004003u32 as i32;
const E_FAIL: HResult = 0x80004005u32 as i32;
const E_NOTIMPL: HResult = 0x80004001u32 as i32;
const E_PENDING: HResult = 0x8000000Au32 as i32;
const CLASS_E_NOAGGREGATION: HResult = 0x80040110u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: HResult = 0x80040111u32 as i32;
const SIGDN_FILESYSPATH: u32 = 0x80058000;
const ECS_ENABLED: u32 = 0;
const ECS_DISABLED: u32 = 1;
const ECS_HIDDEN: u32 = 2;
const CREATE_NO_WINDOW: u32 = 0x08000000;
const MAX_SELECTION: u32 = 10_000;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

const CLSID_CONVERTER: Guid = Guid {
    a: 0x67b51ba1,
    b: 0x8a78,
    c: 0x4b4b,
    d: [0xa5, 0xf9, 0x08, 0xa8, 0x25, 0xbe, 0x91, 0xf8],
};
const IID_IUNKNOWN: Guid = Guid {
    a: 0,
    b: 0,
    c: 0,
    d: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
};
const IID_ICLASSFACTORY: Guid = Guid {
    a: 1,
    b: 0,
    c: 0,
    d: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
};
const IID_IEXPLORERCOMMAND: Guid = Guid {
    a: 0xa08ce4d0,
    b: 0xfa25,
    c: 0x44ab,
    d: [0xb5, 0x7c, 0xc7, 0xb1, 0xc3, 0x23, 0xe0, 0xb9],
};
const IID_IENUMEXPLORERCOMMAND: Guid = Guid {
    a: 0xa88826f8,
    b: 0x186f,
    c: 0x4987,
    d: [0xaa, 0xde, 0xea, 0x0c, 0xef, 0x8f, 0xbf, 0xe8],
};

// Keep these identifiers aligned with OutputFormat in the application core.
const TARGETS: [&str; 21] = [
    "mp3", "wav", "flac", "aac", "ogg", "opus", "aiff", "mp4", "mkv", "mov", "webm", "avi", "gif",
    "png", "jpg", "webp", "bmp", "tiff", "ico", "pdf", "txt",
];

#[link(name = "shlwapi")]
extern "system" {
    fn SHStrDupW(source: *const u16, out: *mut *mut u16) -> HResult;
}
#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(value: *const c_void);
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
    fn GetModuleFileNameW(module: *mut c_void, path: *mut u16, size: u32) -> u32;
}

static OBJECTS: AtomicUsize = AtomicUsize::new(0);
static LOCKS: AtomicUsize = AtomicUsize::new(0);
static REQUEST_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct UnknownVtable {
    query: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HResult,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct FactoryVtable {
    unknown: UnknownVtable,
    create: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *const Guid,
        *mut *mut c_void,
    ) -> HResult,
    lock: unsafe extern "system" fn(*mut c_void, i32) -> HResult,
}

#[repr(C)]
struct ExplorerVtable {
    unknown: UnknownVtable,
    title: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut u16) -> HResult,
    icon: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut u16) -> HResult,
    tooltip: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut u16) -> HResult,
    canonical: unsafe extern "system" fn(*mut c_void, *mut Guid) -> HResult,
    state: unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut u32) -> HResult,
    invoke: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HResult,
    flags: unsafe extern "system" fn(*mut c_void, *mut u32) -> HResult,
    enumerate: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HResult,
}

#[repr(C)]
struct ComObject<T> {
    vtable: *const T,
    refs: AtomicU32,
}

#[repr(C)]
struct ExplorerObject {
    header: ComObject<ExplorerVtable>,
    target: Option<String>,
    parent: bool,
}

#[repr(C)]
struct EnumeratorVtable {
    unknown: UnknownVtable,
    next: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, *mut u32) -> HResult,
    skip: unsafe extern "system" fn(*mut c_void, u32) -> HResult,
    reset: unsafe extern "system" fn(*mut c_void) -> HResult,
    clone: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HResult,
}

#[repr(C)]
struct EnumeratorObject {
    header: ComObject<EnumeratorVtable>,
    cursor: AtomicUsize,
}

static FACTORY_VTABLE: FactoryVtable = FactoryVtable {
    unknown: UnknownVtable {
        query: factory_query,
        add_ref: factory_add,
        release: factory_release,
    },
    create: factory_create,
    lock: factory_lock,
};
static EXPLORER_VTABLE: ExplorerVtable = ExplorerVtable {
    unknown: UnknownVtable {
        query: explorer_query,
        add_ref: explorer_add,
        release: explorer_release,
    },
    title: explorer_title,
    icon: explorer_icon,
    tooltip: explorer_tooltip,
    canonical: explorer_canonical,
    state: explorer_state,
    invoke: explorer_invoke,
    flags: explorer_flags,
    enumerate: explorer_enumerate,
};
static ENUMERATOR_VTABLE: EnumeratorVtable = EnumeratorVtable {
    unknown: UnknownVtable {
        query: enumerator_query,
        add_ref: enumerator_add,
        release: enumerator_release,
    },
    next: enumerator_next,
    skip: enumerator_skip,
    reset: enumerator_reset,
    clone: enumerator_clone,
};

fn allocate<T>(vtable: &'static T) -> *mut c_void {
    OBJECTS.fetch_add(1, Ordering::Relaxed);
    Box::into_raw(Box::new(ComObject {
        vtable,
        refs: AtomicU32::new(1),
    })) as *mut c_void
}

fn allocate_command(target: Option<&str>, parent: bool) -> *mut c_void {
    OBJECTS.fetch_add(1, Ordering::Relaxed);
    Box::into_raw(Box::new(ExplorerObject {
        header: ComObject {
            vtable: &EXPLORER_VTABLE,
            refs: AtomicU32::new(1),
        },
        target: target.map(str::to_owned),
        parent,
    })) as *mut c_void
}

fn allocate_enumerator(cursor: usize) -> *mut c_void {
    OBJECTS.fetch_add(1, Ordering::Relaxed);
    Box::into_raw(Box::new(EnumeratorObject {
        header: ComObject {
            vtable: &ENUMERATOR_VTABLE,
            refs: AtomicU32::new(1),
        },
        cursor: AtomicUsize::new(cursor),
    })) as *mut c_void
}

unsafe fn query<T>(
    this: *mut c_void,
    iid: *const Guid,
    out: *mut *mut c_void,
    supported: Guid,
) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if iid.is_null() {
        return E_POINTER;
    }
    if *iid != IID_IUNKNOWN && *iid != supported {
        return E_NOINTERFACE;
    }
    add::<T>(this);
    *out = this;
    S_OK
}
unsafe fn add<T>(this: *mut c_void) -> u32 {
    (*(this as *mut ComObject<T>))
        .refs
        .fetch_add(1, Ordering::Relaxed)
        + 1
}
unsafe fn release<T>(this: *mut c_void) -> u32 {
    let object = this as *mut ComObject<T>;
    let remaining = (*object).refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(object));
        OBJECTS.fetch_sub(1, Ordering::Release);
    }
    remaining
}
unsafe extern "system" fn factory_query(
    this: *mut c_void,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    query::<FactoryVtable>(this, iid, out, IID_ICLASSFACTORY)
}
unsafe extern "system" fn factory_add(this: *mut c_void) -> u32 {
    add::<FactoryVtable>(this)
}
unsafe extern "system" fn factory_release(this: *mut c_void) -> u32 {
    release::<FactoryVtable>(this)
}
unsafe extern "system" fn explorer_query(
    this: *mut c_void,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    query::<ExplorerVtable>(this, iid, out, IID_IEXPLORERCOMMAND)
}
unsafe extern "system" fn explorer_add(this: *mut c_void) -> u32 {
    add::<ExplorerVtable>(this)
}
unsafe extern "system" fn explorer_release(this: *mut c_void) -> u32 {
    let object = this as *mut ExplorerObject;
    let remaining = (*object).header.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(object));
        OBJECTS.fetch_sub(1, Ordering::Release);
    }
    remaining
}
unsafe extern "system" fn enumerator_query(
    this: *mut c_void,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    query::<EnumeratorVtable>(this, iid, out, IID_IENUMEXPLORERCOMMAND)
}
unsafe extern "system" fn enumerator_add(this: *mut c_void) -> u32 {
    add::<EnumeratorVtable>(this)
}
unsafe extern "system" fn enumerator_release(this: *mut c_void) -> u32 {
    let object = this as *mut EnumeratorObject;
    let remaining = (*object).header.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(object));
        OBJECTS.fetch_sub(1, Ordering::Release);
    }
    remaining
}

unsafe extern "system" fn enumerator_next(
    this: *mut c_void,
    requested: u32,
    out: *mut *mut c_void,
    fetched: *mut u32,
) -> HResult {
    if !fetched.is_null() {
        *fetched = 0;
    }
    if out.is_null() {
        return E_POINTER;
    }
    let cursor = &(*(this as *mut EnumeratorObject)).cursor;
    let mut returned = 0;
    while returned < requested {
        let index = cursor.fetch_add(1, Ordering::AcqRel);
        if index > TARGETS.len() {
            break;
        }
        let target = TARGETS.get(index).copied();
        *out.add(returned as usize) = allocate_command(target, false);
        returned += 1;
    }
    if !fetched.is_null() {
        *fetched = returned;
    }
    if returned == requested {
        S_OK
    } else {
        S_FALSE
    }
}
unsafe extern "system" fn enumerator_skip(this: *mut c_void, count: u32) -> HResult {
    let old = (*(this as *mut EnumeratorObject))
        .cursor
        .fetch_add(count as usize, Ordering::AcqRel);
    if old.saturating_add(count as usize) <= TARGETS.len() + 1 {
        S_OK
    } else {
        S_FALSE
    }
}
unsafe extern "system" fn enumerator_reset(this: *mut c_void) -> HResult {
    (*(this as *mut EnumeratorObject))
        .cursor
        .store(0, Ordering::Release);
    S_OK
}
unsafe extern "system" fn enumerator_clone(this: *mut c_void, out: *mut *mut c_void) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = allocate_enumerator(
        (*(this as *mut EnumeratorObject))
            .cursor
            .load(Ordering::Acquire),
    );
    S_OK
}

unsafe extern "system" fn factory_create(
    _this: *mut c_void,
    outer: *mut c_void,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    let command = allocate_command(None, true);
    let result = explorer_query(command, iid, out);
    explorer_release(command);
    result
}
unsafe extern "system" fn factory_lock(_this: *mut c_void, lock: i32) -> HResult {
    if lock != 0 {
        LOCKS.fetch_add(1, Ordering::Relaxed);
    } else {
        let mut value = LOCKS.load(Ordering::Acquire);
        while value > 0 {
            match LOCKS.compare_exchange_weak(value, value - 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(actual) => value = actual,
            }
        }
    }
    S_OK
}

unsafe fn string_result(value: &str, out: *mut *mut u16) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let wide: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    SHStrDupW(wide.as_ptr(), out)
}
unsafe extern "system" fn explorer_title(
    this: *mut c_void,
    _items: *mut c_void,
    out: *mut *mut u16,
) -> HResult {
    let command = &*(this as *mut ExplorerObject);
    if command.parent {
        string_result("Converti in", out)
    } else if let Some(target) = command.target.as_deref() {
        let label = match target {
            "jpg" => "JPEG".to_owned(),
            "webp" => "WebP".to_owned(),
            "webm" => "WebM".to_owned(),
            "opus" => "Opus".to_owned(),
            other => other.to_uppercase(),
        };
        string_result(&label, out)
    } else {
        string_result("Impostazioni personalizzate…", out)
    }
}
unsafe extern "system" fn explorer_tooltip(
    this: *mut c_void,
    _items: *mut c_void,
    out: *mut *mut u16,
) -> HResult {
    let command = &*(this as *mut ExplorerObject);
    if command.target.is_some() {
        string_result("Converti subito con le impostazioni salvate", out)
    } else {
        string_result("Scegli formato, qualità e destinazione", out)
    }
}
unsafe extern "system" fn explorer_icon(
    _this: *mut c_void,
    _items: *mut c_void,
    out: *mut *mut u16,
) -> HResult {
    match executable_path() {
        Some(path) => string_result(&format!("{},0", path.display()), out),
        None => {
            if !out.is_null() {
                *out = ptr::null_mut();
            }
            E_NOTIMPL
        }
    }
}
unsafe extern "system" fn explorer_canonical(this: *mut c_void, out: *mut Guid) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = CLSID_CONVERTER;
    let command = &*(this as *mut ExplorerObject);
    if !command.parent {
        let index = command
            .target
            .as_deref()
            .and_then(|target| TARGETS.iter().position(|value| *value == target))
            .unwrap_or(TARGETS.len());
        (*out).d[7] ^= index as u8 + 1;
    }
    S_OK
}
unsafe extern "system" fn explorer_flags(this: *mut c_void, out: *mut u32) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    let command = &*(this as *mut ExplorerObject);
    *out = if command.parent {
        1
    } else if command.target.is_none() {
        0x20
    } else {
        0
    }; // HASSUBCOMMANDS / SEPARATORBEFORE
    S_OK
}
unsafe extern "system" fn explorer_enumerate(this: *mut c_void, out: *mut *mut c_void) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if !(*(this as *mut ExplorerObject)).parent {
        return E_NOTIMPL;
    }
    *out = allocate_enumerator(0);
    S_OK
}

// The unused slots preserve the native IShellItemArray / IShellItem ABI.
#[repr(C)]
struct ItemArrayVtable {
    unknown: UnknownVtable,
    bind: *const c_void,
    store: *const c_void,
    descriptions: *const c_void,
    attributes: *const c_void,
    count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HResult,
    item: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HResult,
    enumerate: *const c_void,
}
#[repr(C)]
struct ItemVtable {
    unknown: UnknownVtable,
    bind: *const c_void,
    parent: *const c_void,
    display_name: unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> HResult,
    attributes: *const c_void,
    compare: *const c_void,
}
unsafe fn selection_count(items: *mut c_void) -> Result<u32, HResult> {
    if items.is_null() {
        return Err(E_POINTER);
    }
    let vtable = *(items as *mut *const ItemArrayVtable);
    let mut count = 0;
    let status = ((*vtable).count)(items, &mut count);
    if status < 0 {
        Err(status)
    } else {
        Ok(count)
    }
}
unsafe fn item_path(items: *mut c_void, index: u32) -> Result<PathBuf, HResult> {
    let vtable = *(items as *mut *const ItemArrayVtable);
    let mut item = ptr::null_mut();
    let status = ((*vtable).item)(items, index, &mut item);
    if status < 0 {
        return Err(status);
    }
    if item.is_null() {
        return Err(E_FAIL);
    }
    let item_vtable = *(item as *mut *const ItemVtable);
    let mut text = ptr::null_mut();
    let status = ((*item_vtable).display_name)(item, SIGDN_FILESYSPATH, &mut text);
    ((*item_vtable).unknown.release)(item);
    if status < 0 {
        return Err(status);
    }
    if text.is_null() {
        return Err(E_FAIL);
    }
    let mut length = 0;
    while *text.add(length) != 0 {
        length += 1;
    }
    let path = PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(
        text, length,
    )));
    CoTaskMemFree(text as *const c_void);
    Ok(path)
}
fn target_mask(path: &Path) -> u32 {
    let Some(extension) = path.extension().and_then(|s| s.to_str()) else {
        return 0;
    };
    match extension.to_ascii_lowercase().as_str() {
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "wma" | "aiff" | "aif"
        | "alac" | "ape" => (1 << 7) - 1,
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "m4v" | "mpeg" | "mpg" | "ts" | "mts"
        | "m2ts" | "3gp" | "flv" => (1 << 13) - 1,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "gif" | "ico" => {
            ((1 << 20) - 1) & !((1 << 12) - 1)
        }
        "pdf" => (1 << 13) | (1 << 14) | (1 << 20),
        "doc" | "docx" | "odt" | "rtf" | "txt" | "xlsx" | "xls" | "ods" | "ppt" | "pptx"
        | "odp" => 1 << 19,
        _ => 0,
    }
}
unsafe extern "system" fn explorer_state(
    this: *mut c_void,
    items: *mut c_void,
    slow: i32,
    out: *mut u32,
) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = ECS_HIDDEN;
    let Ok(count) = selection_count(items) else {
        return S_OK;
    };
    if count == 0 {
        return S_OK;
    }
    if count > MAX_SELECTION {
        *out = ECS_DISABLED;
        return S_OK;
    }
    // Windows retries E_PENDING on a background thread with fOkToBeSlow.
    if count > 16 && slow == 0 {
        *out = ECS_DISABLED;
        return E_PENDING;
    }
    let command = &*(this as *mut ExplorerObject);
    let required = command
        .target
        .as_deref()
        .and_then(|target| TARGETS.iter().position(|value| *value == target))
        .map(|index| 1 << index);
    for index in 0..count {
        match item_path(items, index) {
            Ok(path)
                if target_mask(&path) != 0
                    && required.is_none_or(|mask| target_mask(&path) & mask != 0) => {}
            _ => return S_OK,
        }
    }
    *out = ECS_ENABLED;
    S_OK
}

fn executable_path() -> Option<PathBuf> {
    let mut module = ptr::null_mut();
    let mut buffer = vec![0u16; 32768];
    unsafe {
        // FROM_ADDRESS | UNCHANGED_REFCOUNT: identify this DLL without DllMain.
        if GetModuleHandleExW(0x6, DllCanUnloadNow as *const () as *const u16, &mut module) == 0 {
            return None;
        }
        let count = GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) as usize;
        if count == 0 || count >= buffer.len() {
            return None;
        }
        let dll = PathBuf::from(OsString::from_wide(&buffer[..count]));
        Some(dll.parent()?.join("the-converter.exe"))
    }
}

unsafe extern "system" fn explorer_invoke(
    this: *mut c_void,
    items: *mut c_void,
    _bind: *mut c_void,
) -> HResult {
    let command = &*(this as *mut ExplorerObject);
    if command.parent {
        return E_NOTIMPL;
    }
    let count = match selection_count(items) {
        Ok(count) if count > 0 && count <= MAX_SELECTION => count,
        _ => return E_FAIL,
    };
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        match item_path(items, index) {
            Ok(path) => match path.to_str() {
                Some(value) => paths.push(value.to_owned()),
                None => return E_FAIL,
            },
            Err(error) => return error,
        }
    }
    let Some(executable) = executable_path() else {
        return E_FAIL;
    };
    let Ok(payload) = serde_json::to_vec(&paths) else {
        return E_FAIL;
    };
    if payload.len() > 16 * 1024 * 1024 {
        return E_FAIL;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request = std::env::temp_dir().join(format!(
        "the-converter-selection-{}-{timestamp}-{sequence}.json",
        std::process::id()
    ));
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&request)
    {
        Ok(file) => file,
        Err(_) => return E_FAIL,
    };
    if file.write_all(&payload).is_err() {
        drop(file);
        let _ = std::fs::remove_file(&request);
        return E_FAIL;
    }
    drop(file);
    // Command performs Win32 argument quoting. Never interpolate into a shell.
    let mut process = Command::new(executable);
    if let Some(target) = command.target.as_deref() {
        process.arg("--quick").arg("--target").arg(target);
    }
    match process
        .arg("--selection-file")
        .arg(&request)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(_) => S_OK,
        Err(_) => {
            let _ = std::fs::remove_file(&request);
            E_FAIL
        }
    }
}

/// Standard COM activation entry point, called by Explorer's COM surrogate.
/// # Safety
/// The caller must supply valid COM ABI pointers for the requested interface.
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const Guid,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if clsid.is_null() || iid.is_null() {
        return E_POINTER;
    }
    if *clsid != CLSID_CONVERTER {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory = allocate(&FACTORY_VTABLE);
    let result = factory_query(factory, iid, out);
    factory_release(factory);
    result
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HResult {
    if OBJECTS.load(Ordering::Acquire) == 0 && LOCKS.load(Ordering::Acquire) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn com_lifetime_and_interface_contract() {
        unsafe {
            assert_eq!(DllCanUnloadNow(), S_OK);
            let mut factory = ptr::null_mut();
            assert_eq!(
                DllGetClassObject(&IID_IUNKNOWN, &IID_ICLASSFACTORY, &mut factory),
                CLASS_E_CLASSNOTAVAILABLE
            );
            assert!(factory.is_null());
            assert_eq!(
                DllGetClassObject(&CLSID_CONVERTER, &IID_ICLASSFACTORY, &mut factory),
                S_OK
            );
            assert_eq!(DllCanUnloadNow(), S_FALSE);
            let mut command = ptr::null_mut();
            assert_eq!(
                factory_create(factory, factory, &IID_IEXPLORERCOMMAND, &mut command),
                CLASS_E_NOAGGREGATION
            );
            assert!(command.is_null());
            assert_eq!(
                factory_create(
                    factory,
                    ptr::null_mut(),
                    &IID_IEXPLORERCOMMAND,
                    &mut command
                ),
                S_OK
            );
            let mut state = 99;
            assert_eq!(
                explorer_state(command, ptr::null_mut(), 0, &mut state),
                S_OK
            );
            assert_eq!(state, ECS_HIDDEN);
            let mut title = ptr::null_mut();
            assert_eq!(explorer_title(command, ptr::null_mut(), &mut title), S_OK);
            assert!(!title.is_null());
            CoTaskMemFree(title as *const c_void);
            let mut flags = 0;
            assert_eq!(explorer_flags(command, &mut flags), S_OK);
            assert_eq!(flags, 1);
            let mut enumerator = ptr::null_mut();
            assert_eq!(explorer_enumerate(command, &mut enumerator), S_OK);
            let mut children = [ptr::null_mut(); 2];
            let mut fetched = 0;
            assert_eq!(
                enumerator_next(enumerator, 2, children.as_mut_ptr(), &mut fetched),
                S_OK
            );
            assert_eq!(fetched, 2);
            assert_eq!(
                (*(children[0] as *mut ExplorerObject)).target.as_deref(),
                Some("mp3")
            );
            assert_eq!(
                (*(children[1] as *mut ExplorerObject)).target.as_deref(),
                Some("wav")
            );
            for child in children {
                assert_eq!(explorer_release(child), 0);
            }
            let mut clone = ptr::null_mut();
            assert_eq!(enumerator_clone(enumerator, &mut clone), S_OK);
            assert_eq!(enumerator_skip(clone, (TARGETS.len() - 2) as u32), S_OK);
            let mut custom = ptr::null_mut();
            assert_eq!(enumerator_next(clone, 1, &mut custom, &mut fetched), S_OK);
            assert!(!(*(custom as *mut ExplorerObject)).parent);
            assert!((*(custom as *mut ExplorerObject)).target.is_none());
            assert_eq!(explorer_flags(custom, &mut flags), S_OK);
            assert_eq!(flags, 0x20);
            assert_eq!(explorer_release(custom), 0);
            assert_eq!(
                enumerator_next(clone, 1, &mut custom, &mut fetched),
                S_FALSE
            );
            assert_eq!(fetched, 0);
            assert_eq!(enumerator_release(clone), 0);
            assert_eq!(enumerator_reset(enumerator), S_OK);
            assert_eq!(
                enumerator_skip(enumerator, (TARGETS.len() + 1) as u32),
                S_OK
            );
            assert_eq!(
                enumerator_next(enumerator, 1, &mut custom, &mut fetched),
                S_FALSE
            );
            assert_eq!(enumerator_release(enumerator), 0);
            assert_eq!(explorer_release(command), 0);
            assert_eq!(factory_lock(factory, 1), S_OK);
            assert_eq!(factory_release(factory), 0);
            assert_eq!(DllCanUnloadNow(), S_FALSE);
            assert_eq!(factory_lock(ptr::null_mut(), 0), S_OK);
            assert_eq!(DllCanUnloadNow(), S_OK);
        }
    }

    #[test]
    fn target_intersections_match_conversion_capabilities() {
        let formats = |mask: u32| {
            TARGETS
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (mask & (1 << index) != 0).then_some(*value))
                .collect::<Vec<_>>()
        };
        let audio = target_mask(Path::new("musica.WAV"));
        let video = target_mask(Path::new("clip.mp4"));
        let image = target_mask(Path::new("foto.png"));
        let pdf = target_mask(Path::new("documento.pdf"));
        let document = target_mask(Path::new("testo.docx"));
        assert_eq!(
            formats(audio & video),
            vec!["mp3", "wav", "flac", "aac", "ogg", "opus", "aiff"]
        );
        assert_eq!(formats(image & pdf), vec!["png", "jpg"]);
        assert_eq!(formats(document & image), vec!["pdf"]);
        assert_eq!(audio & image, 0);
        assert_eq!(target_mask(Path::new("unknown.exe")), 0);
    }
}
