//! Data models for the file manager
//!
//! This module defines the core data structures used throughout the application.

pub mod attrs;
pub mod cache;
pub mod file_entry;
pub mod job_id;
pub mod leap;
pub mod location;
pub mod marking;
pub mod navigation;
pub mod navigation_cache;
pub mod operation_report;
pub mod pane;
pub mod polling;
pub mod search;
pub mod tab;
pub mod trash;
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

pub use attrs::{AttributeChange, FileOpOutcome, LinkCreateKind, TimestampChange};
pub use cache::{CacheStats, CachedDirectory, DirectoryCache};
pub use file_entry::{format_size, FileEntry, LinkKind};
pub use job_id::JobId;
pub use leap::{BackspaceResult, LeapState};
pub use location::Location;
pub use marking::MarkingModel;
pub use navigation::NavigationHistory;
pub use navigation_cache::NavigationStateCache;
pub use operation_report::{OperationRecord, OperationReport, ReversalAction, UndoAvailability};
pub use pane::{DisplayMode, PaneModel, SortMode, SortOrder};
pub use search::SearchModel;
pub use tab::{TabManager, TabState, TabViewerState};
pub use trash::{EmptyTrashScope, RestoreOutcome, TrashLocation, TrashOutcome, TrashRecord};
pub use ui::{
    split_columns, ActivePane, PaneSplit, UIMode, UIState, ViewerLayout, MIN_PANE_COLUMNS,
    PANE_SPLIT_STEP,
};
pub use viewer::{
    line_number_digits, viewer_text_columns, FileBytes, LineIndex, TextEncoding, ViewerBuffer,
    ViewerMode, ViewerState,
};
