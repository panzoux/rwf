//! Application state management
//!
//! This module defines the central AppState structure and the Transition enum
//! for explicit state changes following the AppState pattern.

use crate::job::{JobManager, JobId, JobSpec};
use crate::model::{TabManager, SearchModel, MarkingModel, UIState, DialogStack, DirectoryCache, ViewerState};
use std::time::Duration;

/// Central application state coordinating all components
#[derive(Debug)]
pub struct AppState {
    /// Tab manager with independent pane states
    pub tabs: TabManager,
    /// Job manager for background operations
    pub jobs: JobManager,
    /// Search state
    pub search: SearchModel,
    /// File marking state
    pub marking: MarkingModel,
    /// UI state
    pub ui: UIState,
    /// Dialog stack
    pub dialogs: DialogStack,
    /// Registered folder manager
    pub registered_folders: crate::model::RegisteredFolderManager,
    /// Directory cache for fast navigation
    pub cache: DirectoryCache,
    /// File viewer state (when in viewer mode)
    pub viewer: Option<ViewerState>,
    /// Application configuration
    pub config: AppConfig,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let mut registered_folders = crate::model::RegisteredFolderManager::new();
        // Try to load registered folders from default path
        let path = crate::model::RegisteredFolderManager::default_path();
        if let Err(e) = registered_folders.load_from_file(&path) {
            tracing::warn!("Failed to load registered folders: {}", e);
        }
        
        Self {
            tabs: TabManager::new(),
            jobs: JobManager::new(config.worker_pool_size),
            search: SearchModel::new(),
            marking: MarkingModel::new(),
            ui: UIState::new(),
            dialogs: DialogStack::new(),
            registered_folders,
            cache: DirectoryCache::new(Duration::from_secs(30)),
            viewer: None,
            config,
        }
    }
    
    /// Get a reference to the current active tab
    pub fn current_tab(&self) -> &crate::model::TabState {
        &self.tabs.tabs[self.tabs.active_index]
    }
    
    /// Get a mutable reference to the current active tab
    pub fn current_tab_mut(&mut self) -> &mut crate::model::TabState {
        &mut self.tabs.tabs[self.tabs.active_index]
    }
    
    /// Get a reference to the active pane in the current tab
    pub fn active_pane(&self) -> &crate::model::PaneModel {
        let tab = self.current_tab();
        match self.ui.active_pane {
            crate::model::ActivePane::Left => &tab.left_pane,
            crate::model::ActivePane::Right => &tab.right_pane,
        }
    }
    
    /// Get a mutable reference to the active pane in the current tab
    pub fn active_pane_mut(&mut self) -> &mut crate::model::PaneModel {
        let active_pane = self.ui.active_pane;
        let tab = self.current_tab_mut();
        match active_pane {
            crate::model::ActivePane::Left => &mut tab.left_pane,
            crate::model::ActivePane::Right => &mut tab.right_pane,
        }
    }
    
    /// Get a reference to the opposite (inactive) pane in the current tab
    pub fn opposite_pane(&self) -> &crate::model::PaneModel {
        let tab = self.current_tab();
        match self.ui.active_pane {
            crate::model::ActivePane::Left => &tab.right_pane,
            crate::model::ActivePane::Right => &tab.left_pane,
        }
    }

    /// Save current state to session storage
    pub fn save_session(&self) -> Result<(), crate::session::SessionError> {
        let session = crate::session::save_session(
            &self.tabs.tabs,
            self.tabs.active_index,
            self.ui.active_pane,
            &self.marking.marked_locations,
        );
        
        let path = crate::session::SessionState::default_path();
        session.save_to_file(&path)
    }

    /// Restore state from session storage
    pub fn restore_session(&mut self) -> Result<(), crate::session::SessionError> {
        let path = crate::session::SessionState::default_path();
        let session = crate::session::SessionState::load_from_file(&path)?;

        // Restore tabs
        self.tabs.tabs = crate::session::restore_tabs(&session);
        
        // Restore active tab index (ensure it's valid)
        if session.active_tab_index < self.tabs.tabs.len() {
            self.tabs.active_index = session.active_tab_index;
        } else {
            self.tabs.active_index = 0;
        }

        // Restore active pane
        self.ui.active_pane = session.active_pane.into();

        // Restore marked locations
        self.marking.marked_locations = crate::session::restore_marked_locations(&session);

        Ok(())
    }

    /// Create a new AppState with session restoration
    pub fn new_with_session(config: AppConfig) -> Self {
        let mut state = Self::new(config);
        
        // Try to restore session, but don't fail if it doesn't work
        if let Err(e) = state.restore_session() {
            tracing::warn!("Failed to restore session: {}", e);
        }
        
        state
    }
}

/// Application configuration
pub use crate::config::AppConfig;

/// Explicit state change operations
#[derive(Debug, Clone)]
pub enum Transition {
    // Navigation transitions
    CursorMove { pane: crate::model::ActivePane, delta: isize },
    CursorJump { pane: crate::model::ActivePane, position: usize },
    ChangeLocation { pane: crate::model::ActivePane, location: crate::model::Location },
    NavigateUp { pane: crate::model::ActivePane },
    NavigateHistory { pane: crate::model::ActivePane, direction: HistoryDirection },
    SwitchPane,
    
    // Tab management
    CreateTab,
    CloseTab { index: usize },
    SwitchTab { index: usize },
    NextTab,
    PrevTab,
    
    // Job operations
    EnqueueJob { spec: JobSpec },
    StartNextJob,
    UpdateJobProgress { job_id: JobId, progress: f64 },
    CompleteJob { job_id: JobId, result: crate::job::OpResult },
    CancelJob { job_id: JobId },
    AcknowledgeCancel { job_id: JobId },
    
    // View operations
    ChangeSortMode { pane: crate::model::ActivePane, mode: crate::model::SortMode },
    ChangeDisplayMode { pane: crate::model::ActivePane, mode: crate::model::DisplayMode },
    SetFileMask { pane: crate::model::ActivePane, mask: Option<String> },
    ToggleHidden,
    Refresh { pane: crate::model::ActivePane },
    RefreshAndClearMarks { pane: crate::model::ActivePane },
    RefreshNoClearMarks { pane: crate::model::ActivePane },
    
    // Pane operations
    SyncPanes,
    SwapPanes,
    
    // Marking operations
    ToggleMark { location: crate::model::Location },
    MarkAll,
    UnmarkAll,
    MarkPattern { pattern: String },
    MarkRange { start: usize, end: usize },
    InvertMarks,
    EnterRangeMarkingMode,
    
    // Search operations
    StartSearch { query: String },
    UpdateSearchQuery { query: String },
    UpdateSearchResults { results: Vec<crate::model::FileEntry> },
    NextSearchResult,
    PrevSearchResult,
    ClearSearch,
    
    // UI operations
    ChangeUIMode { mode: crate::model::UIMode },
    ShowDialog { dialog: crate::model::Dialog },
    CloseDialog,
    UpdateDialogInput { input: String },
    ConfirmDialog,
    CancelDialog,
    ShowContextMenu,
    ShowDriveChangeDialog,
    ShowFileInfo,
    ShowVersion,
    SaveLog,
    LaunchConfigurationProgram,
    
    // Configuration
    ReloadConfig,
    UpdateConfig { config: AppConfig },
    
    // Registered folder operations
    RegisterCurrentFolder { name: String },
    ShowRegisteredFolderDialog,
    NavigateToRegisteredFolder { folder_index: usize },
    MoveToRegisteredFolder { folder_index: usize },
    
    // Viewer operations
    OpenTextViewer { location: crate::model::Location },
    OpenHexViewer { location: crate::model::Location },
    CloseViewer,
    ViewerLoadComplete { contents: Vec<u8> },
    ViewerCycleEncoding,
    ViewerScrollDown,
    ViewerScrollUp,
    ViewerPageDown,
    ViewerPageUp,
    ViewerJumpToTop,
    ViewerJumpToBottom,
    ViewerMoveToLineStart,
    ViewerMoveToLineEnd { viewport_width: usize },
    ViewerStartSearch { query: String },
    ViewerFindNext,
    ViewerFindPrev,
    ViewerClearSearch,
    
    // Pattern rename operations
    ShowPatternRenameDialog,
    UpdatePatternRenamePattern { pattern: String },
    ExecutePatternRename { pattern: String, targets: Vec<crate::model::Location> },
    
    // File comparison and split/join operations
    CompareFiles { left: crate::model::Location, right: crate::model::Location },
    ShowComparisonView { diff: crate::job::FileDiff },
    CloseComparisonView,
    ShowSplitJoinDialog,
    ExecuteFileSplit { source: crate::model::Location, dest_dir: crate::model::Location, chunk_size: u64 },
    ExecuteFileJoin { parts: Vec<crate::model::Location>, dest: crate::model::Location },
    
    // Application control
    Quit,
    ExitAndChangeDirectory,
}

/// History navigation direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HistoryDirection {
    Back,
    Forward,
}

/// Result of applying a state transition
pub struct StateUpdateResult {
    /// Jobs to start
    pub jobs_to_start: Vec<JobSpec>,
    /// Jobs to cancel
    pub jobs_to_cancel: Vec<JobId>,
    /// Panes that need refreshing
    pub panes_to_refresh: Vec<PaneRefresh>,
    /// Whether the UI needs to be redrawn
    pub ui_changed: bool,
}

impl StateUpdateResult {
    /// Create an empty result with no side effects
    pub fn none() -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: Vec::new(),
            panes_to_refresh: Vec::new(),
            ui_changed: false,
        }
    }
    
    /// Create a result that starts a job
    pub fn with_job(job: JobSpec) -> Self {
        Self {
            jobs_to_start: vec![job],
            jobs_to_cancel: Vec::new(),
            panes_to_refresh: Vec::new(),
            ui_changed: true,
        }
    }
    
    /// Create a result that only triggers UI update
    pub fn with_ui_change() -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: Vec::new(),
            panes_to_refresh: Vec::new(),
            ui_changed: true,
        }
    }
    
    /// Create a result that refreshes a specific pane
    pub fn with_refresh(tab_id: usize, pane: crate::model::ActivePane) -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: Vec::new(),
            panes_to_refresh: vec![PaneRefresh { tab_id, pane }],
            ui_changed: true,
        }
    }
    
    /// Create a result that cancels a job
    pub fn with_cancel(job_id: JobId) -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: vec![job_id],
            panes_to_refresh: Vec::new(),
            ui_changed: true,
        }
    }
}

/// Identifies a pane that needs refreshing
#[derive(Debug, Clone)]
pub struct PaneRefresh {
    pub tab_id: usize,
    pub pane: crate::model::ActivePane,
}

/// Pure function to update state based on transition
pub fn update_state(state: &mut AppState, transition: Transition) -> StateUpdateResult {
    match transition {
        // Pane switching
        Transition::SwitchPane => {
            state.ui.active_pane = state.ui.active_pane.opposite();
            StateUpdateResult::with_ui_change()
        }
        
        // Tab management
        Transition::CreateTab => {
            let new_index = state.tabs.create_tab();
            state.tabs.active_index = new_index;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CloseTab { index } => {
            if state.tabs.close_tab(index) {
                StateUpdateResult::with_ui_change()
            } else {
                StateUpdateResult::none()
            }
        }
        
        Transition::NextTab => {
            state.tabs.switch_to_next();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::PrevTab => {
            state.tabs.switch_to_prev();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::SwitchTab { index } => {
            if index < state.tabs.tabs.len() {
                state.tabs.active_index = index;
                StateUpdateResult::with_ui_change()
            } else {
                StateUpdateResult::none()
            }
        }
        
        // Marking operations
        Transition::ToggleMark { location } => {
            state.marking.toggle(location);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::MarkAll => {
            let entries = state.active_pane().entries.clone();
            state.marking.mark_all(&entries);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::UnmarkAll => {
            state.marking.unmark_all();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::MarkPattern { pattern } => {
            let entries = state.active_pane().entries.clone();
            state.marking.mark_pattern(&entries, &pattern);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::MarkRange { start, end } => {
            let entries = state.active_pane().entries.clone();
            state.marking.mark_range(&entries, start, end);
            // Exit range marking mode after marking
            state.ui.range_marking_start = None;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::InvertMarks => {
            let entries = state.active_pane().entries.clone();
            state.marking.invert_marks(&entries);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::EnterRangeMarkingMode => {
            // Store the current cursor position as the start of the range
            let cursor = state.active_pane().cursor;
            state.ui.range_marking_start = Some(cursor);
            StateUpdateResult::with_ui_change()
        }
        
        // Cursor movement
        Transition::CursorMove { pane, delta } => {
            let tab = state.current_tab_mut();
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            
            if !pane_model.entries.is_empty() {
                let new_cursor = (pane_model.cursor as isize + delta)
                    .max(0)
                    .min(pane_model.entries.len() as isize - 1) as usize;
                pane_model.cursor = new_cursor;
                
                // Adjust scroll if needed (assuming visible height of 20 for now)
                let visible_height = 20;
                if pane_model.cursor < pane_model.scroll_offset {
                    pane_model.scroll_offset = pane_model.cursor;
                } else if pane_model.cursor >= pane_model.scroll_offset + visible_height {
                    pane_model.scroll_offset = pane_model.cursor - visible_height + 1;
                }
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CursorJump { pane, position } => {
            let tab = state.current_tab_mut();
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            
            if !pane_model.entries.is_empty() {
                pane_model.cursor = position.min(pane_model.entries.len() - 1);
                
                // Adjust scroll if needed
                let visible_height = 20;
                if pane_model.cursor < pane_model.scroll_offset {
                    pane_model.scroll_offset = pane_model.cursor;
                } else if pane_model.cursor >= pane_model.scroll_offset + visible_height {
                    pane_model.scroll_offset = pane_model.cursor - visible_height + 1;
                }
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        // Navigation
        Transition::ChangeLocation { pane, location } => {
            // Check cache first (before any mutable borrows)
            let cached_entries = state.cache.get(&location);
            
            let tab = state.current_tab_mut();
            
            // Add current location to history
            let current_location = match pane {
                crate::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                crate::model::ActivePane::Right => tab.right_pane.current_location.clone(),
            };
            tab.history.push(pane, current_location);
            
            // Update location
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            pane_model.current_location = location.clone();
            pane_model.cursor = 0;
            pane_model.scroll_offset = 0;
            
            // Use cached entries if available
            if let Some(entries) = cached_entries {
                pane_model.entries = entries;
                pane_model.apply_sort();
                StateUpdateResult::with_ui_change()
            } else {
                // Create job to read directory
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
                StateUpdateResult::with_job(job_spec)
            }
        }
        
        Transition::NavigateUp { pane } => {
            let tab = state.current_tab();
            let current_location = match pane {
                crate::model::ActivePane::Left => &tab.left_pane.current_location,
                crate::model::ActivePane::Right => &tab.right_pane.current_location,
            };
            
            if let Some(parent) = current_location.parent() {
                update_state(state, Transition::ChangeLocation { pane, location: parent })
            } else {
                StateUpdateResult::none()
            }
        }
        
        Transition::NavigateHistory { pane, direction } => {
            // Get location from history first
            let location = {
                let tab = state.current_tab_mut();
                match direction {
                    HistoryDirection::Back => tab.history.go_back(pane),
                    HistoryDirection::Forward => tab.history.go_forward(pane),
                }
            };
            
            if let Some(location) = location {
                // Check cache (before any mutable borrows)
                let cached_entries = state.cache.get(&location);
                
                // Now update the pane
                let tab = state.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.current_location = location.clone();
                pane_model.cursor = 0;
                pane_model.scroll_offset = 0;
                
                // Use cached entries if available
                if let Some(entries) = cached_entries {
                    pane_model.entries = entries;
                    pane_model.apply_sort();
                    StateUpdateResult::with_ui_change()
                } else {
                    // Create job to read directory
                    let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
                    StateUpdateResult::with_job(job_spec)
                }
            } else {
                StateUpdateResult::none()
            }
        }
        
        // Job operations
        Transition::EnqueueJob { spec } => {
            state.jobs.enqueue(spec);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::StartNextJob => {
            if state.jobs.can_start_job() {
                if let Some(spec) = state.jobs.pop_next_job() {
                    state.jobs.start_job(spec.clone());
                    StateUpdateResult::with_job(spec)
                } else {
                    StateUpdateResult::none()
                }
            } else {
                StateUpdateResult::none()
            }
        }
        
        Transition::UpdateJobProgress { job_id, progress } => {
            state.jobs.update_progress(job_id, progress);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CompleteJob { job_id, result } => {
            // Get the job spec before completing it to determine what panes need refreshing
            let job_spec = state.jobs.active.get(&job_id).map(|job| job.spec.clone());
            
            // Show error dialog and log error if job failed
            if let crate::job::OpResult::Failed(ref error_message) = result {
                if let Some(ref spec) = job_spec {
                    let operation_name = match &spec.kind {
                        crate::job::JobKind::ReadDirectory { location } => {
                            tracing::error!(
                                job_id = ?job_id,
                                location = %location.display_path(),
                                error = %error_message,
                                "Read directory operation failed"
                            );
                            "Read directory"
                        }
                        crate::job::JobKind::Copy { sources, dest } => {
                            tracing::error!(
                                job_id = ?job_id,
                                source_count = sources.len(),
                                dest = %dest.display_path(),
                                error = %error_message,
                                "Copy operation failed"
                            );
                            "Copy"
                        }
                        crate::job::JobKind::Move { sources, dest } => {
                            tracing::error!(
                                job_id = ?job_id,
                                source_count = sources.len(),
                                dest = %dest.display_path(),
                                error = %error_message,
                                "Move operation failed"
                            );
                            "Move"
                        }
                        crate::job::JobKind::Delete { targets } => {
                            tracing::error!(
                                job_id = ?job_id,
                                target_count = targets.len(),
                                error = %error_message,
                                "Delete operation failed"
                            );
                            "Delete"
                        }
                        crate::job::JobKind::Mkdir { location } => {
                            tracing::error!(
                                job_id = ?job_id,
                                location = %location.display_path(),
                                error = %error_message,
                                "Create directory operation failed"
                            );
                            "Create directory"
                        }
                        crate::job::JobKind::Rename { from, to } => {
                            tracing::error!(
                                job_id = ?job_id,
                                from = %from.display_path(),
                                to = %to.display_path(),
                                error = %error_message,
                                "Rename operation failed"
                            );
                            "Rename"
                        }
                        crate::job::JobKind::CalculateSize { location } => {
                            tracing::error!(
                                job_id = ?job_id,
                                location = %location.display_path(),
                                error = %error_message,
                                "Calculate size operation failed"
                            );
                            "Calculate size"
                        }
                        crate::job::JobKind::ExtractArchive { archive, dest } => {
                            tracing::error!(
                                job_id = ?job_id,
                                archive = %archive.display_path(),
                                dest = %dest.display_path(),
                                error = %error_message,
                                "Extract archive operation failed"
                            );
                            "Extract archive"
                        }
                        crate::job::JobKind::CreateArchive { sources, dest } => {
                            tracing::error!(
                                job_id = ?job_id,
                                source_count = sources.len(),
                                dest = %dest.display_path(),
                                error = %error_message,
                                "Create archive operation failed"
                            );
                            "Create archive"
                        }
                        crate::job::JobKind::ExecuteCustomFunction { command, working_dir, .. } => {
                            tracing::error!(
                                job_id = ?job_id,
                                command = %command,
                                working_dir = %working_dir.display_path(),
                                error = %error_message,
                                "Execute custom function operation failed"
                            );
                            "Execute custom function"
                        }
                        crate::job::JobKind::Search { location, pattern, .. } => {
                            tracing::error!(
                                job_id = ?job_id,
                                location = %location.display_path(),
                                pattern = %pattern,
                                error = %error_message,
                                "Search operation failed"
                            );
                            "Search"
                        }
                        crate::job::JobKind::LoadFileForViewer { location } => {
                            tracing::error!(
                                job_id = ?job_id,
                                location = %location.display_path(),
                                error = %error_message,
                                "Load file for viewer operation failed"
                            );
                            "Load file for viewer"
                        }
                        crate::job::JobKind::PatternRename { targets, pattern } => {
                            tracing::error!(
                                job_id = ?job_id,
                                target_count = targets.len(),
                                pattern = %pattern,
                                error = %error_message,
                                "Pattern rename operation failed"
                            );
                            "Pattern rename"
                        }
                        crate::job::JobKind::CompareFiles { left, right } => {
                            tracing::error!(
                                job_id = ?job_id,
                                left = %left.display_path(),
                                right = %right.display_path(),
                                error = %error_message,
                                "File comparison operation failed"
                            );
                            "File comparison"
                        }
                        crate::job::JobKind::SplitFile { source, dest_dir, chunk_size } => {
                            tracing::error!(
                                job_id = ?job_id,
                                source = %source.display_path(),
                                dest_dir = %dest_dir.display_path(),
                                chunk_size = %chunk_size,
                                error = %error_message,
                                "File split operation failed"
                            );
                            "File split"
                        }
                        crate::job::JobKind::JoinFiles { parts, dest } => {
                            tracing::error!(
                                job_id = ?job_id,
                                part_count = parts.len(),
                                dest = %dest.display_path(),
                                error = %error_message,
                                "File join operation failed"
                            );
                            "File join"
                        }
                    };
                    
                    let error_dialog = crate::model::Dialog::from_job_failure(operation_name, error_message);
                    state.dialogs.push(error_dialog);
                }
            }
            
            state.jobs.complete_job(job_id, result.clone());
            
            // Handle cache updates based on job type and result
            if let Some(ref spec) = job_spec {
                match &spec.kind {
                    crate::job::JobKind::ReadDirectory { location } => {
                        // If successful, insert into cache
                        if let crate::job::OpResult::Success(crate::job::SuccessData::DirectoryRead(entries)) = &result {
                            state.cache.insert(location.clone(), entries.clone());
                        }
                    }
                    crate::job::JobKind::Copy { dest, .. } |
                    crate::job::JobKind::Move { dest, .. } |
                    crate::job::JobKind::ExtractArchive { dest, .. } => {
                        // Invalidate cache for destination directory
                        state.cache.invalidate(dest);
                    }
                    crate::job::JobKind::Delete { targets } => {
                        // Invalidate cache for parent directories of affected files
                        for target in targets {
                            if let Some(parent) = target.parent() {
                                state.cache.invalidate(&parent);
                            }
                        }
                    }
                    crate::job::JobKind::Rename { from, .. } => {
                        // Invalidate cache for parent directory
                        if let Some(parent) = from.parent() {
                            state.cache.invalidate(&parent);
                        }
                    }
                    crate::job::JobKind::PatternRename { targets, .. } => {
                        // Invalidate cache for parent directories of all renamed files
                        for target in targets {
                            if let Some(parent) = target.parent() {
                                state.cache.invalidate(&parent);
                            }
                        }
                    }
                    crate::job::JobKind::Mkdir { location } => {
                        // Invalidate cache for parent directory
                        if let Some(parent) = location.parent() {
                            state.cache.invalidate(&parent);
                        }
                    }
                    _ => {}
                }
            }
            
            // Determine which panes need refreshing based on job type
            let mut result_obj = StateUpdateResult::with_ui_change();
            
            if let Some(spec) = job_spec {
                match &spec.kind {
                    crate::job::JobKind::ReadDirectory { location } => {
                        // Update panes that are viewing this location
                        if let crate::job::OpResult::Success(crate::job::SuccessData::DirectoryRead(entries)) = &result {
                            for tab in state.tabs.tabs.iter_mut() {
                                if tab.left_pane.current_location == *location {
                                    tab.left_pane.entries = entries.clone();
                                    tab.left_pane.apply_sort();
                                }
                                if tab.right_pane.current_location == *location {
                                    tab.right_pane.entries = entries.clone();
                                    tab.right_pane.apply_sort();
                                }
                            }
                        }
                    }
                    crate::job::JobKind::LoadFileForViewer { .. } => {
                        // Load file contents into viewer
                        if let crate::job::OpResult::Success(crate::job::SuccessData::FileContents(contents)) = result {
                            return update_state(state, Transition::ViewerLoadComplete { contents });
                        }
                    }
                    crate::job::JobKind::CompareFiles { .. } => {
                        // Show comparison view
                        if let crate::job::OpResult::Success(crate::job::SuccessData::ComparisonResult(diff)) = result {
                            return update_state(state, Transition::ShowComparisonView { diff });
                        }
                    }
                    crate::job::JobKind::SplitFile { dest_dir, .. } => {
                        // Refresh the destination directory
                        for (tab_idx, tab) in state.tabs.tabs.iter().enumerate() {
                            if tab.left_pane.current_location == *dest_dir {
                                result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                    tab_id: tab_idx,
                                    pane: crate::model::ActivePane::Left,
                                });
                            }
                            if tab.right_pane.current_location == *dest_dir {
                                result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                    tab_id: tab_idx,
                                    pane: crate::model::ActivePane::Right,
                                });
                            }
                        }
                    }
                    crate::job::JobKind::JoinFiles { dest, .. } => {
                        // Refresh the directory containing the joined file
                        if let Some(parent) = dest.parent() {
                            for (tab_idx, tab) in state.tabs.tabs.iter().enumerate() {
                                if tab.left_pane.current_location == parent {
                                    result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                        tab_id: tab_idx,
                                        pane: crate::model::ActivePane::Left,
                                    });
                                }
                                if tab.right_pane.current_location == parent {
                                    result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                        tab_id: tab_idx,
                                        pane: crate::model::ActivePane::Right,
                                    });
                                }
                            }
                        }
                    }
                    crate::job::JobKind::Copy { dest, .. } |
                    crate::job::JobKind::ExtractArchive { dest, .. } => {
                        // Refresh the pane that contains the destination
                        // Find which tab and pane contains this location
                        for (tab_idx, tab) in state.tabs.tabs.iter().enumerate() {
                            if tab.left_pane.current_location == *dest {
                                result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                    tab_id: tab_idx,
                                    pane: crate::model::ActivePane::Left,
                                });
                            }
                            if tab.right_pane.current_location == *dest {
                                result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                    tab_id: tab_idx,
                                    pane: crate::model::ActivePane::Right,
                                });
                            }
                        }
                    }
                    crate::job::JobKind::Move { sources, dest } => {
                        // For move operations, refresh both source and destination panes
                        // Refresh destination panes
                        for (tab_idx, tab) in state.tabs.tabs.iter().enumerate() {
                            if tab.left_pane.current_location == *dest {
                                result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                    tab_id: tab_idx,
                                    pane: crate::model::ActivePane::Left,
                                });
                            }
                            if tab.right_pane.current_location == *dest {
                                result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                    tab_id: tab_idx,
                                    pane: crate::model::ActivePane::Right,
                                });
                            }
                        }
                        
                        // Refresh source panes (where files were moved from)
                        for source in sources {
                            if let Some(source_parent) = source.parent() {
                                for (tab_idx, tab) in state.tabs.tabs.iter().enumerate() {
                                    if tab.left_pane.current_location == source_parent {
                                        result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Left,
                                        });
                                    }
                                    if tab.right_pane.current_location == source_parent {
                                        result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Right,
                                        });
                                    }
                                }
                            }
                        }
                        
                        // Unmark all moved files
                        state.marking.unmark_all();
                    }
                    crate::job::JobKind::Delete { .. } |
                    crate::job::JobKind::Rename { .. } |
                    crate::job::JobKind::PatternRename { .. } |
                    crate::job::JobKind::Mkdir { .. } => {
                        // Refresh the active pane
                        result_obj.panes_to_refresh.push(crate::state::PaneRefresh {
                            tab_id: state.tabs.active_index,
                            pane: state.ui.active_pane,
                        });
                        
                        // For delete operations, also unmark all deleted files
                        if matches!(spec.kind, crate::job::JobKind::Delete { .. }) {
                            state.marking.unmark_all();
                        }
                    }
                    crate::job::JobKind::CalculateSize { location } => {
                        // Update the FileEntry with the calculated size
                        if let crate::job::OpResult::Success(crate::job::SuccessData::SizeCalculated(size)) = result {
                            // Find and update the entry in all panes
                            for tab in state.tabs.tabs.iter_mut() {
                                // Update in left pane
                                if let Some(entry) = tab.left_pane.entries.iter_mut().find(|e| e.location == *location) {
                                    entry.calculated_size = Some(size);
                                }
                                // Update in right pane
                                if let Some(entry) = tab.right_pane.entries.iter_mut().find(|e| e.location == *location) {
                                    entry.calculated_size = Some(size);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            
            result_obj
        }
        
        Transition::CancelJob { job_id } => {
            if state.jobs.request_cancel(job_id) {
                StateUpdateResult::with_cancel(job_id)
            } else {
                StateUpdateResult::none()
            }
        }
        
        Transition::AcknowledgeCancel { job_id } => {
            state.jobs.acknowledge_cancel(job_id);
            StateUpdateResult::with_ui_change()
        }
        
        // View operations
        Transition::ChangeSortMode { pane, mode } => {
            let tab = state.current_tab_mut();
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            pane_model.sort_mode = mode;
            pane_model.apply_sort();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ChangeDisplayMode { pane, mode } => {
            let tab = state.current_tab_mut();
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            pane_model.display_mode = mode;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::SetFileMask { pane, mask } => {
            let tab = state.current_tab_mut();
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            pane_model.file_mask = mask;
            StateUpdateResult::with_ui_change()
        }
        
        // Dialog operations
        Transition::ShowDialog { dialog } => {
            state.dialogs.push(dialog);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CloseDialog => {
            state.dialogs.pop();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::UpdateDialogInput { input } => {
            state.dialogs.input_buffer = input.clone();
            
            // If in search mode, update search query in real-time
            if state.ui.mode == crate::model::UIMode::Search {
                state.search.query = input;
                
                // Filter entries in real-time
                let entries = state.active_pane().entries.clone();
                state.search.filter_entries(&entries);
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ConfirmDialog => {
            // Handle dialog confirmation based on dialog type
            if let Some(dialog) = state.dialogs.current() {
                match &dialog.content {
                    crate::model::DialogContent::Confirmation { message: _ } => {
                        // Determine what operation to perform based on dialog title
                        let title = dialog.title.as_str();
                        
                        if title == "Copy" {
                            // Extract sources and destination from the confirmation message
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
                            
                            if !sources.is_empty() {
                                let dest = state.opposite_pane().current_location.clone();
                                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Copy {
                                    sources,
                                    dest,
                                });
                                
                                state.dialogs.pop();
                                return StateUpdateResult::with_job(job_spec);
                            }
                        } else if title == "Move" {
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
                            
                            if !sources.is_empty() {
                                let dest = state.opposite_pane().current_location.clone();
                                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Move {
                                    sources,
                                    dest,
                                });
                                
                                state.dialogs.pop();
                                return StateUpdateResult::with_job(job_spec);
                            }
                        } else if title == "Delete" {
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
                            
                            if !targets.is_empty() {
                                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Delete {
                                    targets,
                                });
                                
                                state.dialogs.pop();
                                return StateUpdateResult::with_job(job_spec);
                            }
                        }
                    }
                    crate::model::DialogContent::Input { prompt: _, default_value: _ } => {
                        let title = dialog.title.as_str();
                        let input = state.dialogs.input_buffer.clone();
                        
                        if title == "Search" {
                            // Add to search history and move to first match
                            if !input.is_empty() {
                                state.search.add_to_history(input.clone());
                                
                                // Move cursor to first match if any
                                if let Some(first_result) = state.search.current_result() {
                                    // Find the index of the first result in the pane entries
                                    if let Some(index) = state.active_pane().entries.iter()
                                        .position(|e| e.location == first_result.location) {
                                        state.dialogs.pop();
                                        return update_state(state, Transition::CursorJump {
                                            pane: state.ui.active_pane,
                                            position: index,
                                        });
                                    }
                                }
                            }
                            
                            // Close dialog and exit search mode
                            state.dialogs.pop();
                            return update_state(state, Transition::ChangeUIMode {
                                mode: crate::model::UIMode::Normal,
                            });
                        } else if title == "Rename" {
                            // Rename the current cursor entry
                            if let Some(entry) = state.active_pane().current_entry() {
                                let from = entry.location.clone();
                                let to = from.parent()
                                    .map(|parent| parent.join(&input))
                                    .unwrap_or_else(|| from.clone());
                                
                                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Rename {
                                    from,
                                    to,
                                });
                                
                                state.dialogs.pop();
                                return StateUpdateResult::with_job(job_spec);
                            }
                        } else if title == "Create Directory" {
                            // Create directory in current location
                            let current_location = state.active_pane().current_location.clone();
                            let new_dir_location = current_location.join(&input);
                            
                            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Mkdir {
                                location: new_dir_location,
                            });
                            
                            state.dialogs.pop();
                            return StateUpdateResult::with_job(job_spec);
                        } else if title == "Wildcard Marking" {
                            // Mark files matching the wildcard pattern
                            if !input.is_empty() {
                                state.dialogs.pop();
                                return update_state(state, Transition::MarkPattern { pattern: input });
                            }
                        } else if title == "Register Folder" {
                            // Register the current folder with the given name
                            if !input.is_empty() {
                                state.dialogs.pop();
                                return update_state(state, Transition::RegisterCurrentFolder { name: input });
                            }
                        } else if title == "File Mask Filter" {
                            // Set file mask for the active pane
                            state.dialogs.pop();
                            let mask = if input.is_empty() { None } else { Some(input) };
                            return update_state(state, Transition::SetFileMask {
                                pane: state.ui.active_pane,
                                mask,
                            });
                        }
                    }
                    crate::model::DialogContent::RegisteredFolderSelector { folders: _, filter: _, selected_index } => {
                        // Clone the selected index to avoid borrow checker issues
                        let folder_index = *selected_index;
                        
                        // Determine if this is for navigation or moving files
                        if state.marking.count() > 0 {
                            // Move marked files to the selected folder
                            state.dialogs.pop();
                            return update_state(state, Transition::MoveToRegisteredFolder { folder_index });
                        } else {
                            // Navigate to the selected folder
                            state.dialogs.pop();
                            return update_state(state, Transition::NavigateToRegisteredFolder { folder_index });
                        }
                    }
                    crate::model::DialogContent::PatternRename { pattern, preview: _ } => {
                        // Execute pattern rename with the current pattern
                        let pattern_clone = pattern.clone();
                        
                        // Get target locations (marked files or cursor file)
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
                        
                        if !targets.is_empty() && !pattern_clone.is_empty() {
                            state.dialogs.pop();
                            return update_state(state, Transition::ExecutePatternRename {
                                pattern: pattern_clone,
                                targets,
                            });
                        }
                    }
                    crate::model::DialogContent::SplitJoinDialog { mode, chunk_size_mb } => {
                        // Execute split or join based on mode
                        let mode_clone = *mode;
                        let chunk_size = *chunk_size_mb * 1024 * 1024; // Convert MB to bytes
                        
                        match mode_clone {
                            crate::model::SplitJoinMode::Split => {
                                // Get the cursor file to split
                                if let Some(entry) = state.active_pane().current_entry() {
                                    let source = entry.location.clone();
                                    let dest_dir = state.opposite_pane().current_location.clone();
                                    
                                    state.dialogs.pop();
                                    return update_state(state, Transition::ExecuteFileSplit {
                                        source,
                                        dest_dir,
                                        chunk_size,
                                    });
                                }
                            }
                            crate::model::SplitJoinMode::Join => {
                                // Get marked files to join (or all files matching .part pattern)
                                let parts: Vec<crate::model::Location> = if state.marking.count() > 0 {
                                    state.active_pane()
                                        .entries
                                        .iter()
                                        .filter(|e| state.marking.is_marked(&e.location))
                                        .map(|e| e.location.clone())
                                        .collect()
                                } else {
                                    // Auto-detect .part files
                                    state.active_pane()
                                        .entries
                                        .iter()
                                        .filter(|e| e.name.contains(".part"))
                                        .map(|e| e.location.clone())
                                        .collect()
                                };
                                
                                if !parts.is_empty() {
                                    // Determine destination filename (remove .part000 suffix)
                                    let dest_name = if let Some(first_entry) = state.active_pane().entries.iter().find(|e| parts.contains(&e.location)) {
                                        // Remove .partXXX suffix
                                        first_entry.name.split(".part").next().unwrap_or(&first_entry.name).to_string()
                                    } else {
                                        "joined_file".to_string()
                                    };
                                    
                                    let dest = state.opposite_pane().current_location.join(&dest_name);
                                    
                                    state.dialogs.pop();
                                    return update_state(state, Transition::ExecuteFileJoin {
                                        parts,
                                        dest,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            
            // If we didn't handle the dialog, just close it
            state.dialogs.pop();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CancelDialog => {
            state.dialogs.pop();
            StateUpdateResult::with_ui_change()
        }
        
        // UI mode changes
        Transition::ChangeUIMode { mode } => {
            state.ui.mode = mode;
            StateUpdateResult::with_ui_change()
        }
        
        // Search operations
        Transition::StartSearch { query } => {
            state.search.query = query;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::UpdateSearchQuery { query } => {
            state.search.query = query.clone();
            
            // Filter entries in real-time
            let entries = state.active_pane().entries.clone();
            state.search.filter_entries(&entries);
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ClearSearch => {
            state.search.query.clear();
            state.search.results.clear();
            state.search.current_index = None;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::NextSearchResult => {
            if !state.search.results.is_empty() {
                let current = state.search.current_index.unwrap_or(0);
                state.search.current_index = Some((current + 1) % state.search.results.len());
                StateUpdateResult::with_ui_change()
            } else {
                StateUpdateResult::none()
            }
        }
        
        Transition::PrevSearchResult => {
            if !state.search.results.is_empty() {
                let current = state.search.current_index.unwrap_or(0);
                let new_index = if current == 0 {
                    state.search.results.len() - 1
                } else {
                    current - 1
                };
                state.search.current_index = Some(new_index);
                StateUpdateResult::with_ui_change()
            } else {
                StateUpdateResult::none()
            }
        }
        
        // Registered folder operations
        Transition::RegisterCurrentFolder { name } => {
            let current_location = state.active_pane().current_location.clone();
            let path = current_location.display_path();
            
            let folder = crate::model::RegisteredFolder::new(name, path);
            state.registered_folders.add(folder);
            
            // Save to file
            let save_path = crate::model::RegisteredFolderManager::default_path();
            if let Err(e) = state.registered_folders.save_to_file(&save_path) {
                tracing::error!("Failed to save registered folders: {}", e);
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ShowRegisteredFolderDialog => {
            let folders = state.registered_folders.folders.clone();
            let dialog = crate::model::Dialog::registered_folder_selector(folders);
            state.dialogs.push(dialog);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::NavigateToRegisteredFolder { folder_index } => {
            if let Some(folder) = state.registered_folders.folders.get(folder_index) {
                let expanded_path = state.registered_folders.expand_path(folder);
                let location = crate::model::Location::Local(expanded_path);
                
                // Close the dialog
                state.dialogs.pop();
                
                // Navigate to the folder
                return update_state(state, Transition::ChangeLocation {
                    pane: state.ui.active_pane,
                    location,
                });
            }
            StateUpdateResult::none()
        }
        
        Transition::MoveToRegisteredFolder { folder_index } => {
            if let Some(folder) = state.registered_folders.folders.get(folder_index) {
                let expanded_path = state.registered_folders.expand_path(folder);
                let dest = crate::model::Location::Local(expanded_path);
                
                // Get marked files
                let sources = if state.marking.count() > 0 {
                    state.active_pane()
                        .entries
                        .iter()
                        .filter(|e| state.marking.is_marked(&e.location))
                        .map(|e| e.location.clone())
                        .collect()
                } else {
                    vec![]
                };
                
                if !sources.is_empty() {
                    // Create move job
                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Move {
                        sources,
                        dest,
                    });
                    
                    return StateUpdateResult::with_job(job_spec);
                }
            }
            StateUpdateResult::none()
        }
        
        // Configuration operations
        Transition::ReloadConfig => {
            // Configuration reload is handled externally by loading config
            // and calling UpdateConfig transition
            StateUpdateResult::with_ui_change()
        }
        
        Transition::UpdateConfig { config } => {
            // Update the configuration
            state.config = config.clone();
            
            // Update worker pool size if changed
            if state.jobs.max_parallel != config.worker_pool_size {
                state.jobs.max_parallel = config.worker_pool_size;
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        // Viewer operations
        Transition::OpenTextViewer { location } => {
            // Create viewer state
            let mut viewer = crate::model::ViewerState::new(location.clone());
            viewer.mode = crate::model::ViewerMode::Text;
            state.viewer = Some(viewer);
            
            // Change UI mode to viewer
            state.ui.mode = crate::model::UIMode::Viewer;
            
            // Create job to load file contents
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                location,
            });
            
            StateUpdateResult::with_job(job_spec)
        }
        
        Transition::OpenHexViewer { location } => {
            // Create viewer state
            let mut viewer = crate::model::ViewerState::new(location.clone());
            viewer.mode = crate::model::ViewerMode::Hex;
            state.viewer = Some(viewer);
            
            // Change UI mode to viewer
            state.ui.mode = crate::model::UIMode::Viewer;
            
            // Create job to load file contents
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                location,
            });
            
            StateUpdateResult::with_job(job_spec)
        }
        
        Transition::CloseViewer => {
            state.viewer = None;
            state.ui.mode = crate::model::UIMode::Normal;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerLoadComplete { contents } => {
            if let Some(viewer) = &mut state.viewer {
                viewer.set_contents(contents);
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerCycleEncoding => {
            if let Some(viewer) = &mut state.viewer {
                viewer.cycle_encoding();
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerScrollDown => {
            if let Some(viewer) = &mut state.viewer {
                viewer.scroll_down(20); // TODO: Get viewport height from UI
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerScrollUp => {
            if let Some(viewer) = &mut state.viewer {
                viewer.scroll_up();
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerPageDown => {
            if let Some(viewer) = &mut state.viewer {
                viewer.page_down(20); // TODO: Get viewport height from UI
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerPageUp => {
            if let Some(viewer) = &mut state.viewer {
                viewer.page_up(20); // TODO: Get viewport height from UI
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerJumpToTop => {
            if let Some(viewer) = &mut state.viewer {
                viewer.jump_to_top();
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerJumpToBottom => {
            if let Some(viewer) = &mut state.viewer {
                viewer.jump_to_bottom(20); // TODO: Get viewport height from UI
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerMoveToLineStart => {
            if let Some(viewer) = &mut state.viewer {
                viewer.move_to_line_start();
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerMoveToLineEnd { viewport_width } => {
            if let Some(viewer) = &mut state.viewer {
                viewer.move_to_line_end(viewport_width);
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerStartSearch { query } => {
            if let Some(viewer) = &mut state.viewer {
                viewer.start_search(query);
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerFindNext => {
            if let Some(viewer) = &mut state.viewer {
                viewer.find_next();
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerFindPrev => {
            if let Some(viewer) = &mut state.viewer {
                viewer.find_prev();
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ViewerClearSearch => {
            if let Some(viewer) = &mut state.viewer {
                viewer.clear_search();
            }
            StateUpdateResult::with_ui_change()
        }
        
        // Pattern rename operations
        Transition::ShowPatternRenameDialog => {
            // Get files to rename (marked files or cursor file)
            let targets = if state.marking.count() > 0 {
                state.active_pane()
                    .entries
                    .iter()
                    .filter(|e| state.marking.is_marked(&e.location))
                    .map(|e| e.name.clone())
                    .collect()
            } else if let Some(entry) = state.active_pane().current_entry() {
                vec![entry.name.clone()]
            } else {
                vec![]
            };
            
            if !targets.is_empty() {
                let dialog = crate::model::Dialog::pattern_rename(String::new(), vec![]);
                state.dialogs.push(dialog);
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::UpdatePatternRenamePattern { pattern } => {
            // Get filenames to preview first (before borrowing dialog mutably)
            let filenames = if state.marking.count() > 0 {
                state.active_pane()
                    .entries
                    .iter()
                    .filter(|e| state.marking.is_marked(&e.location))
                    .map(|e| e.name.clone())
                    .collect()
            } else if let Some(entry) = state.active_pane().current_entry() {
                vec![entry.name.clone()]
            } else {
                vec![]
            };
            
            // Generate preview
            let preview = crate::pattern_rename::generate_preview(&filenames, &pattern);
            
            // Update the pattern and preview in the dialog
            if let Some(dialog) = state.dialogs.current_mut() {
                if let Some((pattern_ref, preview_ref)) = dialog.content.as_pattern_rename_mut() {
                    *pattern_ref = pattern.clone();
                    *preview_ref = preview;
                }
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ExecutePatternRename { pattern, targets } => {
            // Create pattern rename job
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::PatternRename {
                targets,
                pattern,
            });
            
            state.dialogs.pop();
            StateUpdateResult::with_job(job_spec)
        }
        
        // File comparison and split/join operations
        Transition::CompareFiles { left, right } => {
            // Create comparison job
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::CompareFiles {
                left,
                right,
            });
            
            StateUpdateResult::with_job(job_spec)
        }
        
        Transition::ShowComparisonView { diff } => {
            // Show comparison view dialog
            let dialog = crate::model::Dialog::comparison_view(diff);
            state.dialogs.push(dialog);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CloseComparisonView => {
            // Close the comparison view dialog
            state.dialogs.pop();
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ShowSplitJoinDialog => {
            // Show split/join dialog
            let dialog = crate::model::Dialog::split_join_dialog();
            state.dialogs.push(dialog);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ExecuteFileSplit { source, dest_dir, chunk_size } => {
            // Create split job
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::SplitFile {
                source,
                dest_dir,
                chunk_size,
            });
            
            state.dialogs.pop();
            StateUpdateResult::with_job(job_spec)
        }
        
        Transition::ExecuteFileJoin { parts, dest } => {
            // Create join job
            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::JoinFiles {
                parts,
                dest,
            });
            
            state.dialogs.pop();
            StateUpdateResult::with_job(job_spec)
        }
        
        // Placeholder implementations for other transitions
        _ => StateUpdateResult::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ActivePane;

    #[test]
    fn test_current_tab() {
        let config = AppConfig::default();
        let state = AppState::new(config);
        
        let tab = state.current_tab();
        assert_eq!(tab.id, 0);
    }

    #[test]
    fn test_current_tab_mut() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let tab = state.current_tab_mut();
        tab.id = 42;
        
        assert_eq!(state.current_tab().id, 42);
    }

    #[test]
    fn test_active_pane_left() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.ui.active_pane = ActivePane::Left;
        
        let pane = state.active_pane();
        // Verify we get the left pane
        assert_eq!(pane.cursor, 0);
    }

    #[test]
    fn test_active_pane_right() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.ui.active_pane = ActivePane::Right;
        
        let pane = state.active_pane();
        // Verify we get the right pane
        assert_eq!(pane.cursor, 0);
    }

    #[test]
    fn test_active_pane_mut() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.ui.active_pane = ActivePane::Left;
        
        {
            let pane = state.active_pane_mut();
            pane.cursor = 5;
        }
        
        assert_eq!(state.current_tab().left_pane.cursor, 5);
        assert_eq!(state.current_tab().right_pane.cursor, 0);
    }

    #[test]
    fn test_opposite_pane_when_left_active() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.ui.active_pane = ActivePane::Left;
        
        // Modify right pane
        state.current_tab_mut().right_pane.cursor = 10;
        
        // opposite_pane should return right pane
        let opposite = state.opposite_pane();
        assert_eq!(opposite.cursor, 10);
    }

    #[test]
    fn test_opposite_pane_when_right_active() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.ui.active_pane = ActivePane::Right;
        
        // Modify left pane
        state.current_tab_mut().left_pane.cursor = 15;
        
        // opposite_pane should return left pane
        let opposite = state.opposite_pane();
        assert_eq!(opposite.cursor, 15);
    }

    #[test]
    fn test_helper_methods_with_multiple_tabs() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Create a second tab
        state.tabs.create_tab();
        state.tabs.active_index = 1;
        
        // Modify the second tab's left pane
        state.current_tab_mut().left_pane.cursor = 20;
        
        // Verify current_tab returns the second tab
        assert_eq!(state.current_tab().id, 1);
        assert_eq!(state.active_pane().cursor, 20);
    }

    #[test]
    fn test_switch_pane_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        assert_eq!(state.ui.active_pane, ActivePane::Left);
        
        let result = update_state(&mut state, Transition::SwitchPane);
        assert!(result.ui_changed);
        assert_eq!(state.ui.active_pane, ActivePane::Right);
        
        let result = update_state(&mut state, Transition::SwitchPane);
        assert!(result.ui_changed);
        assert_eq!(state.ui.active_pane, ActivePane::Left);
    }

    #[test]
    fn test_create_tab_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        assert_eq!(state.tabs.tabs.len(), 1);
        assert_eq!(state.tabs.active_index, 0);
        
        let result = update_state(&mut state, Transition::CreateTab);
        assert!(result.ui_changed);
        assert_eq!(state.tabs.tabs.len(), 2);
        assert_eq!(state.tabs.active_index, 1);
    }

    #[test]
    fn test_close_tab_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Create two more tabs
        state.tabs.create_tab();
        state.tabs.create_tab();
        assert_eq!(state.tabs.tabs.len(), 3);
        
        let result = update_state(&mut state, Transition::CloseTab { index: 1 });
        assert!(result.ui_changed);
        assert_eq!(state.tabs.tabs.len(), 2);
    }

    #[test]
    fn test_close_last_tab_fails() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let result = update_state(&mut state, Transition::CloseTab { index: 0 });
        assert!(!result.ui_changed);
        assert_eq!(state.tabs.tabs.len(), 1);
    }

    #[test]
    fn test_next_tab_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.tabs.create_tab();
        state.tabs.create_tab();
        
        assert_eq!(state.tabs.active_index, 0);
        
        let result = update_state(&mut state, Transition::NextTab);
        assert!(result.ui_changed);
        assert_eq!(state.tabs.active_index, 1);
        
        update_state(&mut state, Transition::NextTab);
        assert_eq!(state.tabs.active_index, 2);
        
        // Should wrap around
        update_state(&mut state, Transition::NextTab);
        assert_eq!(state.tabs.active_index, 0);
    }

    #[test]
    fn test_prev_tab_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        state.tabs.create_tab();
        state.tabs.create_tab();
        
        assert_eq!(state.tabs.active_index, 0);
        
        // Should wrap around to last tab
        let result = update_state(&mut state, Transition::PrevTab);
        assert!(result.ui_changed);
        assert_eq!(state.tabs.active_index, 2);
        
        update_state(&mut state, Transition::PrevTab);
        assert_eq!(state.tabs.active_index, 1);
    }

    #[test]
    fn test_toggle_mark_transition() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let location = Location::Local(PathBuf::from("/test/file.txt"));
        
        assert!(!state.marking.is_marked(&location));
        
        let result = update_state(&mut state, Transition::ToggleMark { location: location.clone() });
        assert!(result.ui_changed);
        assert!(state.marking.is_marked(&location));
        
        let result = update_state(&mut state, Transition::ToggleMark { location: location.clone() });
        assert!(result.ui_changed);
        assert!(!state.marking.is_marked(&location));
    }

    #[test]
    fn test_mark_all_transition() {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some entries to the active pane
        let entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "file2.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file2.txt")),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        state.current_tab_mut().left_pane.entries = entries;
        
        assert_eq!(state.marking.count(), 0);
        
        let result = update_state(&mut state, Transition::MarkAll);
        assert!(result.ui_changed);
        assert_eq!(state.marking.count(), 2);
    }

    #[test]
    fn test_unmark_all_transition() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Mark some locations
        state.marking.mark(Location::Local(PathBuf::from("/test/file1.txt")));
        state.marking.mark(Location::Local(PathBuf::from("/test/file2.txt")));
        assert_eq!(state.marking.count(), 2);
        
        let result = update_state(&mut state, Transition::UnmarkAll);
        assert!(result.ui_changed);
        assert_eq!(state.marking.count(), 0);
    }

    #[test]
    fn test_cursor_move_transition() {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some entries
        let entries: Vec<FileEntry> = (0..10).map(|i| FileEntry {
            name: format!("file{}.txt", i),
            location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }).collect();
        
        state.current_tab_mut().left_pane.entries = entries;
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        
        // Move down
        let result = update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: 3 
        });
        assert!(result.ui_changed);
        assert_eq!(state.current_tab().left_pane.cursor, 3);
        
        // Move up
        update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: -1 
        });
        assert_eq!(state.current_tab().left_pane.cursor, 2);
        
        // Try to move beyond bounds (should clamp)
        update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: 100 
        });
        assert_eq!(state.current_tab().left_pane.cursor, 9);
        
        update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: -100 
        });
        assert_eq!(state.current_tab().left_pane.cursor, 0);
    }

    #[test]
    fn test_cursor_jump_transition() {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some entries
        let entries: Vec<FileEntry> = (0..10).map(|i| FileEntry {
            name: format!("file{}.txt", i),
            location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }).collect();
        
        state.current_tab_mut().left_pane.entries = entries;
        
        let result = update_state(&mut state, Transition::CursorJump { 
            pane: ActivePane::Left, 
            position: 5 
        });
        assert!(result.ui_changed);
        assert_eq!(state.current_tab().left_pane.cursor, 5);
        
        // Jump beyond bounds (should clamp)
        update_state(&mut state, Transition::CursorJump { 
            pane: ActivePane::Left, 
            position: 100 
        });
        assert_eq!(state.current_tab().left_pane.cursor, 9);
    }

    #[test]
    fn test_change_location_creates_job() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let new_location = Location::Local(PathBuf::from("/new/path"));
        
        let result = update_state(&mut state, Transition::ChangeLocation { 
            pane: ActivePane::Left, 
            location: new_location.clone() 
        });
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
    }

    #[test]
    fn test_enqueue_job_transition() {
        use crate::job::{JobSpec, JobKind};
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let job_spec = JobSpec::new(JobKind::ReadDirectory { 
            location: Location::Local(PathBuf::from("/test")) 
        });
        
        assert_eq!(state.jobs.queue.len(), 0);
        
        let result = update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        assert!(result.ui_changed);
        assert_eq!(state.jobs.queue.len(), 1);
    }

    #[test]
    fn test_change_sort_mode_transition() {
        use crate::model::SortMode;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Name);
        
        let result = update_state(&mut state, Transition::ChangeSortMode { 
            pane: ActivePane::Left, 
            mode: SortMode::Size 
        });
        assert!(result.ui_changed);
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Size);
    }

    #[test]
    fn test_save_and_restore_session() {
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Create additional tabs
        update_state(&mut state, Transition::CreateTab);
        update_state(&mut state, Transition::CreateTab);
        
        // After creating 2 tabs, we're on tab 2 (index 2)
        // Switch back to first tab, then to second tab
        update_state(&mut state, Transition::SwitchTab { index: 0 });
        update_state(&mut state, Transition::NextTab);
        
        // Verify we're on the second tab (index 1)
        assert_eq!(state.tabs.active_index, 1, "Should be on second tab after NextTab");
        
        // Mark some files (we'll use dummy locations for testing)
        let loc1 = crate::model::Location::Local(PathBuf::from("/test1"));
        let loc2 = crate::model::Location::Local(PathBuf::from("/test2"));
        state.marking.mark(loc1.clone());
        state.marking.mark(loc2.clone());
        
        // Save session
        let session_path = std::env::temp_dir().join("test_rwf_session.json");
        let session = crate::session::save_session(
            &state.tabs.tabs,
            state.tabs.active_index,
            state.ui.active_pane,
            &state.marking.marked_locations,
        );
        session.save_to_file(&session_path).unwrap();
        
        // Create a new state and restore
        let config2 = AppConfig::default();
        let mut state2 = AppState::new(config2);
        
        let loaded_session = crate::session::SessionState::load_from_file(&session_path).unwrap();
        state2.tabs.tabs = crate::session::restore_tabs(&loaded_session);
        state2.tabs.active_index = loaded_session.active_tab_index;
        state2.marking.marked_locations = crate::session::restore_marked_locations(&loaded_session);
        
        // Verify restoration
        assert_eq!(state2.tabs.tabs.len(), 3);
        assert_eq!(state2.tabs.active_index, 1);
        assert_eq!(state2.marking.count(), 2);
        assert!(state2.marking.is_marked(&loc1));
        assert!(state2.marking.is_marked(&loc2));
        
        // Cleanup
        let _ = std::fs::remove_file(session_path);
    }

    #[test]
    fn test_new_with_session_handles_missing_file() {
        // This should not panic even if session file doesn't exist
        let config = AppConfig::default();
        let state = AppState::new_with_session(config);
        
        // Should have default state with one tab
        assert_eq!(state.tabs.tabs.len(), 1);
        assert_eq!(state.tabs.active_index, 0);
    }
}

// Property-based tests
#[cfg(test)]
#[path = "state_properties.rs"]
mod state_properties;
