//! Input handling and key bindings
//!
//! This module provides configurable key bindings and input event processing.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

use crate::backend::ArchiveHandler;
use crate::model::Location;
use crate::state::Transition;
use crate::AppState;

/// Archive format for compression operations
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ArchiveFormat {
    #[default]
    ZIP,
    SevenZip,
    Tar,
    TarGz,
}

/// Check if a location is an archive file (by name suffix).
/// Uses suffix matching rather than Path::extension() to support double
/// extensions like .tar.gz.
fn is_archive(location: &crate::model::Location) -> bool {
    let path_str = location.display_path();
    let name = path_str.to_lowercase();
    name.ends_with(".zip")
        || name.ends_with(".7z")
        || name.ends_with(".tar")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.gz")
        || name.ends_with(".rar")
        || name.ends_with(".iso")
        || name.ends_with(".lzh")
        || name.ends_with(".lha")
}

/// Configurable key bindings loaded from keybindings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    /// Key bindings for normal mode
    #[serde(rename = "NormalMode", default)]
    pub normal_mode: HashMap<String, Action>,
    /// Key bindings for search mode
    #[serde(rename = "SearchMode", default)]
    pub search_mode: HashMap<String, Action>,
    /// Key bindings for dialog mode
    #[serde(rename = "DialogMode", default)]
    pub dialog_mode: HashMap<String, Action>,
    /// Key bindings for viewer mode
    #[serde(rename = "ViewerMode", default)]
    pub viewer_mode: HashMap<String, Action>,
    /// Key bindings for Leap Navigation mode (F3).
    #[serde(rename = "LeapMode", default)]
    pub leap_mode: HashMap<String, Action>,
    /// Multi-key sequence state
    #[serde(skip)]
    pub pending_sequence: Option<String>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::embedded_defaults()
    }
}

impl KeyBindings {
    /// Load built-in defaults from the embedded JSON resource (single source of truth).
    pub fn embedded_defaults() -> Self {
        Self::load_from_str(include_str!("../../resources/default_keybindings.json"))
            .expect("embedded default_keybindings.json is always valid JSON")
    }

    /// Parse key bindings from a JSON string, starting from an empty state.
    /// Does not merge over any defaults — the string must contain all desired bindings.
    /// Used by `embedded_defaults()` and for testing.
    pub fn load_from_str(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let v: serde_json::Value = serde_json::from_str(content)?;
        let mut result = Self {
            normal_mode: HashMap::new(),
            search_mode: HashMap::new(),
            dialog_mode: HashMap::new(),
            viewer_mode: HashMap::new(),
            leap_mode: HashMap::new(),
            pending_sequence: None,
        };
        Self::apply_from_value(&v, &mut result);
        Ok(result)
    }

    /// Apply bindings from a parsed JSON value into `target`, overwriting any existing entries.
    fn apply_from_value(v: &serde_json::Value, target: &mut Self) {
        // TWF format: top-level "bindings" key maps to NormalMode
        if let Some(bindings) = v.get("bindings").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    target
                        .normal_mode
                        .insert(key.clone(), Self::parse_action_name(action_str));
                }
            }
        }
        // TWF format: "textViewerBindings" maps to ViewerMode
        if let Some(bindings) = v.get("textViewerBindings").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    let stripped = action_str.strip_prefix("TextViewer.").unwrap_or(action_str);
                    target
                        .viewer_mode
                        .insert(key.clone(), Self::parse_viewer_action_name(stripped));
                }
            }
        }
        // Native rwf format: NormalMode / SearchMode / DialogMode / ViewerMode
        if let Some(bindings) = v.get("NormalMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    target
                        .normal_mode
                        .insert(key.clone(), Self::parse_action_name(action_str));
                }
            }
        }
        if let Some(bindings) = v.get("SearchMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    target
                        .search_mode
                        .insert(key.clone(), Self::parse_action_name(action_str));
                }
            }
        }
        if let Some(bindings) = v.get("DialogMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    target
                        .dialog_mode
                        .insert(key.clone(), Self::parse_action_name(action_str));
                }
            }
        }
        if let Some(bindings) = v.get("ViewerMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    target
                        .viewer_mode
                        .insert(key.clone(), Self::parse_viewer_action_name(action_str));
                }
            }
        }
        if let Some(bindings) = v.get("LeapMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    target
                        .leap_mode
                        .insert(key.clone(), Self::parse_leap_action_name(action_str));
                }
            }
        }
    }

    /// Load key bindings from a JSON file, merging over embedded defaults.
    /// Accepts both the native format (`NormalMode` key) and the TWF-compatible
    /// format (`bindings` key). Unknown action strings become `InvokeCustomFunction`.
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&content)?;
        let mut merged = Self::default(); // = embedded_defaults()
        Self::apply_from_value(&v, &mut merged);
        Ok(merged)
    }

    /// Convert an action name string to an `Action`.
    /// Handles: exact rwf variant names, TWF alias names, and custom function names.
    fn parse_action_name(s: &str) -> Action {
        // 1. Try exact deserialization as an Action enum variant
        if let Ok(action) =
            serde_json::from_value::<Action>(serde_json::Value::String(s.to_string()))
        {
            return action;
        }
        // 2. Legacy alias table — maps old/variant names to canonical Actions
        match s {
            "ReloadConfiguration" => return Action::ReloadConfig,
            "LaunchConfigEditor" => return Action::EditConfigFile,
            "ViewFile" => return Action::OpenTextViewer,
            "ViewFileAsText" => return Action::OpenTextViewer,
            "ViewFileAsHex" => return Action::OpenHexViewer,
            "ViewFileAsImage" | "OpenImageViewer" => return Action::OpenTextViewer, // fallback until image viewer exists
            "NavigateToRoot" => return Action::CursorHome,
            "PreviousTab" => return Action::PrevTab,
            "JumpToPath" => return Action::ShowJumpToPathDialog,
            "JumpToFile" => return Action::ShowJumpToFileDialog,
            "ExitApplicationAndChangeDirectory" => return Action::ExitAndChangeDirectory,
            "ShowJobManager" => return Action::JobManager,
            "RegisterCurrentDirectory" => return Action::RegisterCurrentFolder,
            "ToggleMarkAndMoveUp" => return Action::ToggleMarkUp,
            "ShowSortDialog" => return Action::OpenSortDialog,
            "MarkRange" => return Action::RangeMarking,
            "FileMask" => return Action::FileMaskFilter,
            "WildcardMark" => return Action::WildcardMarking,
            "ResizeTaskPanelUp" | "ResizeTaskPaneUp" => return Action::IncreaseTaskPanelHeight,
            "ResizeTaskPanelDown" | "ResizeTaskPaneDown" => return Action::DecreaseTaskPanelHeight,
            _ => {}
        }
        // 3. Unknown → treated as a custom function name (or menu name)
        Action::InvokeCustomFunction(s.to_string())
    }

    /// Convert a viewer action name string to an `Action`.
    fn parse_viewer_action_name(s: &str) -> Action {
        // Try full enum variant names first (e.g. "ViewerScrollDown", "ViewerClose")
        // so that keybindings.json can use the same names as the Action enum.
        if let Ok(action) =
            serde_json::from_value::<Action>(serde_json::Value::String(s.to_string()))
        {
            return action;
        }
        // Then try short TWF-style aliases for backward compatibility.
        match s {
            "FindNext" | "FindPrevious" => Action::ViewerFindNext,
            "FindPrev" => Action::ViewerFindPrev,
            "Search" => Action::ViewerBeginSearch,
            "StartForwardSearch" => Action::ViewerBeginSearch,
            "StartBackwardSearch" => Action::ViewerBeginSearchBackward,
            "GoToFileTop" => Action::ViewerGoToTop,
            "GoToFileBottom" => Action::ViewerGoToBottom,
            "GoToLineStart" => Action::ViewerScrollLeft,
            "GoToLineEnd" => Action::ViewerScrollRight,
            "PageUp" => Action::ViewerPageUp,
            "PageDown" => Action::ViewerPageDown,
            "ToggleHexMode" => Action::ViewerToggleHexMode,
            "CycleEncoding" => Action::ViewerCycleEncoding,
            "Close" => Action::ViewerClose,
            _ => Action::InvokeCustomFunction(s.to_string()),
        }
    }

    /// Convert a leap mode action name string to an `Action`.
    fn parse_leap_action_name(s: &str) -> Action {
        if let Ok(action) =
            serde_json::from_value::<Action>(serde_json::Value::String(s.to_string()))
        {
            return action;
        }
        Action::InvokeCustomFunction(s.to_string())
    }

    /// Look up an action for the given key string in leap mode.
    pub fn lookup_leap_action(&self, key_string: &str) -> Option<Action> {
        self.leap_mode.get(key_string).cloned()
    }

    /// Save key bindings to a JSON file
    pub fn save_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Map a key event to an action, handling multi-key sequences
    pub fn map_key(&mut self, event: &KeyEvent) -> Option<Action> {
        let key_string = format_key_event(event);

        // Check if we're in a multi-key sequence
        if let Some(prefix) = &self.pending_sequence {
            let full_key = format!("{}+{}", prefix, key_string);

            // Try to find the full sequence
            if let Some(action) = self.normal_mode.get(&full_key) {
                self.pending_sequence = None;
                return Some(action.clone());
            }

            // Sequence not found, clear pending
            self.pending_sequence = None;
            return None;
        }

        // Check for direct match
        if let Some(action) = self.normal_mode.get(&key_string) {
            return Some(action.clone());
        }

        // Check if this starts a multi-key sequence
        let potential_sequences: Vec<_> = self
            .normal_mode
            .keys()
            .filter(|k| k.starts_with(&format!("{}+", key_string)))
            .collect();

        if !potential_sequences.is_empty() {
            self.pending_sequence = Some(key_string);
            return Some(Action::PendingSequence);
        }

        None
    }

    /// Check if we're waiting for the next key in a sequence
    pub fn has_pending_sequence(&self) -> bool {
        self.pending_sequence.is_some()
    }

    /// Get the pending sequence prefix for visual feedback
    pub fn get_pending_sequence(&self) -> Option<&str> {
        self.pending_sequence.as_deref()
    }

    /// Clear any pending sequence
    pub fn clear_pending_sequence(&mut self) {
        self.pending_sequence = None;
    }

    /// Returns action-name → sorted key list for NormalMode.
    /// `InvokeCustomFunction(name)` entries use the function name as the key.
    /// `PendingSequence` and `CountDownJob` are excluded.
    pub fn normal_action_to_keys(&self) -> HashMap<String, Vec<String>> {
        Self::invert_bindings(&self.normal_mode)
    }

    /// Returns action-name → sorted key list for ViewerMode.
    pub fn viewer_action_to_keys(&self) -> HashMap<String, Vec<String>> {
        Self::invert_bindings(&self.viewer_mode)
    }

    /// Returns action-name → sorted key list for DialogMode.
    pub fn dialog_action_to_keys(&self) -> HashMap<String, Vec<String>> {
        Self::invert_bindings(&self.dialog_mode)
    }

    /// Returns action-name → sorted key list for LeapMode.
    pub fn leap_action_to_keys(&self) -> HashMap<String, Vec<String>> {
        Self::invert_bindings(&self.leap_mode)
    }

    fn invert_bindings(mode: &HashMap<String, Action>) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (key, action) in mode {
            match action {
                Action::PendingSequence | Action::CountDownJob(_) => continue,
                Action::InvokeCustomFunction(name) => {
                    map.entry(name.clone()).or_default().push(key.clone());
                }
                _ => {
                    map.entry(format!("{:?}", action))
                        .or_default()
                        .push(key.clone());
                }
            }
        }
        for keys in map.values_mut() {
            keys.sort();
        }
        map
    }
}

/// Actions that can be triggered by key bindings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Action {
    // Navigation
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
    MoveCursorToFirst,
    MoveCursorToLast,
    PageUp,
    PageDown,
    EnterDirectory,
    ParentDirectory,
    NavigateToParent,
    SwitchPane,
    SwitchToLeftPane,
    SwitchToRightPane,
    HistoryBack,
    HistoryForward,
    ShowHistoryDialog,

    // File Operations
    Copy,
    Move,
    Delete,
    DeleteForce,
    EmptyTrash,
    Rename,
    PatternRename,
    CreateDirectory,
    CreateFile,
    ShowAttrTimestampDialog,
    ShowCreateLinkDialog,

    // Marking
    ToggleMark,
    ToggleMarkUp,
    MarkAll,
    UnmarkAll,
    ClearMarks,
    WildcardMarking,
    RangeMarking,
    InvertMarks,

    // Sorting
    SortByName,
    SortBySize,
    SortByDate,
    SortByExtension,
    CycleSortMode,
    ToggleSortOrder,
    OpenSortDialog,

    // Search and Filter
    StartSearch,
    FileMaskFilter,
    ClearSearchFilter,
    ExitSearchMode,
    NextMatch,
    PrevMatch,

    // View
    ChangeDisplayMode(u8),
    DisplayModeDetailed,
    DisplayMode1,
    DisplayMode2,
    DisplayMode3,
    DisplayMode4,
    DisplayMode5,
    DisplayMode6,
    DisplayMode7,
    DisplayMode8,
    ToggleHidden,
    Refresh,

    // Tabs
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    TabSelector,

    // Registered Folders
    RegisterCurrentFolder,
    ShowRegisteredFolderDialog,
    MoveToRegisteredFolder,

    // Jump navigation
    ShowJumpToPathDialog,
    ShowJumpToFileDialog,

    // Viewer (open from normal mode)
    OpenTextViewer,
    OpenHexViewer,

    // Viewer mode actions (used in ViewerMode key bindings)
    ViewerClose,
    ViewerToggleHexMode,
    ViewerScrollDown,
    ViewerScrollUp,
    ViewerPageDown,
    ViewerPageUp,
    ViewerGoToTop,
    ViewerGoToBottom,
    ViewerCycleEncoding,
    ViewerBeginSearch,
    ViewerBeginSearchBackward,
    ViewerFindNext,
    ViewerFindPrev,
    ViewerToggleCaseSensitive,
    ViewerClearSearch,
    ViewerScrollLeft,
    ViewerScrollRight,
    ViewerFastScrollUp,
    ViewerFastScrollDown,
    ViewerFastScrollLeft,
    ViewerFastScrollRight,

    // Miscellaneous
    Quit,
    ExitAndChangeDirectory,
    Help,
    RotateHelpLanguage,
    JobManager,
    CalculateDirectorySize,

    // Pane operations
    SyncPanes,
    SwapPanes,

    // Context menu, drive selection, custom functions
    ShowContextMenu,
    ShowDriveChangeDialog,
    ShowCustomFunctionsDialog,

    // Information dialogs
    ShowFileInfoForCursor,
    OpenWithEditor, // open cursor file with Editor from config
    OpenWithSystem, // open cursor entry with OS default association (Ctrl+Enter)
    OpenWith, // open cursor entry via ExtensionAssociation lookup, picker if 2+ matches (Phase 7.3)
    ShowVersion,
    ReloadConfig,
    ShowVersionInfo,        // compact version/system info (backtick key)
    ShowVersionInfoVerbose, // verbose version/system info including config file status (F2)
    SaveLog,
    EditConfigFile,

    // Task panel operations
    ToggleTaskPanel,
    IncreaseTaskPanelHeight,
    DecreaseTaskPanelHeight,
    ScrollTaskPanelUp,
    ScrollTaskPanelDown,

    // Archive operations
    Compress,
    Extract,

    // Test jobs
    CountDownJob(u32), // Countdown test job (parameter: duration in seconds, 0 = default 180)

    // Leap Navigation (F3 mode)
    EnterLeap,
    LeapGoDeeperOrOpen, // Right: enter dir (append "/") or select file
    LeapGoParent,       // Left: go to parent (strip local+"/")
    LeapCursorUp,
    LeapCursorDown,
    LeapClearLocal, // Ctrl+U: clear local filter
    LeapClearAll,   // Ctrl+K: clear all, return to leap root
    LeapConfirm,    // F3 again: exit leap, keep cursor
    LeapCancel,     // Escape: exit leap, restore pre-leap state
    LeapOpenFile,   // Enter: open file or enter dir

    // Internal
    PendingSequence,

    /// Invoke a custom function (or built-in action) by name resolved at runtime.
    /// Used when keybindings.json maps a key to a name that isn't a known Action variant.
    InvokeCustomFunction(String),
}

/// Format a key event as a string for key binding lookup
pub fn format_key_event(event: &KeyEvent) -> String {
    let mut parts = Vec::new();

    // For Char keys, never add "Shift+" — the character value already encodes shift state
    // (e.g., '?' is already the shifted '/', 'A' is already the shifted 'a').
    // For non-Char keys (F-keys, arrows, etc.), include "Shift+" when the modifier is held.
    let is_char = matches!(event.code, KeyCode::Char(_));

    if event.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    // Space has no shifted form, so Shift+Space must be distinguished explicitly
    let is_space = matches!(event.code, KeyCode::Char(' '));
    if event.modifiers.contains(KeyModifiers::SHIFT) && (!is_char || is_space) {
        parts.push("Shift");
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt");
    }

    let key = match event.code {
        KeyCode::Char(c) => {
            if c == ' ' {
                "Space".to_string()
            } else if c.is_ascii_alphabetic() && event.modifiers.contains(KeyModifiers::SHIFT) {
                // crossterm on Windows may send lowercase + SHIFT; normalise to uppercase.
                c.to_ascii_uppercase().to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Esc => "Escape".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => return String::new(),
    };

    parts.push(&key);
    parts.join("+")
}

// ── Duplicate key detection ───────────────────────────────────────────────────

/// A serde visitor that deserializes a JSON object but records any key that
/// appears more than once, along with the first and winning (last) action values.
/// Because serde_json streams tokens one at a time, `MapAccess::next_key` is
/// called for every key in the raw JSON — including duplicates — before any
/// HashMap deduplication occurs.
///
/// Each entry is `(key, first_action, winning_action)`.
struct DupMap(Vec<(String, String, String)>);

impl<'de> serde::Deserialize<'de> for DupMap {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = DupMap;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a map of key bindings")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<DupMap, A::Error> {
                let mut seen = std::collections::HashMap::<String, String>::new();
                let mut dupes = Vec::<(String, String, String)>::new();
                while let Some(key) = map.next_key::<String>()? {
                    let value: String = map.next_value()?;
                    if let Some(first) = seen.get(&key) {
                        dupes.push((key.clone(), first.clone(), value.clone()));
                    } else {
                        seen.insert(key, value);
                    }
                }
                Ok(DupMap(dupes))
            }
        }
        de.deserialize_map(V)
    }
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct KeybindingsForDupCheck {
    #[serde(rename = "NormalMode")]
    normal_mode: Option<DupMap>,
    #[serde(rename = "ViewerMode")]
    viewer_mode: Option<DupMap>,
    #[serde(rename = "DialogMode")]
    dialog_mode: Option<DupMap>,
    #[serde(rename = "SearchMode")]
    search_mode: Option<DupMap>,
    #[serde(rename = "LeapMode")]
    leap_mode: Option<DupMap>,
    #[serde(rename = "bindings")]
    bindings: Option<DupMap>,
}

/// Scan keybindings JSON content for duplicate keys within each mode section.
/// Returns one warning string per duplicate found; empty when the content is clean.
pub fn check_keybindings_content_duplicates(content: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let Ok(check) = serde_json::from_str::<KeybindingsForDupCheck>(content) else {
        return warnings; // JSON syntax errors are reported separately by load_from_file
    };
    let sections = [
        ("NormalMode", check.normal_mode),
        ("ViewerMode", check.viewer_mode),
        ("DialogMode", check.dialog_mode),
        ("SearchMode", check.search_mode),
        ("LeapMode", check.leap_mode),
        ("bindings", check.bindings),
    ];
    for (name, maybe_map) in sections {
        if let Some(DupMap(dupes)) = maybe_map {
            for (key, first, winner) in dupes {
                warnings.push(format!(
                    "[WARN] keybindings.json {name}: key '{key}' bound twice \
                     — '{first}' overridden by '{winner}'"
                ));
            }
        }
    }
    warnings
}

/// Scan a keybindings JSON file for duplicate keys within each mode section.
pub fn check_keybindings_duplicates(path: &std::path::Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    check_keybindings_content_duplicates(&content)
}

fn delete_job_name(targets: &[Location]) -> String {
    let file_name = |loc: &Location| -> String {
        loc.path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| loc.display_path())
    };
    match targets.len() {
        0 => "Delete".to_string(),
        1 => format!("Delete '{}'", file_name(&targets[0])),
        2 => format!(
            "Delete '{}', '{}'",
            file_name(&targets[0]),
            file_name(&targets[1])
        ),
        n => format!(
            "Delete {} files: '{}', '{}'...",
            n,
            file_name(&targets[0]),
            file_name(&targets[1])
        ),
    }
}

/// Expand an `ExtensionAssociation`'s command via `MacroExpander`, returning
/// `(command, working_dir, shell)` on success. Shared by
/// `resolve_extension_association`'s single-match branch and the Open With
/// picker's Confirm handler (`rwf-bin/src/ui/dialog/confirm.rs`) so the two
/// expansion call sites can't drift apart (Phase 7.3).
pub fn expand_association_command(
    state: &AppState,
    assoc: &crate::config::ExtensionAssociation,
) -> Result<(String, Location, Option<String>), String> {
    let expander = crate::macro_expander::MacroExpander::new();
    let func = crate::model::dialog::CustomFunction::new("open", &assoc.command);
    let func = if let Some(ref shell) = assoc.shell {
        func.with_shell(shell)
    } else {
        func
    };
    let command = expander.expand(state, &func)?;
    let working_dir = state.active_pane().current_location.clone();
    let shell = assoc.shell.clone();
    Ok((command, working_dir, shell))
}

/// Resolve the pure-extension `ExtensionAssociation`(s) whose extension matches
/// `ext_lower` (already lowercased, no leading dot). "Pure-extension" means
/// `file_type.is_none()` — a `FileType`-bearing entry can't be validated without
/// magic-byte detection, so it's excluded here (Phase 7.3b). Shared by:
/// - `resolve_extension_association`'s flag-off / non-Local branch,
/// - `candidates_for`'s fallback step (once a detected kind is known but no
///   FileType entry matched it),
/// - the `DetectFileTypesBatch` completion handler's failure path.
pub fn candidates_for_extension(
    state: &AppState,
    ext_lower: &str,
) -> Vec<crate::config::ExtensionAssociation> {
    state
        .extension_associations
        .iter()
        .filter(|a| {
            a.file_type.is_none()
                && a.extension
                    .as_deref()
                    .map(|e| e.trim_start_matches('.').to_lowercase() == ext_lower)
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Resolve `ExtensionAssociation` candidates using both the detected content
/// type and the extension (Phase 7.3b detect-then-resolve pipeline). Shared by
/// the `DetectFileType { ResolveAssociation }` completion handler (single
/// cursor-file / Open With flow) and the `DetectFileTypesBatch` completion
/// handler (batch "Open With..." flow), so the two resolution call sites can't
/// drift apart on the FileType-first/extension-fallback/AND rules.
///
/// - `kind != Unknown`: entries whose `file_type` matches `kind` (via
///   `DetectedKind::matches_file_type_spec`) AND whose `extension` is either
///   unset or matches `ext_lower` are preferred. If that set is non-empty, it's
///   returned as-is.
/// - Otherwise (no FileType match, or `kind == Unknown`): falls back to
///   pure-extension entries via `candidates_for_extension`.
pub fn candidates_for(
    state: &AppState,
    kind: crate::magic::DetectedKind,
    ext_lower: &str,
) -> Vec<crate::config::ExtensionAssociation> {
    if kind != crate::magic::DetectedKind::Unknown {
        let type_matches: Vec<_> = state
            .extension_associations
            .iter()
            .filter(|a| {
                a.file_type
                    .as_deref()
                    .map(|ft| kind.matches_file_type_spec(ft))
                    .unwrap_or(false)
                    && a.extension
                        .as_deref()
                        .map(|e| e.trim_start_matches('.').to_lowercase() == ext_lower)
                        .unwrap_or(true)
            })
            .cloned()
            .collect();
        if !type_matches.is_empty() {
            return type_matches;
        }
    }
    candidates_for_extension(state, ext_lower)
}

/// Resolve the `ExtensionAssociation`(s) potentially matching `entry` and produce
/// the Transition(s) that should follow (Phase 7.3 / 7.3b).
///
/// Two modes, chosen by `magic_byte_detection_enabled` and whether `entry`'s
/// location is `Location::Local`:
///
/// - **Detect-then-resolve** (flag on, Local location): resolution can't happen
///   here — it needs the detected content type, which requires an async job.
///   Instead, this does a cheap pre-check: does *any* `ExtensionAssociation`
///   entry have a chance of matching this file (any entry with `file_type` set,
///   since we don't yet know the kind, OR any entry whose `extension` matches)?
///   If not, return `None` immediately — zero detection cost for the common
///   unassociated file, preserving the FileTypeMapping/viewer fallthrough. If
///   so, return `ResolveAssociationByType`, deferring actual candidate
///   resolution to the `DetectFileType` completion handler
///   (`state/handlers/job.rs`), which has the detected kind in hand.
/// - **Extension-only** (flag off, or non-Local location): unchanged pre-7.3b
///   behavior, using `candidates_for_extension` (pure-extension entries only —
///   a `FileType`-bearing entry can't be validated without detection on this
///   path). No match: `None`. Exactly one match: expand its command and build
///   `ExecuteAssociationChecked`. Two or more matches: `ShowOpenWithPicker`.
fn resolve_extension_association(
    state: &AppState,
    entry: &crate::model::FileEntry,
) -> Option<Vec<Transition>> {
    let ext_lower = entry
        .extension()
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if state.config.magic_byte_detection_enabled
        && matches!(entry.location, crate::model::Location::Local(_))
    {
        let could_match = state.extension_associations.iter().any(|a| {
            a.file_type.is_some()
                || a.extension
                    .as_deref()
                    .map(|e| e.trim_start_matches('.').to_lowercase() == ext_lower)
                    .unwrap_or(false)
        });
        return if could_match {
            debug!(
                "resolve_extension_association: at least one association could match {}, deferring to detect-then-resolve",
                entry.location.display_path()
            );
            Some(vec![Transition::ResolveAssociationByType {
                location: entry.location.clone(),
            }])
        } else {
            None
        };
    }

    if ext_lower.is_empty() {
        return None;
    }
    let candidates = candidates_for_extension(state, &ext_lower);

    match candidates.len() {
        0 => None,
        1 => {
            let assoc = &candidates[0];
            match expand_association_command(state, assoc) {
                Ok((command, working_dir, shell)) => {
                    debug!(
                        "resolve_extension_association: running association command: {}",
                        command
                    );
                    Some(vec![Transition::ExecuteAssociationChecked {
                        path: entry.location.display_path().into(),
                        command,
                        working_dir,
                        shell,
                    }])
                }
                Err(_) => {
                    debug!("resolve_extension_association: association command expansion failed, falling back to viewer");
                    Some(vec![Transition::OpenTextViewer {
                        location: entry.location.clone(),
                    }])
                }
            }
        }
        _ => {
            debug!(
                "resolve_extension_association: {} candidates match extension '{}', showing picker",
                candidates.len(),
                ext_lower
            );
            Some(vec![Transition::ShowOpenWithPicker {
                candidates,
                paths: vec![entry.location.display_path().into()],
            }])
        }
    }
}

/// Map an action to state transitions
pub fn action_to_transitions(state: &AppState, action: &Action) -> Vec<Transition> {
    match action {
        Action::CursorUp => vec![Transition::CursorMove {
            pane: state.ui.active_pane,
            delta: -1,
        }],
        Action::CursorDown => vec![Transition::CursorMove {
            pane: state.ui.active_pane,
            delta: 1,
        }],
        Action::CursorHome => vec![Transition::CursorJump {
            pane: state.ui.active_pane,
            position: 0,
        }],
        Action::MoveCursorToFirst => vec![Transition::CursorJump {
            pane: state.ui.active_pane,
            position: 0,
        }],
        Action::CursorEnd => {
            let pane = state.active_pane();
            let last = pane.entries.len().saturating_sub(1);
            vec![Transition::CursorJump {
                pane: state.ui.active_pane,
                position: last,
            }]
        }
        Action::MoveCursorToLast => {
            let pane = state.active_pane();
            let last = pane.entries.len().saturating_sub(1);
            vec![Transition::CursorJump {
                pane: state.ui.active_pane,
                position: last,
            }]
        }
        Action::PageUp => vec![Transition::CursorMove {
            pane: state.ui.active_pane,
            delta: -20,
        }],
        Action::PageDown => vec![Transition::CursorMove {
            pane: state.ui.active_pane,
            delta: 20,
        }],
        Action::SwitchPane => vec![Transition::SwitchPane],
        Action::SwitchToLeftPane => {
            // Only switch if not already on left pane
            if state.ui.active_pane != crate::model::ActivePane::Left {
                vec![Transition::SwitchPane]
            } else {
                vec![]
            }
        }
        Action::SwitchToRightPane => {
            // Only switch if not already on right pane
            if state.ui.active_pane != crate::model::ActivePane::Right {
                vec![Transition::SwitchPane]
            } else {
                vec![]
            }
        }
        Action::EnterDirectory => {
            if let Some(entry) = state.active_pane().current_entry() {
                debug!(
                    "EnterDirectory: entry = {}, is_dir = {}",
                    entry.name, entry.is_dir
                );

                if entry.is_dir {
                    // Enter directory
                    debug!(
                        "EnterDirectory: entering directory {}",
                        entry.location.display_path()
                    );
                    vec![Transition::ChangeLocation {
                        pane: state.ui.active_pane,
                        location: entry.location.clone(),
                    }]
                } else {
                    // Check if this is an archive file
                    use crate::backend::MultiFormatArchiveHandler;
                    let handler = MultiFormatArchiveHandler::new();
                    if handler.is_archive(&entry.name) {
                        debug!("EnterDirectory: entering archive {}", entry.name);
                        // Enter archive as virtual folder
                        let archive_location = Location::Archive {
                            archive_path: Box::new(entry.location.clone()),
                            inner_path: std::path::PathBuf::new(),
                        };
                        vec![Transition::ChangeLocation {
                            pane: state.ui.active_pane,
                            location: archive_location,
                        }]
                    } else if let Some(transitions) = resolve_extension_association(state, entry) {
                        // Check for file-type extension association (Phase 6.2 / 7.3)
                        transitions
                    } else if let Some(action) = entry.extension().and_then(|ext| {
                        let ext_lower = ext.to_lowercase();
                        state
                            .file_type_map
                            .iter()
                            .find(|m| {
                                m.extension.trim_start_matches('.').to_lowercase() == ext_lower
                            })
                            .and_then(|m| {
                                m.actions
                                    .iter()
                                    .find(|a| **a != crate::config::FileOpenAction::Unknown)
                            })
                    }) {
                        match action {
                            crate::config::FileOpenAction::OsDefault => {
                                debug!(
                                    "EnterDirectory: opening {} via OS default association",
                                    entry.name
                                );
                                vec![Transition::OpenWithSystem {
                                    path: entry.location.display_path(),
                                }]
                            }
                            crate::config::FileOpenAction::Unknown => {
                                unreachable!("filtered out by the .find() above")
                            }
                        }
                    } else if state.config.magic_byte_detection_enabled {
                        debug!(
                            "EnterDirectory: no association/mapping for {}, detecting content type",
                            entry.name
                        );
                        vec![Transition::CheckFallbackFileType {
                            location: entry.location.clone(),
                        }]
                    } else {
                        debug!(
                            "EnterDirectory: no association/mapping for {} and magic-byte detection disabled, opening in text viewer",
                            entry.name
                        );
                        vec![Transition::OpenTextViewer {
                            location: entry.location.clone(),
                        }]
                    }
                }
            } else {
                vec![]
            }
        }
        Action::OpenWith => {
            let pane = state.active_pane();
            let marked: Vec<_> = pane
                .entries
                .iter()
                .filter(|e| pane.marking.is_marked(&e.location))
                .map(|e| e.location.clone())
                .collect();

            if marked.len() >= 2 {
                // Batch flow (Phase 7.3 §3): detect all marked files' content types,
                // group by (DetectedKind, extension), and route each group from the
                // DetectFileTypesBatch completion handler. Exactly 1 or 0 marked falls
                // through below to the ordinary single cursor-file flow.
                let paths: Vec<std::path::PathBuf> =
                    marked.iter().map(|loc| loc.display_path().into()).collect();
                vec![Transition::StartBatchOpenWith { paths }]
            } else if let Some(entry) = pane.current_entry() {
                resolve_extension_association(state, entry).unwrap_or_default()
            } else {
                vec![]
            }
        }
        Action::OpenWithSystem => {
            if let Some(entry) = state.active_pane().current_entry() {
                if entry.is_dir {
                    vec![Transition::ChangeLocation {
                        pane: state.ui.active_pane,
                        location: entry.location.clone(),
                    }]
                } else {
                    use crate::backend::MultiFormatArchiveHandler;
                    let handler = MultiFormatArchiveHandler::new();
                    if handler.is_archive(&entry.name) {
                        let archive_location = Location::Archive {
                            archive_path: Box::new(entry.location.clone()),
                            inner_path: std::path::PathBuf::new(),
                        };
                        vec![Transition::ChangeLocation {
                            pane: state.ui.active_pane,
                            location: archive_location,
                        }]
                    } else {
                        vec![Transition::OpenWithSystem {
                            path: entry.location.display_path(),
                        }]
                    }
                }
            } else {
                vec![]
            }
        }
        Action::ParentDirectory => vec![Transition::NavigateUp {
            pane: state.ui.active_pane,
        }],
        Action::NavigateToParent => vec![Transition::NavigateUp {
            pane: state.ui.active_pane,
        }],
        Action::HistoryBack => vec![Transition::NavigateHistory {
            pane: state.ui.active_pane,
            direction: crate::state::HistoryDirection::Back,
        }],
        Action::HistoryForward => vec![Transition::NavigateHistory {
            pane: state.ui.active_pane,
            direction: crate::state::HistoryDirection::Forward,
        }],
        Action::ShowHistoryDialog => {
            let active_pane = state.ui.active_pane;
            let tab_index = state.tabs.active_index;
            let tab = state.current_tab();

            let build_entries =
                |pane: crate::model::ui::ActivePane| -> (Vec<crate::model::Location>, usize) {
                    let (stack, _) = tab.history.stack_and_pos(pane);
                    let current_loc = match pane {
                        crate::model::ui::ActivePane::Left => {
                            tab.left_pane.current_location.clone()
                        }
                        crate::model::ui::ActivePane::Right => {
                            tab.right_pane.current_location.clone()
                        }
                    };
                    let mut entries = stack.to_vec();
                    if entries.last() != Some(&current_loc) {
                        entries.push(current_loc);
                    }
                    let pos = entries.len().saturating_sub(1);
                    (entries, pos)
                };

            let (left_entries, left_pos) = build_entries(crate::model::ui::ActivePane::Left);
            let (right_entries, right_pos) = build_entries(crate::model::ui::ActivePane::Right);

            if left_entries.is_empty() && right_entries.is_empty() {
                vec![]
            } else {
                vec![Transition::ShowDialog {
                    dialog: crate::model::Dialog::history_dialog(
                        tab_index,
                        active_pane,
                        left_entries,
                        left_pos,
                        right_entries,
                        right_pos,
                    ),
                }]
            }
        }
        Action::ToggleMark => {
            if let Some(entry) = state.active_pane().current_entry() {
                vec![
                    Transition::ToggleMark {
                        location: entry.location.clone(),
                    },
                    Transition::CursorMove {
                        pane: state.ui.active_pane,
                        delta: 1,
                    },
                ]
            } else {
                vec![]
            }
        }
        Action::ToggleMarkUp => {
            if let Some(entry) = state.active_pane().current_entry() {
                vec![
                    Transition::ToggleMark {
                        location: entry.location.clone(),
                    },
                    Transition::CursorMove {
                        pane: state.ui.active_pane,
                        delta: -1,
                    },
                ]
            } else {
                vec![]
            }
        }
        Action::MarkAll => vec![Transition::MarkAll],
        Action::UnmarkAll => vec![Transition::UnmarkAll],
        Action::ClearMarks => vec![Transition::UnmarkAll],
        Action::InvertMarks => vec![Transition::InvertMarks],
        Action::WildcardMarking => {
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::wildcard_mark(),
            }]
        }
        Action::RangeMarking => {
            // Check if we're already in range marking mode
            if let Some(start) = state.ui.range_marking_start {
                // We're in range marking mode, so mark the range
                let end = state.active_pane().cursor;
                vec![Transition::MarkRange { start, end }]
            } else {
                // Enter range marking mode
                vec![Transition::EnterRangeMarkingMode]
            }
        }
        Action::SortByName => vec![Transition::ChangeSortMode {
            pane: state.ui.active_pane,
            mode: crate::model::SortMode::Name,
        }],
        Action::SortBySize => vec![Transition::ChangeSortMode {
            pane: state.ui.active_pane,
            mode: crate::model::SortMode::Size,
        }],
        Action::SortByDate => vec![Transition::ChangeSortMode {
            pane: state.ui.active_pane,
            mode: crate::model::SortMode::Date,
        }],
        Action::SortByExtension => vec![Transition::ChangeSortMode {
            pane: state.ui.active_pane,
            mode: crate::model::SortMode::Extension,
        }],
        Action::CycleSortMode => {
            // Cycle through sort modes: Name -> Size -> Date -> Extension -> Name
            let current_mode = state.active_pane().sort_mode;
            let next_mode = match current_mode {
                crate::model::SortMode::Name => crate::model::SortMode::Size,
                crate::model::SortMode::Size => crate::model::SortMode::Date,
                crate::model::SortMode::Date => crate::model::SortMode::Extension,
                crate::model::SortMode::Extension => crate::model::SortMode::Name,
            };
            vec![Transition::ChangeSortMode {
                pane: state.ui.active_pane,
                mode: next_mode,
            }]
        }
        Action::ToggleSortOrder => {
            let current_order = state.active_pane().sort_order;
            vec![Transition::ChangeSortOrder {
                pane: state.ui.active_pane,
                order: current_order.toggle(),
            }]
        }
        Action::OpenSortDialog => {
            let pane = state.active_pane();
            let dialog = crate::model::Dialog::sort_dialog(pane.sort_mode, pane.sort_order);
            vec![Transition::ShowDialog { dialog }]
        }
        Action::DisplayModeDetailed => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Detailed,
        }],
        Action::DisplayMode1 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(1),
        }],
        Action::DisplayMode2 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(2),
        }],
        Action::DisplayMode3 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(3),
        }],
        Action::DisplayMode4 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(4),
        }],
        Action::DisplayMode5 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(5),
        }],
        Action::DisplayMode6 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(6),
        }],
        Action::DisplayMode7 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(7),
        }],
        Action::DisplayMode8 => vec![Transition::ChangeDisplayMode {
            pane: state.ui.active_pane,
            mode: crate::model::DisplayMode::Columns(8),
        }],
        Action::NewTab => vec![Transition::CreateTab],
        Action::CloseTab => vec![Transition::CloseTab {
            index: state.tabs.active_index,
        }],
        Action::NextTab => vec![Transition::NextTab],
        Action::PrevTab => vec![Transition::PrevTab],
        Action::TabSelector => {
            // Create tab names for the selector dialog
            let tab_names: Vec<String> = state
                .tabs
                .tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| {
                    let left_path = tab.left_pane.current_location.display_path();
                    let right_path = tab.right_pane.current_location.display_path();
                    format!("Tab {}: {} | {}", i + 1, left_path, right_path)
                })
                .collect();

            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::tab_selector(tab_names),
            }]
        }
        Action::Copy => {
            let pane = state.active_pane();
            let active_marked: Vec<_> = pane
                .entries
                .iter()
                .filter(|e| pane.marking.is_marked(&e.location))
                .map(|e| e.location.clone())
                .collect();
            let sources = if !active_marked.is_empty() {
                active_marked
            } else if let Some(entry) = pane.current_entry() {
                vec![entry.location.clone()]
            } else {
                vec![]
            };

            if sources.is_empty() {
                return vec![];
            }

            let dest = state.opposite_pane().current_location.clone();

            // Create job spec with pending transition - will start after conflict check
            // User already pressed 'C' intentionally, progress shown in task pane
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Copy {
                sources: sources.clone(),
                dest,
            });

            vec![Transition::CreatePendingFileJob {
                spec: job_spec,
                name: format!("Copy ({} files)", sources.len()),
                description: format!("Copy {} files", sources.len()),
            }]
        }
        Action::Move => {
            let pane = state.active_pane();
            let active_marked: Vec<_> = pane
                .entries
                .iter()
                .filter(|e| pane.marking.is_marked(&e.location))
                .map(|e| e.location.clone())
                .collect();
            let sources = if !active_marked.is_empty() {
                active_marked
            } else if let Some(entry) = pane.current_entry() {
                vec![entry.location.clone()]
            } else {
                vec![]
            };

            if sources.is_empty() {
                return vec![];
            }

            let dest = state.opposite_pane().current_location.clone();

            // Create job spec with pending transition - will start after conflict check
            // User already pressed 'M' intentionally, progress shown in task pane
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Move {
                sources: sources.clone(),
                dest,
            });

            vec![Transition::CreatePendingFileJob {
                spec: job_spec,
                name: format!("Move ({} files)", sources.len()),
                description: format!("Move {} files", sources.len()),
            }]
        }
        Action::Delete => {
            let pane = state.active_pane();
            let active_marked: Vec<_> = pane
                .entries
                .iter()
                .filter(|e| pane.marking.is_marked(&e.location))
                .map(|e| (e.location.clone(), e.is_dir))
                .collect();
            let targets = if !active_marked.is_empty() {
                active_marked
            } else if let Some(entry) = pane.current_entry() {
                vec![(entry.location.clone(), entry.is_dir)]
            } else {
                vec![]
            };

            if targets.is_empty() {
                return vec![];
            }

            let to_trash = state.config.trash.enabled;
            let force_fallback = state.config.trash.force_fallback;

            if to_trash && !state.config.trash.confirm_before_move {
                let locations: Vec<_> = targets.iter().map(|(loc, _)| loc.clone()).collect();
                let name = delete_job_name(&locations);
                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::MoveToTrash {
                    targets: locations,
                    force_fallback,
                });
                return vec![Transition::CreateAndStartFileJob {
                    spec: job_spec,
                    name: name.clone(),
                    description: name,
                }];
            }

            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::delete_confirm(targets, to_trash, force_fallback),
            }]
        }
        Action::DeleteForce => {
            let pane = state.active_pane();
            let active_marked: Vec<_> = pane
                .entries
                .iter()
                .filter(|e| pane.marking.is_marked(&e.location))
                .map(|e| e.location.clone())
                .collect();
            let targets = if !active_marked.is_empty() {
                active_marked
            } else if let Some(entry) = pane.current_entry() {
                vec![entry.location.clone()]
            } else {
                vec![]
            };

            if targets.is_empty() {
                return vec![];
            }

            let name = delete_job_name(&targets);
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Delete {
                targets: targets.clone(),
            });

            vec![Transition::CreateAndStartFileJob {
                spec: job_spec,
                name: name.clone(),
                description: name,
            }]
        }
        Action::EmptyTrash => {
            let fallback_roots: Vec<std::path::PathBuf> = state
                .tabs
                .tabs
                .iter()
                .flat_map(|tab| {
                    [
                        &tab.left_pane.current_location,
                        &tab.right_pane.current_location,
                    ]
                })
                .filter_map(|loc| loc.path())
                .map(crate::backend::trash::volume_root)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();

            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::EmptyTrash {
                scope: crate::model::EmptyTrashScope::All,
                older_than_days: None,
                fallback_roots,
            });

            vec![Transition::CreateAndStartFileJob {
                spec: job_spec,
                name: "Empty trash".to_string(),
                description: "Empty trash".to_string(),
            }]
        }
        Action::Rename => {
            if let Some(entry) = state.active_pane().current_entry() {
                let current_name = entry.name.clone();
                vec![Transition::ShowDialog {
                    dialog: crate::model::Dialog::simple_rename(current_name),
                }]
            } else {
                vec![]
            }
        }
        Action::PatternRename => {
            // Show pattern rename dialog
            vec![Transition::ShowPatternRenameDialog]
        }
        Action::CreateDirectory => {
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::input("Create Directory", "Directory name:", ""),
            }]
        }
        Action::CreateFile => {
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::input("Create File", "File name:", ""),
            }]
        }
        Action::ShowAttrTimestampDialog => {
            vec![Transition::ShowAttrTimestampDialog]
        }
        Action::ShowCreateLinkDialog => {
            vec![Transition::ShowCreateLinkDialog]
        }
        Action::StartSearch => {
            // Enter search mode (integrated in pane info area)
            vec![
                Transition::ChangeUIMode {
                    mode: crate::model::UIMode::Search,
                },
                Transition::ClearSearch,
            ]
        }
        Action::FileMaskFilter => {
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::file_mask(state.active_pane().file_mask.as_deref()),
            }]
        }
        Action::ClearSearchFilter => {
            // Clear both search and filter
            let mut transitions = vec![];

            // Clear search if in search mode
            if state.ui.mode == crate::model::UIMode::Search {
                transitions.push(Transition::ClearSearch);
                transitions.push(Transition::ChangeUIMode {
                    mode: crate::model::UIMode::Normal,
                });
            }

            // Clear file mask if one is set
            if state.active_pane().file_mask.is_some() {
                transitions.push(Transition::SetFileMask {
                    pane: state.ui.active_pane,
                    mask: None,
                });
            }

            transitions
        }
        Action::ExitSearchMode => {
            // Exit search mode if currently in search mode
            if state.ui.mode == crate::model::UIMode::Search {
                vec![
                    Transition::ClearSearch,
                    Transition::ChangeUIMode {
                        mode: crate::model::UIMode::Normal,
                    },
                    Transition::CloseDialog,
                ]
            } else {
                vec![]
            }
        }
        Action::NextMatch => {
            // Move to next search result
            if state.ui.mode == crate::model::UIMode::Search {
                vec![Transition::NextSearchResult]
            } else {
                vec![]
            }
        }
        Action::PrevMatch => {
            // Move to previous search result
            if state.ui.mode == crate::model::UIMode::Search {
                vec![Transition::PrevSearchResult]
            } else {
                vec![]
            }
        }
        Action::Quit => {
            // If in search mode, exit search mode instead of quitting
            if state.ui.mode == crate::model::UIMode::Search {
                vec![
                    Transition::ClearSearch,
                    Transition::ChangeUIMode {
                        mode: crate::model::UIMode::Normal,
                    },
                    Transition::CloseDialog,
                ]
            } else {
                vec![Transition::Quit]
            }
        }
        Action::ExitAndChangeDirectory => {
            vec![Transition::ExitAndChangeDirectory]
        }
        Action::Help => {
            let lang = &state.config.help_language;
            let descriptions = crate::help_content::ActionDescriptions::load(lang);
            let entries = crate::help_content::build_help_entries(
                &state.config.key_bindings,
                &descriptions,
                &state.custom_functions,
                state.config.help_show_unbound,
                &state.config,
            );
            let mut dialog = crate::model::Dialog::help_with_language(lang);
            if let crate::model::DialogContent::Help(crate::model::HelpDialog {
                entries: ref mut e,
                show_unbound: ref mut u,
                ..
            }) = dialog.content
            {
                *e = entries;
                *u = state.config.help_show_unbound;
            }
            vec![Transition::ShowDialog { dialog }]
        }
        Action::RotateHelpLanguage => {
            // Rotate through available help languages
            // **Validates: Requirements 48.3**
            vec![Transition::RotateHelpLanguage]
        }
        Action::JobManager => {
            // Show job manager dialog
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::job_manager(),
            }]
        }
        Action::CalculateDirectorySize => {
            // Calculate directory size for the current cursor entry
            if let Some(entry) = state.active_pane().current_entry() {
                if entry.is_dir {
                    // Create a job to calculate directory size
                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::CalculateSize {
                        location: entry.location.clone(),
                    });

                    vec![Transition::EnqueueJob { spec: job_spec }]
                } else {
                    // Not a directory, do nothing
                    vec![]
                }
            } else {
                vec![]
            }
        }
        Action::CountDownJob(duration) => {
            // Create a countdown test job that starts immediately
            // We bypass the queue and start the job directly
            let duration_secs = if *duration > 0 { *duration } else { 180 };
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::CountDown {
                duration_secs,
                start_value: duration_secs,
            });
            // Create BackgroundJob for UI, enqueue, AND start immediately
            // All in one transition to ensure atomicity
            vec![Transition::CreateAndStartCountDownJob {
                spec: job_spec,
                name: format!("CountDownJob {}", duration_secs),
                description: "Countdown test job".to_string(),
            }]
        }
        Action::Refresh => {
            // Refresh the current pane by clearing cache and reloading directory
            vec![Transition::Refresh {
                pane: state.ui.active_pane,
            }]
        }
        Action::SyncPanes => {
            // Synchronize opposite pane to active pane's location
            vec![Transition::SyncPanes]
        }
        Action::SwapPanes => {
            // Swap the paths of left and right panes
            vec![Transition::SwapPanes]
        }
        Action::ShowContextMenu => {
            vec![Transition::ShowContextMenu]
        }
        Action::ShowCustomFunctionsDialog => {
            vec![Transition::ShowCustomFunctionsDialog]
        }
        Action::ShowDriveChangeDialog => {
            // Show drive selection dialog
            vec![Transition::ShowDriveChangeDialog]
        }
        Action::ShowFileInfoForCursor => {
            vec![Transition::ShowFileInfo]
        }
        Action::OpenWithEditor => {
            let pane = state.active_pane();
            if let Some(entry) = pane.current_entry() {
                let path = entry.location.display_path();
                vec![Transition::OpenWithEditor { path }]
            } else {
                vec![]
            }
        }
        Action::ShowVersion => {
            // Show version information dialog
            vec![Transition::ShowVersion]
        }
        Action::ReloadConfig => {
            vec![Transition::ReloadConfig]
        }
        Action::ShowVersionInfo | Action::ShowVersionInfoVerbose => {
            // Handled entirely at the app layer (writes to task panel, no state change)
            vec![]
        }
        Action::SaveLog => {
            // Save the current session log to file
            vec![Transition::SaveLog]
        }
        Action::EditConfigFile => {
            // Launch the configured editor with the configuration file
            vec![Transition::EditConfigFile]
        }
        Action::RegisterCurrentFolder => {
            let pane = state.active_pane();
            let loc = match pane.current_entry() {
                Some(entry) if entry.is_dir && entry.name != ".." => &entry.location,
                _ => &pane.current_location,
            };
            let path = loc.display_path();
            let name = loc
                .path()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| path.clone());
            vec![Transition::RegisterCurrentFolder { name, path }]
        }
        Action::ShowRegisteredFolderDialog => {
            // Show registered folder selector dialog
            vec![Transition::ShowRegisteredFolderDialog]
        }
        Action::MoveToRegisteredFolder => {
            if state.active_pane().marking.count() > 0 {
                vec![Transition::ShowRegisteredFolderDialog]
            } else {
                vec![]
            }
        }
        Action::ShowJumpToPathDialog => {
            vec![Transition::ShowJumpToPathDialog]
        }
        Action::ShowJumpToFileDialog => {
            vec![Transition::ShowJumpToFileDialog]
        }
        Action::OpenTextViewer => {
            if let Some(entry) = state.active_pane().current_entry() {
                if !entry.is_dir {
                    debug!("OpenTextViewer: opening text viewer for {}", entry.name);
                    vec![Transition::OpenTextViewer {
                        location: entry.location.clone(),
                    }]
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        Action::OpenHexViewer => {
            if let Some(entry) = state.active_pane().current_entry() {
                if !entry.is_dir {
                    debug!("OpenHexViewer: opening hex viewer for {}", entry.name);
                    vec![Transition::OpenHexViewer {
                        location: entry.location.clone(),
                    }]
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        Action::ToggleTaskPanel => {
            vec![Transition::ToggleTaskPanel]
        }
        Action::IncreaseTaskPanelHeight => {
            vec![Transition::IncreaseTaskPanelHeight]
        }
        Action::DecreaseTaskPanelHeight => {
            vec![Transition::DecreaseTaskPanelHeight]
        }
        Action::ScrollTaskPanelUp => {
            vec![Transition::ScrollTaskPanelUp]
        }
        Action::ScrollTaskPanelDown => {
            vec![Transition::ScrollTaskPanelDown]
        }
        Action::Compress => {
            debug!("Action::Compress triggered");
            let sources = {
                let pane = state.active_pane();
                if pane.marking.count() > 0 {
                    pane.entries
                        .iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.location.clone())
                        .collect()
                } else if let Some(entry) = pane.current_entry() {
                    debug!(
                        "No marked files, using current entry from active pane: {:?}",
                        entry.location
                    );
                    vec![entry.location.clone()]
                } else {
                    vec![]
                }
            };
            let sources = if sources.is_empty() {
                // Active pane is empty, try the opposite pane
                debug!("Active pane is empty, checking opposite pane");
                if let Some(entry) = state.opposite_pane().current_entry() {
                    debug!(
                        "Using current entry from opposite pane: {:?}",
                        entry.location
                    );
                    vec![entry.location.clone()]
                } else {
                    debug!("No files to compress - both panes have no current entry");
                    return vec![];
                }
            } else {
                sources
            };

            debug!("Creating compression dialog with {} file(s)", sources.len());
            // Show compression dialog
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::compression(
                    sources,
                    state.config.text_input.edit_mode,
                ),
            }]
        }
        Action::Extract => {
            // Get current entry under cursor
            if let Some(entry) = state.active_pane().current_entry() {
                // Check if it's an archive (by extension)
                if is_archive(&entry.location) {
                    let dest = state.opposite_pane().current_location.clone();
                    vec![Transition::ShowDialog {
                        dialog: crate::model::Dialog::extraction_confirm(
                            entry.location.clone(),
                            dest,
                            1, // file_count - will be determined when archive is opened
                        ),
                    }]
                } else {
                    vec![] // Not an archive
                }
            } else {
                vec![]
            }
        }
        Action::PendingSequence => vec![],
        Action::InvokeCustomFunction(name) => {
            vec![Transition::InvokeCustomFunctionByName { name: name.clone() }]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{test_state, FileEntryBuilder};

    #[test]
    fn test_default_key_bindings() {
        let bindings = KeyBindings::default();
        assert!(bindings.normal_mode.contains_key("Tab"));
        assert!(bindings.normal_mode.contains_key("Up"));
        assert!(bindings.normal_mode.contains_key("C"));
    }

    #[test]
    fn test_format_key_event() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(format_key_event(&event), "a");

        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(format_key_event(&event), "Ctrl+a");

        // Shift+alpha: uppercase char, no "Shift+" prefix
        let event = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(format_key_event(&event), "A");

        // Shift+non-alpha: char value encodes shift, no "Shift+" prefix
        let event = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(format_key_event(&event), "?");

        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(format_key_event(&event), "Enter");

        // Shift+non-char key: "Shift+" IS included
        let event = KeyEvent::new(KeyCode::F(1), KeyModifiers::SHIFT);
        assert_eq!(format_key_event(&event), "Shift+F1");
    }

    #[test]
    fn test_multi_key_sequence() {
        let mut bindings = KeyBindings::default();

        // First key in sequence
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::PendingSequence));
        assert!(bindings.has_pending_sequence());

        // Second key completes sequence
        let event = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::SortByName));
        assert!(!bindings.has_pending_sequence());
    }

    #[test]
    fn test_invalid_sequence() {
        let mut bindings = KeyBindings::default();

        // First key in sequence
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        bindings.map_key(&event);

        // Invalid second key
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, None);
        assert!(!bindings.has_pending_sequence());
    }

    #[test]
    fn test_normal_action_to_keys_inversion() {
        let bindings = KeyBindings::default();
        let map = bindings.normal_action_to_keys();

        // CursorUp is bound to "Up" and "k"
        let cursor_up = map.get("CursorUp").expect("CursorUp must be in map");
        assert!(cursor_up.contains(&"Up".to_string()));
        assert!(cursor_up.contains(&"k".to_string()));

        // Copy is bound to "C" and "c"
        let copy_keys = map.get("Copy").expect("Copy must be in map");
        assert!(copy_keys.contains(&"C".to_string()));
        assert!(copy_keys.contains(&"c".to_string()));

        // PendingSequence must NOT appear
        assert!(!map.contains_key("PendingSequence"));
        // CountDownJob must NOT appear
        assert!(!map.iter().any(|(k, _)| k.starts_with("CountDownJob")));
    }

    #[test]
    fn test_viewer_action_to_keys_inversion() {
        let bindings = KeyBindings::default();
        let map = bindings.viewer_action_to_keys();

        // ViewerScrollDown is bound to "Down" and "j"
        let keys = map
            .get("ViewerScrollDown")
            .expect("ViewerScrollDown must be in map");
        assert!(keys.contains(&"Down".to_string()));
        assert!(keys.contains(&"j".to_string()));
    }

    #[test]
    fn test_action_to_keys_sorted() {
        let bindings = KeyBindings::default();
        let map = bindings.normal_action_to_keys();

        // All key lists must be sorted
        for keys in map.values() {
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(*keys, sorted, "keys for an action must be sorted");
        }
    }

    #[test]
    fn test_embedded_defaults_has_basic_normal_mode_bindings() {
        // embedded_defaults() should have "Tab" → SwitchPane in NormalMode
        let bindings = KeyBindings::embedded_defaults();
        assert_eq!(bindings.normal_mode.get("Tab"), Some(&Action::SwitchPane));
        assert_eq!(bindings.normal_mode.get("?"), Some(&Action::Help));
    }

    #[test]
    fn test_embedded_defaults_has_leap_mode_bindings() {
        // F3 must enter Leap mode, and LeapMode must have its own bindings,
        // otherwise Leap Navigation is unreachable by default.
        let bindings = KeyBindings::embedded_defaults();
        assert_eq!(bindings.normal_mode.get("F3"), Some(&Action::EnterLeap));
        assert_eq!(bindings.leap_mode.get("F3"), Some(&Action::LeapConfirm));
        assert_eq!(bindings.leap_mode.get("Escape"), Some(&Action::LeapCancel));
        assert_eq!(
            bindings.leap_mode.get("Right"),
            Some(&Action::LeapGoDeeperOrOpen)
        );
        assert_eq!(bindings.leap_mode.get("Left"), Some(&Action::LeapGoParent));
        // "/" commits the current candidate and descends, same as Right — not "go parent".
        assert_eq!(
            bindings.leap_mode.get("/"),
            Some(&Action::LeapGoDeeperOrOpen)
        );
        assert_eq!(
            bindings.leap_mode.get("Ctrl+u"),
            Some(&Action::LeapClearLocal)
        );
        assert_eq!(
            bindings.leap_mode.get("Ctrl+k"),
            Some(&Action::LeapClearAll)
        );
        assert_eq!(bindings.leap_mode.get("Enter"), Some(&Action::LeapOpenFile));
    }

    #[test]
    fn test_load_from_str_starts_empty() {
        // load_from_str with only one binding should have exactly that binding
        let json = r#"{"NormalMode":{"x":"Quit"}}"#;
        let bindings = KeyBindings::load_from_str(json).unwrap();
        assert_eq!(bindings.normal_mode.len(), 1);
        assert_eq!(bindings.normal_mode.get("x"), Some(&Action::Quit));
        assert!(bindings.viewer_mode.is_empty());
    }

    #[test]
    fn open_with_system_on_directory_navigates_like_enter_directory() {
        let mut state = test_state();
        let entry = FileEntryBuilder::new("subdir").dir(true).build();
        let expected_location = entry.location.clone();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::OpenWithSystem);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeLocation { location, .. } => {
                assert_eq!(location, &expected_location)
            }
            other => panic!("expected ChangeLocation, got {:?}", other),
        }
    }

    #[test]
    fn open_with_system_on_archive_navigates_into_it() {
        let mut state = test_state();
        let entry = FileEntryBuilder::new("bundle.zip").dir(false).build();
        let archive_path = entry.location.clone();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::OpenWithSystem);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeLocation {
                location:
                    Location::Archive {
                        archive_path: ap,
                        inner_path,
                    },
                ..
            } => {
                assert_eq!(**ap, archive_path);
                assert_eq!(inner_path, &std::path::PathBuf::new());
            }
            other => panic!("expected archive ChangeLocation, got {:?}", other),
        }
    }

    #[test]
    fn open_with_system_on_plain_file_produces_open_with_system_transition() {
        let mut state = test_state();
        let entry = FileEntryBuilder::new("clip.mp4").dir(false).build();
        let expected_path = entry.location.display_path();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::OpenWithSystem);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::OpenWithSystem { path } => assert_eq!(path, &expected_path),
            other => panic!("expected OpenWithSystem, got {:?}", other),
        }
    }

    #[test]
    fn open_with_system_on_empty_pane_produces_no_transitions() {
        let state = test_state();
        let transitions = action_to_transitions(&state, &Action::OpenWithSystem);
        assert!(transitions.is_empty());
    }

    #[test]
    fn ctrl_enter_is_bound_to_open_with_system() {
        let bindings = KeyBindings::embedded_defaults();
        assert_eq!(
            bindings.normal_mode.get("Ctrl+Enter"),
            Some(&Action::OpenWithSystem)
        );
    }

    #[test]
    fn enter_directory_routes_mapped_extension_to_open_with_system() {
        let mut state = test_state();
        state.file_type_map = vec![crate::config::FileTypeMapping {
            extension: "mp4".to_string(),
            file_type: Some("video/mp4".to_string()),
            actions: vec![crate::config::FileOpenAction::OsDefault],
        }];
        let entry = FileEntryBuilder::new("clip.mp4").dir(false).build();
        let expected_path = entry.location.display_path();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::OpenWithSystem { path } => assert_eq!(path, &expected_path),
            other => panic!("expected OpenWithSystem, got {:?}", other),
        }
    }

    #[test]
    fn enter_directory_unmapped_extension_still_opens_internal_viewer() {
        // Phase 7.3 §6: the final fallback no longer jumps straight to the text
        // viewer — it first detects content type via `CheckFallbackFileType`.
        // The detect-job completion handler (see file_open_integration_tests.rs
        // fallback_open_unknown_kind_opens_text_viewer) is what actually reaches
        // the internal viewer once detection comes back Unknown.
        let mut state = test_state();
        state.file_type_map = vec![crate::config::FileTypeMapping {
            extension: "mp4".to_string(),
            file_type: None,
            actions: vec![crate::config::FileOpenAction::OsDefault],
        }];
        let entry = FileEntryBuilder::new("main.rs").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::CheckFallbackFileType { .. } => {}
            other => panic!("expected CheckFallbackFileType, got {:?}", other),
        }
    }

    #[test]
    fn enter_directory_never_auto_runs_executables() {
        // Phase 7.3 §6: this now routes through content-type detection first
        // (CheckFallbackFileType) rather than opening the text viewer directly.
        // The detect-job completion handler still never auto-runs the file —
        // a detected Pe binary routes to OpenWithSystem (OS default), not to
        // any form of direct execution; see fallback_open_known_binary_kind_opens_with_system
        // in file_open_integration_tests.rs.
        let mut state = test_state();
        // file_type_map deliberately has no "exe" entry (executables are excluded from
        // the default set by design) — this confirms the omission actually protects.
        state.file_type_map = Vec::new();
        let entry = FileEntryBuilder::new("setup.exe").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::CheckFallbackFileType { .. } => {}
            other => panic!("expected CheckFallbackFileType, got {:?}", other),
        }
    }

    #[test]
    fn enter_directory_extension_association_wins_over_file_type_map() {
        let mut state = test_state();
        state.file_type_map = vec![crate::config::FileTypeMapping {
            extension: "mp4".to_string(),
            file_type: None,
            actions: vec![crate::config::FileOpenAction::OsDefault],
        }];
        state.extension_associations = vec![crate::config::ExtensionAssociation {
            extension: Some("mp4".to_string()),
            file_type: None,
            command: "myplayer $F".to_string(),
            description: None,
            shell: None,
        }];
        let entry = FileEntryBuilder::new("clip.mp4").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        // Phase 7.3b: with magic-byte detection on (default) and a Local
        // location, resolution defers to the detect-then-resolve pipeline
        // instead of resolving synchronously — the actual "extension
        // association wins over file_type_map" behavior is exercised end-to-end
        // by file_open_integration_tests.rs's `extension_association_match_produces_execute_association`
        // and friends, which drive the DetectFileType job to completion. Here we
        // only need to confirm the extension-association pre-check still fires
        // (i.e. EnterDirectory doesn't skip straight to file_type_map).
        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ResolveAssociationByType { location } => {
                assert_eq!(location.display_path(), "/test/clip.mp4");
            }
            other => panic!("expected ResolveAssociationByType, got {:?}", other),
        }
    }

    #[test]
    fn enter_directory_skips_unrecognized_action_and_falls_to_next() {
        let mut state = test_state();
        state.file_type_map = vec![crate::config::FileTypeMapping {
            extension: "mp4".to_string(),
            file_type: None,
            actions: vec![
                crate::config::FileOpenAction::Unknown,
                crate::config::FileOpenAction::OsDefault,
            ],
        }];
        let entry = FileEntryBuilder::new("clip.mp4").dir(false).build();
        let expected_path = entry.location.display_path();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::OpenWithSystem { path } => assert_eq!(path, &expected_path),
            other => panic!(
                "expected OpenWithSystem (skipping Unknown), got {:?}",
                other
            ),
        }
    }
}

#[cfg(test)]
mod input_properties;

#[cfg(test)]
mod file_operations_tests;

#[cfg(test)]
mod marking_tests;

mod history_tests;
#[cfg(test)]
mod marking_wildcard_tests;
mod rename_tests;

#[cfg(test)]
mod drive_dialog_tests;

#[cfg(test)]
mod file_info_tests;

#[cfg(test)]
mod pattern_rename_dialog_tests;

#[cfg(test)]
mod help_dialog_tests;

#[cfg(test)]
mod registered_folder_tests;

#[cfg(test)]
mod jump_to_path_tests;

#[cfg(test)]
mod jump_to_file_tests;

#[cfg(test)]
mod sorting_tests;

#[cfg(test)]
mod file_mask_tests;

#[cfg(test)]
mod search_filter_tests;

#[cfg(test)]
mod tab_management_tests;

#[cfg(test)]
mod miscellaneous_tests;
