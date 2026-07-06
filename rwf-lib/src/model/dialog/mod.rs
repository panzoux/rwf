//! Dialog system

#![allow(clippy::unwrap_used)] // TODO(M6): ratchet — see plan/quality_overhaul.md

use crate::job::JobId;
pub use crate::job::PipeToAction;
use crate::model::Location;
use std::collections::HashMap;

mod close_tab_with_active_job;
mod confirmation;
mod context_menu;
mod custom_function_menu;
mod custom_function_selector;
mod delete_confirm;
mod drive_selection;
mod error;
mod extraction_confirm;
mod file_info;
mod file_mask;
mod help;
mod history_dialog;
mod input;
mod job_manager;
mod jump_to_file;
mod jump_to_path;
mod progress;
mod registered_folder_selector;
mod simple_rename;
mod sort;
mod tab_selector;
mod ui_state;
mod version;
mod wildcard_mark;

pub use close_tab_with_active_job::CloseTabWithActiveJobDialog;
pub use confirmation::ConfirmationDialog;
pub use context_menu::ContextMenuDialog;
pub use custom_function_menu::CustomFunctionMenuDialog;
pub use custom_function_selector::CustomFunctionSelectorContent;
pub use delete_confirm::DeleteConfirmDialog;
pub use drive_selection::DriveSelectionDialog;
pub use error::ErrorDialog;
pub use extraction_confirm::ExtractionConfirmDialog;
pub use file_info::FileInfoDialog;
pub use file_mask::FileMaskDialog;
pub use help::HelpDialog;
pub use history_dialog::HistoryDialogContent;
pub use input::InputDialog;
pub use job_manager::JobManagerContent;
pub use jump_to_file::JumpToFileDialog;
pub use jump_to_path::JumpToPathDialog;
pub use progress::ProgressDialog;
pub use registered_folder_selector::RegisteredFolderSelectorContent;
pub use simple_rename::SimpleRenameDialog;
pub use sort::SortDialog;
pub use tab_selector::TabSelectorContent;
pub use ui_state::DialogUiState;
pub use version::VersionDialog;
pub use wildcard_mark::WildcardMarkDialog;

/// Which mode tab is active in the help viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpTab {
    #[default]
    NormalMode,
    ViewerMode,
    LeapMode,
    DialogMode,
    CustomFunctions,
}

impl HelpTab {
    pub fn label(&self) -> &'static str {
        match self {
            HelpTab::NormalMode => "Normal",
            HelpTab::ViewerMode => "Viewer",
            HelpTab::LeapMode => "Leap",
            HelpTab::DialogMode => "Dialog",
            HelpTab::CustomFunctions => "Custom Functions",
        }
    }

    pub fn next(self) -> Self {
        match self {
            HelpTab::NormalMode => HelpTab::ViewerMode,
            HelpTab::ViewerMode => HelpTab::LeapMode,
            HelpTab::LeapMode => HelpTab::DialogMode,
            HelpTab::DialogMode => HelpTab::CustomFunctions,
            HelpTab::CustomFunctions => HelpTab::NormalMode,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            HelpTab::NormalMode => HelpTab::CustomFunctions,
            HelpTab::ViewerMode => HelpTab::NormalMode,
            HelpTab::LeapMode => HelpTab::ViewerMode,
            HelpTab::DialogMode => HelpTab::LeapMode,
            HelpTab::CustomFunctions => HelpTab::DialogMode,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => HelpTab::NormalMode,
            1 => HelpTab::ViewerMode,
            2 => HelpTab::LeapMode,
            3 => HelpTab::DialogMode,
            _ => HelpTab::CustomFunctions,
        }
    }
}

/// A single row in the help viewer
#[derive(Debug, Clone)]
pub struct HelpEntry {
    /// Display category (e.g. "Navigation", "File Operations", "Custom Functions")
    pub category: String,
    /// Human-readable description of what this action does
    pub description: String,
    /// Keys bound to this action (empty = unbound)
    pub keys: Vec<String>,
    /// Action/function name (used for searching; not shown directly)
    pub action_name: String,
    /// Which tab this entry belongs to
    pub tab: HelpTab,
}

/// Dialog stack
#[derive(Debug)]
pub struct DialogStack {
    pub stack: Vec<Dialog>,
    pub input_buffer: String,
}

impl Default for DialogStack {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogStack {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            input_buffer: String::new(),
        }
    }

    /// Push a dialog onto the stack
    pub fn push(&mut self, dialog: Dialog) {
        self.stack.push(dialog);
        self.input_buffer.clear();
    }

    /// Pop the top dialog
    pub fn pop(&mut self) -> Option<Dialog> {
        self.input_buffer.clear();
        self.stack.pop()
    }

    /// Pop the dialog immediately below the top, leaving the top in place.
    /// Used to silently remove the CustomFunctionSelector that sits under a $I Input dialog.
    pub fn pop_below_top(&mut self) {
        if self.stack.len() >= 2 {
            let idx = self.stack.len() - 2;
            self.stack.remove(idx);
        }
    }

    /// Get current dialog
    pub fn current(&self) -> Option<&Dialog> {
        self.stack.last()
    }

    /// Get mutable current dialog
    pub fn current_mut(&mut self) -> Option<&mut Dialog> {
        self.stack.last_mut()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

/// Dialog definition
#[derive(Debug, Clone)]
pub struct Dialog {
    pub title: String,
    pub content: DialogContent,
}

/// Dialog content types
#[derive(Debug, Clone)]
pub enum DialogContent {
    Confirmation(ConfirmationDialog),
    Input(InputDialog),
    Progress(ProgressDialog),
    Help(HelpDialog),
    JobManager(JobManagerContent),
    CloseTabWithActiveJob(CloseTabWithActiveJobDialog),
    CustomFunctionSelector(CustomFunctionSelectorContent),
    /// Second-level menu opened when a menu-type custom function is selected
    CustomFunctionMenu(CustomFunctionMenuDialog),
    RegisteredFolderSelector(RegisteredFolderSelectorContent),
    TabSelector(TabSelectorContent),
    PatternRename {
        find: String,
        find_cursor_pos: usize,
        find_scroll_pos: usize,
        replace: String,
        replace_cursor_pos: usize,
        replace_scroll_pos: usize,
        use_regex: bool,
        case_sensitive: bool,
        preview: Vec<(String, String)>,
        /// 0=find textbox, 1=replace textbox, 2=filelist
        focused_field: usize,
        preview_scroll: usize,
        preview_horizontal_scroll: usize,
        /// Non-None when a collision was detected at confirm time
        error_message: Option<String>,
        /// 0=SIDE-BY-SIDE, 1=Preview (new names), 2=Original (original names); Alt+P cycles
        preview_mode: u8,
        /// true=show all files, false=matching files only; Alt+A toggles
        show_all: bool,
    },
    Error(ErrorDialog),
    ComparisonView {
        diff: crate::job::FileDiff,
        scroll_offset: usize,
    },
    SplitJoinDialog {
        mode: SplitJoinMode,
        chunk_size_mb: u64,
    },
    ContextMenu(ContextMenuDialog),
    DriveSelection(DriveSelectionDialog),
    FileInfo(FileInfoDialog),
    FileConflict {
        conflicts: Vec<ConflictPair>,
        current_index: usize,
        focused_button: usize, // 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename (Textbox), 4=Cancel
        rename_text: String,
        rename_cursor: usize,
        rename_scroll: usize,
        edit_mode: crate::config::EditMode, // Emacs or Vi mode for textbox
        vi_mode: Option<crate::config::ViMode>, // None = Emacs, Some = Vi mode state
        decisions: Vec<ConflictAction>,
        error_message: Option<String>,
        // Vi pending states (persisted between key presses)
        vi_pending_find_backward: Option<bool>,
        vi_pending_operator: Option<u8>, // 0=none, 1=change, 2=delete
        vi_pending_ctrl_x: bool,
        // Undo/Redo history
        history: Vec<String>,
        history_index: usize,
        /// "Copy" or "Move" — used in the dialog title
        operation: String,
    },
    Compression {
        // Data fields
        sources: Vec<crate::model::Location>,
        format: crate::input::ArchiveFormat,
        archive_name: String,
        selected_format_index: usize,
        selected_compression_index: usize,
        compression_level: u32,
        // Interaction state (persists while dialog is open)
        focused_field: usize, // 0=format, 1=compression, 2=name, 3=OK, 4=Cancel
        format_focus_index: usize, // Which format has focus (0-7)
        compression_focus_index: usize, // Which compression level has focus (0-5)
        cursor_pos: usize,    // Cursor position in archive name
        scroll_pos: usize,    // Scroll position in archive name
        // Vi mode support
        edit_mode: crate::config::EditMode,
        vi_mode: Option<crate::config::ViMode>,
        // Vi pending states (persisted between key presses)
        vi_pending_find_backward: Option<bool>,
        vi_pending_operator: Option<u8>, // 0=none, 1=change, 2=delete
        vi_pending_ctrl_x: bool,
        // Undo/Redo history
        history: Vec<String>,
        history_index: usize,
    },
    ExtractionConfirm(ExtractionConfirmDialog),
    DeleteConfirm(DeleteConfirmDialog),
    Version(VersionDialog),
    SortDialog(SortDialog),
    /// File mask filter dialog — single text input for a wildcard pattern
    FileMask(FileMaskDialog),
    /// Wildcard marking dialog — enter a pattern to mark matching entries
    WildcardMark(WildcardMarkDialog),
    /// Simple rename dialog — single text input prefilled with current filename
    SimpleRename(SimpleRenameDialog),
    /// Navigation history list — holds both panes, switches with Tab/h/l
    HistoryDialog(HistoryDialogContent),
    /// Jump to Directory dialog — AND-filtered directory suggestions
    JumpToPath(JumpToPathDialog),
    /// Jump to File dialog — AND-filtered file+directory suggestions
    JumpToFile(JumpToFileDialog),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitJoinMode {
    Split,
    Join,
}

/// Error types for error dialogs
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorType {
    /// General error
    General,
    /// Permission denied error
    Permission,
    /// File not found error
    FileNotFound,
    /// Invalid path error
    InvalidPath,
    /// Operation failed error
    OperationFailed,
}

/// Context menu option
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuOption {
    pub label: String,
    pub action: ContextMenuAction,
}

/// Context menu action
#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuAction {
    Copy,
    Move,
    Delete,
    Rename,
    View,
    CustomFunction(String),
    /// Visual separator — not selectable
    Separator,
}

/// Drive information for drive selection dialog
#[derive(Debug, Clone, PartialEq)]
pub struct DriveInfo {
    pub path: String,
    pub label: String,
    pub drive_type: DriveType,
    pub total_space: Option<u64>,
    pub free_space: Option<u64>,
}

impl DriveInfo {
    /// Returns the display string shown in the drive selection list.
    pub fn display_label(&self) -> String {
        // Home entry: label starts with '~'
        if self.label.starts_with('~') {
            return self.label.clone();
        }
        // Network share: show path
        if self.drive_type == DriveType::Network {
            return self.path.clone();
        }
        let type_str = match self.drive_type {
            DriveType::Local => "Local",
            DriveType::Removable => "Removable",
            _ => "Unknown",
        };
        let path_trimmed = self.path.trim_end_matches(['/', '\\']);
        if self.label.is_empty() {
            format!("{} ({})", path_trimmed, type_str)
        } else {
            format!("{} - {} ({})", path_trimmed, self.label, type_str)
        }
    }
}

/// Drive type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveType {
    Local,
    Network,
    Removable,
    Unknown,
}

/// A single item in a menu_xxx.json file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MenuItem {
    pub name: String,
    /// Custom function name or built-in action name. Empty string = separator.
    #[serde(default)]
    pub action: String,
}

impl MenuItem {
    pub fn is_separator(&self) -> bool {
        self.name.starts_with("-----") || self.action.is_empty()
    }
    pub fn is_selectable(&self) -> bool {
        !self.is_separator()
    }
}

/// Wrapper for the menu_xxx.json file format.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MenuFile {
    #[serde(default)]
    #[allow(dead_code)]
    pub version: String,
    pub menus: Vec<MenuItem>,
}

/// Menu content: either a resolved item list or a filename to load from.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MenuContent {
    /// Resolved list of menu items (after loading)
    Items(Vec<MenuItem>),
    /// Path to a separate menu JSON file (relative to the parent file's directory)
    File(String),
}

/// Custom function definition with macro expansion support.
/// Either `Command` (leaf) or `Menu` (submenu) must be present, not both.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CustomFunction {
    pub name: String,
    /// Shell command to execute (leaf entry). Mutually exclusive with Menu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Submenu: inline item list or filename reference. Resolved to Items after loading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<MenuContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Category for display in the help viewer (default: "Custom Functions" if absent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipe_to_action: Option<PipeToAction>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub os_specific: HashMap<String, OsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_binding: Option<String>,
}

impl CustomFunction {
    pub fn is_menu(&self) -> bool {
        self.menu.is_some()
    }
    pub fn is_command(&self) -> bool {
        self.command.is_some()
    }

    /// Return the resolved menu items, or empty slice if not a menu or not yet resolved.
    pub fn menu_items(&self) -> &[MenuItem] {
        match &self.menu {
            Some(MenuContent::Items(items)) => items,
            _ => &[],
        }
    }
}

/// OS-specific configuration for custom functions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OsConfig {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}

/// Registered folder definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RegisteredFolder {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Job information for display in job manager dialog
#[derive(Debug, Clone)]
pub struct JobInfo {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    pub progress: f64,
    pub source: Option<Location>,
    pub destination: Option<Location>,
    pub details: String,
}

/// Job kind for display
#[derive(Debug, Clone)]
pub enum JobKind {
    ReadDirectory,
    Copy,
    Move,
    Delete,
    Mkdir,
    Rename,
    CalculateSize,
    ExtractArchive,
    CreateArchive,
    ExecuteCustomFunction,
    Search,
}

/// Job state for display
#[derive(Debug, Clone)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

impl Dialog {
    /// Create a confirmation dialog
    pub fn confirmation(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: DialogContent::Confirmation(ConfirmationDialog::new(message.into())),
        }
    }

    /// Create an input dialog
    pub fn input(
        title: impl Into<String>,
        prompt: impl Into<String>,
        default_value: impl Into<String>,
    ) -> Self {
        let dv: String = default_value.into();
        Self {
            title: title.into(),
            content: DialogContent::Input(InputDialog::new(prompt.into(), dv)),
        }
    }

    /// Create a progress dialog
    pub fn progress(
        title: impl Into<String>,
        operation: impl Into<String>,
        progress: f64,
        details: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            content: DialogContent::Progress(ProgressDialog::new(
                operation.into(),
                progress,
                details.into(),
            )),
        }
    }

    /// Create a job manager dialog
    pub fn job_manager() -> Self {
        Self {
            title: "Job Manager".to_string(),
            content: DialogContent::JobManager(JobManagerContent::new()),
        }
    }

    /// Create a file conflict dialog
    pub fn file_conflict(
        conflicts: Vec<crate::model::dialog::ConflictPair>,
        current_index: usize,
        edit_mode: crate::config::EditMode,
        op_name: &str,
    ) -> Self {
        let total = conflicts.len();
        let title = format!(
            "{} - File Conflict ({}/{})",
            op_name,
            current_index + 1,
            total
        );
        let rename_text = if !conflicts.is_empty() {
            conflicts[current_index].source.name.clone()
        } else {
            String::new()
        };
        let rename_cursor = if !conflicts.is_empty() {
            conflicts[current_index].source.name.len()
        } else {
            0
        };
        // Initialize vi_mode based on edit_mode
        let vi_mode = if edit_mode == crate::config::EditMode::Vi {
            Some(crate::config::ViMode::Normal)
        } else {
            None
        };
        Self {
            title,
            content: DialogContent::FileConflict {
                conflicts,
                current_index,
                focused_button: 3, // Rename button focused by default
                rename_text: rename_text.clone(),
                rename_cursor,
                rename_scroll: 0,
                edit_mode,
                vi_mode,
                decisions: Vec::new(),
                error_message: None,
                vi_pending_find_backward: None,
                vi_pending_operator: None,
                vi_pending_ctrl_x: false,
                history: vec![rename_text],
                history_index: 0,
                operation: op_name.to_string(),
            },
        }
    }

    /// Update the dialog title with current progress
    pub fn update_file_conflict_title(&mut self) {
        if let DialogContent::FileConflict {
            conflicts,
            current_index,
            operation,
            ..
        } = &self.content
        {
            let total = conflicts.len();
            self.title = format!(
                "{} - File Conflict ({}/{})",
                operation,
                current_index + 1,
                total
            );
        }
    }

    /// Create a help dialog
    pub fn help() -> Self {
        Self::help_with_language("en")
    }

    /// Create a help dialog with specific language.
    /// Entries are built by the help builder (Step 5); starts empty here.
    pub fn help_with_language(lang: &str) -> Self {
        Self {
            title: "Help".to_string(),
            content: DialogContent::Help(HelpDialog::new(lang.to_string())),
        }
    }

    /// Create a custom function selector dialog
    pub fn custom_function_selector(functions: Vec<CustomFunction>) -> Self {
        Self {
            title: "Custom Functions".to_string(),
            content: DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent::new(
                functions,
            )),
        }
    }

    /// Create a custom function menu dialog (second-level menu from a menu-type entry)
    pub fn custom_function_menu(title: String, items: Vec<MenuItem>) -> Self {
        Self {
            title,
            content: DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog::new(items)),
        }
    }

    /// Create a registered folder selector dialog
    pub fn registered_folder_selector(folders: Vec<RegisteredFolder>) -> Self {
        Self {
            title: "Registered Folders".to_string(),
            content: DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent::new(
                folders,
            )),
        }
    }

    /// Create a tab selector dialog
    pub fn tab_selector(tabs: Vec<String>) -> Self {
        Self {
            title: "Select Tab".to_string(),
            content: DialogContent::TabSelector(TabSelectorContent::new(tabs)),
        }
    }

    /// Create a pattern rename dialog (TWF-style: separate Find/Replace fields)
    pub fn pattern_rename() -> Self {
        Self {
            title: "Pattern Rename".to_string(),
            content: DialogContent::PatternRename {
                find: String::new(),
                find_cursor_pos: 0,
                find_scroll_pos: 0,
                replace: String::new(),
                replace_cursor_pos: 0,
                replace_scroll_pos: 0,
                use_regex: true,
                case_sensitive: false,
                preview: Vec::new(),
                focused_field: 0,
                preview_scroll: 0,
                preview_horizontal_scroll: 0,
                error_message: None,
                preview_mode: 0,
                show_all: true,
            },
        }
    }

    /// Create a context menu dialog with the given custom options.
    /// Falls back to built-in defaults if `extra` is empty.
    pub fn context_menu_with_options(extra: Vec<ContextMenuOption>) -> Self {
        let options = if extra.is_empty() {
            Self::default_context_menu_options()
        } else {
            extra
        };
        Self {
            title: "Context Menu".to_string(),
            content: DialogContent::ContextMenu(ContextMenuDialog::new(options)),
        }
    }

    fn default_context_menu_options() -> Vec<ContextMenuOption> {
        vec![
            ContextMenuOption {
                label: "View".to_string(),
                action: ContextMenuAction::View,
            },
            ContextMenuOption {
                label: "─────".to_string(),
                action: ContextMenuAction::Separator,
            },
            ContextMenuOption {
                label: "Copy".to_string(),
                action: ContextMenuAction::Copy,
            },
            ContextMenuOption {
                label: "Move".to_string(),
                action: ContextMenuAction::Move,
            },
            ContextMenuOption {
                label: "Rename".to_string(),
                action: ContextMenuAction::Rename,
            },
            ContextMenuOption {
                label: "─────".to_string(),
                action: ContextMenuAction::Separator,
            },
            ContextMenuOption {
                label: "Delete".to_string(),
                action: ContextMenuAction::Delete,
            },
        ]
    }

    /// Create a context menu dialog with default built-in options
    pub fn context_menu() -> Self {
        Self::context_menu_with_options(Vec::new())
    }

    /// Create a drive selection dialog
    pub fn drive_selection(drives: Vec<DriveInfo>, pane: crate::model::ui::ActivePane) -> Self {
        let pane_label = match pane {
            crate::model::ui::ActivePane::Left => "Left",
            crate::model::ui::ActivePane::Right => "Right",
        };
        Self {
            title: format!("Select Drive [{}]", pane_label),
            content: DialogContent::DriveSelection(DriveSelectionDialog::new(drives)),
        }
    }

    /// Create a file information dialog
    pub fn file_info(entry: &crate::model::FileEntry) -> Self {
        #[cfg(unix)]
        let (permissions, owner, group) = {
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::fs::PermissionsExt;

            // Try to get metadata for permissions and ownership
            let metadata_result = if let crate::model::Location::Local(path) = &entry.location {
                std::fs::metadata(path).ok()
            } else {
                None
            };

            let permissions = metadata_result.as_ref().map(|m| m.permissions().mode());
            let owner = metadata_result.as_ref().and_then(|m| {
                users::get_user_by_uid(m.uid()).map(|u| u.name().to_string_lossy().to_string())
            });
            let group = metadata_result.as_ref().and_then(|m| {
                users::get_group_by_gid(m.gid()).map(|g| g.name().to_string_lossy().to_string())
            });

            (permissions, owner, group)
        };

        // Try to get created and accessed times
        let (created, accessed) = if let crate::model::Location::Local(path) = &entry.location {
            if let Ok(metadata) = std::fs::metadata(path) {
                let created = metadata.created().ok();
                let accessed = metadata.accessed().ok();
                (created, accessed)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Check if readonly
        let is_readonly = if let crate::model::Location::Local(path) = &entry.location {
            std::fs::metadata(path)
                .ok()
                .map(|m| m.permissions().readonly())
                .unwrap_or(false)
        } else {
            false
        };

        let link_target = entry.link_target.as_ref().map(|p| {
            let s = p.to_string_lossy().into_owned();
            if let Some(stripped) = s.strip_prefix(r"\??\") {
                stripped.to_string()
            } else {
                s
            }
        });

        Self {
            title: "File Information".to_string(),
            content: DialogContent::FileInfo(FileInfoDialog::new(
                entry.name.clone(),
                entry.location.display_path(),
                entry.calculated_size.unwrap_or(entry.size),
                created,
                entry.modified,
                accessed,
                entry.is_dir,
                is_readonly,
                #[cfg(unix)]
                permissions,
                #[cfg(unix)]
                owner,
                #[cfg(unix)]
                group,
                link_target,
                entry.link_kind.clone(),
            )),
        }
    }

    /// Create a sort dialog pre-selected to the pane's current mode and order
    pub fn sort_dialog(
        current_mode: crate::model::SortMode,
        current_order: crate::model::SortOrder,
    ) -> Self {
        use crate::model::{SortMode, SortOrder};
        let selected_mode_index = match current_mode {
            SortMode::Name => 0,
            SortMode::Size => 1,
            SortMode::Date => 2,
            SortMode::Extension => 3,
        };
        let selected_order_index = match current_order {
            SortOrder::Ascending => 0,
            SortOrder::Descending => 1,
        };
        Self {
            title: "Sort".to_string(),
            content: DialogContent::SortDialog(SortDialog::new(
                selected_mode_index,
                selected_order_index,
            )),
        }
    }

    /// Create a file mask filter dialog pre-filled with the pane's current mask
    pub fn file_mask(current_mask: Option<&str>) -> Self {
        let initial = current_mask.unwrap_or("");
        Self {
            title: "File Mask Filter".to_string(),
            content: DialogContent::FileMask(FileMaskDialog::new(initial.to_string())),
        }
    }

    /// Create a wildcard marking dialog
    pub fn wildcard_mark() -> Self {
        Self {
            title: "Wildcard Marking".to_string(),
            content: DialogContent::WildcardMark(WildcardMarkDialog::new()),
        }
    }

    /// Create a simple rename dialog prefilled with the current filename
    pub fn simple_rename(current_name: String) -> Self {
        Self {
            title: "Rename".to_string(),
            content: DialogContent::SimpleRename(SimpleRenameDialog::new(current_name)),
        }
    }

    /// Create a Jump to Directory dialog with pre-fetched candidates
    pub fn jump_to_path(search_root: String, candidates: Vec<String>) -> Self {
        Self {
            title: "Jump to Directory".to_string(),
            content: DialogContent::JumpToPath(JumpToPathDialog::new(search_root, candidates)),
        }
    }

    /// Create a Jump to File dialog with pre-fetched candidates (files + dirs)
    pub fn jump_to_file(search_root: String, candidates: Vec<String>) -> Self {
        Self {
            title: "Jump to File".to_string(),
            content: DialogContent::JumpToFile(JumpToFileDialog::new(search_root, candidates)),
        }
    }

    /// Create a navigation history dialog showing both panes
    pub fn history_dialog(
        tab_index: usize,
        active_pane: crate::model::ui::ActivePane,
        left_entries: Vec<Location>,
        left_current_pos: usize,
        right_entries: Vec<Location>,
        right_current_pos: usize,
    ) -> Self {
        let pane_label = match active_pane {
            crate::model::ui::ActivePane::Left => "Left",
            crate::model::ui::ActivePane::Right => "Right",
        };
        Self {
            title: format!("History [Tab {} | {}]", tab_index + 1, pane_label),
            content: DialogContent::HistoryDialog(HistoryDialogContent::new(
                left_entries,
                right_entries,
                left_current_pos,
                right_current_pos,
                active_pane,
            )),
        }
    }

    /// Create a version information dialog
    pub fn version() -> Self {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let build_date = option_env!("BUILD_DATE").unwrap_or("Unknown").to_string();
        let copyright = "Copyright © 2024 RWF Contributors".to_string();

        Self {
            title: "Version Information".to_string(),
            content: DialogContent::Version(VersionDialog::new(version, build_date, copyright)),
        }
    }

    /// Create an error dialog
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            title: "Error".to_string(),
            content: DialogContent::Error(ErrorDialog::new(
                message.into(),
                None,
                ErrorType::General,
            )),
        }
    }

    /// Create an error dialog with details
    pub fn error_with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            title: "Error".to_string(),
            content: DialogContent::Error(ErrorDialog::new(
                message.into(),
                Some(details.into()),
                ErrorType::General,
            )),
        }
    }

    /// Create a permission error dialog
    pub fn permission_error(message: impl Into<String>) -> Self {
        Self {
            title: "Permission Denied".to_string(),
            content: DialogContent::Error(ErrorDialog::new(
                message.into(),
                Some("This operation requires elevated privileges.".to_string()),
                ErrorType::Permission,
            )),
        }
    }

    /// Create a file not found error dialog
    pub fn file_not_found_error(path: impl Into<String>) -> Self {
        Self {
            title: "File Not Found".to_string(),
            content: DialogContent::Error(ErrorDialog::new(
                format!("The file or directory could not be found: {}", path.into()),
                None,
                ErrorType::FileNotFound,
            )),
        }
    }

    /// Create an invalid path error dialog
    pub fn invalid_path_error(path: impl Into<String>) -> Self {
        Self {
            title: "Invalid Path".to_string(),
            content: DialogContent::Error(ErrorDialog::new(
                format!("The path is invalid: {}", path.into()),
                None,
                ErrorType::InvalidPath,
            )),
        }
    }

    /// Create an operation failed error dialog from JobResult
    pub fn from_job_failure(operation: &str, error_message: &str) -> Self {
        // Detect error type from message
        let error_type = if error_message.to_lowercase().contains("permission")
            || error_message.to_lowercase().contains("access denied")
        {
            ErrorType::Permission
        } else if error_message.to_lowercase().contains("not found") {
            ErrorType::FileNotFound
        } else if error_message.to_lowercase().contains("invalid") {
            ErrorType::InvalidPath
        } else {
            ErrorType::OperationFailed
        };

        let title = match error_type {
            ErrorType::Permission => "Permission Denied",
            ErrorType::FileNotFound => "File Not Found",
            ErrorType::InvalidPath => "Invalid Path",
            _ => "Operation Failed",
        };

        let details = if error_type == ErrorType::Permission {
            Some("This operation requires elevated privileges.".to_string())
        } else {
            None
        };

        Self {
            title: title.to_string(),
            content: DialogContent::Error(ErrorDialog::new(
                format!("{} failed: {}", operation, error_message),
                details,
                error_type,
            )),
        }
    }

    /// Create a comparison view dialog
    pub fn comparison_view(diff: crate::job::FileDiff) -> Self {
        Self {
            title: "File Comparison".to_string(),
            content: DialogContent::ComparisonView {
                diff,
                scroll_offset: 0,
            },
        }
    }

    /// Create a split/join dialog
    pub fn split_join_dialog() -> Self {
        Self {
            title: "Split/Join Files".to_string(),
            content: DialogContent::SplitJoinDialog {
                mode: SplitJoinMode::Split,
                chunk_size_mb: 100, // Default 100MB chunks
            },
        }
    }

    /// Create a compression dialog
    pub fn compression(
        sources: Vec<crate::model::Location>,
        edit_mode: crate::config::EditMode,
    ) -> Self {
        let default_name = if sources.len() == 1 {
            let name = sources[0].display_path();
            // Get just the filename from the path
            std::path::Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("archive")
                .to_string()
        } else {
            "archive".to_string()
        };

        // Initialize vi_mode based on edit_mode
        let vi_mode = if edit_mode == crate::config::EditMode::Vi {
            Some(crate::config::ViMode::Normal)
        } else {
            None
        };

        Self {
            title: "Compress Files".to_string(),
            content: DialogContent::Compression {
                sources,
                format: crate::input::ArchiveFormat::ZIP,
                archive_name: default_name.clone(),
                selected_format_index: 0,
                selected_compression_index: 3, // Default to Normal
                compression_level: 5,          // Default to Normal
                // Initialize interaction state
                focused_field: 0,           // Start with format focused
                format_focus_index: 0,      // First format has focus
                compression_focus_index: 3, // Normal (5) has focus
                cursor_pos: default_name.chars().count(),
                scroll_pos: 0,
                edit_mode,
                vi_mode,
                // Initialize Vi pending states
                vi_pending_find_backward: None,
                vi_pending_operator: None,
                vi_pending_ctrl_x: false,
                // Initialize history
                history: vec![default_name],
                history_index: 0,
            },
        }
    }

    /// Create an extraction confirmation dialog
    pub fn extraction_confirm(
        archive: crate::model::Location,
        dest: crate::model::Location,
        file_count: usize,
    ) -> Self {
        Self {
            title: "Extract Archive".to_string(),
            content: DialogContent::ExtractionConfirm(ExtractionConfirmDialog::new(
                archive, dest, file_count,
            )),
        }
    }

    /// Create a delete confirmation dialog
    pub fn delete_confirm(targets: Vec<(crate::model::Location, bool)>) -> Self {
        let n = targets.len();
        let title = if n == 1 {
            "Delete File".to_string()
        } else {
            format!("Delete {} Files", n)
        };
        Self {
            title,
            content: DialogContent::DeleteConfirm(DeleteConfirmDialog::new(targets)),
        }
    }
}

impl DialogContent {
    /// Check if this dialog requires user input
    pub fn requires_input(&self) -> bool {
        matches!(
            self,
            DialogContent::Input { .. }
                | DialogContent::CustomFunctionSelector(_)
                | DialogContent::RegisteredFolderSelector(_)
                | DialogContent::TabSelector(_)
                | DialogContent::PatternRename { .. }
                | DialogContent::ContextMenu(_)
                | DialogContent::DriveSelection(_)
        )
    }

    /// Check if this dialog is a selector (list-based)
    pub fn is_selector(&self) -> bool {
        matches!(
            self,
            DialogContent::CustomFunctionSelector(_)
                | DialogContent::RegisteredFolderSelector(_)
                | DialogContent::TabSelector(_)
                | DialogContent::JobManager(_)
                | DialogContent::ContextMenu(_)
                | DialogContent::DriveSelection(_)
        )
    }

    /// Get the current selected index for selector dialogs
    pub fn selected_index(&self) -> Option<usize> {
        match self {
            DialogContent::JobManager(JobManagerContent { selected_index, .. }) => {
                Some(*selected_index)
            }
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                selected_index,
                ..
            }) => Some(*selected_index),
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                selected_index,
                ..
            }) => Some(*selected_index),
            DialogContent::TabSelector(TabSelectorContent { selected_index, .. }) => {
                Some(*selected_index)
            }
            DialogContent::ContextMenu(ContextMenuDialog { selected_index, .. }) => {
                Some(*selected_index)
            }
            DialogContent::DriveSelection(DriveSelectionDialog { selected_index, .. }) => {
                Some(*selected_index)
            }
            _ => None,
        }
    }

    /// Update the selected index for selector dialogs
    pub fn set_selected_index(&mut self, new_index: usize) {
        match self {
            DialogContent::JobManager(JobManagerContent { selected_index, .. }) => {
                *selected_index = new_index;
            }
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                selected_index,
                ..
            }) => {
                *selected_index = new_index;
            }
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                selected_index,
                ..
            }) => {
                *selected_index = new_index;
            }
            DialogContent::TabSelector(TabSelectorContent { selected_index, .. }) => {
                *selected_index = new_index;
            }
            DialogContent::ContextMenu(ContextMenuDialog { selected_index, .. }) => {
                *selected_index = new_index;
            }
            DialogContent::DriveSelection(DriveSelectionDialog { selected_index, .. }) => {
                *selected_index = new_index;
            }
            _ => {}
        }
    }

    /// Get the filter string for filterable dialogs
    pub fn filter(&self) -> Option<&str> {
        match self {
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                filter, ..
            }) => Some(filter),
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                filter,
                ..
            }) => Some(filter),
            DialogContent::DriveSelection(DriveSelectionDialog { filter, .. }) => Some(filter),
            _ => None,
        }
    }

    /// Update the filter string for filterable dialogs
    pub fn set_filter(&mut self, new_filter: String) {
        match self {
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                filter, ..
            }) => {
                *filter = new_filter;
            }
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                filter,
                ..
            }) => {
                *filter = new_filter;
            }
            DialogContent::DriveSelection(DriveSelectionDialog { filter, .. }) => {
                *filter = new_filter;
            }
            _ => {}
        }
    }
}

impl CustomFunction {
    /// Create a new leaf custom function with a command
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: Some(command.into()),
            menu: None,
            description: None,
            category: None,
            shell: None,
            working_dir: None,
            pipe_to_action: None,
            os_specific: HashMap::new(),
            key_binding: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the shell
    pub fn with_shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, working_dir: impl Into<String>) -> Self {
        self.working_dir = Some(working_dir.into());
        self
    }

    /// Set the pipe to action
    pub fn with_pipe_to_action(mut self, action: PipeToAction) -> Self {
        self.pipe_to_action = Some(action);
        self
    }

    /// Get the effective command for the current OS, or None for menu entries.
    pub fn get_command(&self) -> Option<&str> {
        #[cfg(target_os = "windows")]
        let os_key = "windows";
        #[cfg(target_os = "macos")]
        let os_key = "macos";
        #[cfg(target_os = "linux")]
        let os_key = "linux";

        if let Some(os_config) = self.os_specific.get(os_key) {
            Some(&os_config.command)
        } else {
            self.command.as_deref()
        }
    }

    /// Get the shell for the current OS
    pub fn get_shell(&self) -> Option<&str> {
        #[cfg(target_os = "windows")]
        let os_key = "windows";
        #[cfg(target_os = "macos")]
        let os_key = "macos";
        #[cfg(target_os = "linux")]
        let os_key = "linux";

        if let Some(os_config) = self.os_specific.get(os_key) {
            os_config.shell.as_deref().or(self.shell.as_deref())
        } else {
            self.shell.as_deref()
        }
    }
}

impl RegisteredFolder {
    /// Create a new registered folder
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            description: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl JobInfo {
    /// Create a new job info
    pub fn new(id: JobId, kind: JobKind, state: JobState) -> Self {
        Self {
            id,
            kind,
            state,
            progress: 0.0,
            source: None,
            destination: None,
            details: String::new(),
        }
    }

    /// Set the progress
    pub fn with_progress(mut self, progress: f64) -> Self {
        self.progress = progress;
        self
    }

    /// Set the source location
    pub fn with_source(mut self, source: Location) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the destination location
    pub fn with_destination(mut self, destination: Location) -> Self {
        self.destination = Some(destination);
        self
    }

    /// Set the details
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = details.into();
        self
    }

    /// Get a display string for the job kind
    pub fn kind_display(&self) -> &str {
        match self.kind {
            JobKind::ReadDirectory => "Read Directory",
            JobKind::Copy => "Copy",
            JobKind::Move => "Move",
            JobKind::Delete => "Delete",
            JobKind::Mkdir => "Create Directory",
            JobKind::Rename => "Rename",
            JobKind::CalculateSize => "Calculate Size",
            JobKind::ExtractArchive => "Extract Archive",
            JobKind::CreateArchive => "Create Archive",
            JobKind::ExecuteCustomFunction => "Execute Function",
            JobKind::Search => "Search",
        }
    }

    /// Get a display string for the job state
    pub fn state_display(&self) -> String {
        match &self.state {
            JobState::Queued => "Queued".to_string(),
            JobState::Running => format!("Running ({}%)", (self.progress * 100.0) as u32),
            JobState::Completed => "Completed".to_string(),
            JobState::Failed(reason) => format!("Failed: {}", reason),
            JobState::Cancelled => "Cancelled".to_string(),
        }
    }
}

/// Job manager dialog helper
pub struct JobManagerDialog {
    jobs: Vec<JobInfo>,
    selected_index: usize,
    scroll_offset: usize,
}

impl JobManagerDialog {
    /// Create a new job manager dialog
    pub fn new(jobs: Vec<JobInfo>) -> Self {
        Self {
            jobs,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Get all jobs
    pub fn jobs(&self) -> &[JobInfo] {
        &self.jobs
    }

    /// Get the selected job
    pub fn selected_job(&self) -> Option<&JobInfo> {
        self.jobs.get(self.selected_index)
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get the scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.jobs.len() {
            self.selected_index += 1;
        }
    }

    /// Move selection to the top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Move selection to the bottom
    pub fn move_to_bottom(&mut self) {
        if !self.jobs.is_empty() {
            self.selected_index = self.jobs.len() - 1;
        }
    }

    /// Update scroll offset for a given visible height
    pub fn update_scroll(&mut self, visible_height: usize) {
        if self.selected_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected_index - visible_height + 1;
        } else if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Get visible jobs for a given height
    pub fn visible_jobs(&self, height: usize) -> &[JobInfo] {
        let start = self.scroll_offset;
        let end = (start + height).min(self.jobs.len());
        &self.jobs[start..end]
    }

    /// Get queued jobs
    pub fn queued_jobs(&self) -> Vec<&JobInfo> {
        self.jobs
            .iter()
            .filter(|job| matches!(job.state, JobState::Queued))
            .collect()
    }

    /// Get active jobs
    pub fn active_jobs(&self) -> Vec<&JobInfo> {
        self.jobs
            .iter()
            .filter(|job| matches!(job.state, JobState::Running))
            .collect()
    }

    /// Get completed jobs
    pub fn completed_jobs(&self) -> Vec<&JobInfo> {
        self.jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.state,
                    JobState::Completed | JobState::Failed(_) | JobState::Cancelled
                )
            })
            .collect()
    }

    /// Update jobs list
    pub fn update_jobs(&mut self, jobs: Vec<JobInfo>) {
        self.jobs = jobs;
        // Ensure selected index is still valid
        if self.selected_index >= self.jobs.len() && !self.jobs.is_empty() {
            self.selected_index = self.jobs.len() - 1;
        }
    }

    /// Check if there are any jobs
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Get total job count
    pub fn total_count(&self) -> usize {
        self.jobs.len()
    }

    /// Get counts by state
    pub fn state_counts(&self) -> (usize, usize, usize) {
        let queued = self.queued_jobs().len();
        let active = self.active_jobs().len();
        let completed = self.completed_jobs().len();
        (queued, active, completed)
    }
}

impl DialogContent {
    /// Create a job manager dialog content with jobs
    pub fn job_manager_with_jobs(_jobs: Vec<JobInfo>) -> Self {
        DialogContent::JobManager(JobManagerContent::new())
    }

    /// Get job manager helper if this is a job manager dialog
    pub fn as_job_manager(&self) -> Option<usize> {
        match self {
            DialogContent::JobManager(JobManagerContent { selected_index, .. }) => {
                Some(*selected_index)
            }
            _ => None,
        }
    }
}

/// Custom function selector dialog helper
pub struct CustomFunctionSelector {
    functions: Vec<CustomFunction>,
    filter: String,
    selected_index: usize,
    filtered_indices: Vec<usize>,
}

impl CustomFunctionSelector {
    /// Create a new custom function selector
    pub fn new(functions: Vec<CustomFunction>) -> Self {
        let filtered_indices: Vec<usize> = (0..functions.len()).collect();
        Self {
            functions,
            filter: String::new(),
            selected_index: 0,
            filtered_indices,
        }
    }

    /// Get all functions
    pub fn functions(&self) -> &[CustomFunction] {
        &self.functions
    }

    /// Get filtered functions
    pub fn filtered_functions(&self) -> Vec<&CustomFunction> {
        self.filtered_indices
            .iter()
            .filter_map(|&i| self.functions.get(i))
            .collect()
    }

    /// Get the selected function
    pub fn selected_function(&self) -> Option<&CustomFunction> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&i| self.functions.get(i))
    }

    /// Get the current filter
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Update the filter and recompute filtered indices
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.update_filtered_indices();
        self.selected_index = 0;
    }

    /// Update filtered indices based on current filter
    fn update_filtered_indices(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.functions.len()).collect();
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.filtered_indices = self
                .functions
                .iter()
                .enumerate()
                .filter(|(_, func)| {
                    func.name.to_lowercase().contains(&filter_lower)
                        || func
                            .description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(&filter_lower))
                            .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.filtered_indices.len() {
            self.selected_index += 1;
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
    }

    /// Check if there are any filtered functions
    pub fn is_empty(&self) -> bool {
        self.filtered_indices.is_empty()
    }

    /// Get filtered count
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Get total count
    pub fn total_count(&self) -> usize {
        self.functions.len()
    }
}

impl DialogContent {
    /// Get custom function selector helper if this is a custom function selector dialog
    pub fn as_custom_function_selector(&self) -> Option<(&[CustomFunction], &str, usize)> {
        match self {
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                functions,
                filter,
                selected_index,
            }) => Some((functions, filter, *selected_index)),
            _ => None,
        }
    }

    /// Get mutable custom function selector data
    pub fn as_custom_function_selector_mut(
        &mut self,
    ) -> Option<(&mut Vec<CustomFunction>, &mut String, &mut usize)> {
        match self {
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                functions,
                filter,
                selected_index,
            }) => Some((functions, filter, selected_index)),
            _ => None,
        }
    }
}

/// Registered folder selector dialog helper
pub struct RegisteredFolderSelector {
    folders: Vec<RegisteredFolder>,
    filter: String,
    selected_index: usize,
    filtered_indices: Vec<usize>,
}

impl RegisteredFolderSelector {
    /// Create a new registered folder selector
    pub fn new(folders: Vec<RegisteredFolder>) -> Self {
        let filtered_indices: Vec<usize> = (0..folders.len()).collect();
        Self {
            folders,
            filter: String::new(),
            selected_index: 0,
            filtered_indices,
        }
    }

    /// Get all folders
    pub fn folders(&self) -> &[RegisteredFolder] {
        &self.folders
    }

    /// Get filtered folders
    pub fn filtered_folders(&self) -> Vec<&RegisteredFolder> {
        self.filtered_indices
            .iter()
            .filter_map(|&i| self.folders.get(i))
            .collect()
    }

    /// Get the selected folder
    pub fn selected_folder(&self) -> Option<&RegisteredFolder> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&i| self.folders.get(i))
    }

    /// Get the current filter
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Update the filter and recompute filtered indices
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.update_filtered_indices();
        self.selected_index = 0;
    }

    /// Update filtered indices based on current filter
    fn update_filtered_indices(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.folders.len()).collect();
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.filtered_indices = self
                .folders
                .iter()
                .enumerate()
                .filter(|(_, folder)| {
                    folder.name.to_lowercase().contains(&filter_lower)
                        || folder.path.to_lowercase().contains(&filter_lower)
                        || folder
                            .description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(&filter_lower))
                            .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.filtered_indices.len() {
            self.selected_index += 1;
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
    }

    /// Check if there are any filtered folders
    pub fn is_empty(&self) -> bool {
        self.filtered_indices.is_empty()
    }

    /// Get filtered count
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Get total count
    pub fn total_count(&self) -> usize {
        self.folders.len()
    }
}

impl DialogContent {
    /// Get registered folder selector helper if this is a registered folder selector dialog
    pub fn as_registered_folder_selector(&self) -> Option<(&[RegisteredFolder], &str, usize)> {
        match self {
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                folders,
                filter,
                selected_index,
            }) => Some((folders, filter, *selected_index)),
            _ => None,
        }
    }

    /// Get mutable registered folder selector data
    pub fn as_registered_folder_selector_mut(
        &mut self,
    ) -> Option<(&mut Vec<RegisteredFolder>, &mut String, &mut usize)> {
        match self {
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                folders,
                filter,
                selected_index,
            }) => Some((folders, filter, selected_index)),
            _ => None,
        }
    }
}

/// Tab selector dialog helper
pub struct TabSelector {
    tabs: Vec<String>,
    selected_index: usize,
}

impl TabSelector {
    /// Create a new tab selector
    pub fn new(tabs: Vec<String>) -> Self {
        Self {
            tabs,
            selected_index: 0,
        }
    }

    /// Get all tabs
    pub fn tabs(&self) -> &[String] {
        &self.tabs
    }

    /// Get the selected tab
    pub fn selected_tab(&self) -> Option<&String> {
        self.tabs.get(self.selected_index)
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Set the selected index
    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.selected_index = index;
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.tabs.len() {
            self.selected_index += 1;
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.tabs.is_empty() {
            self.selected_index = self.tabs.len() - 1;
        }
    }

    /// Check if there are any tabs
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Get tab count
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
}

impl DialogContent {
    /// Get tab selector helper if this is a tab selector dialog
    pub fn as_tab_selector(&self) -> Option<(&[String], usize)> {
        match self {
            DialogContent::TabSelector(TabSelectorContent {
                tabs,
                selected_index,
            }) => Some((tabs, *selected_index)),
            _ => None,
        }
    }

    /// Get mutable tab selector data
    pub fn as_tab_selector_mut(&mut self) -> Option<(&mut Vec<String>, &mut usize)> {
        match self {
            DialogContent::TabSelector(TabSelectorContent {
                tabs,
                selected_index,
            }) => Some((tabs, selected_index)),
            _ => None,
        }
    }

    /// Get context menu data if this is a context menu dialog
    pub fn as_context_menu(&self) -> Option<(&[ContextMenuOption], usize)> {
        match self {
            DialogContent::ContextMenu(ContextMenuDialog {
                options,
                selected_index,
            }) => Some((options, *selected_index)),
            _ => None,
        }
    }

    /// Get mutable context menu data
    pub fn as_context_menu_mut(&mut self) -> Option<(&mut Vec<ContextMenuOption>, &mut usize)> {
        match self {
            DialogContent::ContextMenu(ContextMenuDialog {
                options,
                selected_index,
            }) => Some((options, selected_index)),
            _ => None,
        }
    }

    /// Get drive selection data if this is a drive selection dialog
    pub fn as_drive_selection(&self) -> Option<(&[DriveInfo], usize, &str)> {
        match self {
            DialogContent::DriveSelection(DriveSelectionDialog {
                drives,
                selected_index,
                filter,
            }) => Some((drives, *selected_index, filter.as_str())),
            _ => None,
        }
    }

    /// Get mutable drive selection data
    pub fn as_drive_selection_mut(
        &mut self,
    ) -> Option<(&mut Vec<DriveInfo>, &mut usize, &mut String)> {
        match self {
            DialogContent::DriveSelection(DriveSelectionDialog {
                drives,
                selected_index,
                filter,
            }) => Some((drives, selected_index, filter)),
            _ => None,
        }
    }
}

/// Pattern rename dialog helper
pub struct PatternRenameDialog {
    find: String,
    replace: String,
    use_regex: bool,
    case_sensitive: bool,
    preview: Vec<(String, String)>,
}

impl PatternRenameDialog {
    pub fn new(
        find: String,
        replace: String,
        use_regex: bool,
        case_sensitive: bool,
        preview: Vec<(String, String)>,
    ) -> Self {
        Self {
            find,
            replace,
            use_regex,
            case_sensitive,
            preview,
        }
    }

    pub fn find(&self) -> &str {
        &self.find
    }
    pub fn replace(&self) -> &str {
        &self.replace
    }
    pub fn use_regex(&self) -> bool {
        self.use_regex
    }
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }
    pub fn preview(&self) -> &[(String, String)] {
        &self.preview
    }
    pub fn set_preview(&mut self, preview: Vec<(String, String)>) {
        self.preview = preview;
    }
    pub fn is_empty(&self) -> bool {
        self.preview.is_empty()
    }
    pub fn count(&self) -> usize {
        self.preview.len()
    }
    pub fn get_preview(&self, index: usize) -> Option<&(String, String)> {
        self.preview.get(index)
    }
    pub fn is_valid(&self) -> bool {
        !self.find.is_empty()
    }
}

impl DialogContent {
    /// Get pattern rename fields: (find, replace, use_regex, case_sensitive, preview)
    #[allow(clippy::type_complexity)] // tuple mirrors PatternRename's fields; a type alias would obscure the field-order mapping used at call sites
    pub fn as_pattern_rename(&self) -> Option<(&str, &str, bool, bool, &[(String, String)])> {
        match self {
            DialogContent::PatternRename {
                find,
                replace,
                use_regex,
                case_sensitive,
                preview,
                ..
            } => Some((find, replace, *use_regex, *case_sensitive, preview)),
            _ => None,
        }
    }

    /// Get mutable pattern rename fields
    #[allow(clippy::type_complexity)] // tuple mirrors PatternRename's fields; a type alias would obscure the field-order mapping used at call sites
    pub fn as_pattern_rename_mut(
        &mut self,
    ) -> Option<(
        &mut String,
        &mut String,
        &mut bool,
        &mut bool,
        &mut Vec<(String, String)>,
    )> {
        match self {
            DialogContent::PatternRename {
                find,
                replace,
                use_regex,
                case_sensitive,
                preview,
                ..
            } => Some((find, replace, use_regex, case_sensitive, preview)),
            _ => None,
        }
    }

    /// Get split/join dialog data
    pub fn as_split_join(&self) -> Option<(SplitJoinMode, u64)> {
        match self {
            DialogContent::SplitJoinDialog {
                mode,
                chunk_size_mb,
            } => Some((*mode, *chunk_size_mb)),
            _ => None,
        }
    }

    /// Get mutable split/join dialog data
    pub fn as_split_join_mut(&mut self) -> Option<(&mut SplitJoinMode, &mut u64)> {
        match self {
            DialogContent::SplitJoinDialog {
                mode,
                chunk_size_mb,
            } => Some((mode, chunk_size_mb)),
            _ => None,
        }
    }
}

/// AND-filter paths: all whitespace-split tokens must appear as case-insensitive substrings.
pub fn filter_jump_to_path_suggestions(candidates: &[String], query: &str) -> Vec<String> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();
    if tokens.is_empty() {
        candidates.to_vec()
    } else {
        candidates
            .iter()
            .filter(|p| {
                let lower = p.to_lowercase();
                tokens.iter().all(|t| lower.contains(t.as_str()))
            })
            .cloned()
            .collect()
    }
}

/// AND-filter file/dir paths: same semantics as filter_jump_to_path_suggestions.
pub fn filter_jump_to_file_suggestions(candidates: &[String], query: &str) -> Vec<String> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();
    if tokens.is_empty() {
        candidates.to_vec()
    } else {
        candidates
            .iter()
            .filter(|p| {
                let lower = p.to_lowercase();
                tokens.iter().all(|t| lower.contains(t.as_str()))
            })
            .cloned()
            .collect()
    }
}

/// Wrapper format for custom_functions.json (TWF-compatible)
#[derive(Debug, serde::Deserialize)]
struct CustomFunctionsFile {
    #[serde(rename = "Version", default)]
    #[allow(dead_code)]
    version: String,
    #[serde(rename = "Functions")]
    functions: Vec<CustomFunction>,
}

/// Load custom functions from a JSON file.
/// Accepts the wrapper format `{"Version":"1.0","Functions":[...]}` and a bare
/// array `[...]`. Menu entries whose `Menu` field is a filename string are loaded
/// recursively from the same directory as the parent file.
pub fn load_custom_functions(
    path: &std::path::Path,
) -> Result<Vec<CustomFunction>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
    let content = std::fs::read_to_string(path)?;

    let mut functions = parse_custom_functions(&content)?;
    resolve_menu_files(&mut functions, base_dir);
    Ok(functions)
}

fn parse_custom_functions(
    content: &str,
) -> Result<Vec<CustomFunction>, Box<dyn std::error::Error>> {
    // Preferred: wrapper object with Version + Functions
    if let Ok(file) = serde_json::from_str::<CustomFunctionsFile>(content) {
        return Ok(file.functions);
    }
    // Fallback: bare array
    if let Ok(functions) = serde_json::from_str::<Vec<CustomFunction>>(content) {
        return Ok(functions);
    }
    // Surface error from the wrapper format as primary
    let file: CustomFunctionsFile = serde_json::from_str(content)?;
    Ok(file.functions)
}

/// Resolve `Menu: "filename.json"` references into item lists.
/// Inline `Items` entries are left as-is (nested menus not supported in 6.6 scope).
fn resolve_menu_files(functions: &mut [CustomFunction], base_dir: &std::path::Path) {
    for func in functions.iter_mut() {
        if let Some(MenuContent::File(filename)) = &func.menu {
            let menu_path = base_dir.join(filename);
            match std::fs::read_to_string(&menu_path) {
                Ok(content) => match serde_json::from_str::<MenuFile>(&content) {
                    Ok(menu_file) => {
                        func.menu = Some(MenuContent::Items(menu_file.menus));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse menu file {:?}: {}", menu_path, e);
                        func.menu = Some(MenuContent::Items(Vec::new()));
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read menu file {:?}: {}", menu_path, e);
                    func.menu = Some(MenuContent::Items(Vec::new()));
                }
            }
        }
        // Inline Items stay as-is; no recursive nesting in 6.6
    }
}

// ============================================================================
// File Conflict Dialog Types
// ============================================================================

/// A single file conflict between source and destination
#[derive(Debug, Clone)]
pub struct ConflictPair {
    pub source: crate::model::FileEntry,
    pub dest: crate::model::FileEntry,
    pub source_path: crate::model::Location,
    pub dest_path: crate::model::Location,
    pub is_directory: bool,
}

/// User's decision for resolving a conflict
#[derive(Debug, Clone)]
pub enum ConflictAction {
    Force,
    OverwriteIfNewer,
    Skip,
    Rename { new_name: String },
}

/// Status of a file conflict for display
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictStatus {
    SameSizeDate,
    SameSizeSourceNewer,
    SameSizeDestNewer,
    DifferentSizeSourceNewer,
    DifferentSizeDestNewer,
    DirectoryMerge,
}

impl ConflictPair {
    /// Determine the conflict status for display
    pub fn get_status(&self) -> ConflictStatus {
        if self.is_directory {
            ConflictStatus::DirectoryMerge
        } else if self.source.size == self.dest.size {
            if self.source.modified == self.dest.modified {
                ConflictStatus::SameSizeDate
            } else if self.source.modified > self.dest.modified {
                ConflictStatus::SameSizeSourceNewer
            } else {
                ConflictStatus::SameSizeDestNewer
            }
        } else {
            // Different sizes
            if self.source.modified > self.dest.modified {
                ConflictStatus::DifferentSizeSourceNewer
            } else {
                ConflictStatus::DifferentSizeDestNewer
            }
        }
    }

    /// Get display indicator and message for the conflict status
    pub fn get_status_message(&self) -> (&'static str, String) {
        match self.get_status() {
            ConflictStatus::SameSizeDate => ("✓", "Same size and date".to_string()),
            ConflictStatus::SameSizeSourceNewer => ("✓", "Same size, Source is newer".to_string()),
            ConflictStatus::SameSizeDestNewer => {
                ("⚠", "Same size, Destination is newer".to_string())
            }
            ConflictStatus::DifferentSizeSourceNewer => (
                "⚠",
                format!(
                    "Different size (Source: {}, Dest: {})",
                    crate::model::file_entry::format_size(self.source.size),
                    crate::model::file_entry::format_size(self.dest.size)
                ),
            ),
            ConflictStatus::DifferentSizeDestNewer => (
                "⚠",
                format!(
                    "Different size (Source: {}, Dest: {})",
                    crate::model::file_entry::format_size(self.source.size),
                    crate::model::file_entry::format_size(self.dest.size)
                ),
            ),
            ConflictStatus::DirectoryMerge => ("⚠", "Will merge directories".to_string()),
        }
    }
}

#[cfg(test)]
mod custom_function_tests {
    use super::*;

    #[test]
    fn test_custom_function_deserialization() {
        let json = r#"{
            "Name": "Test Function",
            "Command": "echo $F",
            "Shell": "bash",
            "Description": "Test description"
        }"#;

        let func: CustomFunction = serde_json::from_str(json).unwrap();
        assert_eq!(func.name, "Test Function");
        assert_eq!(func.command, Some("echo $F".to_string()));
        assert_eq!(func.shell, Some("bash".to_string()));
    }

    #[test]
    fn test_os_specific_command() {
        let func = CustomFunction {
            name: "Test".to_string(),
            command: Some("default command".to_string()),
            menu: None,
            description: None,
            category: None,
            shell: None,
            working_dir: None,
            pipe_to_action: None,
            os_specific: {
                let mut map = HashMap::new();
                map.insert(
                    "linux".to_string(),
                    OsConfig {
                        command: "linux command".to_string(),
                        shell: None,
                    },
                );
                map
            },
            key_binding: None,
        };

        #[cfg(target_os = "linux")]
        assert_eq!(func.get_command(), Some("linux command"));

        #[cfg(not(target_os = "linux"))]
        assert_eq!(func.get_command(), Some("default command"));
    }
}

/// Manager for registered folders with persistence and environment variable expansion
#[derive(Debug)]
pub struct RegisteredFolderManager {
    pub folders: Vec<RegisteredFolder>,
}

impl RegisteredFolderManager {
    pub fn new() -> Self {
        Self {
            folders: Vec::new(),
        }
    }

    /// Load registered folders from a JSON file
    pub fn load_from_file(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            self.folders = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    /// Save registered folders to a JSON file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.folders)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add a new registered folder
    pub fn add(&mut self, folder: RegisteredFolder) {
        self.folders.push(folder);
    }

    /// Remove a registered folder by index
    pub fn remove(&mut self, index: usize) -> Option<RegisteredFolder> {
        if index < self.folders.len() {
            Some(self.folders.remove(index))
        } else {
            None
        }
    }

    /// Expand environment variables in a folder path
    pub fn expand_path(&self, folder: &RegisteredFolder) -> std::path::PathBuf {
        let expanded = self.expand_env_vars(&folder.path);
        std::path::PathBuf::from(expanded)
    }

    /// Expand environment variables in a path string
    ///
    /// Supports multiple formats:
    /// - Windows: %VAR%
    /// - Unix: $VAR, ${VAR}
    /// - PowerShell: $env:VAR
    /// - Windows: %VAR%
    /// - PowerShell: $env:VAR
    /// - Unix with braces: ${VAR}
    /// - Unix simple: $VAR
    ///
    /// # Thread Safety
    /// This method is thread-safe for concurrent reads. It only reads environment
    /// variables using `std::env::var()` and does not modify any shared state.
    /// Multiple threads can safely call this method simultaneously.
    pub fn expand_env_vars(&self, path: &str) -> String {
        let mut result = path.to_string();
        let mut all_replacements = Vec::new();

        // Collect ALL patterns from the ORIGINAL string first, before any replacements
        // This prevents issues where one replacement invalidates indices for later patterns

        // Windows style: %VAR%
        #[cfg(target_os = "windows")]
        {
            let pattern = regex::Regex::new(r"%([^%]+)%").unwrap();
            for cap in pattern.captures_iter(path) {
                if let Some(var_name) = cap.get(1) {
                    if let Ok(value) = std::env::var(var_name.as_str()) {
                        all_replacements.push((
                            cap.get(0).unwrap().start(),
                            cap.get(0).unwrap().end(),
                            value,
                        ));
                    }
                }
            }
        }

        // PowerShell style: $env:VAR
        let ps_pattern = regex::Regex::new(r"\$env:([A-Za-z_][A-Za-z0-9_]*)").unwrap();
        for cap in ps_pattern.captures_iter(path) {
            if let Some(var_name) = cap.get(1) {
                if let Ok(value) = std::env::var(var_name.as_str()) {
                    all_replacements.push((
                        cap.get(0).unwrap().start(),
                        cap.get(0).unwrap().end(),
                        value,
                    ));
                }
            }
        }

        // Unix style with braces: ${VAR}
        let braces_pattern = regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
        for cap in braces_pattern.captures_iter(path) {
            if let Some(var_name) = cap.get(1) {
                if let Ok(value) = std::env::var(var_name.as_str()) {
                    all_replacements.push((
                        cap.get(0).unwrap().start(),
                        cap.get(0).unwrap().end(),
                        value,
                    ));
                }
            }
        }

        // Unix style without braces: $VAR (but not $env: which was already handled)
        // We need to exclude matches that are part of $env: or ${
        let simple_pattern = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
        for cap in simple_pattern.captures_iter(path) {
            let full_match = cap.get(0).unwrap();
            let start = full_match.start();
            let end = full_match.end();

            // Skip if this is part of $env: (check if "env:" follows the $)
            if path[start..].starts_with("$env:") {
                continue;
            }

            // Skip if this is part of ${ (check if '{' immediately follows the $)
            if path.as_bytes().get(start + 1) == Some(&b'{') {
                continue;
            }

            // Skip if this position is already covered by another replacement
            let already_covered = all_replacements
                .iter()
                .any(|(s, e, _)| (start >= *s && start < *e) || (end > *s && end <= *e));

            if !already_covered {
                if let Some(var_name) = cap.get(1) {
                    if let Ok(value) = std::env::var(var_name.as_str()) {
                        all_replacements.push((start, end, value));
                    }
                }
            }
        }

        // Sort by start position (descending) to apply replacements from end to start
        // This ensures indices remain valid as we modify the string
        all_replacements.sort_by(|a, b| b.0.cmp(&a.0));

        // Apply all replacements in reverse order
        for (start, end, value) in all_replacements {
            result.replace_range(start..end, &value);
        }

        result
    }

    /// Filter folders by query (searches name and path)
    pub fn filter(&self, query: &str) -> Vec<&RegisteredFolder> {
        if query.is_empty() {
            self.folders.iter().collect()
        } else {
            let query_lower = query.to_lowercase();
            self.folders
                .iter()
                .filter(|f| {
                    f.name.to_lowercase().contains(&query_lower)
                        || f.path.to_lowercase().contains(&query_lower)
                })
                .collect()
        }
    }

    /// Get the default path for registered_directory.json
    pub fn default_path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("rwf")
            .join("registered_directory.json")
    }
}

impl Default for RegisteredFolderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod registered_folder_tests {
    use super::*;

    #[test]
    fn test_new_registered_folder() {
        let folder = RegisteredFolder::new("Home".to_string(), "/home/user".to_string());
        assert_eq!(folder.name, "Home");
        assert_eq!(folder.path, "/home/user");
        assert_eq!(folder.description, None);
    }

    #[test]
    fn test_registered_folder_with_description() {
        let folder = RegisteredFolder::new("Home".to_string(), "/home/user".to_string())
            .with_description("My home directory".to_string());
        assert_eq!(folder.description, Some("My home directory".to_string()));
    }

    #[test]
    fn test_manager_add_remove() {
        let mut manager = RegisteredFolderManager::new();
        let folder = RegisteredFolder::new("Test".to_string(), "/test".to_string());

        manager.add(folder.clone());
        assert_eq!(manager.folders.len(), 1);

        let removed = manager.remove(0);
        assert_eq!(removed, Some(folder));
        assert_eq!(manager.folders.len(), 0);
    }

    #[test]
    fn test_expand_env_vars_unix_simple() {
        let manager = RegisteredFolderManager::new();
        std::env::set_var("TEST_VAR_UNIX_SIMPLE", "test_value");

        let result = manager.expand_env_vars("$TEST_VAR_UNIX_SIMPLE/path");
        assert_eq!(result, "test_value/path");

        std::env::remove_var("TEST_VAR_UNIX_SIMPLE");
    }

    #[test]
    fn test_expand_env_vars_unix_braces() {
        let manager = RegisteredFolderManager::new();
        std::env::set_var("TEST_VAR_UNIX_BRACES", "test_value");

        let result = manager.expand_env_vars("${TEST_VAR_UNIX_BRACES}/path");
        assert_eq!(result, "test_value/path");

        std::env::remove_var("TEST_VAR_UNIX_BRACES");
    }

    #[test]
    fn test_expand_env_vars_powershell() {
        let manager = RegisteredFolderManager::new();
        std::env::set_var("TEST_VAR_POWERSHELL", "test_value");

        let result = manager.expand_env_vars("$env:TEST_VAR_POWERSHELL/path");
        assert_eq!(result, "test_value/path");

        std::env::remove_var("TEST_VAR_POWERSHELL");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_expand_env_vars_windows() {
        let manager = RegisteredFolderManager::new();
        std::env::set_var("TEST_VAR_WINDOWS", "test_value");

        let result = manager.expand_env_vars("%TEST_VAR_WINDOWS%/path");
        assert_eq!(result, "test_value/path");

        std::env::remove_var("TEST_VAR_WINDOWS");
    }

    #[test]
    fn test_expand_path() {
        let manager = RegisteredFolderManager::new();
        std::env::set_var("TEST_VAR_EXPAND_PATH", "test_value");

        let folder =
            RegisteredFolder::new("Test".to_string(), "$TEST_VAR_EXPAND_PATH/path".to_string());
        let expanded = manager.expand_path(&folder);
        assert_eq!(expanded, std::path::PathBuf::from("test_value/path"));

        std::env::remove_var("TEST_VAR_EXPAND_PATH");
    }

    #[test]
    fn test_filter_empty_query() {
        let mut manager = RegisteredFolderManager::new();
        manager.add(RegisteredFolder::new(
            "Home".to_string(),
            "/home".to_string(),
        ));
        manager.add(RegisteredFolder::new(
            "Work".to_string(),
            "/work".to_string(),
        ));

        let filtered = manager.filter("");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_name() {
        let mut manager = RegisteredFolderManager::new();
        manager.add(RegisteredFolder::new(
            "Home".to_string(),
            "/home".to_string(),
        ));
        manager.add(RegisteredFolder::new(
            "Work".to_string(),
            "/work".to_string(),
        ));

        let filtered = manager.filter("home");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Home");
    }

    #[test]
    fn test_filter_by_path() {
        let mut manager = RegisteredFolderManager::new();
        manager.add(RegisteredFolder::new(
            "Home".to_string(),
            "/home/user".to_string(),
        ));
        manager.add(RegisteredFolder::new(
            "Work".to_string(),
            "/work/project".to_string(),
        ));

        let filtered = manager.filter("project");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Work");
    }

    #[test]
    fn test_filter_case_insensitive() {
        let mut manager = RegisteredFolderManager::new();
        manager.add(RegisteredFolder::new(
            "Home".to_string(),
            "/home".to_string(),
        ));

        let filtered = manager.filter("HOME");
        assert_eq!(filtered.len(), 1);
    }
}
