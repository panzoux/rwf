//! Key bindings and the `Action` vocabulary (moved down from `input` so `config`/
//! `config_layers`/`help_content` need not depend on it).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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
        Self::load_from_str(crate::defaults::DEFAULT_KEYBINDINGS)
            .expect("embedded default_keybindings.json is always valid JSON")
    }

    /// Parse key bindings from a JSON string, starting from an empty state.
    /// Does not merge over any defaults — the string must contain all desired bindings.
    /// Used by `embedded_defaults()` and for testing.
    pub fn load_from_str(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let v: serde_json::Value = serde_json::from_str(content)?;
        Ok(Self::apply_over(Self::empty(), &v))
    }

    /// Apply bindings from a parsed JSON value into `target`, overwriting any existing entries.
    /// A key whose value is `null` or `"None"` is unbound instead (Phase 7.26): with the
    /// built-in defaults always loaded underneath, that is the only way to remove one.
    pub(crate) fn apply_from_value(v: &serde_json::Value, target: &mut Self) {
        fn apply(
            section: Option<&serde_json::Value>,
            map: &mut HashMap<String, Action>,
            parse: impl Fn(&str) -> Action,
        ) {
            let Some(bindings) = section.and_then(|b| b.as_object()) else {
                return;
            };
            for (key, val) in bindings {
                if is_unbind_value(val) {
                    map.remove(key);
                } else if let Some(action_str) = val.as_str() {
                    map.insert(key.clone(), parse(action_str));
                }
            }
        }
        // TWF format: top-level "bindings" key maps to NormalMode
        apply(
            v.get("bindings"),
            &mut target.normal_mode,
            Self::parse_action_name,
        );
        // TWF format: "textViewerBindings" maps to ViewerMode
        apply(v.get("textViewerBindings"), &mut target.viewer_mode, |s| {
            Self::parse_viewer_action_name(s.strip_prefix("TextViewer.").unwrap_or(s))
        });
        // Native rwf format: NormalMode / SearchMode / DialogMode / ViewerMode / LeapMode
        apply(
            v.get("NormalMode"),
            &mut target.normal_mode,
            Self::parse_action_name,
        );
        apply(
            v.get("SearchMode"),
            &mut target.search_mode,
            Self::parse_action_name,
        );
        apply(
            v.get("DialogMode"),
            &mut target.dialog_mode,
            Self::parse_action_name,
        );
        apply(
            v.get("ViewerMode"),
            &mut target.viewer_mode,
            Self::parse_viewer_action_name,
        );
        apply(
            v.get("LeapMode"),
            &mut target.leap_mode,
            Self::parse_leap_action_name,
        );
    }

    /// Apply a user keybindings document over `base` (the built-in defaults, or an
    /// empty set when the user file replaces them). See `crate::config_layers`.
    pub fn apply_over(mut base: Self, v: &serde_json::Value) -> Self {
        Self::apply_from_value(v, &mut base);
        base
    }

    /// Total number of bindings across every mode.
    pub fn binding_count(&self) -> usize {
        self.normal_mode.len()
            + self.search_mode.len()
            + self.dialog_mode.len()
            + self.viewer_mode.len()
            + self.leap_mode.len()
    }

    /// An empty binding set (no defaults).
    pub fn empty() -> Self {
        Self {
            normal_mode: HashMap::new(),
            search_mode: HashMap::new(),
            dialog_mode: HashMap::new(),
            viewer_mode: HashMap::new(),
            leap_mode: HashMap::new(),
            pending_sequence: None,
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
    ShowTrashBrowser,
    Rename,
    PatternRename,
    CreateDirectory,
    CreateFile,
    ShowAttrTimestampDialog,
    ShowCreateLinkDialog,
    ShowOperationReport,

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
    /// Move the *other* pane to the active pane's directory.
    SyncPanes,
    /// Move the *active* pane to the other pane's directory (default `o`).
    SyncToOtherPane,
    SwapPanes,

    // Context menu, drive selection, custom functions
    ShowContextMenu,
    /// Native OS shell context menu (Windows Explorer menu) for the marked files, or the cursor file.
    ShowShellContextMenu,
    ShowDriveChangeDialog,
    ShowCustomFunctionsDialog,

    // Information dialogs
    ShowFileInfoForCursor,
    OpenWithEditor, // open cursor file with Editor from config
    OpenWithSystem, // open cursor entry with OS default association (Ctrl+Enter)
    OpenWith, // open cursor entry via ExtensionAssociation lookup, picker if 2+ matches (Phase 7.3)
    ShowVersion,
    ReloadConfig,
    // Background polling control for the active pane's drive (Phase 7.5 D10e). No default
    // key binding; meant for a custom menu.
    StopPolling,
    StartPolling,
    TogglePolling,
    ShowVersionInfo,        // compact version/system info (backtick key)
    ShowVersionInfoVerbose, // verbose version/system info including config file status (F2)
    SaveLog,
    EditConfigFile,

    // Diagnostics (Phase 7.15)
    //
    // Handled directly in `App::handle_key_event`, not via
    // `action_to_transitions` — a diagnostic session is app-level state, not
    // `AppState`, so it produces no `Transition`. They fall through to the
    // wildcard arm there, which is correct.
    /// Start or stop a diagnostic recording session.
    ToggleDiagnosticSession,
    /// Capture the current screen and state into the running session.
    DiagnosticSnapshot,

    // Pane width (7.25)
    WidenLeftPane,
    WidenRightPane,
    ResetPaneSplit,

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
        // Delete and Insert were missing until 2026-08-11 and so fell to the
        // catch-all below, which returns an empty string. An empty string matches
        // no binding, which made these keys impossible to bind *at all* — writing
        // "Delete" or "Shift+Delete" in keybindings.json simply had no effect, with
        // no error to explain why. (Dialogs were unaffected: they match on
        // `KeyCode` directly rather than going through this function.)
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
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
                    // `null` unbinds the key (Phase 7.26); report it by name.
                    let value = map
                        .next_value::<Option<String>>()?
                        .unwrap_or_else(|| "null".to_string());
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

/// `true` for a keybinding value that unbinds its key: `null` or `"None"`.
pub fn is_unbind_value(val: &serde_json::Value) -> bool {
    val.is_null() || val.as_str() == Some("None")
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
