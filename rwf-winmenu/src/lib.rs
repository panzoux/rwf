//! Native Windows Explorer context menu (the right-click menu, including shell
//! extensions) for one or more files in the same folder.
//!
//! Rust port of `panzoux/win_ctxmenu` (`ctxmenu.cpp`). On non-Windows platforms
//! [`show_for_paths`] returns an error, so callers need no `cfg` of their own.

use std::path::PathBuf;

#[cfg(windows)]
mod imp;

/// A terminal cell to open the menu at, plus the grid it is measured in.
///
/// The menu is positioned by scaling the terminal window's client area by
/// `col / cols` and `row / rows`, so the caller needs no font metrics. The result is
/// exact for a plain console window; terminals with their own chrome inside the client
/// area (a tab strip) land within a line or two of the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellAnchor {
    pub col: u16,
    pub row: u16,
    /// Terminal width in cells.
    pub cols: u16,
    /// Terminal height in cells.
    pub rows: u16,
}

/// Show the shell context menu for `paths` at the mouse cursor, run the chosen
/// command, and return once the command (e.g. a Properties dialog) has settled.
///
/// Same as [`show_for_paths_at`] with no anchor.
///
/// All paths must share one parent folder (true for a pane's marked files). The
/// menu runs on a dedicated COM-STA thread, so this is safe to call from any
/// thread but blocks until the menu is dismissed.
pub fn show_for_paths(paths: &[PathBuf]) -> Result<(), String> {
    show_for_paths_at(paths, None)
}

/// Like [`show_for_paths`], but opens the menu at `anchor` (a terminal cell) when it
/// can be resolved to a screen position, falling back to the mouse cursor otherwise.
pub fn show_for_paths_at(paths: &[PathBuf], anchor: Option<CellAnchor>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no files to show a context menu for".to_string());
    }
    let first_parent = paths[0].parent();
    if paths.iter().any(|p| p.parent() != first_parent) {
        return Err("files must be in the same folder".to_string());
    }
    #[cfg(windows)]
    {
        let paths = paths.to_vec();
        std::thread::spawn(move || imp::show(&paths, anchor))
            .join()
            .map_err(|_| "context menu thread panicked".to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = anchor;
        Err("the shell context menu is only available on Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_mixed_folders() {
        assert!(show_for_paths(&[]).is_err());
        let mixed = [PathBuf::from("a/x.txt"), PathBuf::from("b/y.txt")];
        assert_eq!(
            show_for_paths(&mixed),
            Err("files must be in the same folder".to_string())
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_off_windows() {
        let err = show_for_paths(&[PathBuf::from("a/x.txt")]).unwrap_err();
        assert!(err.contains("only available on Windows"));
    }
}
