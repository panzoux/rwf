//! Shared test fixtures and helpers.
//!
//! Compiled only for tests (`#[cfg(test)] pub mod test_utils;` in `lib.rs`).
//! Use these instead of re-declaring per-file `create_test_*` helpers:
//!
//! - [`test_state`] / [`AppStateBuilder`] — `AppState` construction
//! - [`entry`] / [`entries`] / [`numbered_entries`] / [`FileEntryBuilder`] — `FileEntry` fixtures
//! - [`temp_dir`] / [`state_with_temp_dirs`] — filesystem-backed setups
//! - [`open_dialog`] / [`current_dialog`] — dialog launch and access
//!
//! Tests whose setup differs *intentionally* from these defaults should keep
//! their own local helpers rather than force-fit the shared ones.

use std::path::PathBuf;
use std::time::SystemTime;

use tempfile::TempDir;

use crate::config::AppConfig;
use crate::model::{ActivePane, Dialog, FileEntry, Location};
use crate::state::{update_state, AppState, Transition};

/// An `AppState` with default config — the most common test starting point.
pub fn test_state() -> AppState {
    AppState::new(AppConfig::default())
}

/// Builder for [`FileEntry`] with test defaults.
///
/// Defaults: location `/test/<name>`, size 100, regular file, not hidden,
/// not marked, modified = now, no symlink info.
#[derive(Clone)]
pub struct FileEntryBuilder {
    entry: FileEntry,
}

impl FileEntryBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            entry: FileEntry {
                name: name.to_string(),
                location: Location::Local(PathBuf::from(format!("/test/{name}"))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
                is_symlink: false,
                link_target: None,
                link_kind: None,
            },
        }
    }

    pub fn size(mut self, size: u64) -> Self {
        self.entry.size = size;
        self
    }

    pub fn dir(mut self, is_dir: bool) -> Self {
        self.entry.is_dir = is_dir;
        self
    }

    pub fn hidden(mut self, is_hidden: bool) -> Self {
        self.entry.is_hidden = is_hidden;
        self
    }

    pub fn marked(mut self, marked: bool) -> Self {
        self.entry.marked = marked;
        self
    }

    /// Set the full location (overrides the `/test/<name>` default).
    pub fn location(mut self, location: Location) -> Self {
        self.entry.location = location;
        self
    }

    /// Set the location from a local path string.
    pub fn path(mut self, path: &str) -> Self {
        self.entry.location = Location::Local(PathBuf::from(path));
        self
    }

    pub fn modified(mut self, modified: SystemTime) -> Self {
        self.entry.modified = modified;
        self
    }

    pub fn build(self) -> FileEntry {
        self.entry
    }
}

/// Shorthand: a default file entry named `name` (see [`FileEntryBuilder`] defaults).
pub fn entry(name: &str) -> FileEntry {
    FileEntryBuilder::new(name).build()
}

/// Default entries with the given names.
pub fn entries(names: &[&str]) -> Vec<FileEntry> {
    names.iter().map(|n| entry(n)).collect()
}

/// `count` default entries named `file0.txt` .. `file{count-1}.txt`.
pub fn numbered_entries(count: usize) -> Vec<FileEntry> {
    (0..count).map(|i| entry(&format!("file{i}.txt"))).collect()
}

/// Builder for [`AppState`]: default config plus per-pane overrides.
///
/// Only the fields you set are touched; everything else keeps the
/// `AppState::new` defaults.
#[derive(Default)]
pub struct AppStateBuilder {
    config: Option<AppConfig>,
    left_location: Option<Location>,
    right_location: Option<Location>,
    left_entries: Option<Vec<FileEntry>>,
    right_entries: Option<Vec<FileEntry>>,
    left_cursor: Option<usize>,
    right_cursor: Option<usize>,
    active_pane: Option<ActivePane>,
}

impl AppStateBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config(mut self, config: AppConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn left_location(mut self, location: Location) -> Self {
        self.left_location = Some(location);
        self
    }

    pub fn right_location(mut self, location: Location) -> Self {
        self.right_location = Some(location);
        self
    }

    /// Set the left pane's location from a local path string.
    pub fn left_path(self, path: &str) -> Self {
        self.left_location(Location::Local(PathBuf::from(path)))
    }

    /// Set the right pane's location from a local path string.
    pub fn right_path(self, path: &str) -> Self {
        self.right_location(Location::Local(PathBuf::from(path)))
    }

    pub fn left_entries(mut self, entries: Vec<FileEntry>) -> Self {
        self.left_entries = Some(entries);
        self
    }

    pub fn right_entries(mut self, entries: Vec<FileEntry>) -> Self {
        self.right_entries = Some(entries);
        self
    }

    pub fn left_cursor(mut self, cursor: usize) -> Self {
        self.left_cursor = Some(cursor);
        self
    }

    pub fn right_cursor(mut self, cursor: usize) -> Self {
        self.right_cursor = Some(cursor);
        self
    }

    pub fn active_pane(mut self, pane: ActivePane) -> Self {
        self.active_pane = Some(pane);
        self
    }

    pub fn build(self) -> AppState {
        let mut state = AppState::new(self.config.unwrap_or_default());
        {
            let tab = state.current_tab_mut();
            if let Some(location) = self.left_location {
                tab.left_pane.current_location = location;
            }
            if let Some(location) = self.right_location {
                tab.right_pane.current_location = location;
            }
            if let Some(entries) = self.left_entries {
                tab.left_pane.entries = entries;
            }
            if let Some(entries) = self.right_entries {
                tab.right_pane.entries = entries;
            }
            if let Some(cursor) = self.left_cursor {
                tab.left_pane.cursor = cursor;
            }
            if let Some(cursor) = self.right_cursor {
                tab.right_pane.cursor = cursor;
            }
        }
        if let Some(pane) = self.active_pane {
            state.ui.active_pane = pane;
        }
        state
    }
}

/// A fresh temporary directory (panics on failure — test-only).
pub fn temp_dir() -> TempDir {
    TempDir::new().expect("failed to create temp dir")
}

/// An `AppState` whose pane locations point at two fresh temp directories.
///
/// Keep the returned `TempDir`s alive for the duration of the test —
/// dropping them deletes the directories.
pub fn state_with_temp_dirs() -> (AppState, TempDir, TempDir) {
    let left = temp_dir();
    let right = temp_dir();
    let state = AppStateBuilder::new()
        .left_location(Location::Local(left.path().to_path_buf()))
        .right_location(Location::Local(right.path().to_path_buf()))
        .build();
    (state, left, right)
}

/// Apply `transition` and return the dialog it opened.
///
/// Panics if no dialog is open afterwards.
pub fn open_dialog(state: &mut AppState, transition: Transition) -> &Dialog {
    update_state(state, transition);
    current_dialog(state)
}

/// The currently open dialog. Panics if none is open.
pub fn current_dialog(state: &AppState) -> &Dialog {
    state
        .dialogs
        .current()
        .expect("expected a dialog to be open")
}
