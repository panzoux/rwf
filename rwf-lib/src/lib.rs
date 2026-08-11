//! Two-Pane File Manager Library
//!
//! This library provides the core functionality for a two-pane file manager
//! built using the rwf pattern.
//!
//! ## Architecture
//!
//! - **AppState**: Central application state coordinating all components
//! - **Transition**: Explicit state change operations
//! - **JobManager**: Manages background file operations via rwf Worker Pool
//! - **FilesystemBackend**: Abstraction for file I/O operations
//!
//! ## Key Principles
//!
//! 1. All file I/O operations execute as Jobs in the rwf Worker Pool
//! 2. State changes occur through explicit Transition enum values
//! 3. Pure state functions return StateUpdateResult with side effects
//! 4. FIFO job ordering with cooperative cancellation

pub mod backend;
pub mod config;
pub mod diagnostics;
pub mod event_receiver;
pub mod help_content;
pub mod input;
pub mod job;
pub mod leap_filter;
pub mod log_manager;
pub mod logging;
pub mod macro_expander;
pub mod magic;
pub mod model;
pub mod pattern_rename;
pub mod pipe_to_action;
pub mod session;
pub mod state;
pub mod volume_info;
pub mod worker_pool;

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod error_handling_tests;

#[cfg(test)]
mod custom_function_integration_tests;

#[cfg(test)]
mod archive_integration_tests;

#[cfg(test)]
mod registered_folder_integration_tests;

#[cfg(test)]
mod search_integration_tests;

#[cfg(test)]
mod file_filtering_integration_tests;

#[cfg(test)]
mod config_keybindings_tests;

#[cfg(test)]
mod config_display_tests;

#[cfg(test)]
mod config_reload_tests;

#[cfg(test)]
mod config_launch_integration_tests;

#[cfg(test)]
mod config_integration_tests;

#[cfg(test)]
mod cache_integration_tests;

#[cfg(test)]
mod viewer_integration_tests;

#[cfg(test)]
mod pattern_rename_integration_tests;

#[cfg(test)]
mod comparison_split_join_integration_tests;

#[cfg(test)]
mod advanced_marking_integration_tests;

#[cfg(test)]
mod directory_size_integration_tests;

#[cfg(test)]
mod scan_trash_confirm_integration_tests;

#[cfg(test)]
mod list_trash_browser_integration_tests;

#[cfg(test)]
mod e2e_workflow_integration_tests;

#[cfg(test)]
mod concurrent_operations_integration_tests;

#[cfg(test)]
mod error_recovery_integration_tests;

#[cfg(test)]
mod pane_sync_swap_integration_tests;

#[cfg(test)]
mod multi_language_help_integration_tests;

#[cfg(test)]
mod context_menu_drive_selection_tests;

#[cfg(test)]
mod file_info_version_tests;

#[cfg(test)]
mod log_management_integration_tests;

#[cfg(test)]
mod exit_cd_integration_tests;

#[cfg(test)]
mod task_panel_management_integration_tests;

#[cfg(test)]
mod scrolling_integration_tests;

#[cfg(test)]
mod edge_case_properties;

#[cfg(test)]
mod comprehensive_phase8_integration_tests;

#[cfg(test)]
mod sevenz_integration_tests;

#[cfg(test)]
mod tar_integration_tests;

#[cfg(test)]
mod archive_format_recognition_tests;

#[cfg(test)]
mod help_viewer_tests;

#[cfg(test)]
mod file_open_integration_tests;

pub use backend::{FilesystemBackend, LocalFilesystemBackend};
pub use event_receiver::{map_job_event_to_transition, process_next_event, process_pending_events};
pub use help_content::{
    DEFAULT_CUSTOM_FUNCTIONS, DEFAULT_EXTENSION_ASSOCIATIONS, DEFAULT_FILE_TYPE_MAP,
    DEFAULT_MENU_CONFIG,
};
pub use input::{
    action_to_transitions, check_keybindings_content_duplicates, check_keybindings_duplicates,
    expand_association_command, format_key_event, Action, ArchiveFormat, KeyBindings,
};
pub use job::{Job, JobId, JobKind, JobManager, JobResult, JobSpec};
pub use log_manager::{LogEntry, LogEntryLevel, LogManager};
pub use logging::{default_log_dir, init_logging, LogLevel};
pub use macro_expander::MacroExpander;
pub use model::{DialogContent, FileEntry, Location, PaneModel, TabManager, TabState};
pub use pattern_rename::{apply_rename_pattern, generate_preview, validate_inputs};
pub use pipe_to_action::{process_pipe_to_action, PipeToActionResult};
pub use session::{
    restore_marked_locations, restore_tabs, save_session, SessionError, SessionState,
};
pub use state::{AppConfig, AppState, StateUpdateResult, Transition};
pub use volume_info::{
    calculate_marked_stats, format_top_separator_info, get_drive_or_share_name, MarkedFileStats,
};
pub use worker_pool::{JobEvent, WorkerPool};
