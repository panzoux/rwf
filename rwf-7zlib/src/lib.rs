//! Run-time binding to 7-Zip's shared library — `7z.dll` on Windows, `7z.so` on
//! Linux / macOS — for listing and extracting archives in-process.
//!
//! The library is loaded with `libloading` when it is first needed, so rwf builds and
//! runs without 7-Zip. Only 7-Zip 23 and newer are accepted (see [`ffi`] for why).
//!
//! All `unsafe` code of the 7-Zip integration lives in this crate; every block carries
//! a `// SAFETY:` comment.
#![allow(unsafe_code)] // FFI to 7-Zip; see the crate docs.

mod com;
mod ffi;

use ffi::*;
use std::ffi::c_void;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// Errors from loading 7-Zip or operating on an archive through it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot load {}: {reason}", path.display())]
    Load { path: PathBuf, reason: String },
    #[error("{} is not a usable 7-Zip library: {reason}", path.display())]
    Unsupported { path: PathBuf, reason: String },
    #[error("7-Zip cannot open {} as an archive", .0.display())]
    CannotOpen(PathBuf),
    #[error("7-Zip call {call} failed (HRESULT {hresult:#010x})")]
    Call { call: &'static str, hresult: u32 },
    #[error("operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `NArchive::NExtract::NOperationResult`: how extracting one item ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpResult {
    Ok,
    /// The compression method is not in this 7-Zip build (e.g. RAR in Homebrew's
    /// and Debian's 7z.so).
    UnsupportedMethod,
    DataError,
    CrcError,
    Unavailable,
    UnexpectedEnd,
    DataAfterEnd,
    IsNotArc,
    HeadersError,
    WrongPassword,
    Other(i32),
}

impl From<i32> for OpResult {
    fn from(v: i32) -> Self {
        match v {
            0 => OpResult::Ok,
            1 => OpResult::UnsupportedMethod,
            2 => OpResult::DataError,
            3 => OpResult::CrcError,
            4 => OpResult::Unavailable,
            5 => OpResult::UnexpectedEnd,
            6 => OpResult::DataAfterEnd,
            7 => OpResult::IsNotArc,
            8 => OpResult::HeadersError,
            9 => OpResult::WrongPassword,
            other => OpResult::Other(other),
        }
    }
}

impl std::fmt::Display for OpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            OpResult::Ok => "ok",
            OpResult::UnsupportedMethod => "unsupported compression method",
            OpResult::DataError => "data error",
            OpResult::CrcError => "CRC error",
            OpResult::Unavailable => "data unavailable",
            OpResult::UnexpectedEnd => "unexpected end of data",
            OpResult::DataAfterEnd => "data after the end of the payload",
            OpResult::IsNotArc => "not an archive",
            OpResult::HeadersError => "header error",
            OpResult::WrongPassword => "password-protected (passwords are not supported)",
            OpResult::Other(v) => return write!(f, "error {v}"),
        };
        f.write_str(s)
    }
}

/// One archive format the loaded library can open.
#[derive(Debug, Clone)]
pub struct Format {
    pub name: String,
    /// Lower-case extensions without the dot, as 7-Zip reports them (`"rar r00"`).
    pub extensions: Vec<String>,
    class_id: Guid,
}

/// A loaded 7-Zip library.
pub struct Library {
    path: PathBuf,
    version: (u16, u16),
    create_object: CreateObjectFn,
    formats: Vec<Format>,
    // Declared last so it is dropped after everything that points into it.
    _lib: libloading::Library,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library")
            .field("path", &self.path)
            .field("version", &self.version)
            .field("formats", &self.formats.len())
            .finish()
    }
}

/// Where 7-Zip's library usually is on this platform, in search order.
pub fn default_locations() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(windows)]
    {
        // A 64-bit rwf can only load the 64-bit DLL and vice versa; a mismatch just
        // fails to load and the next candidate is tried.
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(dir) = std::env::var_os(var) {
                let p = PathBuf::from(dir).join("7-Zip").join("7z.dll");
                if !v.contains(&p) {
                    v.push(p);
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        for p in [
            "/usr/lib/7zip/7z.so",                  // Debian / Ubuntu `7zip`
            "/usr/lib64/7zip/7z.so",                // Fedora-style lib64
            "/usr/libexec/7zip/7z.so",              //
            "/opt/homebrew/lib/7z.so",              // Homebrew, Apple silicon
            "/usr/local/lib/7z.so",                 // Homebrew, Intel macOS
            "/home/linuxbrew/.linuxbrew/lib/7z.so", // Homebrew on Linux
        ] {
            v.push(PathBuf::from(p));
        }
    }
    v
}

impl Library {
    /// Load the first usable library: `configured` if given (an error if that fails),
    /// otherwise the first of [`default_locations`] that exists and loads.
    pub fn find(configured: Option<&Path>) -> Result<Self, Error> {
        if let Some(p) = configured {
            return Self::load(p);
        }
        let mut last = None;
        for p in default_locations() {
            if !p.is_file() {
                continue;
            }
            match Self::load(&p) {
                Ok(lib) => return Ok(lib),
                Err(e) => {
                    tracing::info!("7-Zip candidate skipped: {e}");
                    last = Some(e);
                }
            }
        }
        Err(last.unwrap_or_else(|| Error::Load {
            path: PathBuf::from("7z"),
            reason: "7-Zip's library was not found in the usual places".into(),
        }))
    }

    /// Load the library at `path` and read its list of formats.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let load_err = |reason: String| Error::Load {
            path: path.to_path_buf(),
            reason,
        };
        let unsupported = |reason: &str| Error::Unsupported {
            path: path.to_path_buf(),
            reason: reason.into(),
        };
        // SAFETY: loading runs the library's initialisers. 7-Zip's have no
        // preconditions; the path is the user's 7-Zip installation or configuration.
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| load_err(e.to_string()))?;

        // SAFETY: the symbol types match 7-Zip's exported C functions (ffi.rs); the
        // function pointers are only used while `lib` (kept in Self) is loaded.
        let (create_object, num_formats, handler_prop, module_prop) = unsafe {
            let get = |name: &[u8]| lib.get::<*const c_void>(name).is_ok();
            if !get(b"CreateObject\0") || !get(b"GetNumberOfFormats\0") {
                return Err(unsupported(
                    "CreateObject / GetNumberOfFormats are not exported",
                ));
            }
            (
                *lib.get::<CreateObjectFn>(b"CreateObject\0")
                    .map_err(|e| load_err(e.to_string()))?,
                *lib.get::<GetNumberOfFormatsFn>(b"GetNumberOfFormats\0")
                    .map_err(|e| load_err(e.to_string()))?,
                *lib.get::<GetHandlerProperty2Fn>(b"GetHandlerProperty2\0")
                    .map_err(|_| unsupported("GetHandlerProperty2 is not exported"))?,
                lib.get::<GetModulePropFn>(b"GetModuleProp\0")
                    .ok()
                    .map(|s| *s),
            )
        };

        // GetModuleProp exists from 7-Zip 23 on; older builds and p7zip have the
        // virtual-destructor vtable layout this crate does not speak.
        let module_prop = module_prop
            .ok_or_else(|| unsupported("7-Zip 23 or newer is required (no GetModuleProp)"))?;
        let mut pv = PropVariant::empty();
        // SAFETY: GetModuleProp fills `pv`, which `take` then owns and frees.
        let (itype, ver) = unsafe {
            module_prop(MODPROP_INTERFACE_TYPE, &mut pv);
            let itype = take(&mut pv, false);
            module_prop(MODPROP_VERSION, &mut pv);
            (itype, take(&mut pv, false))
        };
        if !matches!(itype, Value::U32(0) | Value::Empty) {
            return Err(unsupported(
                "its interfaces use virtual destructors (unsupported ABI)",
            ));
        }
        let version = match ver {
            Value::U32(v) => ((v >> 16) as u16, (v & 0xffff) as u16),
            _ => (0, 0),
        };

        let mut n = 0u32;
        // SAFETY: GetNumberOfFormats writes one u32.
        if unsafe { num_formats(&mut n) } != S_OK {
            return Err(unsupported("GetNumberOfFormats failed"));
        }
        let mut formats = Vec::with_capacity(n as usize);
        for i in 0..n {
            // SAFETY: GetHandlerProperty2 fills `pv` for format `i < n`; `take`
            // owns and frees each value.
            let (name, class_id, exts) = unsafe {
                handler_prop(i, HPROP_NAME, &mut pv);
                let name = take(&mut pv, false);
                handler_prop(i, HPROP_CLASS_ID, &mut pv);
                let class_id = take(&mut pv, true);
                handler_prop(i, HPROP_EXTENSION, &mut pv);
                (name, class_id, take(&mut pv, false))
            };
            let Value::Bytes(id) = class_id else { continue };
            if id.len() != 16 {
                continue;
            }
            // SAFETY: 16 bytes, read unaligned into a plain-data GUID.
            let class_id = unsafe { std::ptr::read_unaligned(id.as_ptr() as *const Guid) };
            formats.push(Format {
                name: match name {
                    Value::Str(s) => s,
                    _ => format!("#{i}"),
                },
                extensions: match exts {
                    Value::Str(s) => s.split_whitespace().map(|e| e.to_lowercase()).collect(),
                    _ => Vec::new(),
                },
                class_id,
            });
        }
        tracing::info!(
            "loaded 7-Zip {}.{:02} from {} ({} formats)",
            version.0,
            version.1,
            path.display(),
            formats.len()
        );
        Ok(Library {
            path: path.to_path_buf(),
            version,
            create_object,
            formats,
            _lib: lib,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `(major, minor)`, e.g. `(26, 6)` for 7-Zip 26.06.
    pub fn version(&self) -> (u16, u16) {
        self.version
    }

    pub fn formats(&self) -> &[Format] {
        &self.formats
    }

    /// Formats that claim the file name's last extension, in 7-Zip's order.
    pub fn formats_for(&self, file_name: &str) -> Vec<&Format> {
        let ext = extension_of(file_name);
        self.formats
            .iter()
            .filter(|f| f.extensions.contains(&ext))
            .collect()
    }

    /// Open `path` with the first format that claims its extension and accepts it.
    pub fn open(&self, path: &Path) -> Result<Archive<'_>, Error> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for f in self.formats_for(&name) {
            if let Some(a) = self.try_open(f, path)? {
                return Ok(a);
            }
        }
        Err(Error::CannotOpen(path.to_path_buf()))
    }

    fn try_open(&self, format: &Format, path: &Path) -> Result<Option<Archive<'_>>, Error> {
        let mut obj: *mut c_void = std::ptr::null_mut();
        // SAFETY: CreateObject writes an IInArchive pointer (with one reference) or
        // leaves it null; both GUIDs are valid for the call.
        let hr = unsafe { (self.create_object)(&format.class_id, &IID_IIN_ARCHIVE, &mut obj) };
        if hr != S_OK || obj.is_null() {
            return Ok(None);
        }
        let archive = Archive {
            _lib: self,
            obj,
            format: format.name.clone(),
            items: Vec::new(),
        };
        let stream = com::new_in_stream(std::fs::File::open(path)?);
        let max_check: u64 = 1 << 23;
        // SAFETY: `obj` is a live IInArchive; `stream` is our IInStream (7-Zip
        // AddRefs what it keeps). A null open callback is allowed.
        let hr = unsafe {
            let hr = (archive.vtbl().open)(obj, stream, &max_check, std::ptr::null_mut());
            com::release(stream);
            hr
        };
        if hr != S_OK {
            return Ok(None); // S_FALSE: not this format. `archive` drop releases obj.
        }
        let mut archive = archive;
        archive.read_items(path)?;
        Ok(Some(archive))
    }
}

/// Lower-case last extension of a file name, without the dot.
fn extension_of(file_name: &str) -> String {
    match file_name.rfind('.') {
        Some(i) if i + 1 < file_name.len() => file_name[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// One entry of an opened archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 7-Zip's item index, used to extract it.
    pub index: u32,
    /// Path inside the archive with `/` separators and no leading `/`.
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub encrypted: bool,
}

/// An archive opened by 7-Zip. Not `Send`: use it on the thread that opened it.
pub struct Archive<'lib> {
    _lib: &'lib Library,
    obj: *mut c_void,
    format: String,
    items: Vec<Item>,
}

impl Drop for Archive<'_> {
    fn drop(&mut self) {
        // SAFETY: `obj` is a live IInArchive we hold one reference to; Close is
        // harmless on an archive that failed to open.
        unsafe {
            (self.vtbl().close)(self.obj);
            (self.vtbl().base.release)(self.obj);
        }
    }
}

impl<'lib> Archive<'lib> {
    fn vtbl(&self) -> &IInArchiveVtbl {
        // SAFETY: `obj` is a live COM object whose first field is its vtable pointer.
        unsafe { &**(self.obj as *const *const IInArchiveVtbl) }
    }

    fn prop(&self, index: u32, pid: u32) -> Value {
        let mut pv = PropVariant::empty();
        // SAFETY: GetProperty fills `pv` (or leaves it empty); `take` owns it.
        unsafe {
            (self.vtbl().get_property)(self.obj, index, pid, &mut pv);
            take(&mut pv, false)
        }
    }

    fn read_items(&mut self, path: &Path) -> Result<(), Error> {
        let mut n = 0u32;
        // SAFETY: GetNumberOfItems writes one u32.
        let hr = unsafe { (self.vtbl().get_number_of_items)(self.obj, &mut n) };
        if hr != S_OK {
            return Err(Error::Call {
                call: "GetNumberOfItems",
                hresult: hr as u32,
            });
        }
        let fallback_name = default_item_name(path);
        let mut items = Vec::with_capacity(n as usize);
        for i in 0..n {
            let raw = match self.prop(i, KPID_PATH) {
                Value::Str(s) => s,
                _ => String::new(),
            };
            let mut p = normalize_item_path(&raw);
            if p.is_empty() {
                // Single-stream formats (gz, xz, bz2 …) have no stored name.
                p = fallback_name.clone();
            }
            items.push(Item {
                index: i,
                path: p,
                is_dir: self.prop(i, KPID_IS_DIR).as_bool(),
                size: self.prop(i, KPID_SIZE).as_u64().unwrap_or(0),
                modified: match self.prop(i, KPID_MTIME) {
                    Value::FileTime(ft) => filetime_to_system_time(ft),
                    _ => None,
                },
                encrypted: self.prop(i, KPID_ENCRYPTED).as_bool(),
            });
        }
        self.items = items;
        Ok(())
    }

    /// 7-Zip's name of the format that opened the archive (`"Rar"`, `"zip"` …).
    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Extract the items with the given indices (all items if `indices` is empty).
    ///
    /// For every item 7-Zip reaches, `target(index)` returns where its data goes:
    /// a writer, `None` to drop the data (directories), or an I/O error to stop.
    /// `is_cancelled` is polled during the run; returning `true` aborts with
    /// [`Error::Cancelled`]. On success, returns each item's [`OpResult`].
    pub fn extract(
        &self,
        indices: &[u32],
        target: &mut com::Target<'_>,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<(u32, OpResult)>, Error> {
        let mut sorted: Vec<u32> = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let shared = Arc::new(com::Shared::default());
        let ctx = com::ExtractCtx {
            target: std::sync::Mutex::new(target),
            is_cancelled,
            current: std::sync::Mutex::new(None),
            results: std::sync::Mutex::new(Vec::new()),
        };
        let cb = com::new_extract_callback(&ctx, Arc::clone(&shared));
        let (ptr, len) = if sorted.is_empty() {
            (std::ptr::null(), u32::MAX) // 7-Zip: all items
        } else {
            (sorted.as_ptr(), sorted.len() as u32)
        };
        // SAFETY: `obj` is a live IInArchive; `ptr`/`len` describe a sorted index
        // array (or null + u32::MAX for all); `cb` is our callback, detached from
        // `ctx` before `ctx` is dropped, and released after.
        let hr = unsafe {
            let hr = (self.vtbl().extract)(self.obj, ptr, len, 0, cb);
            com::detach(cb);
            com::release(cb);
            hr
        };
        if let Some(e) = shared.io_error.lock().ok().and_then(|mut e| e.take()) {
            return Err(Error::Io(e));
        }
        if shared.cancelled.load(std::sync::atomic::Ordering::Relaxed) || hr == E_ABORT {
            return Err(Error::Cancelled);
        }
        if hr != S_OK {
            return Err(Error::Call {
                call: "Extract",
                hresult: hr as u32,
            });
        }
        let results = ctx.results.into_inner().unwrap_or_default();
        Ok(results
            .into_iter()
            .map(|(i, r)| (i, OpResult::from(r)))
            .collect())
    }
}

pub use com::Target;

/// Convenience: a boxed file writer for [`Archive::extract`] targets.
pub fn file_sink(file: std::fs::File) -> Box<dyn Write + Send> {
    Box::new(std::io::BufWriter::new(file))
}

/// 7-Zip item path → `/`-separated, no leading `/`, no empty or `.` components.
pub fn normalize_item_path(raw: &str) -> String {
    #[cfg(windows)]
    let raw = raw.replace('\\', "/");
    raw.split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Name for the single item of a stream format that stores none: the archive's
/// name without its last extension (`a.tar.gz` → `a.tar`, `a.tgz` → `a.tar`).
pub fn default_item_name(archive: &Path) -> String {
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = extension_of(&name);
    let stem = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name.as_str(),
    };
    match ext.as_str() {
        "tgz" | "tbz" | "tbz2" | "txz" | "tzst" => format!("{stem}.tar"),
        _ => stem.to_string(),
    }
}

/// Map an item path to a path under `dest`, rejecting anything that would escape it
/// (`..`, absolute paths, drive prefixes). `None` means "do not write this item".
pub fn safe_join(dest: &Path, item_path: &str) -> Option<PathBuf> {
    let mut out = dest.to_path_buf();
    let mut any = false;
    for c in item_path.split(['/', '\\']) {
        if c.is_empty() || c == "." {
            continue;
        }
        if c == ".." || c.contains(':') {
            return None;
        }
        out.push(c);
        any = true;
    }
    any.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_last_and_lowercase() {
        assert_eq!(extension_of("A.TAR.GZ"), "gz");
        assert_eq!(extension_of("x.Rar"), "rar");
        assert_eq!(extension_of("noext"), "");
        assert_eq!(extension_of("trailing."), "");
    }

    #[test]
    fn item_paths_are_normalized() {
        assert_eq!(normalize_item_path("/a//b/./c"), "a/b/c");
        assert_eq!(normalize_item_path(""), "");
        #[cfg(windows)]
        assert_eq!(normalize_item_path(r"dir\sub\f.txt"), "dir/sub/f.txt");
    }

    #[test]
    fn stream_formats_get_a_name_from_the_archive() {
        assert_eq!(default_item_name(Path::new("/x/a.tar.gz")), "a.tar");
        assert_eq!(default_item_name(Path::new("a.tgz")), "a.tar");
        assert_eq!(default_item_name(Path::new("log.xz")), "log");
        assert_eq!(default_item_name(Path::new(".hidden")), ".hidden");
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let d = Path::new("dest");
        assert_eq!(safe_join(d, "a/b.txt"), Some(d.join("a").join("b.txt")));
        assert_eq!(safe_join(d, "../evil"), None);
        assert_eq!(safe_join(d, "a/../../evil"), None);
        assert_eq!(safe_join(d, "C:/Windows/x"), None);
        assert_eq!(safe_join(d, "/abs/x"), Some(d.join("abs").join("x")));
        assert_eq!(safe_join(d, ""), None);
    }

    #[test]
    fn op_results_map_from_7zip_codes() {
        assert_eq!(OpResult::from(0), OpResult::Ok);
        assert_eq!(OpResult::from(1), OpResult::UnsupportedMethod);
        assert_eq!(OpResult::from(9), OpResult::WrongPassword);
        assert_eq!(OpResult::from(42), OpResult::Other(42));
    }
}
