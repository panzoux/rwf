//! Input handling and key bindings
//!
//! This module provides configurable key bindings and input event processing.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

use crate::state::Transition;
use crate::AppState;
use crate::model::Location;
use crate::backend::ArchiveHandler;

/// Archive format for compression operations
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    ZIP,
    SevenZip,
    Tar,
    TarGz,
}

impl Default for ArchiveFormat {
    fn default() -> Self {
        Self::ZIP
    }
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
    /// Multi-key sequence state
    #[serde(skip)]
    pub pending_sequence: Option<String>,
}

impl KeyBindings {
    /// Create default TWF-compatible key bindings
    pub fn twf_defaults() -> Self {
        let mut normal_mode = HashMap::new();
        
        // Navigation
        normal_mode.insert("Tab".to_string(), Action::SwitchPane);
        normal_mode.insert("Up".to_string(), Action::CursorUp);
        normal_mode.insert("Down".to_string(), Action::CursorDown);
        normal_mode.insert("k".to_string(), Action::CursorUp);
        normal_mode.insert("j".to_string(), Action::CursorDown);
        normal_mode.insert("^".to_string(), Action::MoveCursorToFirst);
        normal_mode.insert("$".to_string(), Action::MoveCursorToLast);
        normal_mode.insert("g".to_string(), Action::MoveCursorToFirst);
        normal_mode.insert("G".to_string(), Action::MoveCursorToLast);
        normal_mode.insert("PageUp".to_string(), Action::PageUp);
        normal_mode.insert("PageDown".to_string(), Action::PageDown);
        normal_mode.insert("Enter".to_string(), Action::EnterDirectory);
        normal_mode.insert("Backspace".to_string(), Action::NavigateToParent);
        normal_mode.insert("Left".to_string(), Action::SwitchToLeftPane);
        normal_mode.insert("Right".to_string(), Action::SwitchToRightPane);
        normal_mode.insert("h".to_string(), Action::SwitchToLeftPane);
        normal_mode.insert("Alt+Left".to_string(), Action::HistoryBack);
        normal_mode.insert("Alt+Right".to_string(), Action::HistoryForward);
        normal_mode.insert("H".to_string(), Action::ShowHistoryDialog);
        
        // Marking
        normal_mode.insert("Space".to_string(), Action::ToggleMark);
        normal_mode.insert("Shift+Space".to_string(), Action::ToggleMarkUp);
        normal_mode.insert("*".to_string(), Action::FileMaskFilter);
        normal_mode.insert("A".to_string(), Action::MarkAll);
        normal_mode.insert("Ctrl+u".to_string(), Action::UnmarkAll);
        normal_mode.insert("@".to_string(), Action::WildcardMarking);
        normal_mode.insert("Ctrl+Space".to_string(), Action::RangeMarking);
        normal_mode.insert("Home".to_string(), Action::InvertMarks);
        normal_mode.insert("End".to_string(), Action::ClearMarks);
        
        // File operations
        normal_mode.insert("C".to_string(), Action::Copy);
        normal_mode.insert("c".to_string(), Action::Copy);
        normal_mode.insert("M".to_string(), Action::Move);
        normal_mode.insert("m".to_string(), Action::Move);
        normal_mode.insert("D".to_string(), Action::Delete);
        normal_mode.insert("d".to_string(), Action::Delete);
        normal_mode.insert("Ctrl+d".to_string(), Action::DeleteForce);
        normal_mode.insert("R".to_string(), Action::PatternRename);
        normal_mode.insert("r".to_string(), Action::Rename);
        normal_mode.insert("K".to_string(), Action::CreateDirectory);
        
        // Sorting
        normal_mode.insert("s+n".to_string(), Action::SortByName);
        normal_mode.insert("s+s".to_string(), Action::SortBySize);
        normal_mode.insert("s+d".to_string(), Action::SortByDate);
        normal_mode.insert("s+e".to_string(), Action::SortByExtension);
        normal_mode.insert("s+o".to_string(), Action::ToggleSortOrder);
        
        // Search and filter
        normal_mode.insert("/".to_string(), Action::StartSearch);
        normal_mode.insert("Ctrl+f".to_string(), Action::StartSearch);
        normal_mode.insert("f".to_string(), Action::FileMaskFilter);
        normal_mode.insert("Ctrl+k".to_string(), Action::ClearSearchFilter);
        normal_mode.insert("Escape".to_string(), Action::Quit);
        
        // Refresh
        normal_mode.insert("F5".to_string(), Action::Refresh);
        
        // Tab management
        normal_mode.insert("Ctrl+n".to_string(), Action::NewTab);
        normal_mode.insert("Alt+z".to_string(), Action::NewTab);
        normal_mode.insert("Ctrl+t".to_string(), Action::TabSelector);
        normal_mode.insert("Ctrl+w".to_string(), Action::CloseTab);
        normal_mode.insert("Ctrl+Right".to_string(), Action::NextTab);
        normal_mode.insert("Ctrl+PageDown".to_string(), Action::NextTab);
        normal_mode.insert("Alt+l".to_string(), Action::NextTab);
        normal_mode.insert("Ctrl+Left".to_string(), Action::PrevTab);
        normal_mode.insert("Ctrl+PageUp".to_string(), Action::PrevTab);
        normal_mode.insert("Alt+h".to_string(), Action::PrevTab);
        normal_mode.insert("Ctrl+b".to_string(), Action::TabSelector);
        
        // Registered folders
        normal_mode.insert("B".to_string(), Action::RegisterCurrentFolder);
        normal_mode.insert("I".to_string(), Action::ShowRegisteredFolderDialog);
        normal_mode.insert("F".to_string(), Action::ShowRegisteredFolderDialog);
        normal_mode.insert("Alt+M".to_string(), Action::MoveToRegisteredFolder); // Alt+Shift+M: navigate/move to registered folder

        // Jump navigation
        normal_mode.insert("J".to_string(), Action::ShowJumpToPathDialog);
        normal_mode.insert("N".to_string(), Action::ShowJumpToFileDialog);

        // Viewer
        normal_mode.insert("v".to_string(), Action::OpenTextViewer);
        normal_mode.insert("X".to_string(), Action::OpenHexViewer);

        // File info
        normal_mode.insert("i".to_string(), Action::ShowFileInfoForCursor);
        normal_mode.insert("e".to_string(), Action::OpenWithEditor);

        // Miscellaneous
        normal_mode.insert("q".to_string(), Action::Quit);
        normal_mode.insert("Q".to_string(), Action::ExitAndChangeDirectory);
        normal_mode.insert("?".to_string(), Action::Help);
        normal_mode.insert("F1".to_string(), Action::Help);
        // Job management
        normal_mode.insert("Alt+j".to_string(), Action::JobManager);
        normal_mode.insert("Ctrl+j".to_string(), Action::JobManager);
        
        // Test jobs
        normal_mode.insert("9".to_string(), Action::CountDownJob(0));  // 0 = default 180 seconds
        
        normal_mode.insert("Alt+s".to_string(), Action::CalculateDirectorySize);
        
        // Pane operations
        normal_mode.insert("o".to_string(), Action::SyncPanes);   // sync active pane to match other
        normal_mode.insert("O".to_string(), Action::SwapPanes);   // Shift+O: swap the two panes

        // Context menu, drive selection, custom functions
        normal_mode.insert("\\".to_string(), Action::ShowContextMenu);
        normal_mode.insert("L".to_string(), Action::ShowDriveChangeDialog);
        normal_mode.insert("T".to_string(), Action::ShowCustomFunctionsDialog); // Shift+T

        // Config reload (Shift+Z = "Z")
        normal_mode.insert("Z".to_string(), Action::ReloadConfig);

        // Version / system info (outputs to task panel, not a modal dialog)
        normal_mode.insert("`".to_string(), Action::ShowVersionInfo);
        normal_mode.insert("F2".to_string(), Action::ShowVersionInfoVerbose);
        
        // Task panel operations
        normal_mode.insert("t".to_string(), Action::ToggleTaskPanel);
        normal_mode.insert("Ctrl+Up".to_string(), Action::IncreaseTaskPanelHeight);
        normal_mode.insert("Ctrl+Down".to_string(), Action::DecreaseTaskPanelHeight);
        normal_mode.insert("Shift+Up".to_string(), Action::ScrollTaskPanelUp);
        normal_mode.insert("Shift+Down".to_string(), Action::ScrollTaskPanelDown);

        // Archive operations
        normal_mode.insert("p".to_string(), Action::Compress);
        normal_mode.insert("u".to_string(), Action::Extract);

        // Viewer mode bindings
        let mut viewer_mode = HashMap::new();
        viewer_mode.insert("Escape".to_string(), Action::ViewerClose);
        viewer_mode.insert("q".to_string(), Action::ViewerClose);
        viewer_mode.insert("b".to_string(), Action::ViewerToggleHexMode);
        viewer_mode.insert("F8".to_string(), Action::ViewerToggleHexMode);
        viewer_mode.insert("j".to_string(), Action::ViewerScrollDown);
        viewer_mode.insert("Down".to_string(), Action::ViewerScrollDown);
        viewer_mode.insert("k".to_string(), Action::ViewerScrollUp);
        viewer_mode.insert("Up".to_string(), Action::ViewerScrollUp);
        viewer_mode.insert("PageDown".to_string(), Action::ViewerPageDown);
        viewer_mode.insert("PageUp".to_string(), Action::ViewerPageUp);
        viewer_mode.insert("Ctrl+f".to_string(), Action::ViewerPageDown);
        viewer_mode.insert("Space".to_string(), Action::ViewerPageDown);
        viewer_mode.insert("Ctrl+b".to_string(), Action::ViewerPageUp);
        viewer_mode.insert("F5".to_string(), Action::ViewerGoToTop);
        viewer_mode.insert("g".to_string(), Action::ViewerGoToTop);
        viewer_mode.insert("<".to_string(), Action::ViewerGoToTop);
        viewer_mode.insert("Home".to_string(), Action::ViewerGoToTop);
        viewer_mode.insert("F6".to_string(), Action::ViewerGoToBottom);
        viewer_mode.insert("G".to_string(), Action::ViewerGoToBottom);
        viewer_mode.insert(">".to_string(), Action::ViewerGoToBottom);
        viewer_mode.insert("End".to_string(), Action::ViewerGoToBottom);
        viewer_mode.insert("e".to_string(), Action::ViewerCycleEncoding);
        viewer_mode.insert("/".to_string(), Action::ViewerBeginSearch);
        viewer_mode.insert("?".to_string(), Action::ViewerBeginSearchBackward);
        viewer_mode.insert("F3".to_string(), Action::ViewerFindNext);
        viewer_mode.insert("Shift+F3".to_string(), Action::ViewerFindPrev);
        viewer_mode.insert("n".to_string(), Action::ViewerFindNext);
        viewer_mode.insert("N".to_string(), Action::ViewerFindPrev);
        viewer_mode.insert("Ctrl+~".to_string(), Action::ViewerToggleCaseSensitive);
        viewer_mode.insert("Ctrl+^".to_string(), Action::ViewerToggleCaseSensitive);
        viewer_mode.insert("Ctrl+u".to_string(), Action::ViewerClearSearch);
        viewer_mode.insert("Left".to_string(),       Action::ViewerScrollLeft);
        viewer_mode.insert("Right".to_string(),      Action::ViewerScrollRight);
        viewer_mode.insert("Shift+Left".to_string(),  Action::ViewerFastScrollLeft);
        viewer_mode.insert("Shift+Right".to_string(), Action::ViewerFastScrollRight);
        viewer_mode.insert("Shift+Up".to_string(),    Action::ViewerFastScrollUp);
        viewer_mode.insert("Shift+Down".to_string(),  Action::ViewerFastScrollDown);

        Self {
            normal_mode,
            search_mode: HashMap::new(),
            dialog_mode: HashMap::new(),
            viewer_mode,
            pending_sequence: None,
        }
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::twf_defaults()
    }
}

impl KeyBindings {
    
    /// Load key bindings from a JSON file, merging over defaults.
    /// Accepts both the native format (`NormalMode` key) and the TWF-compatible
    /// format (`bindings` key). Unknown action strings become `InvokeCustomFunction`.
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&content)?;

        let mut merged = Self::default();

        // TWF format: top-level "bindings" key maps to NormalMode
        if let Some(bindings) = v.get("bindings").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    let action = Self::parse_action_name(action_str);
                    merged.normal_mode.insert(key.clone(), action);
                }
            }
        }

        // TWF format: "textViewerBindings" maps to ViewerMode
        if let Some(bindings) = v.get("textViewerBindings").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    // Strip "TextViewer." prefix if present
                    let stripped = action_str.strip_prefix("TextViewer.").unwrap_or(action_str);
                    let action = Self::parse_viewer_action_name(stripped);
                    merged.viewer_mode.insert(key.clone(), action);
                }
            }
        }

        // Native rwf format: NormalMode / SearchMode / ViewerMode keys with Action enum names
        if let Some(bindings) = v.get("NormalMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    let action = Self::parse_action_name(action_str);
                    merged.normal_mode.insert(key.clone(), action);
                }
            }
        }
        if let Some(bindings) = v.get("SearchMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    let action = Self::parse_action_name(action_str);
                    merged.search_mode.insert(key.clone(), action);
                }
            }
        }
        if let Some(bindings) = v.get("DialogMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    let action = Self::parse_action_name(action_str);
                    merged.dialog_mode.insert(key.clone(), action);
                }
            }
        }
        if let Some(bindings) = v.get("ViewerMode").and_then(|b| b.as_object()) {
            for (key, val) in bindings {
                if let Some(action_str) = val.as_str() {
                    let action = Self::parse_viewer_action_name(action_str);
                    merged.viewer_mode.insert(key.clone(), action);
                }
            }
        }

        Ok(merged)
    }

    /// Convert an action name string to an `Action`.
    /// Handles: exact rwf variant names, TWF alias names, and custom function names.
    fn parse_action_name(s: &str) -> Action {
        // 1. Try exact deserialization as an Action enum variant
        if let Ok(action) = serde_json::from_value::<Action>(serde_json::Value::String(s.to_string())) {
            return action;
        }
        // 2. Legacy alias table — maps old/variant names to canonical Actions
        match s {
            "ReloadConfiguration"               => return Action::ReloadConfig,
            "LaunchConfigEditor"                => return Action::EditConfigFile,
            "ViewFile"                          => return Action::OpenTextViewer,
            "ViewFileAsText"                    => return Action::OpenTextViewer,
            "ViewFileAsHex"                     => return Action::OpenHexViewer,
            "ViewFileAsImage" | "OpenImageViewer" => return Action::OpenTextViewer, // fallback until image viewer exists
            "NavigateToRoot"                    => return Action::CursorHome,
            "PreviousTab"                       => return Action::PrevTab,
            "JumpToPath"                        => return Action::ShowJumpToPathDialog,
            "JumpToFile"                        => return Action::ShowJumpToFileDialog,
            "ExitApplicationAndChangeDirectory" => return Action::ExitAndChangeDirectory,
            "ShowJobManager"                    => return Action::JobManager,
            "RegisterCurrentDirectory"          => return Action::RegisterCurrentFolder,
            "ToggleMarkAndMoveUp"               => return Action::ToggleMarkUp,
            "ShowSortDialog"                    => return Action::OpenSortDialog,
            "MarkRange"                         => return Action::RangeMarking,
            "FileMask"                          => return Action::FileMaskFilter,
            "WildcardMark"                      => return Action::WildcardMarking,
            "ResizeTaskPanelUp" | "ResizeTaskPaneUp"     => return Action::IncreaseTaskPanelHeight,
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
        if let Ok(action) = serde_json::from_value::<Action>(serde_json::Value::String(s.to_string())) {
            return action;
        }
        // Then try short TWF-style aliases for backward compatibility.
        match s {
            "FindNext" | "FindPrevious" => Action::ViewerFindNext,
            "FindPrev"                  => Action::ViewerFindPrev,
            "Search"                    => Action::ViewerBeginSearch,
            "StartForwardSearch"        => Action::ViewerBeginSearch,
            "StartBackwardSearch"       => Action::ViewerBeginSearchBackward,
            "GoToFileTop"               => Action::ViewerGoToTop,
            "GoToFileBottom"            => Action::ViewerGoToBottom,
            "GoToLineStart"             => Action::ViewerScrollLeft,
            "GoToLineEnd"               => Action::ViewerScrollRight,
            "PageUp"                    => Action::ViewerPageUp,
            "PageDown"                  => Action::ViewerPageDown,
            "ToggleHexMode"             => Action::ViewerToggleHexMode,
            "CycleEncoding"             => Action::ViewerCycleEncoding,
            "Close"                     => Action::ViewerClose,
            _ => Action::InvokeCustomFunction(s.to_string()),
        }
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
        let potential_sequences: Vec<_> = self.normal_mode.keys()
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
    Rename,
    PatternRename,
    CreateDirectory,
    
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
    OpenWithEditor,         // open cursor file with EditorCommand from config
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
    CountDownJob(u32),  // Countdown test job (parameter: duration in seconds, 0 = default 180)

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
        2 => format!("Delete '{}', '{}'", file_name(&targets[0]), file_name(&targets[1])),
        n => format!("Delete {} files: '{}', '{}'...", n, file_name(&targets[0]), file_name(&targets[1])),
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
                debug!("EnterDirectory: entry = {}, is_dir = {}", entry.name, entry.is_dir);
                
                if entry.is_dir {
                    // Enter directory
                    debug!("EnterDirectory: entering directory {}", entry.location.display_path());
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
                    } else {
                        // Check for file-type extension association (Phase 6.2)
                        let assoc = entry.extension().and_then(|ext| {
                            let ext_lower = ext.to_lowercase();
                            state.extension_associations.iter().find(|a| {
                                a.extension.trim_start_matches('.').to_lowercase() == ext_lower
                            })
                        });
                        if let Some(assoc) = assoc {
                            let expander = crate::macro_expander::MacroExpander::new();
                            let func = crate::model::dialog::CustomFunction::new("open", &assoc.command);
                            let func = if let Some(ref shell) = assoc.shell {
                                func.with_shell(shell)
                            } else { func };
                            match expander.expand(state, &func) {
                                Ok(command) => {
                                    debug!("EnterDirectory: running association command: {}", command);
                                    let working_dir = state.active_pane().current_location.clone();
                                    let shell = assoc.shell.clone();
                                    vec![Transition::ExecuteAssociation { command, working_dir, shell }]
                                }
                                Err(_) => {
                                    debug!("EnterDirectory: association command expansion failed, falling back to viewer");
                                    vec![Transition::OpenTextViewer { location: entry.location.clone() }]
                                }
                            }
                        } else {
                            debug!("EnterDirectory: opening text viewer for {}", entry.name);
                            vec![Transition::OpenTextViewer { location: entry.location.clone() }]
                        }
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

            let build_entries = |pane: crate::model::ui::ActivePane| -> (Vec<crate::model::Location>, usize) {
                let (stack, _) = tab.history.stack_and_pos(pane);
                let current_loc = match pane {
                    crate::model::ui::ActivePane::Left  => tab.left_pane.current_location.clone(),
                    crate::model::ui::ActivePane::Right => tab.right_pane.current_location.clone(),
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
                        tab_index, active_pane,
                        left_entries, left_pos,
                        right_entries, right_pos,
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
            let tab_names: Vec<String> = state.tabs.tabs.iter()
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
            let active_marked: Vec<_> = pane.entries.iter()
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
            let active_marked: Vec<_> = pane.entries.iter()
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
            let active_marked: Vec<_> = pane.entries.iter()
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

            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::delete_confirm(targets),
            }]
        }
        Action::DeleteForce => {
            let pane = state.active_pane();
            let active_marked: Vec<_> = pane.entries.iter()
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
                dialog: crate::model::Dialog {
                    title: "Create Directory".to_string(),
                    content: crate::model::DialogContent::Input {
                        prompt: "Directory name:".to_string(),
                        default_value: String::new(),
                    },
                },
            }]
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
            // Show help dialog with configured language
            // **Validates: Requirements 48.2, 48.5**
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::help_with_language(&state.config.help_language),
            }]
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
            vec![
                Transition::CreateAndStartCountDownJob {
                    spec: job_spec,
                    name: format!("CountDownJob {}", duration_secs),
                    description: "Countdown test job".to_string(),
                },
            ]
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
            let name = loc.path()
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
                    pane.entries.iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.location.clone())
                        .collect()
                } else if let Some(entry) = pane.current_entry() {
                    debug!("No marked files, using current entry from active pane: {:?}", entry.location);
                    vec![entry.location.clone()]
                } else {
                    vec![]
                }
            };
            let sources = if sources.is_empty() {
                // Active pane is empty, try the opposite pane
                debug!("Active pane is empty, checking opposite pane");
                if let Some(entry) = state.opposite_pane().current_entry() {
                    debug!("Using current entry from opposite pane: {:?}", entry.location);
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
                dialog: crate::model::Dialog::compression(sources, state.config.text_input.edit_mode),
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
                    vec![]  // Not an archive
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
}

#[cfg(test)]
mod input_properties;

#[cfg(test)]
mod file_operations_tests;

#[cfg(test)]
mod marking_tests;

#[cfg(test)]
mod marking_wildcard_tests;
mod rename_tests;
mod history_tests;

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
