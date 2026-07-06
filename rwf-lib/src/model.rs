//! Data models for the file manager
//!
//! This module defines the core data structures used throughout the application.

pub mod cache;
pub mod dialog;
pub mod file_entry;
pub mod leap;
pub mod location;
pub mod marking;
pub mod navigation;
pub mod navigation_cache;
pub mod pane;
pub mod search;
pub mod tab;
pub mod ui;
pub mod viewer;

#[cfg(test)]
mod location_properties;

#[cfg(test)]
mod pane_properties;

#[cfg(test)]
mod tab_properties;

#[cfg(test)]
mod marking_properties;

#[cfg(test)]
mod registered_folder_properties;

#[cfg(test)]
mod cache_properties;

pub use cache::{CacheStats, CachedDirectory, DirectoryCache};
pub use dialog::{
    CloseTabWithActiveJobDialog, ConfirmationDialog, ContextMenuAction, ContextMenuDialog,
    ContextMenuOption, CustomFunction, CustomFunctionSelector, DeleteConfirmDialog, Dialog,
    DialogContent, DialogStack, DialogUiState, DriveInfo, DriveSelectionDialog, DriveType,
    ErrorDialog, ErrorType, ExtractionConfirmDialog, FileInfoDialog, FileMaskDialog, HelpDialog,
    HistoryDialogContent, InputDialog, JobInfo, JobKind, JobManagerDialog, JobState,
    JumpToFileDialog, JumpToPathDialog, OsConfig, PatternRenameDialog, PipeToAction,
    ProgressDialog, RegisteredFolder, RegisteredFolderManager, RegisteredFolderSelector,
    RegisteredFolderSelectorContent, SimpleRenameDialog, SortDialog, SplitJoinMode, TabSelector,
    TabSelectorContent, VersionDialog, WildcardMarkDialog,
};
pub use file_entry::{format_size, FileEntry, LinkKind};
pub use leap::{BackspaceResult, LeapState};
pub use location::Location;
pub use marking::MarkingModel;
pub use navigation::NavigationHistory;
pub use navigation_cache::NavigationStateCache;
pub use pane::{DisplayMode, PaneModel, SortMode, SortOrder};
pub use search::SearchModel;
pub use tab::{TabManager, TabState, TabViewerState};
pub use ui::{ActivePane, UIMode, UIState, ViewerLayout};
pub use viewer::{FileBytes, LineIndex, TextEncoding, ViewerBuffer, ViewerMode, ViewerState};
