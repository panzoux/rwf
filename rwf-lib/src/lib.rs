//! Two-Pane File Manager Library
//!
//! This library provides the core functionality for a two-pane file manager
//! built using the Reactive Worker Framework (rwf) pattern.
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

pub mod state;
pub mod job;
pub mod backend;
pub mod model;
pub mod worker_pool;
pub mod event_receiver;
pub mod input;
pub mod logging;
pub mod log_manager;
pub mod session;
pub mod macro_expander;
pub mod pipe_to_action;
pub mod pattern_rename;
pub mod config;
pub mod volume_info;

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
mod e2e_workflow_integration_tests;

#[cfg(test)]
mod concurrent_operations_integration_tests;

#[cfg(test)]
mod error_recovery_integration_tests;

#[cfg(test)]
mod pane_sync_swap_integration_tests;

#[cfg(test)]
mod context_menu_drive_selection_tests;

#[cfg(test)]
mod file_info_version_tests;

#[cfg(test)]
mod log_management_integration_tests;

#[cfg(test)]
mod edge_case_properties;

pub use state::{AppState, Transition, StateUpdateResult, AppConfig};
pub use job::{JobManager, JobId, JobSpec, JobKind, Job, JobResult};
pub use model::{Location, FileEntry, PaneModel, TabState, TabManager};
pub use worker_pool::{WorkerPool, JobEvent};
pub use event_receiver::{map_job_event_to_transition, process_pending_events, process_next_event};
pub use backend::{FilesystemBackend, LocalFilesystemBackend};
pub use input::{KeyBindings, Action, format_key_event, action_to_transitions};
pub use logging::{LogLevel, init_logging, default_log_dir};
pub use log_manager::{LogManager, LogEntry, LogEntryLevel};
pub use session::{SessionState, SessionError, save_session, restore_tabs, restore_marked_locations};
pub use macro_expander::MacroExpander;
pub use pipe_to_action::{process_pipe_to_action, PipeToActionResult};
pub use pattern_rename::{apply_pattern, generate_preview, validate_pattern};
pub use volume_info::{VolumeInfo, VolumeType, MarkedFileStats, get_drive_or_share_name, calculate_marked_stats, format_top_separator_info};
