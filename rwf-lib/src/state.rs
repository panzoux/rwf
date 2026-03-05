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
    UpdatePaneHeight { height: usize },
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
    UpdateConfig { config: Box<AppConfig> },
    
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
    use tracing::debug;
    
    match transition {
        // Pane switching
        Transition::SwitchPane => {
            let old_pane = state.ui.active_pane;
            state.ui.active_pane = state.ui.active_pane.opposite();
            debug!("SwitchPane transition: {:?} -> {:?}", old_pane, state.ui.active_pane);
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
            // Get visible height and scroll offset config before mutable borrow
            let visible_height = state.ui.layout.pane_height;
            let scroll_margin = state.config.ui.scroll_offset; // Use configured scroll offset
            
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
                
                // Only scroll if we have more entries than visible height
                if pane_model.entries.len() > visible_height {
                    // Calculate cursor position within visible area (design document algorithm)
                    let cursor_in_view = pane_model.cursor.saturating_sub(pane_model.scroll_offset);
                    let old_scroll = pane_model.scroll_offset;
                    
                    debug!("CursorMove scroll calc: cursor={}, old_scroll={}, cursor_in_view={}, scroll_margin={}, visible_height={}, entries={}",
                           pane_model.cursor, old_scroll, cursor_in_view, scroll_margin, visible_height, pane_model.entries.len());
                    
                    // Scroll up if cursor too close to top
                    if cursor_in_view < scroll_margin && pane_model.cursor > 0 {
                        pane_model.scroll_offset = pane_model.cursor.saturating_sub(scroll_margin);
                        debug!("  -> Scroll UP triggered: new_scroll={}", pane_model.scroll_offset);
                    }
                    // Scroll down if cursor too close to bottom
                    else if visible_height > scroll_margin {
                        let bottom_trigger = visible_height.saturating_sub(scroll_margin);
                        let max_offset = pane_model.entries.len().saturating_sub(visible_height) - 1;
                        
                        // Check if we're in the "end zone" where scroll_margin can't be maintained
                        let end_zone_start = pane_model.entries.len().saturating_sub(0).saturating_sub(scroll_margin) - 1;
                        
                        debug!("  ->> End zone: cursor={}, end_zone_start={}, scroll_offset={}, max_offset={}", 
                                   pane_model.cursor, end_zone_start, pane_model.scroll_offset, max_offset);
                        if pane_model.cursor >= end_zone_start {
                            // Near the end - just set scroll to max_offset to avoid blank lines
                            pane_model.scroll_offset = max_offset;
                            debug!("  -> End zone: cursor={}, end_zone_start={}, scroll_offset={}", 
                                   pane_model.cursor, end_zone_start, pane_model.scroll_offset);
                        } else if cursor_in_view >= bottom_trigger {
                            // Normal scrolling - maintain scroll_margin
                            let desired_offset = pane_model.cursor.saturating_sub(bottom_trigger);
                            pane_model.scroll_offset = desired_offset.min(max_offset);
                            debug!("  -> Scroll DOWN triggered: new_scroll={}, max={}", pane_model.scroll_offset, max_offset);
                        }
                    }
                } else {
                    // If all entries fit, no scrolling needed
                    pane_model.scroll_offset = 0;
                }
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        Transition::CursorJump { pane, position } => {
            // Get visible height and scroll offset config before mutable borrow
            let visible_height = state.ui.layout.pane_height;
            let scroll_margin = state.config.ui.scroll_offset; // Use configured scroll offset
            
            let tab = state.current_tab_mut();
            let pane_model = match pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            
            if !pane_model.entries.is_empty() {
                pane_model.cursor = position.min(pane_model.entries.len() - 1);
                
                // Only scroll if we have more entries than visible height
                if pane_model.entries.len() > visible_height {
                    // Calculate cursor position within visible area (design document algorithm)
                    let cursor_in_view = pane_model.cursor.saturating_sub(pane_model.scroll_offset);
                    let old_scroll = pane_model.scroll_offset;
                    
                    debug!("CursorJump scroll calc: cursor={}, old_scroll={}, cursor_in_view={}, scroll_margin={}, visible_height={}, entries={}",
                           pane_model.cursor, old_scroll, cursor_in_view, scroll_margin, visible_height, pane_model.entries.len());
                    
                    // First, ensure cursor is visible (handle large jumps)
                    if pane_model.cursor < pane_model.scroll_offset {
                        // Cursor jumped above visible area
                        pane_model.scroll_offset = pane_model.cursor.saturating_sub(scroll_margin);
                        debug!("  -> Cursor above viewport: new_scroll={}", pane_model.scroll_offset);
                    } else if pane_model.cursor >= pane_model.scroll_offset + visible_height {
                        // Cursor jumped below visible area
                        let desired_offset = pane_model.cursor + scroll_margin + 1 - visible_height;
                        let max_offset = pane_model.entries.len().saturating_sub(visible_height) - 1;
                        pane_model.scroll_offset = desired_offset.min(max_offset);
                        debug!("  -> Cursor below viewport: new_scroll={}", pane_model.scroll_offset);
                    } else {
                        // Cursor is visible, apply smooth scrolling logic
                        let cursor_in_view = pane_model.cursor.saturating_sub(pane_model.scroll_offset);
                        
                        // Scroll up if cursor too close to top
                        if cursor_in_view < scroll_margin && pane_model.cursor > 0 {
                            pane_model.scroll_offset = pane_model.cursor.saturating_sub(scroll_margin);
                            debug!("  -> Scroll UP triggered: new_scroll={}", pane_model.scroll_offset);
                        }
                        // Scroll down if cursor too close to bottom
                        else if visible_height > scroll_margin {
                            let bottom_trigger = visible_height.saturating_sub(scroll_margin);
                            let max_offset = pane_model.entries.len().saturating_sub(visible_height) - 1;
                            
                            // Check if we're in the "end zone" where scroll_margin can't be maintained
                            let end_zone_start = pane_model.entries.len().saturating_sub(1).saturating_sub(scroll_margin);
                            
                            if pane_model.cursor >= end_zone_start {
                                // Near the end - just set scroll to max_offset to avoid blank lines
                                pane_model.scroll_offset = max_offset;
                                debug!("  -> End zone: cursor={}, end_zone_start={}, scroll_offset={}", 
                                       pane_model.cursor, end_zone_start, pane_model.scroll_offset);
                            } else if cursor_in_view >= bottom_trigger {
                                // Normal scrolling - maintain scroll_margin
                                let desired_offset = pane_model.cursor.saturating_sub(bottom_trigger);
                                pane_model.scroll_offset = desired_offset.min(max_offset);
                                debug!("  -> Scroll DOWN triggered: new_scroll={}, max={}", pane_model.scroll_offset, max_offset);
                            }
                        }
                    }
                } else {
                    // If all entries fit, no scrolling needed
                    pane_model.scroll_offset = 0;
                }
            }
            
            StateUpdateResult::with_ui_change()
        }
        
        // Navigation
        Transition::ChangeLocation { pane, location } => {
            debug!("ChangeLocation: pane = {:?}, location = {}", pane, location.display_path());
            debug!("ChangeLocation: location debug format: {:?}", location);
            
            // Check cache first (before any mutable borrows)
            let cached_entries = state.cache.get(&location);
            
            if cached_entries.is_some() {
                debug!("ChangeLocation: using cached entries for {}", location.display_path());
            } else {
                debug!("ChangeLocation: cache miss, will create ReadDirectory job for {}", location.display_path());
            }
            
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
            debug!("ChangeLocation: pane location after update: {:?}", pane_model.current_location);
            pane_model.cursor = 0;
            pane_model.scroll_offset = 0;
            
            // Use cached entries if available
            if let Some(entries) = cached_entries {
                debug!("ChangeLocation: loaded {} cached entries", entries.len());
                pane_model.entries = entries;
                pane_model.apply_sort();
                StateUpdateResult::with_ui_change()
            } else {
                // Create job to read directory
                debug!("ChangeLocation: creating ReadDirectory job");
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
            
            debug!("NavigateUp: current_location = {}", current_location.display_path());
            
            if let Some(parent) = current_location.parent() {
                debug!("NavigateUp: parent = {}", parent.display_path());
                update_state(state, Transition::ChangeLocation { pane, location: parent })
            } else {
                debug!("NavigateUp: no parent (at root)");
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
            // Debug logging before job_spec check
            debug!("CompleteJob: job_id={:?}, result type={}", job_id, match &result {
                crate::job::OpResult::Success(_) => "Success",
                crate::job::OpResult::Failed(_) => "Failed",
                crate::job::OpResult::Cancelled => "Cancelled",
            });
            
            // Get the job spec before completing it to determine what panes need refreshing
            let job_spec = state.jobs.active.get(&job_id).map(|job| job.spec.clone());
            debug!("CompleteJob: job_spec found={}", job_spec.is_some());
            
            // Debug log the SuccessData variant
            if let crate::job::OpResult::Success(ref success_data) = result {
                let variant_name = match success_data {
                    crate::job::SuccessData::DirectoryRead(entries) => format!("DirectoryRead({} entries)", entries.len()),
                    crate::job::SuccessData::None => "None".to_string(),
                    crate::job::SuccessData::SizeCalculated(size) => format!("SizeCalculated({})", size),
                    crate::job::SuccessData::CustomFunctionOutput(output) => format!("CustomFunctionOutput({} bytes)", output.len()),
                    crate::job::SuccessData::SearchResults(results) => format!("SearchResults({} results)", results.len()),
                    crate::job::SuccessData::FileContents(contents) => format!("FileContents({} bytes)", contents.len()),
                    crate::job::SuccessData::ComparisonResult(_) => "ComparisonResult".to_string(),
                };
                debug!("CompleteJob: SuccessData variant={}", variant_name);
            }
            
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
                            debug!("CompleteJob: ReadDirectory succeeded for {}, got {} entries", 
                                   location.display_path(), entries.len());
                            debug!("CompleteJob: Job location debug format: {:?}", location);
                            
                            for (tab_idx, tab) in state.tabs.tabs.iter_mut().enumerate() {
                                debug!("CompleteJob: Checking tab {} - left: {}, right: {}", 
                                       tab_idx, 
                                       tab.left_pane.current_location.display_path(),
                                       tab.right_pane.current_location.display_path());
                                debug!("CompleteJob: Tab {} left pane location debug: {:?}", tab_idx, tab.left_pane.current_location);
                                debug!("CompleteJob: Tab {} right pane location debug: {:?}", tab_idx, tab.right_pane.current_location);
                                
                                let left_matches = tab.left_pane.current_location == *location;
                                let right_matches = tab.right_pane.current_location == *location;
                                
                                debug!("CompleteJob: Tab {} left_matches={}, right_matches={}", tab_idx, left_matches, right_matches);
                                
                                if left_matches {
                                    debug!("CompleteJob: updating left pane of tab {} with {} entries", tab_idx, entries.len());
                                    tab.left_pane.entries = entries.clone();
                                    tab.left_pane.apply_sort();
                                    
                                    // Ensure cursor is visible by adjusting scroll if needed
                                    if tab.left_pane.cursor >= tab.left_pane.entries.len() {
                                        tab.left_pane.cursor = tab.left_pane.entries.len().saturating_sub(1);
                                    }
                                    let visible_height = state.ui.layout.pane_height;
                                    if tab.left_pane.cursor >= visible_height {
                                        // Position cursor at bottom of visible area
                                        tab.left_pane.scroll_offset = tab.left_pane.cursor + 1 - visible_height;
                                    }
                                }
                                if right_matches {
                                    debug!("CompleteJob: updating right pane of tab {} with {} entries", tab_idx, entries.len());
                                    tab.right_pane.entries = entries.clone();
                                    tab.right_pane.apply_sort();
                                    
                                    // Ensure cursor is visible by adjusting scroll if needed
                                    if tab.right_pane.cursor >= tab.right_pane.entries.len() {
                                        tab.right_pane.cursor = tab.right_pane.entries.len().saturating_sub(1);
                                    }
                                    let visible_height = state.ui.layout.pane_height;
                                    if tab.right_pane.cursor >= visible_height {
                                        // Position cursor at bottom of visible area
                                        tab.right_pane.scroll_offset = tab.right_pane.cursor + 1 - visible_height;
                                    }
                                }
                            }
                        } else {
                            debug!("CompleteJob: ReadDirectory failed for {}", location.display_path());
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
        
        Transition::Refresh { pane } => {
            // Clear cache for the current location and reload directory
            let tab = state.current_tab();
            let location = match pane {
                crate::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                crate::model::ActivePane::Right => tab.right_pane.current_location.clone(),
            };
            
            // Invalidate cache for this location
            state.cache.invalidate(&location);
            
            // Create job to read directory
            let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
            StateUpdateResult::with_job(job_spec)
        }
        
        Transition::RefreshAndClearMarks { pane } => {
            // Clear marks first
            state.marking.unmark_all();
            
            // Then refresh
            let tab = state.current_tab();
            let location = match pane {
                crate::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                crate::model::ActivePane::Right => tab.right_pane.current_location.clone(),
            };
            
            state.cache.invalidate(&location);
            let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
            StateUpdateResult::with_job(job_spec)
        }
        
        Transition::RefreshNoClearMarks { pane } => {
            // Just refresh without clearing marks
            let tab = state.current_tab();
            let location = match pane {
                crate::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                crate::model::ActivePane::Right => tab.right_pane.current_location.clone(),
            };
            
            state.cache.invalidate(&location);
            let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
            StateUpdateResult::with_job(job_spec)
        }
        
        // Pane operations
        Transition::SyncPanes => {
            // Navigate opposite pane to active pane's location
            let active_location = state.active_pane().current_location.clone();
            let opposite_pane = state.ui.active_pane.opposite();
            
            debug!("SyncPanes: syncing {} pane to {}", 
                   match opposite_pane {
                       crate::model::ActivePane::Left => "left",
                       crate::model::ActivePane::Right => "right",
                   },
                   active_location.display_path());
            
            // Check cache first
            let cached_entries = state.cache.get(&active_location);
            
            // Update opposite pane location
            let tab = state.current_tab_mut();
            let opposite_pane_model = match opposite_pane {
                crate::model::ActivePane::Left => &mut tab.left_pane,
                crate::model::ActivePane::Right => &mut tab.right_pane,
            };
            
            // Add current location to history
            tab.history.push(opposite_pane, opposite_pane_model.current_location.clone());
            
            // Update location
            opposite_pane_model.current_location = active_location.clone();
            opposite_pane_model.cursor = 0;
            opposite_pane_model.scroll_offset = 0;
            
            // Use cached entries if available
            if let Some(entries) = cached_entries {
                debug!("SyncPanes: using cached entries ({} entries)", entries.len());
                opposite_pane_model.entries = entries;
                opposite_pane_model.apply_sort();
                StateUpdateResult::with_ui_change()
            } else {
                // Create job to read directory
                debug!("SyncPanes: creating ReadDirectory job");
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { 
                    location: active_location 
                });
                StateUpdateResult::with_job(job_spec)
            }
        }
        
        Transition::SwapPanes => {
            // Exchange current_location of both panes
            let tab = state.current_tab_mut();
            
            debug!("SwapPanes: swapping left ({}) and right ({})", 
                   tab.left_pane.current_location.display_path(),
                   tab.right_pane.current_location.display_path());
            
            // Swap locations
            std::mem::swap(&mut tab.left_pane.current_location, &mut tab.right_pane.current_location);
            
            // Maintain cursor positions and marked files (they stay with their panes)
            // No need to swap cursor or marked files - they remain with their respective panes
            
            // Get locations for both panes
            let left_location = tab.left_pane.current_location.clone();
            let right_location = tab.right_pane.current_location.clone();
            
            // Check cache for both locations (before mutable borrow)
            let left_cached = state.cache.get(&left_location);
            let right_cached = state.cache.get(&right_location);
            
            // Now update the panes with cached data if available
            let tab = state.current_tab_mut();
            
            // Update left pane
            let left_needs_job = if let Some(entries) = left_cached {
                debug!("SwapPanes: using cached entries for left pane ({} entries)", entries.len());
                tab.left_pane.entries = entries;
                tab.left_pane.apply_sort();
                false
            } else {
                true
            };
            
            // Update right pane
            let right_needs_job = if let Some(entries) = right_cached {
                debug!("SwapPanes: using cached entries for right pane ({} entries)", entries.len());
                tab.right_pane.entries = entries;
                tab.right_pane.apply_sort();
                false
            } else {
                true
            };
            
            // Create jobs for any panes that weren't cached
            let mut result = StateUpdateResult::with_ui_change();
            
            if left_needs_job {
                debug!("SwapPanes: creating ReadDirectory job for left pane");
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { 
                    location: left_location 
                });
                result.jobs_to_start.push(job_spec);
            }
            
            if right_needs_job {
                debug!("SwapPanes: creating ReadDirectory job for right pane");
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { 
                    location: right_location 
                });
                result.jobs_to_start.push(job_spec);
            }
            
            result
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
                    crate::model::DialogContent::ContextMenu { options, selected_index } => {
                        // Clone the selected option to avoid borrow checker issues
                        let selected_option = options.get(*selected_index).cloned();
                        
                        state.dialogs.pop();
                        
                        if let Some(option) = selected_option {
                            match &option.action {
                                crate::model::ContextMenuAction::Copy => {
                                    // Show copy confirmation dialog
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
                                        let total_size: u64 = state.active_pane()
                                            .entries
                                            .iter()
                                            .filter(|e| sources.contains(&e.location))
                                            .map(|e| e.size)
                                            .sum();
                                        let size_str = crate::model::format_size(total_size);
                                        let message = if sources.len() == 1 {
                                            format!("Copy {} ({}) to {}?", sources[0].display_path(), size_str, dest.display_path())
                                        } else {
                                            format!("Copy {} files ({}) to {}?", sources.len(), size_str, dest.display_path())
                                        };
                                        let dialog = crate::model::Dialog::confirmation("Copy", message);
                                        state.dialogs.push(dialog);
                                    }
                                }
                                crate::model::ContextMenuAction::Move => {
                                    // Show move confirmation dialog
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
                                        let message = if sources.len() == 1 {
                                            format!("Move {} to {}?", sources[0].display_path(), dest.display_path())
                                        } else {
                                            format!("Move {} files to {}?", sources.len(), dest.display_path())
                                        };
                                        let dialog = crate::model::Dialog::confirmation("Move", message);
                                        state.dialogs.push(dialog);
                                    }
                                }
                                crate::model::ContextMenuAction::Delete => {
                                    // Show delete confirmation dialog
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
                                        let message = if targets.len() == 1 {
                                            format!("Delete {}?", targets[0].display_path())
                                        } else {
                                            format!("Delete {} files?", targets.len())
                                        };
                                        let dialog = crate::model::Dialog::confirmation("Delete", message);
                                        state.dialogs.push(dialog);
                                    }
                                }
                                crate::model::ContextMenuAction::Rename => {
                                    // Show rename input dialog
                                    if let Some(entry) = state.active_pane().current_entry() {
                                        let dialog = crate::model::Dialog::input("Rename", "New name:", &entry.name);
                                        state.dialogs.push(dialog);
                                    }
                                }
                                crate::model::ContextMenuAction::View => {
                                    // Open text viewer
                                    if let Some(entry) = state.active_pane().current_entry() {
                                        let location = entry.location.clone();
                                        return update_state(state, Transition::OpenTextViewer { location });
                                    }
                                }
                                crate::model::ContextMenuAction::CustomFunction(name) => {
                                    // Trigger custom function
                                    // TODO: Load and execute custom function by name
                                    let _ = name; // Suppress unused warning for now
                                }
                            }
                        }
                    }
                    crate::model::DialogContent::DriveSelection { drives, selected_index } => {
                        // Navigate to the selected drive
                        if let Some(drive) = drives.get(*selected_index) {
                            let location = crate::model::Location::Local(std::path::PathBuf::from(&drive.path));
                            let pane = state.ui.active_pane;
                            
                            state.dialogs.pop();
                            return update_state(state, Transition::ChangeLocation { pane, location });
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
        
        Transition::UpdatePaneHeight { height } => {
            state.ui.layout.pane_height = height;
            StateUpdateResult::none()
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
            // Update worker pool size if changed
            let new_worker_pool_size = config.worker_pool_size;
            
            // Update the configuration
            state.config = *config;
            
            if state.jobs.max_parallel != new_worker_pool_size {
                state.jobs.max_parallel = new_worker_pool_size;
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
        
        // Context menu and drive selection
        Transition::ShowContextMenu => {
            // Show context menu dialog
            let dialog = crate::model::Dialog::context_menu();
            state.dialogs.push(dialog);
            StateUpdateResult::with_ui_change()
        }
        
        Transition::ShowDriveChangeDialog => {
            // Get all available drives
            let drives = crate::volume_info::get_all_drives();
            
            // Show drive selection dialog
            let dialog = crate::model::Dialog::drive_selection(drives);
            state.dialogs.push(dialog);
            StateUpdateResult::with_ui_change()
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

    // Bug Condition Exploration Tests
    // These tests demonstrate the scrolling bugs on the UNFIXED code
    // They are EXPECTED TO FAIL on the current implementation
    //
    // COUNTEREXAMPLES FOUND (documented from test failures):
    //
    // Test 1: test_bug_premature_scrolling_with_blank_lines
    //   Input: cursor=66→67, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70
    //   Expected: scroll_offset=51 (max_offset, to avoid blank lines)
    //   Actual (unfixed): scroll_offset=51 but with cursor_in_view=16 instead of triggering earlier
    //   Bug: Scrolling triggers too late (at cursor_in_view=17 instead of 16), causing issues
    //   The fix ensures scrolling triggers at cursor_in_view >= 15 and positions correctly
    //
    // Test 2: test_bug_scroll_margin_violation_at_last_entry
    //   Input: cursor=73 (last entry), scroll_offset=55, visible_height=19, scroll_margin=3, total_entries=74
    //   Expected: scroll_offset=55 (max_offset, cursor on last line is acceptable per requirement 2.4)
    //   Actual (unfixed): scroll_offset=55 (same, but this test verifies no blank lines)
    //   Note: When at max_offset with cursor at last entry, cursor_in_view=18 (last line) is correct
    //
    // Test 3: test_bug_correct_trigger_position
    //   Input: cursor=65→66, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70
    //   Expected: scroll_offset=51 (scrolling should trigger at cursor_in_view >= 15)
    //   Actual: scroll_offset=50 (no scrolling triggered)
    //   Bug: Scrolling doesn't trigger at cursor_in_view=16 when it should (>= bottom_trigger=15)
    //   The condition uses > instead of >=, causing it to trigger one position too late
    //
    // ROOT CAUSE CONFIRMED:
    // 1. Trigger condition uses > instead of >= (triggers at cursor_in_view=17 instead of 16)
    // 2. Naive increment by 1 doesn't position cursor at desired line
    // 3. No special handling for last entry to maintain scroll_margin
    
    #[test]
    fn test_bug_premature_scrolling_with_blank_lines() {
        // Test Case 1: Premature scroll trigger with blank lines
        // cursor=66→67, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let mut config = AppConfig::default();
        config.ui.scroll_offset = 3; // scroll_margin
        let mut state = AppState::new(config);
        
        // Set visible height to 19
        state.ui.layout.pane_height = 19;
        
        // Create 70 entries
        let entries: Vec<FileEntry> = (0..70).map(|i| FileEntry {
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
        state.current_tab_mut().left_pane.cursor = 66;
        state.current_tab_mut().left_pane.scroll_offset = 50;
        
        // Move cursor from 66 to 67
        update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: 1 
        });
        
        let pane = &state.current_tab().left_pane;
        let cursor_in_view = pane.cursor.saturating_sub(pane.scroll_offset);
        let visible_height = state.ui.layout.pane_height;
        let scroll_margin = state.config.ui.scroll_offset;
        let bottom_trigger = visible_height - scroll_margin - 1; // Should be 15 (0-indexed)
        let max_offset = pane.entries.len().saturating_sub(visible_height); // 70 - 19 = 51
        
        // Expected behavior: scroll_offset should be at max_offset (51) to avoid blank lines
        // desired_offset would be 67 - 15 = 52, but we clamp to max_offset = 51
        // This gives cursor_in_view = 67 - 51 = 16, which is acceptable when at max_offset
        assert_eq!(pane.scroll_offset, max_offset, 
            "scroll_offset should be at max_offset to avoid blank lines. Expected {}, got {}. cursor={}, cursor_in_view={}", 
            max_offset, pane.scroll_offset, pane.cursor, cursor_in_view);
        
        // Verify no blank lines: viewport should show exactly visible_height entries
        let visible_entries = pane.entries.len().saturating_sub(pane.scroll_offset).min(visible_height);
        assert_eq!(visible_entries, visible_height,
            "Viewport should show exactly {} entries with no blank lines. Got {} visible entries. scroll_offset={}, total_entries={}",
            visible_height, visible_entries, pane.scroll_offset, pane.entries.len());
    }
    
    #[test]
    fn test_bug_scroll_margin_violation_at_last_entry() {
        // Test Case 2: Scroll margin violation at last entry
        // cursor=73, scroll_offset=55, visible_height=19, scroll_margin=3, total_entries=74
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let mut config = AppConfig::default();
        config.ui.scroll_offset = 3; // scroll_margin
        let mut state = AppState::new(config);
        
        // Set visible height to 19
        state.ui.layout.pane_height = 19;
        
        // Create 74 entries
        let entries: Vec<FileEntry> = (0..74).map(|i| FileEntry {
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
        state.current_tab_mut().left_pane.cursor = 73; // Last entry
        state.current_tab_mut().left_pane.scroll_offset = 55;
        
        // Trigger a cursor move to force scroll calculation
        update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: 0 
        });
        
        let pane = &state.current_tab().left_pane;
        let cursor_in_view = pane.cursor.saturating_sub(pane.scroll_offset);
        let visible_height = state.ui.layout.pane_height;
        let scroll_margin = state.config.ui.scroll_offset;
        let max_offset = pane.entries.len().saturating_sub(visible_height); // 74 - 19 = 55
        
        // Expected behavior: when at last entry and max_offset, cursor can be on last line
        // This is acceptable per requirement 2.4: "cursor on the last line, with no blank lines"
        // scroll_offset should be at max_offset (55)
        assert_eq!(pane.scroll_offset, max_offset,
            "scroll_offset should be at max_offset when cursor is at last entry. Expected {}, got {}. cursor={}, cursor_in_view={}",
            max_offset, pane.scroll_offset, pane.cursor, cursor_in_view);
        
        // Verify no blank lines: viewport should show exactly visible_height entries
        let visible_entries = pane.entries.len().saturating_sub(pane.scroll_offset).min(visible_height);
        assert_eq!(visible_entries, visible_height,
            "Viewport should show exactly {} entries with no blank lines. Got {} visible entries.",
            visible_height, visible_entries);
    }
    
    #[test]
    fn test_bug_correct_trigger_position() {
        // Test Case 3: Correct trigger position test
        // cursor=65→66, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70
        // Should trigger at cursor_in_view=16 (>= bottom_trigger where bottom_trigger=15)
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let mut config = AppConfig::default();
        config.ui.scroll_offset = 3; // scroll_margin
        let mut state = AppState::new(config);
        
        // Set visible height to 19
        state.ui.layout.pane_height = 19;
        
        // Create 70 entries
        let entries: Vec<FileEntry> = (0..70).map(|i| FileEntry {
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
        state.current_tab_mut().left_pane.cursor = 65;
        state.current_tab_mut().left_pane.scroll_offset = 50;
        
        // Move cursor from 65 to 66 (cursor_in_view will be 16)
        update_state(&mut state, Transition::CursorMove { 
            pane: ActivePane::Left, 
            delta: 1 
        });
        
        let pane = &state.current_tab().left_pane;
        let cursor_in_view = pane.cursor.saturating_sub(pane.scroll_offset);
        let visible_height = state.ui.layout.pane_height;
        let scroll_margin = state.config.ui.scroll_offset;
        let bottom_trigger = visible_height - scroll_margin - 1; // Should be 15
        
        // Expected behavior: scrolling should trigger when cursor_in_view >= bottom_trigger (15)
        // At cursor=66, cursor_in_view=16, which is >= 15, so scrolling should trigger
        // scroll_offset should be adjusted to keep cursor at bottom_trigger position
        // Expected scroll_offset = cursor - bottom_trigger = 66 - 15 = 51
        assert_eq!(pane.scroll_offset, 51,
            "Scrolling should trigger at cursor_in_view >= {} (bottom_trigger). Expected scroll_offset=51, got {}. cursor={}, cursor_in_view={}",
            bottom_trigger, pane.scroll_offset, pane.cursor, cursor_in_view);
        
        // Verify cursor is positioned at bottom_trigger line
        let new_cursor_in_view = pane.cursor.saturating_sub(pane.scroll_offset);
        assert_eq!(new_cursor_in_view, bottom_trigger,
            "After scrolling, cursor should be at bottom_trigger position ({}), but got cursor_in_view={}",
            bottom_trigger, new_cursor_in_view);
    }

    // ============================================================================
    // PRESERVATION PROPERTY TESTS
    // These tests capture baseline behavior for non-buggy inputs that must be preserved
    // Expected to PASS on unfixed code
    // ============================================================================

    use proptest::prelude::*;

    // Helper function to create test entries
    fn create_test_entries(count: usize) -> Vec<crate::model::FileEntry> {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        (0..count).map(|i| FileEntry {
            name: format!("file{}.txt", i),
            location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }).collect()
    }

    // Helper function to setup state with given parameters
    fn setup_test_state(
        total_entries: usize,
        cursor: usize,
        scroll_offset: usize,
        visible_height: usize,
        scroll_margin: usize,
    ) -> AppState {
        let mut config = AppConfig::default();
        config.ui.scroll_offset = scroll_margin;
        let mut state = AppState::new(config);
        
        state.ui.layout.pane_height = visible_height;
        state.current_tab_mut().left_pane.entries = create_test_entries(total_entries);
        state.current_tab_mut().left_pane.cursor = cursor;
        state.current_tab_mut().left_pane.scroll_offset = scroll_offset;
        
        state
    }

    proptest! {
        #[test]
        fn prop_preservation_scroll_up_behavior(
            // Generate scroll states where cursor_in_view < scroll_margin
            total_entries in 20usize..100,
            scroll_margin in 1usize..5,
            visible_height in 10usize..25,
        ) {
            // Ensure we have enough entries to scroll
            prop_assume!(total_entries > visible_height);
            
            // Generate a scroll state where cursor is near the top
            // cursor_in_view should be < scroll_margin to trigger scroll up
            let scroll_offset = scroll_margin + 5; // Start with some scroll
            let cursor_in_view = scroll_margin.saturating_sub(1); // One less than margin
            let cursor = scroll_offset + cursor_in_view;
            
            // Ensure cursor is valid
            prop_assume!(cursor > 0 && cursor < total_entries);
            
            let mut state = setup_test_state(
                total_entries,
                cursor,
                scroll_offset,
                visible_height,
                scroll_margin,
            );
            
            let old_scroll_offset = state.current_tab().left_pane.scroll_offset;
            
            // Move cursor up by 1
            update_state(&mut state, Transition::CursorMove { 
                pane: crate::model::ActivePane::Left, 
                delta: -1 
            });
            
            let pane = &state.current_tab().left_pane;
            let new_cursor = pane.cursor;
            let new_scroll_offset = pane.scroll_offset;
            
            // **Validates: Requirements 3.2**
            // Property: When scrolling up (cursor_in_view < scroll_margin),
            // scroll_offset should be cursor - scroll_margin
            let expected_scroll_offset = new_cursor.saturating_sub(scroll_margin);
            prop_assert_eq!(
                new_scroll_offset,
                expected_scroll_offset,
                "Scroll up behavior: scroll_offset should be cursor - scroll_margin. \
                 cursor={}, scroll_margin={}, expected={}, got={}",
                new_cursor, scroll_margin, expected_scroll_offset, new_scroll_offset
            );
        }
    }

    proptest! {
        #[test]
        fn prop_preservation_small_list_behavior(
            // Generate scroll states where total_entries <= visible_height
            total_entries in 1usize..20,
            visible_height in 20usize..30,
            scroll_margin in 1usize..5,
        ) {
            // Ensure small list condition
            prop_assume!(total_entries <= visible_height);
            
            let cursor = total_entries / 2; // Middle of list
            
            let mut state = setup_test_state(
                total_entries,
                cursor,
                0, // scroll_offset should be 0 for small lists
                visible_height,
                scroll_margin,
            );
            
            // Move cursor down
            update_state(&mut state, Transition::CursorMove { 
                pane: crate::model::ActivePane::Left, 
                delta: 1 
            });
            
            let pane = &state.current_tab().left_pane;
            
            // **Validates: Requirements 3.1, 3.4**
            // Property: When total_entries <= visible_height, scroll_offset should remain 0
            prop_assert_eq!(
                pane.scroll_offset,
                0,
                "Small list behavior: scroll_offset should be 0 when total_entries <= visible_height. \
                 total_entries={}, visible_height={}, scroll_offset={}",
                total_entries, visible_height, pane.scroll_offset
            );
        }
    }

    proptest! {
        #[test]
        fn prop_preservation_cursor_jump_above_viewport(
            // Generate scroll states for cursor jumps above viewport
            total_entries in 30usize..100,
            visible_height in 10usize..25,
            scroll_margin in 1usize..5,
        ) {
            // Ensure we have enough entries to scroll
            prop_assume!(total_entries > visible_height);
            
            // Start with cursor in middle, scroll offset in middle
            let initial_scroll_offset = total_entries / 2;
            let initial_cursor = initial_scroll_offset + visible_height / 2;
            
            // Jump to a position above the viewport
            let jump_target = initial_scroll_offset.saturating_sub(10);
            
            // Ensure valid state
            prop_assume!(jump_target < initial_scroll_offset);
            prop_assume!(initial_cursor < total_entries);
            
            let mut state = setup_test_state(
                total_entries,
                initial_cursor,
                initial_scroll_offset,
                visible_height,
                scroll_margin,
            );
            
            // Jump cursor above viewport
            update_state(&mut state, Transition::CursorJump { 
                pane: crate::model::ActivePane::Left, 
                position: jump_target 
            });
            
            let pane = &state.current_tab().left_pane;
            
            // **Validates: Requirements 3.3**
            // Property: When cursor jumps above viewport, scroll_offset should be cursor - scroll_margin
            let expected_scroll_offset = pane.cursor.saturating_sub(scroll_margin);
            prop_assert_eq!(
                pane.scroll_offset,
                expected_scroll_offset,
                "Cursor jump above viewport: scroll_offset should be cursor - scroll_margin. \
                 cursor={}, scroll_margin={}, expected={}, got={}",
                pane.cursor, scroll_margin, expected_scroll_offset, pane.scroll_offset
            );
        }
    }

    proptest! {
        #[test]
        fn prop_preservation_cursor_jump_below_viewport(
            // Generate scroll states for cursor jumps below viewport
            total_entries in 30usize..100,
            visible_height in 10usize..25,
            scroll_margin in 1usize..5,
        ) {
            // Ensure we have enough entries to scroll
            prop_assume!(total_entries > visible_height);
            
            // Start with cursor near top
            let initial_scroll_offset = 5;
            let initial_cursor = initial_scroll_offset + 2;
            
            // Jump to a position below the viewport
            let jump_target = initial_scroll_offset + visible_height + 10;
            
            // Ensure valid state
            prop_assume!(jump_target < total_entries);
            prop_assume!(jump_target >= initial_scroll_offset + visible_height);
            
            let mut state = setup_test_state(
                total_entries,
                initial_cursor,
                initial_scroll_offset,
                visible_height,
                scroll_margin,
            );
            
            // Jump cursor below viewport
            update_state(&mut state, Transition::CursorJump { 
                pane: crate::model::ActivePane::Left, 
                position: jump_target 
            });
            
            let pane = &state.current_tab().left_pane;
            
            // **Validates: Requirements 3.3**
            // Property: When cursor jumps below viewport, scroll_offset should position cursor
            // with scroll_margin spacing from bottom
            let expected_scroll_offset = {
                let desired_offset = pane.cursor + scroll_margin + 1 - visible_height;
                let max_offset = total_entries.saturating_sub(visible_height);
                desired_offset.min(max_offset)
            };
            
            prop_assert_eq!(
                pane.scroll_offset,
                expected_scroll_offset,
                "Cursor jump below viewport: scroll_offset calculation should match original logic. \
                 cursor={}, scroll_margin={}, visible_height={}, total_entries={}, expected={}, got={}",
                pane.cursor, scroll_margin, visible_height, total_entries, expected_scroll_offset, pane.scroll_offset
            );
        }
    }
}

// Property-based tests
#[cfg(test)]
#[path = "state_properties.rs"]
mod state_properties;
