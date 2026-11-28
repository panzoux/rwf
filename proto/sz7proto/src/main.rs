//! Prototype for rwf #30: load 7-Zip's 7z.dll / 7z.so at run time, enumerate formats,
//! open an archive through a Rust-implemented IInStream, list items, and extract
//! every item into a Rust-implemented ISequentialOutStream (7-Zip checks CRC itself
//! and reports it through SetOperationResult).
//!
//! usage: sz7proto <path-to-7z.dll|7z.so> <archive>...

use std::ffi::c_void;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

type HRESULT = i32;
const S_OK: HRESULT = 0;
const E_NOINTERFACE: HRESULT = 0x8000_4002_u32 as i32;
const E_FAIL: HRESULT = 0x8000_4005_u32 as i32;
const E_INVALIDARG: HRESULT = 0x8007_0057_u32 as i32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Guid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const fn iid(group: u8, sub: u8) -> Guid {
    Guid { d1: 0x2317_0F69, d2: 0x40C1, d3: 0x278A, d4: [0, 0, 0, group, 0, sub, 0, 0] }
}
const IID_IUNKNOWN: Guid = Guid { d1: 0, d2: 0, d3: 0, d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] };
const IID_ISEQUENTIAL_IN_STREAM: Guid = iid(3, 0x01);
const IID_ISEQUENTIAL_OUT_STREAM: Guid = iid(3, 0x02);
const IID_IIN_STREAM: Guid = iid(3, 0x03);
const IID_IPROGRESS: Guid = iid(0, 0x05);
const IID_IARCHIVE_EXTRACT_CALLBACK: Guid = iid(6, 0x20);
const IID_IIN_ARCHIVE: Guid = iid(6, 0x60);

// PROPVARIANT: 8-byte header + union. Windows x64: 16-byte union (24 total).
// 7-Zip's own non-Windows definition has no 16-byte members: 8-byte union (16 total).
#[cfg(windows)]
const PV_WORDS: usize = 2;
#[cfg(not(windows))]
const PV_WORDS: usize = 1;
#[repr(C)]
struct PropVariant {
    vt: u16,
    r1: u16,
    r2: u16,
    r3: u16,
    val: [u64; PV_WORDS],
}
impl PropVariant {
    fn empty() -> Self {
        PropVariant { vt: 0, r1: 0, r2: 0, r3: 0, val: [0; PV_WORDS] }
    }
}
const VT_EMPTY: u16 = 0;
const VT_BSTR: u16 = 8;
const VT_BOOL: u16 = 11;
const VT_UI4: u16 = 19;
const VT_UI8: u16 = 21;
const VT_FILETIME: u16 = 64;

// kpid
const KPID_PATH: u32 = 3;
const KPID_IS_DIR: u32 = 6;
const KPID_SIZE: u32 = 7;
const KPID_ATTRIB: u32 = 9;
const KPID_MTIME: u32 = 12;
const KPID_ENCRYPTED: u32 = 15;
const KPID_CRC: u32 = 19;
const KPID_METHOD: u32 = 22;

// NArchive::NHandlerPropID
const HPROP_NAME: u32 = 0;
const HPROP_CLASS_ID: u32 = 1;
const HPROP_EXTENSION: u32 = 2;

// ---- BSTR handling -------------------------------------------------------------
#[cfg(windows)]
mod bstr {
    #[link(name = "oleaut32")]
    unsafe extern "system" {
        pub fn SysFreeString(b: *mut u16);
        pub fn SysStringByteLen(b: *const u16) -> u32;
    }
    pub type Char = u16;
}
#[cfg(not(windows))]
mod bstr {
    // 7-Zip on Linux/macOS: BSTR = wchar_t* (4 bytes), with a 4-byte byte-length prefix,
    // allocated by its own SysAllocString (malloc of len + prefix). Free with libc free(ptr - 4).
    pub type Char = u32;
    pub unsafe fn SysStringByteLen(b: *const Char) -> u32 {
        unsafe { *(b as *const u32).sub(1) }
    }
    unsafe extern "C" {
        fn free(p: *mut std::ffi::c_void);
    }
    pub unsafe fn SysFreeString(b: *mut Char) {
        if !b.is_null() {
            unsafe { free((b as *mut u32).sub(1) as *mut std::ffi::c_void) }
        }
    }
}

unsafe fn bstr_bytes(p: *const bstr::Char) -> Vec<u8> {
    if p.is_null() {
        return Vec::new();
    }
    let n = unsafe { bstr::SysStringByteLen(p) } as usize;
    unsafe { std::slice::from_raw_parts(p as *const u8, n) }.to_vec()
}
unsafe fn bstr_string(p: *const bstr::Char) -> String {
    let bytes = unsafe { bstr_bytes(p) };
    #[cfg(windows)]
    {
        let u: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&u)
    }
    #[cfg(not(windows))]
    {
        bytes
            .chunks_exact(4)
            .filter_map(|c| char::from_u32(u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
            .collect()
    }
}

#[derive(Debug)]
enum Value {
    Empty,
    Str(String),
    Bytes(Vec<u8>),
    Bool(bool),
    U32(u32),
    U64(u64),
    FileTime(u64),
    Other(u16),
}
/// Take ownership of a PROPVARIANT filled by 7-Zip and free it.
unsafe fn take(pv: &mut PropVariant, raw_bstr: bool) -> Value {
    let v = match pv.vt {
        VT_EMPTY => Value::Empty,
        VT_BSTR => {
            let p = pv.val[0] as *mut bstr::Char;
            let v = if raw_bstr {
                Value::Bytes(unsafe { bstr_bytes(p) })
            } else {
                Value::Str(unsafe { bstr_string(p) })
            };
            unsafe { bstr::SysFreeString(p) };
            v
        }
        VT_BOOL => Value::Bool((pv.val[0] as u16) != 0),
        VT_UI4 => Value::U32(pv.val[0] as u32),
        VT_UI8 => Value::U64(pv.val[0]),
        VT_FILETIME => Value::FileTime(pv.val[0]),
        other => Value::Other(other),
    };
    *pv = PropVariant::empty();
    v
}

fn filetime_utc(ft: u64) -> chrono::DateTime<chrono::Utc> {
    let unix_100ns = ft as i64 - 116_444_736_000_000_000;
    chrono::DateTime::from_timestamp(unix_100ns.div_euclid(10_000_000), (unix_100ns.rem_euclid(10_000_000) * 100) as u32)
        .unwrap_or_default()
}

// ---- 7-Zip side vtables (we call these) -----------------------------------------
#[repr(C)]
struct IUnknownVtbl {
    qi: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}
#[repr(C)]
struct IInArchiveVtbl {
    base: IUnknownVtbl,
    open: unsafe extern "system" fn(*mut c_void, *mut c_void, *const u64, *mut c_void) -> HRESULT,
    close: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    get_number_of_items: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    get_property: unsafe extern "system" fn(*mut c_void, u32, u32, *mut PropVariant) -> HRESULT,
    extract: unsafe extern "system" fn(*mut c_void, *const u32, u32, i32, *mut c_void) -> HRESULT,
}

// ---- Rust-implemented COM objects (7-Zip calls these) ---------------------------
macro_rules! com_refcount {
    ($ty:ty, $add:ident, $rel:ident) => {
        unsafe extern "system" fn $add(this: *mut c_void) -> u32 {
            let o = unsafe { &*(this as *const $ty) };
            o.refs.fetch_add(1, Ordering::SeqCst) + 1
        }
        unsafe extern "system" fn $rel(this: *mut c_void) -> u32 {
            let o = unsafe { &*(this as *const $ty) };
            let n = o.refs.fetch_sub(1, Ordering::SeqCst) - 1;
            if n == 0 {
                drop(unsafe { Box::from_raw(this as *mut $ty) });
            }
            n
        }
    };
}

// IInStream over a std::fs::File
#[repr(C)]
struct InStreamVtbl {
    base: IUnknownVtbl,
    read: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32) -> HRESULT,
    seek: unsafe extern "system" fn(*mut c_void, i64, u32, *mut u64) -> HRESULT,
}
#[repr(C)]
struct InStream {
    vtbl: *const InStreamVtbl,
    refs: AtomicU32,
    file: Mutex<std::fs::File>,
}
com_refcount!(InStream, in_add_ref, in_release);
unsafe extern "system" fn in_qi(this: *mut c_void, riid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    let id = unsafe { *riid };
    if id == IID_IUNKNOWN || id == IID_ISEQUENTIAL_IN_STREAM || id == IID_IIN_STREAM {
        unsafe { in_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn in_read(this: *mut c_void, data: *mut c_void, size: u32, processed: *mut u32) -> HRESULT {
    let o = unsafe { &*(this as *const InStream) };
    let buf = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, size as usize) };
    let r = o.file.lock().map_err(|_| ()).and_then(|mut f| f.read(buf).map_err(|_| ()));
    let n = r.unwrap_or(0) as u32;
    if !processed.is_null() {
        unsafe { *processed = n };
    }
    if r.is_ok() { S_OK } else { E_FAIL }
}
unsafe extern "system" fn in_seek(this: *mut c_void, offset: i64, origin: u32, new_pos: *mut u64) -> HRESULT {
    let o = unsafe { &*(this as *const InStream) };
    let from = match origin {
        0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return E_INVALIDARG,
    };
    match o.file.lock().map_err(|_| ()).and_then(|mut f| f.seek(from).map_err(|_| ())) {
        Ok(p) => {
            if !new_pos.is_null() {
                unsafe { *new_pos = p };
            }
            S_OK
        }
        Err(_) => E_FAIL,
    }
}
static IN_STREAM_VTBL: InStreamVtbl = InStreamVtbl {
    base: IUnknownVtbl { qi: in_qi, add_ref: in_add_ref, release: in_release },
    read: in_read,
    seek: in_seek,
};

// ISequentialOutStream that only counts bytes
#[repr(C)]
struct OutStreamVtbl {
    base: IUnknownVtbl,
    write: unsafe extern "system" fn(*mut c_void, *const c_void, u32, *mut u32) -> HRESULT,
}
#[repr(C)]
struct OutStream {
    vtbl: *const OutStreamVtbl,
    refs: AtomicU32,
    written: Arc<AtomicU64>,
}
com_refcount!(OutStream, out_add_ref, out_release);
unsafe extern "system" fn out_qi(this: *mut c_void, riid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    let id = unsafe { *riid };
    if id == IID_IUNKNOWN || id == IID_ISEQUENTIAL_OUT_STREAM {
        unsafe { out_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn out_write(this: *mut c_void, _data: *const c_void, size: u32, processed: *mut u32) -> HRESULT {
    let o = unsafe { &*(this as *const OutStream) };
    o.written.fetch_add(size as u64, Ordering::SeqCst);
    if !processed.is_null() {
        unsafe { *processed = size };
    }
    S_OK
}
static OUT_STREAM_VTBL: OutStreamVtbl = OutStreamVtbl {
    base: IUnknownVtbl { qi: out_qi, add_ref: out_add_ref, release: out_release },
    write: out_write,
};

// IArchiveExtractCallback (derives from IProgress)
#[repr(C)]
struct ExtractCbVtbl {
    base: IUnknownVtbl,
    set_total: unsafe extern "system" fn(*mut c_void, u64) -> HRESULT,
    set_completed: unsafe extern "system" fn(*mut c_void, *const u64) -> HRESULT,
    get_stream: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, i32) -> HRESULT,
    prepare_operation: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    set_operation_result: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
}
#[repr(C)]
struct ExtractCb {
    vtbl: *const ExtractCbVtbl,
    refs: AtomicU32,
    state: Mutex<(u32, Arc<AtomicU64>)>,
    results: Mutex<Vec<(u32, u64, i32)>>,
}
com_refcount!(ExtractCb, cb_add_ref, cb_release);
unsafe extern "system" fn cb_qi(this: *mut c_void, riid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    let id = unsafe { *riid };
    if id == IID_IUNKNOWN || id == IID_IPROGRESS || id == IID_IARCHIVE_EXTRACT_CALLBACK {
        unsafe { cb_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn cb_set_total(_: *mut c_void, _: u64) -> HRESULT {
    S_OK
}
unsafe extern "system" fn cb_set_completed(_: *mut c_void, _: *const u64) -> HRESULT {
    S_OK
}
unsafe extern "system" fn cb_get_stream(this: *mut c_void, index: u32, out: *mut *mut c_void, mode: i32) -> HRESULT {
    eprintln!("trace: GetStream({index}, mode={mode})");
    let o = unsafe { &*(this as *const ExtractCb) };
    unsafe { *out = std::ptr::null_mut() };
    if mode != 0 {
        return S_OK; // test / skip
    }
    let counter = Arc::new(AtomicU64::new(0));
    if let Ok(mut s) = o.state.lock() {
        *s = (index, Arc::clone(&counter));
    }
    let stream = Box::new(OutStream { vtbl: &OUT_STREAM_VTBL, refs: AtomicU32::new(1), written: counter });
    unsafe { *out = Box::into_raw(stream) as *mut c_void };
    S_OK
}
unsafe extern "system" fn cb_prepare(_: *mut c_void, _: i32) -> HRESULT {
    S_OK
}
unsafe extern "system" fn cb_set_result(this: *mut c_void, op_res: i32) -> HRESULT {
    eprintln!("trace: SetOperationResult({op_res})");
    let o = unsafe { &*(this as *const ExtractCb) };
    if let (Ok(s), Ok(mut r)) = (o.state.lock(), o.results.lock()) {
        r.push((s.0, s.1.load(Ordering::SeqCst), op_res));
    }
    S_OK
}
static EXTRACT_CB_VTBL: ExtractCbVtbl = ExtractCbVtbl {
    base: IUnknownVtbl { qi: cb_qi, add_ref: cb_add_ref, release: cb_release },
    set_total: cb_set_total,
    set_completed: cb_set_completed,
    get_stream: cb_get_stream,
    prepare_operation: cb_prepare,
    set_operation_result: cb_set_result,
};

// ---- external codecs (Debian's /usr/lib/7zip/Codecs/Rar.so) ----------------------
// 7z.so does not load Codecs/*.so itself. The 7z CLI loads them and hands each archive
// object an ICompressCodecsInfo through ISetCompressCodecsInfo; we do the same.
const IID_ICOMPRESS_CODECS_INFO: Guid = iid(4, 0x60);
const IID_ISET_COMPRESS_CODECS_INFO: Guid = iid(4, 0x61);
// NMethodPropID
const MPROP_DECODER: u32 = 2;
const MPROP_ENCODER: u32 = 3;
const MPROP_DECODER_IS_ASSIGNED: u32 = 7;
const MPROP_ENCODER_IS_ASSIGNED: u32 = 8;

type GetNumberOfMethodsFn = unsafe extern "system" fn(*mut u32) -> HRESULT;
type GetMethodPropertyFn = unsafe extern "system" fn(u32, u32, *mut PropVariant) -> HRESULT;
type CreateCoderFn = unsafe extern "system" fn(u32, *const Guid, *mut *mut c_void) -> HRESULT;

struct CodecLib {
    _lib: libloading::Library,
    get_prop: GetMethodPropertyFn,
    create_object: Option<CreateObjectFn>,
    create_decoder: Option<CreateCoderFn>,
    create_encoder: Option<CreateCoderFn>,
}
struct CodecRef {
    lib: usize,
    index: u32,
}
struct Codecs {
    libs: Vec<CodecLib>,
    methods: Vec<CodecRef>,
}

#[repr(C)]
struct CodecsInfoVtbl {
    base: IUnknownVtbl,
    get_num_methods: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    get_property: unsafe extern "system" fn(*mut c_void, u32, u32, *mut PropVariant) -> HRESULT,
    create_decoder: unsafe extern "system" fn(*mut c_void, u32, *const Guid, *mut *mut c_void) -> HRESULT,
    create_encoder: unsafe extern "system" fn(*mut c_void, u32, *const Guid, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
struct CodecsInfo {
    vtbl: *const CodecsInfoVtbl,
    refs: AtomicU32,
    codecs: Codecs,
}
com_refcount!(CodecsInfo, ci_add_ref, ci_release);
unsafe extern "system" fn ci_qi(this: *mut c_void, riid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    let id = unsafe { *riid };
    if id == IID_IUNKNOWN || id == IID_ICOMPRESS_CODECS_INFO {
        unsafe { ci_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
}
fn ci_codecs<'a>(this: *mut c_void) -> &'a Codecs {
    &unsafe { &*(this as *const CodecsInfo) }.codecs
}
unsafe extern "system" fn ci_num(this: *mut c_void, n: *mut u32) -> HRESULT {
    eprintln!("trace: ICompressCodecsInfo::GetNumMethods");
    unsafe { *n = ci_codecs(this).methods.len() as u32 };
    S_OK
}
/// The coder's GUID (kDecoder / kEncoder), if the module has one.
fn coder_guid(c: &Codecs, index: u32, pid: u32) -> Option<Guid> {
    let m = c.methods.get(index as usize)?;
    let mut pv = PropVariant::empty();
    unsafe { (c.libs[m.lib].get_prop)(m.index, pid, &mut pv) };
    match unsafe { take(&mut pv, true) } {
        Value::Bytes(b) if b.len() == 16 => Some(unsafe { std::ptr::read_unaligned(b.as_ptr() as *const Guid) }),
        _ => None,
    }
}
unsafe extern "system" fn ci_prop(this: *mut c_void, index: u32, pid: u32, value: *mut PropVariant) -> HRESULT {
    let c = ci_codecs(this);
    eprintln!("trace: ICompressCodecsInfo::GetProperty({index}, {pid})");
    let Some(m) = c.methods.get(index as usize) else { return E_INVALIDARG };
    if pid == MPROP_DECODER_IS_ASSIGNED || pid == MPROP_ENCODER_IS_ASSIGNED {
        let which = if pid == MPROP_DECODER_IS_ASSIGNED { MPROP_DECODER } else { MPROP_ENCODER };
        let assigned = coder_guid(c, index, which).is_some();
        let mut pv = PropVariant::empty();
        pv.vt = VT_BOOL;
        pv.val[0] = if assigned { 0xFFFF } else { 0 }; // VARIANT_TRUE = -1
        unsafe { std::ptr::write(value, pv) };
        return S_OK;
    }
    unsafe { (c.libs[m.lib].get_prop)(m.index, pid, value) }
}
unsafe fn ci_create(this: *mut c_void, index: u32, iid: *const Guid, out: *mut *mut c_void, encoder: bool) -> HRESULT {
    let c = ci_codecs(this);
    let Some(m) = c.methods.get(index as usize) else { return E_INVALIDARG };
    let lib = &c.libs[m.lib];
    eprintln!("trace: ICompressCodecsInfo::Create{}({index}, iid={:?})", if encoder { "Encoder" } else { "Decoder" }, unsafe { *iid });
    let direct = if encoder { lib.create_encoder } else { lib.create_decoder };
    if let Some(f) = direct {
        let hr = unsafe { f(m.index, iid, out) };
        eprintln!("trace:   -> hr={hr:#x} obj={:?}", unsafe { *out });
        return hr;
    }
    match (lib.create_object, coder_guid(c, index, if encoder { MPROP_ENCODER } else { MPROP_DECODER })) {
        (Some(f), Some(g)) => unsafe { f(&g, iid, out) },
        _ => E_FAIL,
    }
}
unsafe extern "system" fn ci_decoder(this: *mut c_void, i: u32, iid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    unsafe { ci_create(this, i, iid, out, false) }
}
unsafe extern "system" fn ci_encoder(this: *mut c_void, i: u32, iid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    unsafe { ci_create(this, i, iid, out, true) }
}
static CODECS_INFO_VTBL: CodecsInfoVtbl = CodecsInfoVtbl {
    base: IUnknownVtbl { qi: ci_qi, add_ref: ci_add_ref, release: ci_release },
    get_num_methods: ci_num,
    get_property: ci_prop,
    create_decoder: ci_decoder,
    create_encoder: ci_encoder,
};

#[repr(C)]
struct SetCodecsVtbl {
    base: IUnknownVtbl,
    set_compress_codecs_info: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

/// Load every shared library in `<dir of 7z lib>/Codecs/`. Returns a COM object that
/// lives for the whole process (we keep our own reference forever), or null.
fn load_codecs(lib_path: &str) -> *mut c_void {
    let dir = std::path::Path::new(lib_path).parent().map(|p| p.join("Codecs"));
    let Some(Ok(rd)) = dir.as_ref().map(std::fs::read_dir) else {
        println!("codecs: no {:?}", dir);
        return std::ptr::null_mut();
    };
    let mut libs = Vec::new();
    let mut methods = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "so" && ext != "dll" {
            continue;
        }
        let Ok(lib) = (unsafe { libloading::Library::new(&p) }) else {
            println!("codecs: cannot load {}", p.display());
            continue;
        };
        let num = unsafe { lib.get::<GetNumberOfMethodsFn>(b"GetNumberOfMethods") }.map(|s| *s);
        let get_prop = unsafe { lib.get::<GetMethodPropertyFn>(b"GetMethodProperty") }.map(|s| *s);
        let (Ok(num), Ok(get_prop)) = (num, get_prop) else {
            println!("codecs: {} is not a codec module", p.display());
            continue;
        };
        let create_object = unsafe { lib.get::<CreateObjectFn>(b"CreateObject") }.ok().map(|s| *s);
        let create_decoder = unsafe { lib.get::<CreateCoderFn>(b"CreateDecoder") }.ok().map(|s| *s);
        let create_encoder = unsafe { lib.get::<CreateCoderFn>(b"CreateEncoder") }.ok().map(|s| *s);
        let has_set_codecs = unsafe { lib.get::<*const c_void>(b"SetCodecs") }.is_ok();
        // Debian 23.01's Rar.so never fills its CRC function pointer (crash in CrcCalc).
        if std::env::var_os("SZ_CRC_INIT").is_some() {
            match unsafe { lib.get::<unsafe extern "C" fn()>(b"CrcGenerateTable") } {
                Ok(f) => {
                    unsafe { f() };
                    println!("codecs: called CrcGenerateTable in {}", p.display());
                }
                Err(_) => println!("codecs: CrcGenerateTable not exported by {}", p.display()),
            }
        }
        type GetModulePropFn = unsafe extern "system" fn(u32, *mut PropVariant) -> HRESULT;
        if let Ok(gmp) = unsafe { lib.get::<GetModulePropFn>(b"GetModuleProp") } {
            let mut pv = PropVariant::empty();
            unsafe { gmp(0, &mut pv) };
            let itype = unsafe { take(&mut pv, false) };
            unsafe { gmp(1, &mut pv) };
            let ver = unsafe { take(&mut pv, false) };
            println!("codecs: {} GetModuleProp interface type={itype:?} version={ver:?}", p.display());
        } else {
            println!("codecs: {} has no GetModuleProp", p.display());
        }
        let mut n = 0u32;
        unsafe { num(&mut n) };
        let li = libs.len();
        let mut names = Vec::new();
        for i in 0..n {
            let mut pv = PropVariant::empty();
            unsafe { get_prop(i, 1, &mut pv) }; // kName
            names.push(match unsafe { take(&mut pv, false) } {
                Value::Str(s) => s,
                v => format!("{v:?}"),
            });
            methods.push(CodecRef { lib: li, index: i });
        }
        println!(
            "codecs: {} -> {n} methods [{}] CreateDecoder={} CreateObject={} SetCodecs={}",
            p.display(),
            names.join(" "),
            create_decoder.is_some(),
            create_object.is_some(),
            has_set_codecs
        );
        libs.push(CodecLib { _lib: lib, get_prop, create_object, create_decoder, create_encoder });
    }
    if methods.is_empty() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(CodecsInfo {
        vtbl: &CODECS_INFO_VTBL,
        refs: AtomicU32::new(1),
        codecs: Codecs { libs, methods },
    })) as *mut c_void
}

/// Give the archive object our codec list, as the 7z CLI does right after CreateObject.
fn attach_codecs(archive: *mut c_void, codecs: *mut c_void) -> bool {
    if codecs.is_null() {
        return false;
    }
    let vt = unsafe { &**(archive as *const *const IUnknownVtbl) };
    let mut set: *mut c_void = std::ptr::null_mut();
    if unsafe { (vt.qi)(archive, &IID_ISET_COMPRESS_CODECS_INFO, &mut set) } != S_OK || set.is_null() {
        return false;
    }
    let svt = unsafe { &**(set as *const *const SetCodecsVtbl) };
    let hr = unsafe { (svt.set_compress_codecs_info)(set, codecs) };
    unsafe { (svt.base.release)(set) };
    hr == S_OK
}

// ---- RAR through unrar-ng (for 7z.so builds without a RAR decoder) ------------------
fn unrar_check(path: &str) -> bool {
    let t = Instant::now();
    let listing = match unrar_ng::Archive::new(path).open_for_listing() {
        Ok(l) => l,
        Err(e) => {
            println!("unrar-ng: open_for_listing failed: {e}");
            return false;
        }
    };
    let mut expected = Vec::new();
    for e in listing {
        match e {
            Ok(e) if !e.is_directory() => expected.push((e.filename.display().to_string(), e.unpacked_size)),
            Ok(_) => {}
            Err(e) => {
                println!("unrar-ng: list error {e}");
                return false;
            }
        }
    }
    let mut a = match unrar_ng::Archive::new(path).open_for_processing() {
        Ok(a) => a,
        Err(e) => {
            println!("unrar-ng: open_for_processing failed: {e}");
            return false;
        }
    };
    let mut ok = true;
    let mut i = 0;
    loop {
        let h = match a.read_header() {
            Ok(Some(h)) => h,
            Ok(None) => break,
            Err(e) => {
                println!("unrar-ng: header error {e}");
                return false;
            }
        };
        if h.entry().is_directory() {
            a = match h.skip() {
                Ok(r) => r,
                Err(e) => {
                    println!("unrar-ng: skip error {e}");
                    return false;
                }
            };
            continue;
        }
        match h.read() {
            Ok((data, rest)) => {
                let (name, size) = expected.get(i).cloned().unwrap_or_default();
                let good = data.len() as u64 == size;
                ok &= good;
                println!("  unrar-ng [{i}] {name} bytes={} expected={size} {}", data.len(), if good { "OK" } else { "NG" });
                a = rest;
            }
            Err(e) => {
                println!("unrar-ng: read error {e}");
                return false;
            }
        }
        i += 1;
    }
    let all = ok && i == expected.len();
    println!("unrar-ng ALL {} ({} files) in {:?}", if all { "OK" } else { "NG" }, i, t.elapsed());
    all
}

// ---- driver ---------------------------------------------------------------------
struct Format {
    name: String,
    class_id: Guid,
    exts: Vec<String>,
}

type CreateObjectFn = unsafe extern "system" fn(*const Guid, *const Guid, *mut *mut c_void) -> HRESULT;
type GetNumberOfFormatsFn = unsafe extern "system" fn(*mut u32) -> HRESULT;
type GetHandlerProperty2Fn = unsafe extern "system" fn(u32, u32, *mut PropVariant) -> HRESULT;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib_path = &args[1];
    let t0 = Instant::now();
    let lib = unsafe { libloading::Library::new(lib_path) }.expect("load 7z library");
    let create: libloading::Symbol<CreateObjectFn> = unsafe { lib.get(b"CreateObject") }.expect("CreateObject");
    let nformats: libloading::Symbol<GetNumberOfFormatsFn> =
        unsafe { lib.get(b"GetNumberOfFormats") }.expect("GetNumberOfFormats");
    let hprop: libloading::Symbol<GetHandlerProperty2Fn> =
        unsafe { lib.get(b"GetHandlerProperty2") }.expect("GetHandlerProperty2");
    println!("loaded {} in {:?}", lib_path, t0.elapsed());
    println!("sizeof(PROPVARIANT) as used here = {}", std::mem::size_of::<PropVariant>());
    type GetModulePropFn = unsafe extern "system" fn(u32, *mut PropVariant) -> HRESULT;
    match unsafe { lib.get::<GetModulePropFn>(b"GetModuleProp") } {
        Ok(gmp) => {
            let mut pv = PropVariant::empty();
            unsafe { gmp(0, &mut pv) };
            let itype = unsafe { take(&mut pv, false) };
            unsafe { gmp(1, &mut pv) };
            let ver = unsafe { take(&mut pv, false) };
            let ver = match ver {
                Value::U32(v) => format!("{}.{:02}", v >> 16, v & 0xffff),
                v => format!("{v:?}"),
            };
            println!("GetModuleProp: interface type (1 = virtual destructor in IUnknown) = {itype:?}, version = {ver}");
            if let Value::U32(1) = itype {
                println!("ABI has virtual destructor slots -- this prototype does not support it");
                std::process::exit(2);
            }
        }
        Err(_) => {
            println!("GetModuleProp not exported: 7-Zip older than 23 or p7zip -- unsupported ABI");
            std::process::exit(2);
        }
    }

    let mut n = 0u32;
    assert_eq!(unsafe { nformats(&mut n) }, S_OK);
    let mut formats = Vec::new();
    for i in 0..n {
        let mut pv = PropVariant::empty();
        unsafe { hprop(i, HPROP_NAME, &mut pv) };
        let name = match unsafe { take(&mut pv, false) } {
            Value::Str(s) => s,
            v => format!("{v:?}"),
        };
        unsafe { hprop(i, HPROP_CLASS_ID, &mut pv) };
        let class_id = match unsafe { take(&mut pv, true) } {
            Value::Bytes(b) if b.len() == 16 => unsafe { std::ptr::read_unaligned(b.as_ptr() as *const Guid) },
            v => panic!("class id of {name}: {v:?}"),
        };
        unsafe { hprop(i, HPROP_EXTENSION, &mut pv) };
        let exts = match unsafe { take(&mut pv, false) } {
            Value::Str(s) => s.split_whitespace().map(str::to_string).collect(),
            _ => vec![],
        };
        formats.push(Format { name, class_id, exts });
    }
    println!(
        "{} formats: {}",
        formats.len(),
        formats.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(" ")
    );

    let codecs = load_codecs(lib_path);

    let mut failures = 0;
    for arc in &args[2..] {
        let mut opened = false;
        let mut seven_ok = false;
        println!("\n=== {arc}");
        let ext = std::path::Path::new(arc)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // Candidates: formats claiming the extension first, then everything else.
        let mut order: Vec<&Format> = formats.iter().filter(|f| f.exts.iter().any(|e| *e == ext)).collect();
        order.extend(formats.iter().filter(|f| !f.exts.iter().any(|e| *e == ext)));

        for f in order {
            let t = Instant::now();
            let mut obj: *mut c_void = std::ptr::null_mut();
            if unsafe { create(&f.class_id, &IID_IIN_ARCHIVE, &mut obj) } != S_OK || obj.is_null() {
                continue;
            }
            let vt = unsafe { &**(obj as *const *const IInArchiveVtbl) };
            let attached = attach_codecs(obj, codecs);
            let file = std::fs::File::open(arc).expect("open archive");
            let stream = Box::into_raw(Box::new(InStream {
                vtbl: &IN_STREAM_VTBL,
                refs: AtomicU32::new(1),
                file: Mutex::new(file),
            })) as *mut c_void;
            let max_check: u64 = 1 << 23;
            let hr = unsafe { (vt.open)(obj, stream, &max_check, std::ptr::null_mut()) };
            if hr != S_OK {
                unsafe { (vt.base.release)(obj) };
                unsafe { in_release(stream) };
                continue;
            }
            let mut count = 0u32;
            unsafe { (vt.get_number_of_items)(obj, &mut count) };
            println!("opened as {} ({} items) in {:?}, external codecs attached={attached}", f.name, count, t.elapsed());
            let mut sizes = Vec::new();
            for i in 0..count {
                let mut get = |pid: u32| {
                    let mut pv = PropVariant::empty();
                    unsafe { (vt.get_property)(obj, i, pid, &mut pv) };
                    unsafe { take(&mut pv, false) }
                };
                let path = get(KPID_PATH);
                let is_dir = get(KPID_IS_DIR);
                let size = get(KPID_SIZE);
                let mtime = get(KPID_MTIME);
                let attrib = get(KPID_ATTRIB);
                let crc = get(KPID_CRC);
                let enc = get(KPID_ENCRYPTED);
                let method = get(KPID_METHOD);
                let mt = match mtime {
                    Value::FileTime(ft) => format!(
                        "{} (local {})",
                        filetime_utc(ft).format("%Y-%m-%dT%H:%M:%SZ"),
                        filetime_utc(ft).with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")
                    ),
                    v => format!("{v:?}"),
                };
                sizes.push(match size {
                    Value::U64(s) => s,
                    Value::U32(s) => s as u64,
                    _ => 0,
                });
                println!(
                    "  [{i}] {:?} dir={:?} size={:?} mtime={} attrib={:?} crc={:?} enc={:?} method={:?}",
                    path, is_dir, sizes[i as usize], mt, attrib, crc, enc, method
                );
            }

            let te = Instant::now();
            let cb = Box::into_raw(Box::new(ExtractCb {
                vtbl: &EXTRACT_CB_VTBL,
                refs: AtomicU32::new(1),
                state: Mutex::new((0, Arc::new(AtomicU64::new(0)))),
                results: Mutex::new(Vec::new()),
            })) as *mut c_void;
            let hr = unsafe { (vt.extract)(obj, std::ptr::null(), u32::MAX, 0, cb) };
            let results = unsafe { &*(cb as *const ExtractCb) }.results.lock().map(|r| r.clone()).unwrap_or_default();
            println!("extract hr={hr:#x} in {:?}", te.elapsed());
            let mut ok = true;
            for (idx, bytes, res) in &results {
                let expect = sizes.get(*idx as usize).copied().unwrap_or(0);
                let good = *res == 0 && *bytes == expect;
                ok &= good;
                println!("  [{idx}] opRes={res} bytes={bytes} expected={expect} {}", if good { "OK" } else { "NG" });
            }
            let all = ok && results.len() == count as usize;
            println!("ALL {}", if all { "OK" } else { "NG" });
            seven_ok = all;
            opened = true;

            unsafe { cb_release(cb) };
            unsafe { (vt.close)(obj) };
            unsafe { (vt.base.release)(obj) };
            unsafe { in_release(stream) };
            break;
        }
        if !opened {
            println!("could not open with any format");
        }
        // RAR: always try unrar-ng too, so CI shows whether it works on every OS.
        let is_rar = arc.to_lowercase().ends_with(".rar");
        let unrar_ok = if is_rar { Some(unrar_check(arc)) } else { None };
        println!("RESULT {arc}: 7-Zip={} unrar-ng={}", if seven_ok { "OK" } else { "NG" }, match unrar_ok {
            Some(true) => "OK",
            Some(false) => "NG",
            None => "-",
        });
        let pass = (seven_ok || unrar_ok == Some(true)) && unrar_ok != Some(false);
        if !pass && !arc.contains("enc") {
            failures += 1;
        }
    }
    println!("
failures: {failures}");
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
