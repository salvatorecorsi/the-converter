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
const UNKNOWN_SELECTION_MASK: u32 = u32::MAX;
const SELECTION_CONTAINS_AUDIO: u32 = 1 << 31;
const AUDIO_TARGETS: u32 = (1 << 7) - 1;
const TEXT_TARGET: u32 = 1 << 20;
const OPENAI_CREDENTIAL_TARGET: &str = "Corsivo.TheConverter.OpenAI";

#[repr(C)]
struct Credential {
    flags: u32,
    kind: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written: [u32; 2],
    blob_size: u32,
    blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[link(name = "advapi32")]
extern "system" {
    fn CredReadW(target: *const u16, kind: u32, flags: u32, out: *mut *mut Credential) -> i32;
    fn CredFree(buffer: *mut c_void);
}

fn credential_metadata_available(credential: &Credential) -> bool {
    credential.kind == 1
        && !credential.blob.is_null()
        && (20..=2048).contains(&credential.blob_size)
}

fn transcription_key_available() -> bool {
    let target: Vec<u16> = OPENAI_CREDENTIAL_TARGET
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut credential = ptr::null_mut();
    unsafe {
        if CredReadW(target.as_ptr(), 1, 0, &mut credential) == 0 || credential.is_null() {
            return false;
        }
        // Only inspect metadata. The DLL never copies, decodes or logs the key.
        let available = credential_metadata_available(&*credential);
        if !(*credential).blob.is_null() && (*credential).blob_size <= 2560 {
            // WinCred owns this temporary copy, not the saved credential. Wipe
            // it without reading it before returning the allocation to WinCred.
            for offset in 0..(*credential).blob_size as usize {
                ptr::write_volatile((*credential).blob.add(offset), 0);
            }
        }
        CredFree(credential.cast());
        available
    }
}

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
const ALL_TARGETS: u32 = (1 << TARGETS.len()) - 1;

fn text_target_title(selection: u32) -> &'static str {
    if selection != UNKNOWN_SELECTION_MASK && selection & SELECTION_CONTAINS_AUDIO != 0 {
        "TXT con OpenAI"
    } else {
        "TXT"
    }
}

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
    selection_mask: AtomicU32,
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
    mask: u32,
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
        selection_mask: AtomicU32::new(UNKNOWN_SELECTION_MASK),
    })) as *mut c_void
}

fn allocate_enumerator(cursor: usize, mask: u32) -> *mut c_void {
    OBJECTS.fetch_add(1, Ordering::Relaxed);
    Box::into_raw(Box::new(EnumeratorObject {
        header: ComObject {
            vtable: &ENUMERATOR_VTABLE,
            refs: AtomicU32::new(1),
        },
        cursor: AtomicUsize::new(cursor),
        mask,
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
    let mask = (*(this as *mut EnumeratorObject)).mask;
    let mut returned = 0;
    while returned < requested {
        let index = cursor.fetch_add(1, Ordering::AcqRel);
        if index > TARGETS.len() {
            break;
        }
        if (index < TARGETS.len() && mask & (1 << index) == 0)
            || (index == TARGETS.len() && mask & ALL_TARGETS == 0)
        {
            continue;
        }
        let target = TARGETS.get(index).copied();
        let child = allocate_command(target, false);
        (*(child as *mut ExplorerObject))
            .selection_mask
            .store(mask, Ordering::Release);
        *out.add(returned as usize) = child;
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
    let object = &*(this as *mut EnumeratorObject);
    for _ in 0..count {
        loop {
            let index = object.cursor.fetch_add(1, Ordering::AcqRel);
            if index > TARGETS.len() {
                return S_FALSE;
            }
            if (index < TARGETS.len() && object.mask & (1 << index) != 0)
                || (index == TARGETS.len() && object.mask & ALL_TARGETS != 0)
            {
                break;
            }
        }
    }
    S_OK
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
        (*(this as *mut EnumeratorObject)).mask,
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
            "txt" => text_target_title(command.selection_mask.load(Ordering::Acquire)).to_owned(),
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
    let mask = (*(this as *mut ExplorerObject))
        .selection_mask
        .load(Ordering::Acquire);
    // Some callers enumerate before GetState. Only filter an actual per-object
    // selection snapshot; child GetState and Invoke always validate their inputs.
    *out = allocate_enumerator(0, mask);
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
    attributes: unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> HResult,
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
    let mut attributes = 0;
    let attributes_status = ((*item_vtable).attributes)(item, 0x60000000, &mut attributes);
    if attributes_status < 0 || attributes & 0x20000000 != 0 || attributes & 0x40000000 == 0 {
        ((*item_vtable).unknown.release)(item);
        return Err(E_FAIL);
    }
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
fn target_mask(path: &Path, transcription_enabled: bool) -> u32 {
    let Some(extension) = path.extension().and_then(|s| s.to_str()) else {
        return 0;
    };
    match extension.to_ascii_lowercase().as_str() {
        "mp3" | "mpga" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "wma" | "aiff"
        | "aif" | "alac" | "ape" => {
            AUDIO_TARGETS
                | if transcription_enabled {
                    TEXT_TARGET
                } else {
                    0
                }
        }
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

fn common_target_mask<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
    transcription_enabled: bool,
) -> u32 {
    let mut paths = paths.into_iter();
    let Some(first) = paths.next() else {
        return 0;
    };
    paths.fold(target_mask(first, transcription_enabled), |mask, path| {
        mask & target_mask(path, transcription_enabled)
    })
}

fn needs_transcription_metadata(paths: &[PathBuf], target: Option<&str>) -> bool {
    (target.is_none() || target == Some("txt"))
        && paths
            .iter()
            .any(|path| target_mask(path, false) == AUDIO_TARGETS)
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
    let command = &*(this as *mut ExplorerObject);
    command
        .selection_mask
        .store(UNKNOWN_SELECTION_MASK, Ordering::Release);
    let Ok(count) = selection_count(items) else {
        return S_OK;
    };
    if count == 0 {
        command.selection_mask.store(0, Ordering::Release);
        return S_OK;
    }
    if count > MAX_SELECTION {
        command.selection_mask.store(0, Ordering::Release);
        *out = ECS_DISABLED;
        return S_OK;
    }
    // Windows retries E_PENDING on a background thread with fOkToBeSlow.
    if count > 16 && slow == 0 {
        *out = ECS_DISABLED;
        return E_PENDING;
    }
    let required = command
        .target
        .as_deref()
        .and_then(|target| TARGETS.iter().position(|value| *value == target))
        .map(|index| 1 << index);
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        match item_path(items, index) {
            Ok(path) => paths.push(path),
            _ => {
                command.selection_mask.store(0, Ordering::Release);
                return S_OK;
            }
        }
    }
    let transcription_enabled = needs_transcription_metadata(&paths, command.target.as_deref())
        && transcription_key_available();
    let mask = common_target_mask(paths.iter().map(PathBuf::as_path), transcription_enabled);
    let contains_audio = paths
        .iter()
        .any(|path| target_mask(path, false) == AUDIO_TARGETS);
    command.selection_mask.store(
        mask | if mask != 0 && contains_audio {
            SELECTION_CONTAINS_AUDIO
        } else {
            0
        },
        Ordering::Release,
    );
    if mask != 0 && required.is_none_or(|required| mask & required != 0) {
        *out = ECS_ENABLED;
    }
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
    let mut selected_paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        match item_path(items, index) {
            Ok(path) => selected_paths.push(path),
            Err(error) => return error,
        }
    }
    let transcription_enabled =
        needs_transcription_metadata(&selected_paths, command.target.as_deref())
            && transcription_key_available();
    let mask = common_target_mask(
        selected_paths.iter().map(PathBuf::as_path),
        transcription_enabled,
    );
    if mask == 0
        || command.target.as_deref().is_some_and(|target| {
            TARGETS
                .iter()
                .position(|value| *value == target)
                .is_none_or(|index| mask & (1 << index) == 0)
        })
    {
        return E_FAIL;
    }
    let mut paths = Vec::with_capacity(selected_paths.len());
    for path in &selected_paths {
        match path.to_str() {
            Some(value) => paths.push(value.to_owned()),
            None => return E_FAIL,
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
            let filtered = allocate_enumerator(0, 1 << 19);
            let mut filtered_children = [ptr::null_mut(); 3];
            assert_eq!(
                enumerator_next(filtered, 3, filtered_children.as_mut_ptr(), &mut fetched),
                S_FALSE
            );
            assert_eq!(fetched, 2); // PDF, then custom settings; no impossible targets.
            assert_eq!(
                (*(filtered_children[0] as *mut ExplorerObject))
                    .target
                    .as_deref(),
                Some("pdf")
            );
            assert!((*(filtered_children[1] as *mut ExplorerObject))
                .target
                .is_none());
            for child in &filtered_children[..2] {
                assert_eq!(explorer_release(*child), 0);
            }
            assert_eq!(enumerator_reset(filtered), S_OK);
            assert_eq!(enumerator_skip(filtered, 1), S_OK);
            assert_eq!(
                enumerator_next(filtered, 1, &mut custom, &mut fetched),
                S_OK
            );
            assert!((*(custom as *mut ExplorerObject)).target.is_none());
            assert_eq!(explorer_release(custom), 0);
            assert_eq!(enumerator_release(filtered), 0);
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
        let audio = target_mask(Path::new("musica.WAV"), false);
        let video = target_mask(Path::new("clip.mp4"), true);
        let image = target_mask(Path::new("foto.png"), true);
        let pdf = target_mask(Path::new("documento.pdf"), false);
        let document = target_mask(Path::new("testo.docx"), true);
        assert_eq!(
            formats(audio & video),
            vec!["mp3", "wav", "flac", "aac", "ogg", "opus", "aiff"]
        );
        assert_eq!(formats(image & pdf), vec!["png", "jpg"]);
        assert_eq!(formats(document & image), vec!["pdf"]);
        assert_eq!(audio & image, 0);
        assert_eq!(target_mask(Path::new("unknown.exe"), true), 0);
        for extension in [
            "txt", "doc", "docx", "odt", "rtf", "xls", "xlsx", "ods", "ppt", "pptx", "odp",
        ] {
            assert_eq!(
                formats(target_mask(Path::new(&format!("file.{extension}")), true)),
                vec!["pdf"]
            );
        }
        let audio_with_key = target_mask(Path::new("musica.mpga"), true);
        assert_eq!(audio_with_key, audio | TEXT_TARGET);
        assert_eq!(formats(audio_with_key & video), formats(audio)); // Video is never transcribed.
        assert_eq!(formats(audio_with_key & pdf), vec!["txt"]);
        assert_eq!(audio & pdf, 0);
        let batch = [Path::new("testo.txt"), Path::new("foto.png")];
        assert_eq!(formats(common_target_mask(batch, true)), vec!["pdf"]);
        assert_eq!(
            common_target_mask([Path::new("testo.txt"), Path::new("audio.wav")], true),
            0
        );
        assert_eq!(
            common_target_mask([Path::new("audio.wav"), Path::new("unknown.exe")], true),
            0
        );
        assert_eq!(common_target_mask(std::iter::empty(), true), 0);
        assert_eq!(text_target_title(UNKNOWN_SELECTION_MASK), "TXT");
        assert_eq!(text_target_title(TEXT_TARGET), "TXT");
        assert_eq!(
            text_target_title(TEXT_TARGET | SELECTION_CONTAINS_AUDIO),
            "TXT con OpenAI"
        );
    }

    #[test]
    fn credential_availability_reads_metadata_without_dereferencing_the_secret() {
        let mut credential = Credential {
            flags: 0,
            kind: 1,
            target_name: ptr::null_mut(),
            comment: ptr::null_mut(),
            last_written: [0, 0],
            blob_size: 20,
            blob: ptr::NonNull::<u8>::dangling().as_ptr(),
            persist: 2,
            attribute_count: 0,
            attributes: ptr::null_mut(),
            target_alias: ptr::null_mut(),
            user_name: ptr::null_mut(),
        };
        assert!(credential_metadata_available(&credential));
        credential.blob_size = 2048;
        assert!(credential_metadata_available(&credential));
        for length in [0, 19, 2049] {
            credential.blob_size = length;
            assert!(!credential_metadata_available(&credential));
        }
        credential.blob_size = 20;
        credential.kind = 2;
        assert!(!credential_metadata_available(&credential));
        credential.kind = 1;
        credential.blob = ptr::null_mut();
        assert!(!credential_metadata_available(&credential));
    }
}
