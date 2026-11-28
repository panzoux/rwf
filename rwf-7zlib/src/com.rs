//! COM objects implemented in Rust that 7-Zip calls back into: the input stream
//! over the archive file, the output stream for each extracted item, and the
//! extract callback.
//!
//! Every object starts with its vtable pointer, keeps an atomic reference count and
//! frees itself when the count reaches zero. 7-Zip may call the extract callback and
//! the output streams from its own decoder threads (one call at a time), so all
//! shared state is behind `Mutex` / atomics and the closures handed in are `Send`.

use crate::ffi::*;
use std::ffi::c_void;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

macro_rules! com_refcount {
    ($ty:ty, $add:ident, $rel:ident) => {
        unsafe extern "system" fn $add(this: *mut c_void) -> u32 {
            // SAFETY: 7-Zip only calls AddRef on a live object it got from us.
            let o = unsafe { &*(this as *const $ty) };
            o.refs.fetch_add(1, Ordering::AcqRel) + 1
        }
        unsafe extern "system" fn $rel(this: *mut c_void) -> u32 {
            // SAFETY: as above; the object stays alive until the count drops to zero.
            let o = unsafe { &*(this as *const $ty) };
            let n = o.refs.fetch_sub(1, Ordering::AcqRel) - 1;
            if n == 0 {
                // SAFETY: the object was created by Box::into_raw and this was the
                // last reference, so nobody can reach it any more.
                drop(unsafe { Box::from_raw(this as *mut $ty) });
            }
            n
        }
    };
}

/// Shared `QueryInterface` body: hand out `this` for any of `ids`.
///
/// # Safety
/// `riid` and `out` must be valid pointers, `this` a live object whose AddRef is `add`.
unsafe fn query(
    this: *mut c_void,
    riid: *const Guid,
    out: *mut *mut c_void,
    ids: &[Guid],
    add: unsafe extern "system" fn(*mut c_void) -> u32,
) -> HResult {
    if out.is_null() || riid.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: both pointers were checked for null and come from 7-Zip (caller).
    unsafe {
        if ids.contains(&*riid) {
            add(this);
            *out = this;
            S_OK
        } else {
            *out = std::ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

// ---- IInStream over a file ---------------------------------------------------------
#[repr(C)]
pub struct InStream {
    vtbl: *const InStreamVtbl,
    refs: AtomicU32,
    file: Mutex<std::fs::File>,
}

com_refcount!(InStream, in_add_ref, in_release);

unsafe extern "system" fn in_qi(
    this: *mut c_void,
    riid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    let ids = [IID_IUNKNOWN, IID_ISEQUENTIAL_IN_STREAM, IID_IIN_STREAM];
    // SAFETY: forwarded from 7-Zip's QueryInterface call on our object.
    unsafe { query(this, riid, out, &ids, in_add_ref) }
}

unsafe extern "system" fn in_read(
    this: *mut c_void,
    data: *mut c_void,
    size: u32,
    processed: *mut u32,
) -> HResult {
    // SAFETY: `this` is a live InStream; 7-Zip passes a writable buffer of `size` bytes.
    let (o, buf) = unsafe {
        (
            &*(this as *const InStream),
            std::slice::from_raw_parts_mut(data as *mut u8, size as usize),
        )
    };
    let r = match o.file.lock() {
        Ok(mut f) => f.read(buf).ok(),
        Err(_) => None,
    };
    if !processed.is_null() {
        // SAFETY: checked for null; 7-Zip passes a valid out-pointer.
        unsafe { *processed = r.unwrap_or(0) as u32 };
    }
    if r.is_some() {
        S_OK
    } else {
        E_FAIL
    }
}

unsafe extern "system" fn in_seek(
    this: *mut c_void,
    offset: i64,
    origin: u32,
    new_pos: *mut u64,
) -> HResult {
    // SAFETY: `this` is a live InStream.
    let o = unsafe { &*(this as *const InStream) };
    let from = match origin {
        0 if offset >= 0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return E_INVALIDARG,
    };
    let r = match o.file.lock() {
        Ok(mut f) => f.seek(from).ok(),
        Err(_) => None,
    };
    match r {
        Some(p) => {
            if !new_pos.is_null() {
                // SAFETY: checked for null; 7-Zip passes a valid out-pointer.
                unsafe { *new_pos = p };
            }
            S_OK
        }
        None => E_FAIL,
    }
}

static IN_STREAM_VTBL: InStreamVtbl = InStreamVtbl {
    base: IUnknownVtbl {
        query_interface: in_qi,
        add_ref: in_add_ref,
        release: in_release,
    },
    read: in_read,
    seek: in_seek,
};

/// A new `IInStream` with one reference, owned by the caller.
pub fn new_in_stream(file: std::fs::File) -> *mut c_void {
    Box::into_raw(Box::new(InStream {
        vtbl: &IN_STREAM_VTBL,
        refs: AtomicU32::new(1),
        file: Mutex::new(file),
    })) as *mut c_void
}

/// Drop one reference to an object created by `new_in_stream` / `new_extract_callback`.
///
/// # Safety
/// `obj` must be a live COM object on which the caller holds a reference.
pub unsafe fn release(obj: *mut c_void) {
    // SAFETY: every COM object starts with its vtable pointer (caller's contract).
    unsafe {
        let vt = &**(obj as *const *const IUnknownVtbl);
        (vt.release)(obj);
    }
}

// ---- extraction --------------------------------------------------------------------
/// What the caller does with each item: give a writer, `None` to skip the data
/// (directories), or an error to stop.
pub type Target<'a> = dyn FnMut(u32) -> std::io::Result<Option<Box<dyn Write + Send>>> + Send + 'a;

/// State shared by the callback and the output streams. It outlives the `Extract`
/// call only through `Arc`s, never through the borrowed closures.
#[derive(Default)]
pub struct Shared {
    pub io_error: Mutex<Option<std::io::Error>>,
    pub cancelled: AtomicBool,
}

impl Shared {
    fn fail(&self, e: std::io::Error) {
        if let Ok(mut slot) = self.io_error.lock() {
            slot.get_or_insert(e);
        }
    }
}

/// The borrowed part, valid only during `IInArchive::Extract`.
pub struct ExtractCtx<'a> {
    pub target: Mutex<&'a mut Target<'a>>,
    pub is_cancelled: &'a (dyn Fn() -> bool + Sync),
    pub current: Mutex<Option<u32>>,
    pub results: Mutex<Vec<(u32, i32)>>,
}

#[repr(C)]
struct OutStream {
    vtbl: *const OutStreamVtbl,
    refs: AtomicU32,
    sink: Mutex<Box<dyn Write + Send>>,
    shared: Arc<Shared>,
}

impl Drop for OutStream {
    fn drop(&mut self) {
        if let Ok(mut s) = self.sink.lock() {
            if let Err(e) = s.flush() {
                self.shared.fail(e);
            }
        }
    }
}

com_refcount!(OutStream, out_add_ref, out_release);

unsafe extern "system" fn out_qi(
    this: *mut c_void,
    riid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    let ids = [IID_IUNKNOWN, IID_ISEQUENTIAL_OUT_STREAM];
    // SAFETY: forwarded from 7-Zip's QueryInterface call on our object.
    unsafe { query(this, riid, out, &ids, out_add_ref) }
}

unsafe extern "system" fn out_write(
    this: *mut c_void,
    data: *const c_void,
    size: u32,
    processed: *mut u32,
) -> HResult {
    // SAFETY: `this` is a live OutStream; 7-Zip passes `size` readable bytes.
    let (o, buf) = unsafe {
        (
            &*(this as *const OutStream),
            std::slice::from_raw_parts(data as *const u8, size as usize),
        )
    };
    if !processed.is_null() {
        // SAFETY: checked for null; 7-Zip passes a valid out-pointer.
        unsafe { *processed = 0 };
    }
    if o.shared.cancelled.load(Ordering::Relaxed) {
        return E_ABORT;
    }
    let r = match o.sink.lock() {
        Ok(mut s) => s.write_all(buf),
        Err(_) => Err(std::io::Error::other("output stream lock poisoned")),
    };
    match r {
        Ok(()) => {
            if !processed.is_null() {
                // SAFETY: as above.
                unsafe { *processed = size };
            }
            S_OK
        }
        Err(e) => {
            o.shared.fail(e);
            E_FAIL
        }
    }
}

static OUT_STREAM_VTBL: OutStreamVtbl = OutStreamVtbl {
    base: IUnknownVtbl {
        query_interface: out_qi,
        add_ref: out_add_ref,
        release: out_release,
    },
    write: out_write,
};

#[repr(C)]
pub struct ExtractCallback {
    vtbl: *const ExtractCallbackVtbl,
    refs: AtomicU32,
    /// `*const ExtractCtx`; null once `Extract` has returned.
    ctx: AtomicPtr<c_void>,
    shared: Arc<Shared>,
}

com_refcount!(ExtractCallback, cb_add_ref, cb_release);

impl ExtractCallback {
    /// The live context, or `None` after `detach`.
    ///
    /// # Safety
    /// The pointer stored by `new_extract_callback` must still be valid unless null.
    unsafe fn ctx<'a>(&self) -> Option<&'a ExtractCtx<'a>> {
        let p = self.ctx.load(Ordering::Acquire);
        // SAFETY: non-null means the ExtractCtx on `Archive::extract`'s stack is alive.
        (!p.is_null()).then(|| unsafe { &*(p as *const ExtractCtx<'a>) })
    }

    fn check_cancel(&self, ctx: &ExtractCtx<'_>) -> bool {
        let c = self.shared.cancelled.load(Ordering::Relaxed) || (ctx.is_cancelled)();
        if c {
            self.shared.cancelled.store(true, Ordering::Relaxed);
        }
        c
    }
}

unsafe extern "system" fn cb_qi(
    this: *mut c_void,
    riid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    let ids = [IID_IUNKNOWN, IID_IPROGRESS, IID_IARCHIVE_EXTRACT_CALLBACK];
    // SAFETY: forwarded from 7-Zip's QueryInterface call on our object.
    unsafe { query(this, riid, out, &ids, cb_add_ref) }
}

unsafe extern "system" fn cb_set_total(_: *mut c_void, _: u64) -> HResult {
    S_OK
}

unsafe extern "system" fn cb_set_completed(this: *mut c_void, _: *const u64) -> HResult {
    // SAFETY: `this` is a live ExtractCallback; ctx is valid while non-null.
    let o = unsafe { &*(this as *const ExtractCallback) };
    // SAFETY: see ExtractCallback::ctx.
    match unsafe { o.ctx() } {
        Some(ctx) if !o.check_cancel(ctx) => S_OK,
        _ => E_ABORT,
    }
}

unsafe extern "system" fn cb_get_stream(
    this: *mut c_void,
    index: u32,
    out: *mut *mut c_void,
    ask_mode: i32,
) -> HResult {
    if out.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: `out` checked for null; `this` is a live ExtractCallback.
    let o = unsafe {
        *out = std::ptr::null_mut();
        &*(this as *const ExtractCallback)
    };
    // SAFETY: see ExtractCallback::ctx.
    let Some(ctx) = (unsafe { o.ctx() }) else {
        return E_ABORT;
    };
    if o.check_cancel(ctx) {
        return E_ABORT;
    }
    if let Ok(mut c) = ctx.current.lock() {
        *c = Some(index);
    }
    if ask_mode != 0 {
        return S_OK; // test / skip: no stream wanted
    }
    let made = match ctx.target.lock() {
        Ok(mut target) => (*target)(index),
        Err(_) => Err(std::io::Error::other("extract target lock poisoned")),
    };
    match made {
        Ok(Some(sink)) => {
            let stream = Box::new(OutStream {
                vtbl: &OUT_STREAM_VTBL,
                refs: AtomicU32::new(1),
                sink: Mutex::new(sink),
                shared: Arc::clone(&o.shared),
            });
            // SAFETY: `out` is valid (checked); 7-Zip takes over the one reference.
            unsafe { *out = Box::into_raw(stream) as *mut c_void };
            S_OK
        }
        Ok(None) => S_OK,
        Err(e) => {
            o.shared.fail(e);
            E_FAIL
        }
    }
}

unsafe extern "system" fn cb_prepare(_: *mut c_void, _: i32) -> HResult {
    S_OK
}

unsafe extern "system" fn cb_set_result(this: *mut c_void, op_res: i32) -> HResult {
    // SAFETY: `this` is a live ExtractCallback.
    let o = unsafe { &*(this as *const ExtractCallback) };
    // SAFETY: see ExtractCallback::ctx.
    if let Some(ctx) = unsafe { o.ctx() } {
        let current = ctx.current.lock().ok().and_then(|mut c| c.take());
        if let (Some(index), Ok(mut r)) = (current, ctx.results.lock()) {
            r.push((index, op_res));
        }
    }
    S_OK
}

static EXTRACT_CALLBACK_VTBL: ExtractCallbackVtbl = ExtractCallbackVtbl {
    base: IUnknownVtbl {
        query_interface: cb_qi,
        add_ref: cb_add_ref,
        release: cb_release,
    },
    set_total: cb_set_total,
    set_completed: cb_set_completed,
    get_stream: cb_get_stream,
    prepare_operation: cb_prepare,
    set_operation_result: cb_set_result,
};

/// A new extract callback with one reference, owned by the caller, pointing at `ctx`.
/// The caller must call [`detach`] before `ctx` goes away.
pub fn new_extract_callback(ctx: &ExtractCtx<'_>, shared: Arc<Shared>) -> *mut c_void {
    Box::into_raw(Box::new(ExtractCallback {
        vtbl: &EXTRACT_CALLBACK_VTBL,
        refs: AtomicU32::new(1),
        ctx: AtomicPtr::new(ctx as *const ExtractCtx<'_> as *mut c_void),
        shared,
    })) as *mut c_void
}

/// Cut the callback loose from its borrowed context, so a reference 7-Zip might
/// still hold can no longer reach it.
///
/// # Safety
/// `cb` must be a live object created by [`new_extract_callback`].
pub unsafe fn detach(cb: *mut c_void) {
    // SAFETY: caller's contract.
    let o = unsafe { &*(cb as *const ExtractCallback) };
    o.ctx.store(std::ptr::null_mut(), Ordering::Release);
}
