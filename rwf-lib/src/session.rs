//! Session state persistence
//!
//! This module handles saving and restoring application state across sessions,
//! including tab states, pane locations, and marked files.

use crate::model::{ActivePane, Location, TabState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Session state that can be persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Saved tab states
    pub tabs: Vec<SavedTabState>,
    /// Index of the active tab
    pub active_tab_index: usize,
    /// Active pane (Left or Right)
    pub active_pane: SavedActivePane,
    /// Marked file locations
    pub marked_locations: Vec<SavedLocation>,
    /// Task panel visibility
    #[serde(default = "default_show_task_panel")]
    pub show_task_panel: bool,
    /// Task panel height
    #[serde(default = "default_task_panel_height")]
    pub task_panel_height: usize,
}

fn default_show_task_panel() -> bool {
    true
}

fn default_task_panel_height() -> usize {
    5
}

/// Saved tab state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedTabState {
    /// Tab ID
    pub id: usize,
    /// Left pane location
    pub left_location: SavedLocation,
    /// Right pane location
    pub right_location: SavedLocation,
    /// Left pane cursor position
    pub left_cursor: usize,
    /// Right pane cursor position
    pub right_cursor: usize,
    /// Which pane the cursor was on in this tab. Defaulted: session files written
    /// before the cursor side became per-tab have no such field, and fall back to
    /// `SessionState::active_pane` for the active tab (see `AppState::restore_session`).
    #[serde(default)]
    pub active_pane: SavedActivePane,
}

/// Saved location (simplified for serialization)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SavedLocation {
    Local(PathBuf),
    // Future: Add Ssh, Cloud, Archive variants when needed
}

/// Saved active pane
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum SavedActivePane {
    #[default]
    Left,
    Right,
}

impl SessionState {
    /// Create a new empty session state
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            active_pane: SavedActivePane::Left,
            marked_locations: Vec::new(),
            show_task_panel: true,
            task_panel_height: 5,
        }
    }

    /// Save session state to a file
    pub fn save_to_file(&self, path: &Path) -> Result<(), SessionError> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SessionError::IoError(e.to_string()))?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| SessionError::SerializationError(e.to_string()))?;

        std::fs::write(path, json).map_err(|e| SessionError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Load session state from a file
    pub fn load_from_file(path: &Path) -> Result<Self, SessionError> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let json =
            std::fs::read_to_string(path).map_err(|e| SessionError::IoError(e.to_string()))?;

        let state = serde_json::from_str(&json)
            .map_err(|e| SessionError::DeserializationError(e.to_string()))?;

        Ok(state)
    }

    /// Get the default session file path
    pub fn default_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("rwf");
        path.push("session.json");
        path
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Session error types
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
}

/// Convert Location to SavedLocation
impl From<&Location> for SavedLocation {
    fn from(location: &Location) -> Self {
        match location {
            Location::Local(path) => SavedLocation::Local(path.clone()),
            // For now, we only support Local locations in session persistence
            // Future: Add support for other location types
            _ => SavedLocation::Local(PathBuf::from("/")),
        }
    }
}

/// Convert SavedLocation to Location
impl From<SavedLocation> for Location {
    fn from(saved: SavedLocation) -> Self {
        match saved {
            SavedLocation::Local(path) => Location::Local(path),
        }
    }
}

/// Convert ActivePane to SavedActivePane
impl From<ActivePane> for SavedActivePane {
    fn from(pane: ActivePane) -> Self {
        match pane {
            ActivePane::Left => SavedActivePane::Left,
            ActivePane::Right => SavedActivePane::Right,
        }
    }
}

/// Convert SavedActivePane to ActivePane
impl From<SavedActivePane> for ActivePane {
    fn from(saved: SavedActivePane) -> Self {
        match saved {
            SavedActivePane::Left => ActivePane::Left,
            SavedActivePane::Right => ActivePane::Right,
        }
    }
}

/// Create SessionState from AppState
pub fn save_session(
    tabs: &[TabState],
    active_tab_index: usize,
    active_pane: ActivePane,
    marked_locations: &HashSet<Location>,
    show_task_panel: bool,
    task_panel_height: usize,
) -> SessionState {
    let saved_tabs = tabs
        .iter()
        .map(|tab| SavedTabState {
            id: tab.id,
            left_location: (&tab.left_pane.current_location).into(),
            right_location: (&tab.right_pane.current_location).into(),
            left_cursor: tab.left_pane.cursor,
            right_cursor: tab.right_pane.cursor,
            active_pane: tab.active_pane.into(),
        })
        .collect();

    let saved_marked: Vec<SavedLocation> = marked_locations.iter().map(|loc| loc.into()).collect();

    SessionState {
        tabs: saved_tabs,
        active_tab_index,
        active_pane: active_pane.into(),
        marked_locations: saved_marked,
        show_task_panel,
        task_panel_height,
    }
}

/// Nearest existing ancestor of a path that could not be read, if any.
///
/// A saved path can disappear between runs for entirely ordinary reasons — a
/// removed git worktree, an unmounted drive, a cleaned build directory. Landing
/// on the nearest surviving ancestor keeps the user near where they were: a
/// deleted `…/worktrees/foo` opens `…/worktrees`, one step from the intent.
///
/// **This blocks and must never be called on the UI thread.** Every `exists()`
/// here is a filesystem round trip, and on an unreachable network host each one
/// costs a full name-resolution timeout — as does each ancestor above it. It ran
/// eagerly inside `restore_tabs` until 2026-09-10, before the first frame was
/// drawn, which is exactly the multi-second black screen at startup reported in
/// diagnostic bundle `20260910-203646`. It is now reached only through
/// `JobKind::ResolveFallbackPath`, on a worker thread, and only after a pane's
/// own `ReadDirectory` has already failed.
///
/// Returns `None` when the path is readable after all, or when nothing in the
/// chain survives — the caller then leaves the pane where it is so the error
/// names the real path rather than silently landing somewhere unrelated.
pub fn nearest_existing_ancestor(requested: &Location) -> Option<Location> {
    let path = requested.path()?;
    if path.exists() {
        return None;
    }
    let mut candidate = path.parent();
    while let Some(dir) = candidate {
        if dir.exists() {
            return Some(Location::Local(dir.to_path_buf()));
        }
        candidate = dir.parent();
    }
    None
}

/// Restore tab states from SessionState.
///
/// Paths are restored exactly as saved, with **no filesystem access at all** —
/// this runs before the first frame is drawn, so a single unreachable path here
/// costs the user a black screen. A path that turns out to be unreadable is
/// handled after the fact by [`nearest_existing_ancestor`], reached through
/// `JobKind::ResolveFallbackPath` on a worker thread.
pub fn restore_tabs(session: &SessionState) -> Vec<TabState> {
    if session.tabs.is_empty() {
        // If no saved tabs, create a default tab
        vec![TabState::new(0)]
    } else {
        let tabs = session
            .tabs
            .iter()
            .map(|saved_tab| {
                let mut tab = TabState::new(saved_tab.id);
                // Restored as saved, unprobed. See `nearest_existing_ancestor`.
                tab.left_pane.current_location = saved_tab.left_location.clone().into();
                tab.right_pane.current_location = saved_tab.right_location.clone().into();
                tab.left_pane.cursor = saved_tab.left_cursor;
                tab.right_pane.cursor = saved_tab.right_cursor;
                tab.active_pane = saved_tab.active_pane.into();

                // Don't adjust scroll_offset here - let the normal scrolling logic
                // handle it when entries are loaded via CompleteJob transition.
                // The cursor movement logic will ensure the cursor is visible.

                tab
            })
            .collect();
        tabs
    }
}

/// Restore marked locations from SessionState
pub fn restore_marked_locations(session: &SessionState) -> HashSet<Location> {
    session
        .marked_locations
        .iter()
        .map(|saved| saved.clone().into())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_session_state_creation() {
        let session = SessionState::new();
        assert_eq!(session.tabs.len(), 0);
        assert_eq!(session.active_tab_index, 0);
        assert!(matches!(session.active_pane, SavedActivePane::Left));
        assert_eq!(session.marked_locations.len(), 0);
    }

    #[test]
    fn test_save_and_load_session() {
        let temp_dir = std::env::temp_dir();
        let session_path = temp_dir.join("test_session.json");

        // Create a session state
        let mut session = SessionState::new();
        session.tabs.push(SavedTabState {
            id: 0,
            left_location: SavedLocation::Local(PathBuf::from("/home/user")),
            right_location: SavedLocation::Local(PathBuf::from("/tmp")),
            left_cursor: 5,
            right_cursor: 10,
            active_pane: SavedActivePane::Left,
        });
        session.active_tab_index = 0;
        session.active_pane = SavedActivePane::Right;

        // Save to file
        session.save_to_file(&session_path).unwrap();

        // Load from file
        let loaded = SessionState::load_from_file(&session_path).unwrap();

        assert_eq!(loaded.tabs.len(), 1);
        assert_eq!(loaded.active_tab_index, 0);
        assert!(matches!(loaded.active_pane, SavedActivePane::Right));
        assert_eq!(loaded.tabs[0].left_cursor, 5);
        assert_eq!(loaded.tabs[0].right_cursor, 10);

        // Cleanup
        let _ = std::fs::remove_file(session_path);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let temp_dir = std::env::temp_dir();
        let session_path = temp_dir.join("nonexistent_session.json");

        // Should return empty session state
        let loaded = SessionState::load_from_file(&session_path).unwrap();
        assert_eq!(loaded.tabs.len(), 0);
    }

    #[test]
    fn test_location_conversion() {
        let location = Location::Local(PathBuf::from("/home/user"));
        let saved: SavedLocation = (&location).into();
        let restored: Location = saved.into();

        assert_eq!(location, restored);
    }

    #[test]
    fn test_active_pane_conversion() {
        let pane = ActivePane::Left;
        let saved: SavedActivePane = pane.into();
        let restored: ActivePane = saved.into();

        assert_eq!(pane, restored);
    }

    #[test]
    fn test_save_session_with_multiple_tabs() {
        let tabs = vec![TabState::new(0), TabState::new(1), TabState::new(2)];

        let marked = HashSet::new();
        let session = save_session(&tabs, 1, ActivePane::Right, &marked, true, 5);

        assert_eq!(session.tabs.len(), 3);
        assert_eq!(session.active_tab_index, 1);
        assert!(matches!(session.active_pane, SavedActivePane::Right));
    }

    #[test]
    fn test_restore_tabs_empty_session() {
        let session = SessionState::new();
        let tabs = restore_tabs(&session);

        // Should create a default tab
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].id, 0);
    }

    #[test]
    fn test_restore_tabs_with_saved_state() {
        let mut session = SessionState::new();
        session.tabs.push(SavedTabState {
            id: 0,
            left_location: SavedLocation::Local(PathBuf::from("/home")),
            right_location: SavedLocation::Local(PathBuf::from("/tmp")),
            left_cursor: 0,
            right_cursor: 0,
            active_pane: SavedActivePane::Left,
        });
        session.tabs.push(SavedTabState {
            id: 1,
            left_location: SavedLocation::Local(PathBuf::from("/var")),
            right_location: SavedLocation::Local(PathBuf::from("/opt")),
            left_cursor: 0,
            right_cursor: 0,
            active_pane: SavedActivePane::Left,
        });

        let tabs = restore_tabs(&session);

        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].id, 0);
        assert_eq!(tabs[1].id, 1);
    }

    #[test]
    fn test_restore_marked_locations() {
        let mut session = SessionState::new();
        session
            .marked_locations
            .push(SavedLocation::Local(PathBuf::from("/file1")));
        session
            .marked_locations
            .push(SavedLocation::Local(PathBuf::from("/file2")));

        let marked = restore_marked_locations(&session);

        assert_eq!(marked.len(), 2);
        assert!(marked.contains(&Location::Local(PathBuf::from("/file1"))));
        assert!(marked.contains(&Location::Local(PathBuf::from("/file2"))));
    }

    #[test]
    fn test_session_save_integration() {
        use crate::model::{ActivePane, TabState};
        use std::collections::HashSet;

        // Create tabs with different locations and cursor positions
        let mut tab1 = TabState::new(0);
        tab1.left_pane.current_location = Location::Local(PathBuf::from("/home/user"));
        tab1.right_pane.current_location = Location::Local(PathBuf::from("/tmp"));
        tab1.left_pane.cursor = 5;
        tab1.right_pane.cursor = 10;

        let mut tab2 = TabState::new(1);
        tab2.left_pane.current_location = Location::Local(PathBuf::from("/var/log"));
        tab2.right_pane.current_location = Location::Local(PathBuf::from("/opt"));
        tab2.left_pane.cursor = 3;
        tab2.right_pane.cursor = 7;

        let tabs = vec![tab1, tab2];

        // Create marked locations
        let mut marked = HashSet::new();
        marked.insert(Location::Local(PathBuf::from("/home/user/file1.txt")));
        marked.insert(Location::Local(PathBuf::from("/tmp/file2.txt")));

        // Save session
        let session = save_session(&tabs, 1, ActivePane::Right, &marked, true, 5);

        // Verify session state
        assert_eq!(session.tabs.len(), 2);
        assert_eq!(session.active_tab_index, 1);
        assert!(matches!(session.active_pane, SavedActivePane::Right));
        assert_eq!(session.marked_locations.len(), 2);

        // Verify tab 1
        assert_eq!(session.tabs[0].id, 0);
        assert_eq!(session.tabs[0].left_cursor, 5);
        assert_eq!(session.tabs[0].right_cursor, 10);

        // Verify tab 2
        assert_eq!(session.tabs[1].id, 1);
        assert_eq!(session.tabs[1].left_cursor, 3);
        assert_eq!(session.tabs[1].right_cursor, 7);
    }

    #[test]
    fn test_session_restore_integration() {
        let temp_dir = std::env::temp_dir();
        let session_path = temp_dir.join("test_session_restore.json");

        // Create a session with multiple tabs and marked files
        let mut session = SessionState::new();
        session.tabs.push(SavedTabState {
            id: 0,
            left_location: SavedLocation::Local(PathBuf::from("/home/user")),
            right_location: SavedLocation::Local(PathBuf::from("/tmp")),
            left_cursor: 5,
            right_cursor: 10,
            active_pane: SavedActivePane::Left,
        });
        session.tabs.push(SavedTabState {
            id: 1,
            left_location: SavedLocation::Local(PathBuf::from("/var/log")),
            right_location: SavedLocation::Local(PathBuf::from("/opt")),
            left_cursor: 3,
            right_cursor: 7,
            active_pane: SavedActivePane::Left,
        });
        session.active_tab_index = 1;
        session.active_pane = SavedActivePane::Right;
        session
            .marked_locations
            .push(SavedLocation::Local(PathBuf::from("/file1")));
        session
            .marked_locations
            .push(SavedLocation::Local(PathBuf::from("/file2")));

        // Save to file
        session.save_to_file(&session_path).unwrap();

        // Load from file
        let loaded = SessionState::load_from_file(&session_path).unwrap();

        // Verify loaded state
        assert_eq!(loaded.tabs.len(), 2);
        assert_eq!(loaded.active_tab_index, 1);
        assert!(matches!(loaded.active_pane, SavedActivePane::Right));
        assert_eq!(loaded.marked_locations.len(), 2);

        // Restore tabs
        let tabs = restore_tabs(&loaded);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].left_pane.cursor, 5);
        assert_eq!(tabs[0].right_pane.cursor, 10);
        assert_eq!(tabs[1].left_pane.cursor, 3);
        assert_eq!(tabs[1].right_pane.cursor, 7);

        // Restore marked locations
        let marked = restore_marked_locations(&loaded);
        assert_eq!(marked.len(), 2);

        // Cleanup
        let _ = std::fs::remove_file(session_path);
    }

    #[test]
    fn test_marked_file_persistence() {
        use std::collections::HashSet;

        // Create marked locations
        let mut marked = HashSet::new();
        marked.insert(Location::Local(PathBuf::from("/home/user/doc1.txt")));
        marked.insert(Location::Local(PathBuf::from("/home/user/doc2.txt")));
        marked.insert(Location::Local(PathBuf::from("/tmp/temp.txt")));

        // Save session with marked files
        let tabs = vec![TabState::new(0)];
        let session = save_session(&tabs, 0, crate::model::ActivePane::Left, &marked, true, 5);

        // Verify marked locations are saved
        assert_eq!(session.marked_locations.len(), 3);

        // Restore marked locations
        let restored_marked = restore_marked_locations(&session);
        assert_eq!(restored_marked.len(), 3);
        assert!(restored_marked.contains(&Location::Local(PathBuf::from("/home/user/doc1.txt"))));
        assert!(restored_marked.contains(&Location::Local(PathBuf::from("/home/user/doc2.txt"))));
        assert!(restored_marked.contains(&Location::Local(PathBuf::from("/tmp/temp.txt"))));
    }

    #[test]
    fn test_session_persistence_with_empty_marked_files() {
        use std::collections::HashSet;

        let tabs = vec![TabState::new(0)];
        let marked = HashSet::new();
        let session = save_session(&tabs, 0, crate::model::ActivePane::Left, &marked, true, 5);

        assert_eq!(session.marked_locations.len(), 0);

        let restored_marked = restore_marked_locations(&session);
        assert_eq!(restored_marked.len(), 0);
    }

    #[test]
    fn test_cursor_position_persistence() {
        use crate::model::TabState;

        let mut tab = TabState::new(0);
        tab.left_pane.cursor = 42;
        tab.right_pane.cursor = 99;

        let tabs = vec![tab];
        let marked = std::collections::HashSet::new();
        let session = save_session(&tabs, 0, crate::model::ActivePane::Left, &marked, true, 5);

        assert_eq!(session.tabs[0].left_cursor, 42);
        assert_eq!(session.tabs[0].right_cursor, 99);

        let restored_tabs = restore_tabs(&session);
        assert_eq!(restored_tabs[0].left_pane.cursor, 42);
        assert_eq!(restored_tabs[0].right_pane.cursor, 99);
    }

    #[test]
    fn test_active_pane_persistence() {
        use crate::model::{ActivePane, TabState};
        use std::collections::HashSet;

        let tabs = vec![TabState::new(0)];
        let marked = HashSet::new();

        // Test Left pane
        let session_left = save_session(&tabs, 0, ActivePane::Left, &marked, true, 5);
        assert!(matches!(session_left.active_pane, SavedActivePane::Left));
        let restored_left: ActivePane = session_left.active_pane.into();
        assert_eq!(restored_left, ActivePane::Left);

        // Test Right pane
        let session_right = save_session(&tabs, 0, ActivePane::Right, &marked, true, 5);
        assert!(matches!(session_right.active_pane, SavedActivePane::Right));
        let restored_right: ActivePane = session_right.active_pane.into();
        assert_eq!(restored_right, ActivePane::Right);
    }

    /// The cursor side is per tab, so it has to survive a save/restore round trip per
    /// tab -- not collapse onto the session-wide value, which is only the active tab's.
    #[test]
    fn test_each_tab_keeps_its_own_cursor_side_across_a_round_trip() {
        use crate::model::{ActivePane, TabState};
        use std::collections::HashSet;

        let mut tabs = vec![TabState::new(0), TabState::new(1), TabState::new(2)];
        tabs[0].active_pane = ActivePane::Left;
        tabs[1].active_pane = ActivePane::Right;
        tabs[2].active_pane = ActivePane::Left;

        let session = save_session(&tabs, 1, ActivePane::Right, &HashSet::new(), true, 5);
        let restored = restore_tabs(&session);

        assert_eq!(restored[0].active_pane, ActivePane::Left);
        assert_eq!(
            restored[1].active_pane,
            ActivePane::Right,
            "tab 1's cursor side was not carried through the session"
        );
        assert_eq!(restored[2].active_pane, ActivePane::Left);
    }

    /// Session files written before the cursor side became per-tab have no such field.
    /// They must still load, with the tabs defaulting to Left; `AppState::restore_session`
    /// then puts the session-wide value back on the active tab.
    #[test]
    fn test_session_file_without_per_tab_cursor_side_still_loads() {
        let json = r#"{
            "tabs": [{
                "id": 0,
                "left_location": { "Local": "/home/user" },
                "right_location": { "Local": "/tmp" },
                "left_cursor": 5,
                "right_cursor": 10
            }],
            "active_tab_index": 0,
            "active_pane": "Right",
            "marked_locations": []
        }"#;

        let session: SessionState =
            serde_json::from_str(json).expect("a pre-per-tab session file must still parse");
        assert_eq!(session.tabs.len(), 1);
        assert!(matches!(session.tabs[0].active_pane, SavedActivePane::Left));
        assert!(matches!(session.active_pane, SavedActivePane::Right));
    }

    #[test]
    fn test_multiple_tabs_persistence() {
        use crate::model::TabState;
        use std::collections::HashSet;

        let mut tabs = Vec::new();
        for i in 0..5 {
            let mut tab = TabState::new(i);
            tab.left_pane.current_location = Location::Local(PathBuf::from(format!("/path{}", i)));
            tab.right_pane.current_location =
                Location::Local(PathBuf::from(format!("/other{}", i)));
            tab.left_pane.cursor = i * 2;
            tab.right_pane.cursor = i * 3;
            tabs.push(tab);
        }

        let marked = HashSet::new();
        let session = save_session(&tabs, 2, crate::model::ActivePane::Right, &marked, true, 5);

        assert_eq!(session.tabs.len(), 5);
        assert_eq!(session.active_tab_index, 2);

        let restored_tabs = restore_tabs(&session);
        assert_eq!(restored_tabs.len(), 5);

        for (i, tab) in restored_tabs.iter().enumerate().take(5) {
            assert_eq!(tab.id, i);
            assert_eq!(tab.left_pane.cursor, i * 2);
            assert_eq!(tab.right_pane.cursor, i * 3);
        }
    }
}

#[cfg(test)]
mod restore_missing_path_tests {
    use super::*;

    fn saved_tab(left: PathBuf, right: PathBuf) -> SavedTabState {
        SavedTabState {
            id: 0,
            left_location: SavedLocation::Local(left),
            right_location: SavedLocation::Local(right),
            left_cursor: 0,
            right_cursor: 0,
            active_pane: SavedActivePane::Left,
        }
    }

    /// A saved path that no longer exists is now restored **verbatim**, with no
    /// filesystem access at all. Probing here used to happen before the first frame
    /// was drawn, so one unreachable network path cost a multi-second black screen
    /// at startup (diagnostic bundle `20260910-203646`). The repair moved to
    /// `JobKind::ResolveFallbackPath`, on a worker thread, after the pane's own
    /// `ReadDirectory` has failed.
    #[test]
    fn restore_does_not_probe_the_filesystem() {
        let temp = tempfile::tempdir().expect("tempdir");
        let existing = temp.path().to_path_buf();
        let vanished = existing.join("worktrees").join("deleted-one");

        let mut session = SessionState::new();
        session.tabs = vec![saved_tab(existing.clone(), vanished.clone())];

        let tabs = restore_tabs(&session);

        assert_eq!(
            tabs[0].left_pane.current_location,
            Location::Local(existing),
            "an existing path must be restored untouched"
        );
        assert_eq!(
            tabs[0].right_pane.current_location,
            Location::Local(vanished),
            "a vanished path must also be restored untouched — restore must not stat it"
        );
    }

    /// The repair itself, now on the worker side: nearest surviving ancestor, so a
    /// deleted `…/worktrees/foo` lands in `…/worktrees` rather than somewhere
    /// unrelated.
    #[test]
    fn nearest_existing_ancestor_walks_up_to_the_first_survivor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let existing = temp.path().to_path_buf();
        let vanished = existing.join("worktrees").join("deleted-one");

        assert_eq!(
            nearest_existing_ancestor(&Location::Local(vanished)),
            Some(Location::Local(existing.clone())),
            "must land on the nearest directory that still exists"
        );
        assert_eq!(
            nearest_existing_ancestor(&Location::Local(existing)),
            None,
            "a readable path needs no fallback"
        );
    }
}
