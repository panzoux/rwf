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

/// Configurable key bindings loaded from keybindings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    /// Key bindings for normal mode
    pub normal_mode: HashMap<String, Action>,
    /// Key bindings for search mode
    pub search_mode: HashMap<String, Action>,
    /// Key bindings for dialog mode
    pub dialog_mode: HashMap<String, Action>,
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
        normal_mode.insert("G".to_string(), Action::MoveCursorToFirst);
        normal_mode.insert("Shift+G".to_string(), Action::MoveCursorToLast);
        normal_mode.insert("PageUp".to_string(), Action::PageUp);
        normal_mode.insert("PageDown".to_string(), Action::PageDown);
        normal_mode.insert("Enter".to_string(), Action::EnterDirectory);
        normal_mode.insert("Backspace".to_string(), Action::NavigateToParent);
        normal_mode.insert("Left".to_string(), Action::SwitchToLeftPane);
        normal_mode.insert("Right".to_string(), Action::SwitchToRightPane);
        normal_mode.insert("Alt+Left".to_string(), Action::HistoryBack);
        normal_mode.insert("Alt+Right".to_string(), Action::HistoryForward);
        
        // Marking
        normal_mode.insert("Space".to_string(), Action::ToggleMark);
        normal_mode.insert("*".to_string(), Action::WildcardMarking);
        normal_mode.insert("A".to_string(), Action::MarkAll);
        normal_mode.insert("Ctrl+u".to_string(), Action::UnmarkAll);
        normal_mode.insert("@".to_string(), Action::WildcardMarking);
        normal_mode.insert("Ctrl+Space".to_string(), Action::RangeMarking);
        normal_mode.insert("Home".to_string(), Action::InvertMarks);
        normal_mode.insert("End".to_string(), Action::ClearMarks);
        
        // File operations
        normal_mode.insert("C".to_string(), Action::Copy);
        normal_mode.insert("M".to_string(), Action::Move);
        normal_mode.insert("D".to_string(), Action::Delete);
        normal_mode.insert("R".to_string(), Action::Rename);
        normal_mode.insert("Shift+R".to_string(), Action::PatternRename);
        normal_mode.insert("Shift+K".to_string(), Action::CreateDirectory);
        
        // Sorting
        normal_mode.insert("S".to_string(), Action::CycleSortMode);
        
        // Search and filter
        normal_mode.insert("/".to_string(), Action::StartSearch);
        normal_mode.insert("F".to_string(), Action::StartSearch);
        normal_mode.insert("Ctrl+f".to_string(), Action::StartSearch);
        normal_mode.insert(":".to_string(), Action::FileMaskFilter);
        normal_mode.insert("Ctrl+k".to_string(), Action::ClearSearchFilter);
        normal_mode.insert("Escape".to_string(), Action::Quit);
        
        // Refresh
        normal_mode.insert("F5".to_string(), Action::Refresh);
        
        // Tab management
        normal_mode.insert("Ctrl+n".to_string(), Action::NewTab);
        normal_mode.insert("Alt+Z".to_string(), Action::NewTab);
        normal_mode.insert("Ctrl+t".to_string(), Action::TabSelector);
        normal_mode.insert("Ctrl+w".to_string(), Action::CloseTab);
        normal_mode.insert("Ctrl+Right".to_string(), Action::NextTab);
        normal_mode.insert("Ctrl+PageDown".to_string(), Action::NextTab);
        normal_mode.insert("Alt+L".to_string(), Action::NextTab);
        normal_mode.insert("Ctrl+Left".to_string(), Action::PrevTab);
        normal_mode.insert("Ctrl+PageUp".to_string(), Action::PrevTab);
        normal_mode.insert("Alt+H".to_string(), Action::PrevTab);
        normal_mode.insert("Ctrl+b".to_string(), Action::TabSelector);
        
        // Registered folders
        normal_mode.insert("Shift+B".to_string(), Action::RegisterCurrentFolder);
        normal_mode.insert("I".to_string(), Action::ShowRegisteredFolderDialog);
        normal_mode.insert("Shift+F".to_string(), Action::ShowRegisteredFolderDialog);
        normal_mode.insert("Shift+M".to_string(), Action::MoveToRegisteredFolder);
        
        // Miscellaneous
        normal_mode.insert("Q".to_string(), Action::Quit);
        normal_mode.insert("Shift+Q".to_string(), Action::ExitAndChangeDirectory);
        normal_mode.insert("?".to_string(), Action::Help);
        normal_mode.insert("F1".to_string(), Action::Help);
        normal_mode.insert("Alt+J".to_string(), Action::JobManager);
        normal_mode.insert("Ctrl+Shift+Right".to_string(), Action::JobManager);
        normal_mode.insert("H".to_string(), Action::CalculateDirectorySize);
        
        // Pane operations
        normal_mode.insert("O".to_string(), Action::SyncPanes);
        normal_mode.insert("Shift+O".to_string(), Action::SwapPanes);
        
        // Context menu and drive selection
        normal_mode.insert("\\".to_string(), Action::ShowContextMenu);
        normal_mode.insert("`".to_string(), Action::ShowContextMenu);
        normal_mode.insert("Shift+L".to_string(), Action::ShowDriveChangeDialog);
        
        Self {
            normal_mode,
            search_mode: HashMap::new(),
            dialog_mode: HashMap::new(),
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
    
    /// Load key bindings from a JSON file
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let bindings: Self = serde_json::from_str(&content)?;
        Ok(bindings)
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
    
    // File Operations
    Copy,
    Move,
    Delete,
    Rename,
    PatternRename,
    CreateDirectory,
    
    // Marking
    ToggleMark,
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
    
    // Search and Filter
    StartSearch,
    FileMaskFilter,
    ClearSearchFilter,
    ExitSearchMode,
    NextMatch,
    PrevMatch,
    
    // View
    ChangeDisplayMode(u8),
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
    
    // Miscellaneous
    Quit,
    ExitAndChangeDirectory,
    Help,
    JobManager,
    CalculateDirectorySize,
    
    // Pane operations
    SyncPanes,
    SwapPanes,
    
    // Context menu and drive selection
    ShowContextMenu,
    ShowDriveChangeDialog,
    
    // Information dialogs
    ShowFileInfoForCursor,
    ShowVersion,
    
    // Internal
    PendingSequence,
}

/// Format a key event as a string for key binding lookup
pub fn format_key_event(event: &KeyEvent) -> String {
    let mut parts = Vec::new();

    if event.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift");
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt");
    }

    let key = match event.code {
        KeyCode::Char(c) => {
            // Handle space specially to match key binding format
            if c == ' ' {
                "Space".to_string()
            } else if c.is_ascii_alphabetic() {
                // Always uppercase alphabetic characters for consistency with keybindings
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
                    use crate::backend::ZipArchiveHandler;
                    let handler = ZipArchiveHandler::new();
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
                        debug!("EnterDirectory: not a directory or archive, ignoring");
                        vec![]
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
        Action::MarkAll => vec![Transition::MarkAll],
        Action::UnmarkAll => vec![Transition::UnmarkAll],
        Action::ClearMarks => vec![Transition::UnmarkAll],
        Action::InvertMarks => vec![Transition::InvertMarks],
        Action::WildcardMarking => {
            // Show wildcard marking dialog
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog {
                    title: "Wildcard Marking".to_string(),
                    content: crate::model::DialogContent::Input {
                        prompt: "Enter pattern (* and ? wildcards):".to_string(),
                        default_value: String::new(),
                    },
                },
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
            // Get marked files or current cursor entry
            let sources = if state.marking.count() > 0 {
                state.active_pane()
                    .entries
                    .iter()
                    .filter(|e| state.marking.is_marked(&e.location))
                    .map(|e| e.location.clone())
                    .collect()
            } else if let Some(entry) = state.active_pane().current_entry() {
                vec![entry.location.clone()]
            } else {
                vec![]
            };
            
            if sources.is_empty() {
                return vec![];
            }
            
            let dest = state.opposite_pane().current_location.clone();
            
            // Calculate total size of files to be copied
            let total_size: u64 = state.active_pane()
                .entries
                .iter()
                .filter(|e| sources.contains(&e.location))
                .map(|e| e.size)
                .sum();
            
            // Format size for display
            let size_str = crate::model::format_size(total_size);
            
            // Show confirmation dialog with source files, destination, and total size
            let message = if sources.len() == 1 {
                format!(
                    "Copy {} ({}) to {}?",
                    sources[0].display_path(),
                    size_str,
                    dest.display_path()
                )
            } else {
                format!(
                    "Copy {} files ({}) to {}?",
                    sources.len(),
                    size_str,
                    dest.display_path()
                )
            };
            
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog {
                    title: "Copy".to_string(),
                    content: crate::model::DialogContent::Confirmation { message },
                },
            }]
        }
        Action::Move => {
            // Get marked files or current cursor entry
            let sources = if state.marking.count() > 0 {
                state.active_pane()
                    .entries
                    .iter()
                    .filter(|e| state.marking.is_marked(&e.location))
                    .map(|e| e.location.clone())
                    .collect()
            } else if let Some(entry) = state.active_pane().current_entry() {
                vec![entry.location.clone()]
            } else {
                vec![]
            };
            
            if sources.is_empty() {
                return vec![];
            }
            
            let dest = state.opposite_pane().current_location.clone();
            
            // Show confirmation dialog
            let message = if sources.len() == 1 {
                format!("Move {} to {}?", sources[0].display_path(), dest.display_path())
            } else {
                format!("Move {} files to {}?", sources.len(), dest.display_path())
            };
            
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog {
                    title: "Move".to_string(),
                    content: crate::model::DialogContent::Confirmation { message },
                },
            }]
        }
        Action::Delete => {
            // Get marked files or current cursor entry
            let targets = if state.marking.count() > 0 {
                state.active_pane()
                    .entries
                    .iter()
                    .filter(|e| state.marking.is_marked(&e.location))
                    .map(|e| e.location.clone())
                    .collect()
            } else if let Some(entry) = state.active_pane().current_entry() {
                vec![entry.location.clone()]
            } else {
                vec![]
            };
            
            if targets.is_empty() {
                return vec![];
            }
            
            // Show confirmation dialog
            let message = if targets.len() == 1 {
                format!("Delete {}?", targets[0].display_path())
            } else {
                format!("Delete {} files?", targets.len())
            };
            
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog {
                    title: "Delete".to_string(),
                    content: crate::model::DialogContent::Confirmation { message },
                },
            }]
        }
        Action::Rename => {
            // Only rename the current cursor entry
            if let Some(entry) = state.active_pane().current_entry() {
                let current_name = entry.name.clone();
                
                vec![Transition::ShowDialog {
                    dialog: crate::model::Dialog {
                        title: "Rename".to_string(),
                        content: crate::model::DialogContent::Input {
                            prompt: "New name:".to_string(),
                            default_value: current_name,
                        },
                    },
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
            // Enter search mode and show search input dialog
            vec![
                Transition::ChangeUIMode {
                    mode: crate::model::UIMode::Search,
                },
                Transition::ShowDialog {
                    dialog: crate::model::Dialog::input(
                        "Search",
                        "Enter search pattern (* and ? wildcards, /regex/, /regex/i):",
                        "",
                    ),
                },
            ]
        }
        Action::FileMaskFilter => {
            // Show file mask filter dialog
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::input(
                    "File Mask Filter",
                    "Enter file mask pattern (* and ? wildcards):",
                    state.active_pane().file_mask.clone().unwrap_or_default(),
                ),
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
            // Show help dialog with all key bindings
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::help(),
            }]
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
            // Show context menu dialog
            vec![Transition::ShowContextMenu]
        }
        Action::ShowDriveChangeDialog => {
            // Show drive selection dialog
            vec![Transition::ShowDriveChangeDialog]
        }
        Action::ShowFileInfoForCursor => {
            // Show file information dialog for current cursor entry
            vec![Transition::ShowFileInfo]
        }
        Action::ShowVersion => {
            // Show version information dialog
            vec![Transition::ShowVersion]
        }
        Action::RegisterCurrentFolder => {
            // Show input dialog to get folder name
            vec![Transition::ShowDialog {
                dialog: crate::model::Dialog::input(
                    "Register Folder",
                    "Enter folder name:",
                    "",
                ),
            }]
        }
        Action::ShowRegisteredFolderDialog => {
            // Show registered folder selector dialog
            vec![Transition::ShowRegisteredFolderDialog]
        }
        Action::MoveToRegisteredFolder => {
            // Check if there are marked files
            if state.marking.count() > 0 {
                // Show registered folder selector dialog for moving files
                vec![Transition::ShowRegisteredFolderDialog]
            } else {
                vec![]
            }
        }
        Action::PendingSequence => vec![],
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

        let event = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(format_key_event(&event), "Shift+A");

        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(format_key_event(&event), "Enter");
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
mod sorting_tests;

#[cfg(test)]
mod search_filter_tests;

#[cfg(test)]
mod tab_management_tests;

#[cfg(test)]
mod miscellaneous_tests;
