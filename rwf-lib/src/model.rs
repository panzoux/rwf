//! Data models for the file manager
//!
//! This module defines the core data structures used throughout the application.

pub mod location;
pub mod file_entry;
pub mod pane;
pub mod tab;
pub mod search;
pub mod marking;
pub mod navigation;
pub mod ui;
pub mod dialog;
pub mod cache;
pub mod viewer;
pub mod navigation_cache;

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

pub use location::Location;
pub use file_entry::{FileEntry, format_size};
pub use pane::{PaneModel, SortMode, SortOrder, DisplayMode};
pub use tab::{TabState, TabManager, TabViewerState};
pub use search::SearchModel;
pub use marking::MarkingModel;
pub use navigation::NavigationHistory;
pub use ui::{UIState, ActivePane, UIMode, ViewerLayout};
pub use dialog::{
    DialogStack, Dialog, DialogContent, CustomFunction, RegisteredFolder, RegisteredFolderManager,
    PipeToAction, OsConfig, JobInfo, JobKind, JobState, JobManagerDialog,
    CustomFunctionSelector, RegisteredFolderSelector, TabSelector, PatternRenameDialog,
    ErrorType, SplitJoinMode, ContextMenuOption, ContextMenuAction, DriveInfo, DriveType
};
pub use cache::{DirectoryCache, CachedDirectory, CacheStats};
pub use viewer::{ViewerState, ViewerMode, TextEncoding, ViewerBuffer, LineIndex, FileBytes};
pub use navigation_cache::NavigationStateCache;
