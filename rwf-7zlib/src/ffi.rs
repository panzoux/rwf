//! The raw 7-Zip ABI: GUIDs, `PROPVARIANT`, `BSTR` and the vtable layouts.
//!
//! Only the 7-Zip 23+ layout is supported: `IUnknown` without a virtual destructor
//! (`GetModuleProp(kInterfaceType) == 0`). 7-Zip 22 and older, and p7zip, put two
//! destructor slots into every vtable, which would shift every method below.

use std::ffi::c_void;
use std::time::{Duration, SystemTime};

pub type HResult = i32;
pub const S_OK: HResult = 0;
pub const E_NOINTERFACE: HResult = 0x8000_4002_u32 as i32;
pub const E_ABORT: HResult = 0x8000_4004_u32 as i32;
pub const E_FAIL: HResult = 0x8000_4005_u32 as i32;
pub const E_INVALIDARG: HResult = 0x8007_0057_u32 as i32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Guid {
    pub d1: u32,
    pub d2: u16,
    pub d3: u16,
    pub d4: [u8; 8],
}

/// 7-Zip interface IDs are `{23170F69-40C1-278A-0000-00gg00ss0000}`.
const fn iid(group: u8, sub: u8) -> Guid {
    Guid {
        d1: 0x2317_0F69,
        d2: 0x40C1,
        d3: 0x278A,
        d4: [0, 0, 0, group, 0, sub, 0, 0],
    }
}
pub const IID_IUNKNOWN: Guid = Guid {
    d1: 0,
    d2: 0,
    d3: 0,
    d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
pub const IID_IPROGRESS: Guid = iid(0, 0x05);
pub const IID_ISEQUENTIAL_IN_STREAM: Guid = iid(3, 0x01);
pub const IID_ISEQUENTIAL_OUT_STREAM: Guid = iid(3, 0x02);
pub const IID_IIN_STREAM: Guid = iid(3, 0x03);
pub const IID_IARCHIVE_EXTRACT_CALLBACK: Guid = iid(6, 0x20);
pub const IID_IIN_ARCHIVE: Guid = iid(6, 0x60);

// kpid*
pub const KPID_PATH: u32 = 3;
pub const KPID_IS_DIR: u32 = 6;
pub const KPID_SIZE: u32 = 7;
pub const KPID_MTIME: u32 = 12;
pub const KPID_ENCRYPTED: u32 = 15;

// NArchive::NHandlerPropID
pub const HPROP_NAME: u32 = 0;
pub const HPROP_CLASS_ID: u32 = 1;
pub const HPROP_EXTENSION: u32 = 2;

// NModulePropID
pub const MODPROP_INTERFACE_TYPE: u32 = 0;
pub const MODPROP_VERSION: u32 = 1;

const VT_EMPTY: u16 = 0;
const VT_BSTR: u16 = 8;
const VT_BOOL: u16 = 11;
const VT_UI4: u16 = 19;
const VT_UI8: u16 = 21;
const VT_FILETIME: u16 = 64;

/// `PROPVARIANT`: an 8-byte header and a union. On Windows x64 the union is 16 bytes
/// (24 in all); 7-Zip's own non-Windows definition has no 16-byte member (16 in all).
#[cfg(windows)]
const PV_WORDS: usize = 2;
#[cfg(not(windows))]
const PV_WORDS: usize = 1;

#[repr(C)]
pub struct PropVariant {
    vt: u16,
    r1: u16,
    r2: u16,
    r3: u16,
    val: [u64; PV_WORDS],
}

impl PropVariant {
    pub fn empty() -> Self {
        PropVariant {
            vt: VT_EMPTY,
            r1: 0,
            r2: 0,
            r3: 0,
            val: [0; PV_WORDS],
        }
    }
}

// ---- BSTR --------------------------------------------------------------------------
#[cfg(windows)]
mod bstr {
    pub type Char = u16;
    #[link(name = "oleaut32")]
    extern "system" {
        pub fn SysFreeString(b: *mut Char);
        pub fn SysStringByteLen(b: *const Char) -> u32;
    }
}
#[cfg(not(windows))]
mod bstr {
    // 7-Zip on Linux / macOS: BSTR = wchar_t* (4 bytes) with a 4-byte byte-length
    // prefix, allocated by 7-Zip's own SysAllocString with malloc.
    pub type Char = u32;
    extern "C" {
        fn free(p: *mut std::ffi::c_void);
    }
    /// # Safety
    /// `b` must be a non-null BSTR allocated by 7-Zip.
    pub unsafe fn SysStringByteLen(b: *const Char) -> u32 {
        // SAFETY: a 7-Zip BSTR is preceded by its u32 byte length (caller's contract).
        unsafe { *(b as *const u32).sub(1) }
    }
    /// # Safety
    /// `b` must be null or a BSTR allocated by 7-Zip that is not used afterwards.
    pub unsafe fn SysFreeString(b: *mut Char) {
        if !b.is_null() {
            // SAFETY: the allocation starts at the length prefix (caller's contract).
            unsafe { free((b as *mut u32).sub(1) as *mut std::ffi::c_void) }
        }
    }
}

/// A `PROPVARIANT` value copied out into Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Empty,
    Str(String),
    Bytes(Vec<u8>),
    Bool(bool),
    U32(u32),
    U64(u64),
    FileTime(u64),
    Other(u16),
}

impl Value {
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::U64(v) => Some(*v),
            Value::U32(v) => Some(u64::from(*v)),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
}

/// Copy a `PROPVARIANT` that 7-Zip filled into a [`Value`], free what it owned and
/// reset it to `VT_EMPTY`. With `raw_bstr`, a `BSTR` is returned as its bytes (7-Zip
/// passes 16-byte class IDs that way).
///
/// # Safety
/// `pv` must be empty or have been filled by 7-Zip; a `BSTR` in it must be valid and
/// owned by `pv`.
pub unsafe fn take(pv: &mut PropVariant, raw_bstr: bool) -> Value {
    let v = match pv.vt {
        VT_EMPTY => Value::Empty,
        VT_BSTR => {
            let p = pv.val[0] as *mut bstr::Char;
            let v = if p.is_null() {
                Value::Str(String::new())
            } else {
                // SAFETY: a non-null VT_BSTR value is a 7-Zip BSTR owned by `pv`
                // (caller's contract); its byte length is read from its prefix.
                let bytes = unsafe {
                    let n = bstr::SysStringByteLen(p) as usize;
                    std::slice::from_raw_parts(p as *const u8, n).to_vec()
                };
                if raw_bstr {
                    Value::Bytes(bytes)
                } else {
                    Value::Str(decode_wide(&bytes))
                }
            };
            // SAFETY: `pv` owned the BSTR and is reset to VT_EMPTY below, so it is
            // freed exactly once and never read again.
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

/// Decode a BSTR's bytes: UTF-16LE on Windows, UTF-32 (`wchar_t`) elsewhere.
fn decode_wide(bytes: &[u8]) -> String {
    #[cfg(windows)]
    {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    }
    #[cfg(not(windows))]
    {
        bytes
            .chunks_exact(4)
            .map(|c| {
                char::from_u32(u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .unwrap_or(char::REPLACEMENT_CHARACTER)
            })
            .collect()
    }
}

/// FILETIME (100 ns ticks since 1601-01-01 UTC) to `SystemTime`.
pub fn filetime_to_system_time(ft: u64) -> Option<SystemTime> {
    const UNIX_EPOCH_AS_FILETIME: u64 = 116_444_736_000_000_000;
    let since_unix =
        |ticks: u64| Duration::new(ticks / 10_000_000, ((ticks % 10_000_000) * 100) as u32);
    if ft >= UNIX_EPOCH_AS_FILETIME {
        SystemTime::UNIX_EPOCH.checked_add(since_unix(ft - UNIX_EPOCH_AS_FILETIME))
    } else {
        SystemTime::UNIX_EPOCH.checked_sub(since_unix(UNIX_EPOCH_AS_FILETIME - ft))
    }
}

// ---- exported functions ------------------------------------------------------------
pub type CreateObjectFn =
    unsafe extern "system" fn(*const Guid, *const Guid, *mut *mut c_void) -> HResult;
pub type GetNumberOfFormatsFn = unsafe extern "system" fn(*mut u32) -> HResult;
pub type GetHandlerProperty2Fn = unsafe extern "system" fn(u32, u32, *mut PropVariant) -> HResult;
pub type GetModulePropFn = unsafe extern "system" fn(u32, *mut PropVariant) -> HResult;

// ---- vtables -----------------------------------------------------------------------
#[repr(C)]
pub struct IUnknownVtbl {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HResult,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
}

/// `IInArchive` up to `Extract` (the methods after it are not used).
#[repr(C)]
pub struct IInArchiveVtbl {
    pub base: IUnknownVtbl,
    pub open:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const u64, *mut c_void) -> HResult,
    pub close: unsafe extern "system" fn(*mut c_void) -> HResult,
    pub get_number_of_items: unsafe extern "system" fn(*mut c_void, *mut u32) -> HResult,
    pub get_property: unsafe extern "system" fn(*mut c_void, u32, u32, *mut PropVariant) -> HResult,
    pub extract:
        unsafe extern "system" fn(*mut c_void, *const u32, u32, i32, *mut c_void) -> HResult,
}

/// `IInStream` (`ISequentialInStream::Read` + `Seek`).
#[repr(C)]
pub struct InStreamVtbl {
    pub base: IUnknownVtbl,
    pub read: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32) -> HResult,
    pub seek: unsafe extern "system" fn(*mut c_void, i64, u32, *mut u64) -> HResult,
}

/// `ISequentialOutStream`.
#[repr(C)]
pub struct OutStreamVtbl {
    pub base: IUnknownVtbl,
    pub write: unsafe extern "system" fn(*mut c_void, *const c_void, u32, *mut u32) -> HResult,
}

/// `IArchiveExtractCallback` (derives from `IProgress`).
#[repr(C)]
pub struct ExtractCallbackVtbl {
    pub base: IUnknownVtbl,
    pub set_total: unsafe extern "system" fn(*mut c_void, u64) -> HResult,
    pub set_completed: unsafe extern "system" fn(*mut c_void, *const u64) -> HResult,
    pub get_stream: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, i32) -> HResult,
    pub prepare_operation: unsafe extern "system" fn(*mut c_void, i32) -> HResult,
    pub set_operation_result: unsafe extern "system" fn(*mut c_void, i32) -> HResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_epochs() {
        assert_eq!(
            filetime_to_system_time(116_444_736_000_000_000),
            Some(SystemTime::UNIX_EPOCH)
        );
        // 2001-12-28T13:43:22Z, the RAR fixture's first entry seen from a UTC runner.
        let t = filetime_to_system_time(126_540_206_020_000_000).unwrap();
        let secs = t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(secs, 1_009_547_002);
        assert!(filetime_to_system_time(0).is_some());
    }

    #[test]
    fn propvariant_size_matches_the_platform_abi() {
        let expected = if cfg!(windows) { 24 } else { 16 };
        assert_eq!(std::mem::size_of::<PropVariant>(), expected);
    }

    #[test]
    fn take_resets_to_empty() {
        let mut pv = PropVariant::empty();
        pv.vt = VT_UI4;
        pv.val[0] = 7;
        // SAFETY: a VT_UI4 owns nothing.
        assert_eq!(unsafe { take(&mut pv, false) }, Value::U32(7));
        assert_eq!(pv.vt, VT_EMPTY);
    }
}
