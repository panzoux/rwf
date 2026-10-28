//! Native Windows Explorer context menu (the right-click menu, including shell
//! extensions) for one or more files in the same folder.
//!
//! Rust port of `panzoux/win_ctxmenu` (`ctxmenu.cpp`). On non-Windows platforms
//! [`show_for_paths`] returns an error, so callers need no `cfg` of their own.

use std::path::PathBuf;

#[cfg(windows)]
mod imp;

/// Show the shell context menu for `paths` at the mouse cursor, run the chosen
/// command, and return once the command (e.g. a Properties dialog) has settled.
///
/// All paths must share one parent folder (true for a pane's marked files). The
/// menu runs on a dedicated COM-STA thread, so this is safe to call from any
/// thread but blocks until the menu is dismissed.
pub fn show_for_paths(paths: &[PathBuf]) -> Result<(), String> {
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
        std::thread::spawn(move || imp::show(&paths))
            .join()
            .map_err(|_| "context menu thread panicked".to_string())?
    }
    #[cfg(not(windows))]
    {
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
