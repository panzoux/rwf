//! Win32 implementation. Port of `ctxmenu.cpp` from `panzoux/win_ctxmenu`.

// Shell COM / Win32 FFI needs raw pointers; this crate is the second place in the
// workspace allowed to use unsafe (see also rwf-lib/src/volume_info.rs). Every
// unsafe block carries a SAFETY comment.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::collections::HashSet;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::CellAnchor;
use windows::core::{w, ComInterface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent, SHParseDisplayName,
    CMF_NORMAL, CMIC_MASK_PTINVOKE, CMINVOKECOMMANDINFOEX,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, EnumWindows, GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetWindowThreadProcessId, IsWindowVisible, KillTimer, LoadCursorW, PostMessageW,
    PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer, TrackPopupMenuEx,
    TranslateMessage, HMENU, IDC_ARROW, MSG, SW_SHOWNORMAL, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_DRAWITEM, WM_INITMENUPOPUP, WM_MEASUREITEM, WM_NULL, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW,
    WS_POPUP,
};

thread_local! {
    // The window proc runs on the thread that created the window (the STA thread),
    // so thread-locals stand in for the C++ globals.
    static PCM2: RefCell<Option<IContextMenu2>> = const { RefCell::new(None) };
    static PCM3: RefCell<Option<IContextMenu3>> = const { RefCell::new(None) };
}

/// `CMIC_MASK_UNICODE` (= `SEE_MASK_UNICODE`); not exported by windows 0.52.
const CMIC_MASK_UNICODE: u32 = 0x0000_4000;
const ID_CMD_FIRST: u32 = 1;
const ID_CMD_LAST: u32 = 0x7FFF;
/// How long to wait for a window (Properties, a rename box...) opened by the command.
const NEW_WINDOW_GRACE_MS: u32 = 2000;

pub(crate) fn show(paths: &[PathBuf], anchor: Option<CellAnchor>) -> Result<(), String> {
    // SAFETY: plain COM init on this dedicated thread; paired with CoUninitialize below.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
        .map_err(|e| format!("CoInitializeEx failed: {e}"))?;
    let result = show_inner(paths, anchor);
    // SAFETY: matches the successful CoInitializeEx above.
    unsafe { CoUninitialize() };
    result
}

/// Absolute PIDLs freed with `CoTaskMemFree` on drop.
struct PidlList(Vec<*mut ITEMIDLIST>);

impl Drop for PidlList {
    fn drop(&mut self) {
        for pidl in &self.0 {
            // SAFETY: each pointer came from SHParseDisplayName and is freed exactly once.
            unsafe { CoTaskMemFree(Some(*pidl as *const _)) };
        }
    }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn show_inner(paths: &[PathBuf], anchor: Option<CellAnchor>) -> Result<(), String> {
    // Parse every path to a PIDL and bind the (shared) parent folder.
    let mut pidls = PidlList(Vec::new());
    let mut children: Vec<*const ITEMIDLIST> = Vec::new();
    let mut parent: Option<IShellFolder> = None;
    for path in paths {
        let w = wide(path);
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        // SAFETY: `w` is NUL-terminated and outlives the call; `pidl` is a valid out-pointer.
        unsafe { SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut pidl, 0, None) }
            .map_err(|e| format!("SHParseDisplayName failed for {}: {e}", path.display()))?;
        pidls.0.push(pidl);
        let mut child: *mut ITEMIDLIST = std::ptr::null_mut();
        // SAFETY: `pidl` is a valid absolute PIDL; on success `child` points inside it,
        // so it stays valid until `pidls` is dropped.
        let folder: IShellFolder = unsafe { SHBindToParent(pidl, Some(&mut child)) }
            .map_err(|e| format!("SHBindToParent failed for {}: {e}", path.display()))?;
        children.push(child);
        if parent.is_none() {
            parent = Some(folder);
        }
    }
    let parent = parent.ok_or_else(|| "no files".to_string())?;

    let hwnd = create_host_window()?;
    let result = run_menu(hwnd, &parent, &children, anchor);
    // SAFETY: `hwnd` was created on this thread by create_host_window.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    PCM3.with(|c| *c.borrow_mut() = None);
    PCM2.with(|c| *c.borrow_mut() = None);
    result
}

unsafe extern "system" fn host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Forward owner-draw / submenu messages so shell-extension items render.
    let handled = PCM3.with(|c| {
        c.borrow().as_ref().and_then(|pcm3| {
            let mut result = LRESULT(0);
            // SAFETY: forwarding the window message the system gave us.
            unsafe { pcm3.HandleMenuMsg2(msg, wparam, lparam, Some(&mut result)) }
                .ok()
                .map(|_| result)
        })
    });
    if let Some(result) = handled {
        return result;
    }
    if matches!(msg, WM_INITMENUPOPUP | WM_DRAWITEM | WM_MEASUREITEM) {
        let handled2 = PCM2.with(|c| {
            c.borrow().as_ref().map(|pcm2| {
                // SAFETY: forwarding the window message the system gave us.
                let _ = unsafe { pcm2.HandleMenuMsg(msg, wparam, lparam) };
            })
        });
        if handled2.is_some() {
            return LRESULT(0);
        }
    }
    // SAFETY: default handling for a message we did not consume.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn create_host_window() -> Result<HWND, String> {
    // SAFETY: standard window-class registration and creation on the current thread.
    unsafe {
        let hinst = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW failed: {e}"))?;
        let class_name = w!("RwfCtxMenuHostWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(host_wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        // Returns 0 if the class is already registered (second invocation); that is fine.
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            hinst,
            None,
        );
        if hwnd.0 == 0 {
            return Err("CreateWindowExW failed".to_string());
        }
        Ok(hwnd)
    }
}

/// The terminal window the cell grid lives in: the console window of a classic console,
/// else the foreground window (Windows Terminal hosts a hidden pseudo-console window).
fn terminal_window() -> Option<HWND> {
    // SAFETY: plain queries with no pointer arguments.
    unsafe {
        let console = GetConsoleWindow();
        if console.0 != 0 && IsWindowVisible(console).as_bool() {
            return Some(console);
        }
        let fg = GetForegroundWindow();
        (fg.0 != 0).then_some(fg)
    }
}

/// Screen position of the bottom-left of `anchor`'s cell, by scaling the terminal
/// window's client rectangle over the cell grid.
fn anchor_to_screen(anchor: CellAnchor) -> Option<POINT> {
    if anchor.cols == 0 || anchor.rows == 0 {
        return None;
    }
    let hwnd = terminal_window()?;
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: `rect` and `origin` are valid out-pointers; `hwnd` is a live window handle.
    unsafe {
        GetClientRect(hwnd, &mut rect).ok()?;
        if !ClientToScreen(hwnd, &mut origin).as_bool() {
            return None;
        }
    }
    let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
    if w <= 0 || h <= 0 {
        return None;
    }
    // Bottom edge of the row, so the menu opens just under the file name.
    Some(POINT {
        x: origin.x + w * i32::from(anchor.col) / i32::from(anchor.cols),
        y: origin.y + h * (i32::from(anchor.row) + 1) / i32::from(anchor.rows),
    })
}

/// Make the popup menu keyboard-ready immediately (Up/Down/Enter/Esc), even though
/// the foreground window belongs to the terminal's thread.
fn prepare_for_keyboard(owner: HWND) {
    // SAFETY: Win32 focus calls on windows we own / the current foreground window.
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = if fg.0 != 0 {
            GetWindowThreadProcessId(fg, None)
        } else {
            0
        };
        let my_thread = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != my_thread;
        if attached {
            let _ = AttachThreadInput(my_thread, fg_thread, BOOL(1));
        }
        let _ = SetActiveWindow(owner);
        let _ = SetForegroundWindow(owner);
        let _ = BringWindowToTop(owner);
        let _ = SetFocus(owner);
        if attached {
            let _ = AttachThreadInput(my_thread, fg_thread, BOOL(0));
        }
    }
}

fn run_menu(
    hwnd: HWND,
    parent: &IShellFolder,
    children: &[*const ITEMIDLIST],
    anchor: Option<CellAnchor>,
) -> Result<(), String> {
    // SAFETY: `children` are valid child PIDLs kept alive by the caller's PidlList;
    // `hwnd` is our live host window.
    let pcm: IContextMenu = unsafe { parent.GetUIObjectOf(hwnd, children, None) }
        .map_err(|e| format!("GetUIObjectOf(IContextMenu) failed: {e}"))?;
    PCM3.with(|c| *c.borrow_mut() = pcm.cast::<IContextMenu3>().ok());
    if PCM3.with(|c| c.borrow().is_none()) {
        PCM2.with(|c| *c.borrow_mut() = pcm.cast::<IContextMenu2>().ok());
    }

    // SAFETY: creates a menu we destroy below on every path.
    let hmenu = unsafe { CreatePopupMenu() }.map_err(|e| format!("CreatePopupMenu failed: {e}"))?;
    let result = track_and_invoke(hwnd, &pcm, hmenu, anchor);
    // SAFETY: `hmenu` was created above and is not used afterwards.
    unsafe {
        let _ = DestroyMenu(hmenu);
    }
    result
}

fn track_and_invoke(
    hwnd: HWND,
    pcm: &IContextMenu,
    hmenu: HMENU,
    anchor: Option<CellAnchor>,
) -> Result<(), String> {
    // SAFETY: `hmenu` is a valid popup menu.
    unsafe { pcm.QueryContextMenu(hmenu, 0, ID_CMD_FIRST, ID_CMD_LAST, CMF_NORMAL) }
        .map_err(|e| format!("QueryContextMenu failed: {e}"))?;

    // Resolve before `prepare_for_keyboard` steals the foreground from the terminal.
    let pt = anchor.and_then(anchor_to_screen).unwrap_or_else(|| {
        let mut pt = POINT::default();
        // SAFETY: `pt` is a valid out-pointer.
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        pt
    });
    prepare_for_keyboard(hwnd);

    // SAFETY: modal menu on our host window; returns the chosen command id (0 = none).
    let cmd = unsafe {
        TrackPopupMenuEx(
            hmenu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
            pt.x,
            pt.y,
            hwnd,
            None,
        )
    }
    .0 as u32;
    // Documented trick so the menu does not "stick".
    // SAFETY: posting a benign message to our own window.
    unsafe {
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    }

    if !(ID_CMD_FIRST..=ID_CMD_LAST).contains(&cmd) {
        return Ok(()); // dismissed
    }

    // Snapshot visible windows so we can wait for any dialog the command opens
    // (Properties runs in explorer.exe on Windows 11, so owner matching won't do).
    let before = visible_windows();

    let verb = (cmd - ID_CMD_FIRST) as usize;
    let info = CMINVOKECOMMANDINFOEX {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
        fMask: CMIC_MASK_UNICODE | CMIC_MASK_PTINVOKE,
        hwnd,
        // MAKEINTRESOURCE: the verb is an ordinal smuggled through the string pointer.
        lpVerb: PCSTR(verb as *const u8),
        lpVerbW: PCWSTR(verb as *const u16),
        nShow: SW_SHOWNORMAL.0,
        ptInvoke: pt,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised; the ordinal verbs are valid for QueryContextMenu ids.
    unsafe { pcm.InvokeCommand(&info as *const _ as *const _) }
        .map_err(|e| format!("InvokeCommand failed: {e}"))?;

    wait_for_command_windows(hwnd, &before);
    Ok(())
}

unsafe extern "system" fn collect_visible(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `&mut HashSet<isize>` passed by visible_windows.
    let set = unsafe { &mut *(lparam.0 as *mut HashSet<isize>) };
    // SAFETY: `hwnd` is supplied by EnumWindows.
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        set.insert(hwnd.0);
    }
    BOOL(1)
}

fn visible_windows() -> HashSet<isize> {
    let mut set = HashSet::new();
    // SAFETY: the callback only runs during this call and `set` outlives it.
    unsafe {
        let _ = EnumWindows(Some(collect_visible), LPARAM(&mut set as *mut _ as isize));
    }
    set
}

/// Keep pumping messages (the shell may need them to finish the command) until a
/// window opened by the command has appeared and closed, or the grace time passed.
fn wait_for_command_windows(hwnd: HWND, before: &HashSet<isize>) {
    // SAFETY: timer + message pump on the thread owning `hwnd`.
    unsafe {
        let started = std::time::Instant::now();
        let mut appeared = false;
        SetTimer(hwnd, 1, 100, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if msg.message == WM_TIMER && msg.hwnd == hwnd {
                let new_windows = visible_windows().difference(before).count();
                if new_windows > 0 {
                    appeared = true;
                } else if appeared
                    || started.elapsed().as_millis() >= u128::from(NEW_WINDOW_GRACE_MS)
                {
                    PostQuitMessage(0);
                }
            }
        }
        let _ = KillTimer(hwnd, 1);
    }
}
