//! Application state management
//!
//! This module defines the central AppState structure and the Transition enum
//! for explicit state changes following the AppState pattern.

use crate::job::{JobManager, JobId, JobSpec, BackgroundJobManager};
use crate::model::{TabManager, SearchModel, MarkingModel, UIState, DialogStack, DirectoryCache, ViewerState, NavigationStateCache};
use crate::log_manager::LogManager;
use std::time::Duration;
use tracing::debug;

/// Central application state coordinating all components
#[derive(Debug)]
pub struct AppState {
    /// Tab manager with independent pane states
    pub tabs: TabManager,
    /// Job manager for background operations
    pub jobs: JobManager,
    /// Background job manager for UI display
    pub background_jobs: BackgroundJobManager,
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
    /// Navigation state cache for cursor/scroll position memory
    pub navigation_cache: NavigationStateCache,
    /// File viewer state (when in viewer mode)
    pub viewer: Option<ViewerState>,
    /// Session log manager
    pub log_manager: LogManager,
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
        
        // Create log manager with configured settings
        let log_path = if config.log_save_path.starts_with('/') || config.log_save_path.contains(':') {
            // Absolute path
            std::path::PathBuf::from(&config.log_save_path)
        } else {
            // Relative to data directory
            crate::logging::default_log_dir().parent().unwrap().join(&config.log_save_path)
        };
        
        let log_manager = LogManager::new(
            config.max_log_lines_in_memory,
            log_path,
            config.log_file_progress_threshold_ms,
        );
        
        Self {
            tabs: TabManager::new(),
            jobs: JobManager::new(config.worker_pool_size),
            background_jobs: BackgroundJobManager::new(
                config.job_manager.max_simultaneous_jobs,
                Duration::from_secs(config.job_manager.job_retention_period_secs)
            ),
            search: SearchModel::new(),
            marking: MarkingModel::new(),
            ui: UIState::new(),
            dialogs: DialogStack::new(),
            registered_folders,
            cache: DirectoryCache::new(Duration::from_secs(30)),
            navigation_cache: NavigationStateCache::new(),
            viewer: None,
            log_manager,
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
            self.ui.layout.show_task_panel,
            self.ui.layout.task_panel_height,
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

        // Restore task panel settings
        self.ui.layout.show_task_panel = session.show_task_panel;
        self.ui.layout.task_panel_height = session.task_panel_height;

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

    // --- Transition Handlers ---

    fn handle_navigation_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::SwitchPane => {
                let old_pane = self.ui.active_pane;
                self.ui.active_pane = self.ui.active_pane.opposite();
                debug!("SwitchPane transition: {:?} -> {:?}", old_pane, self.ui.active_pane);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CursorMove { pane, delta } => {
                let visible_height = self.ui.layout.pane_height;
                let scroll_margin = self.config.ui.scroll_offset;
                
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                
                if !pane_model.entries.is_empty() {
                    let new_cursor = (pane_model.cursor as isize + *delta)
                        .max(0)
                        .min(pane_model.entries.len() as isize - 1) as usize;
                    pane_model.cursor = new_cursor;
                    pane_model.update_scroll(visible_height, scroll_margin);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CursorJump { pane, position } => {
                let visible_height = self.ui.layout.pane_height;
                let scroll_margin = self.config.ui.scroll_offset;
                
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                
                if !pane_model.entries.is_empty() {
                    pane_model.cursor = (*position).min(pane_model.entries.len().saturating_sub(1));
                    pane_model.update_scroll(visible_height, scroll_margin);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ChangeLocation { pane, location } => {
                debug!("ChangeLocation: pane = {:?}, location = {}", pane, location.display_path());
                
                let cached_entries = self.cache.get(location);
                
                let tab = self.current_tab();
                let (current_loc, current_cursor, current_scroll) = match pane {
                    crate::model::ActivePane::Left => (
                        tab.left_pane.current_location.clone(),
                        tab.left_pane.cursor,
                        tab.left_pane.scroll_offset,
                    ),
                    crate::model::ActivePane::Right => (
                        tab.right_pane.current_location.clone(),
                        tab.right_pane.cursor,
                        tab.right_pane.scroll_offset,
                    ),
                };
                
                self.navigation_cache.save(current_loc.clone(), current_cursor, current_scroll);
                let restored_position = self.navigation_cache.restore(location);
                
                let tab_mut = self.current_tab_mut();
                tab_mut.history.push(*pane, current_loc);
                
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab_mut.left_pane,
                    crate::model::ActivePane::Right => &mut tab_mut.right_pane,
                };
                pane_model.current_location = location.clone();
                
                if let Some((cached_cursor, cached_scroll)) = restored_position {
                    pane_model.cursor = cached_cursor;
                    pane_model.scroll_offset = cached_scroll;
                } else {
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;
                }
                
                if let Some(entries) = cached_entries {
                    pane_model.entries = entries;
                    pane_model.apply_sort();
                    if !pane_model.entries.is_empty() {
                        pane_model.cursor = pane_model.cursor.min(pane_model.entries.len() - 1);
                    } else {
                        pane_model.cursor = 0;
                        pane_model.scroll_offset = 0;
                    }
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location: location.clone() });
                    Some(StateUpdateResult::with_job(job_spec))
                }
            }
            Transition::NavigateUp { pane } => {
                let tab = self.current_tab();
                let current_location = match pane {
                    crate::model::ActivePane::Left => &tab.left_pane.current_location,
                    crate::model::ActivePane::Right => &tab.right_pane.current_location,
                };
                
                if let Some(parent) = current_location.parent() {
                    self.handle_navigation_transition(&Transition::ChangeLocation { 
                        pane: *pane, 
                        location: parent 
                    })
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::NavigateHistory { pane, direction } => {
                let location = {
                    let tab = self.current_tab_mut();
                    match direction {
                        HistoryDirection::Back => tab.history.go_back(*pane),
                        HistoryDirection::Forward => tab.history.go_forward(*pane),
                    }
                };
                
                if let Some(location) = location {
                    let cached_entries = self.cache.get(&location);
                    let tab = self.current_tab_mut();
                    let pane_model = match pane {
                        crate::model::ActivePane::Left => &mut tab.left_pane,
                        crate::model::ActivePane::Right => &mut tab.right_pane,
                    };
                    pane_model.current_location = location.clone();
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;
                    
                    if let Some(entries) = cached_entries {
                        pane_model.entries = entries;
                        pane_model.apply_sort();
                        Some(StateUpdateResult::with_ui_change())
                    } else {
                        let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
                        Some(StateUpdateResult::with_job(job_spec))
                    }
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            _ => None,
        }
    }

    fn handle_tab_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::CreateTab => {
                let new_index = self.tabs.create_tab();
                self.tabs.active_index = new_index;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CloseTab { index } => {
                let active_job_ids: Vec<u32> = self.background_jobs.get_active_jobs()
                    .filter(|j| j.tab_id == *index)
                    .map(|j| j.id.short_id)
                    .collect();

                if !active_job_ids.is_empty() {
                    let tab_name = format!("Tab {}", index + 1);
                    let dialog = crate::model::Dialog {
                        title: "Confirm Close Tab".to_string(),
                        content: crate::model::DialogContent::CloseTabWithActiveJob {
                            tab_index: *index,
                            tab_name,
                            job_ids: active_job_ids,
                            focused_field: 0,
                        },
                    };
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    if self.tabs.close_tab(*index) {
                        Some(StateUpdateResult::with_ui_change())
                    } else {
                        Some(StateUpdateResult::none())
                    }
                }
            }
            Transition::NextTab => {
                self.tabs.switch_to_next();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::PrevTab => {
                self.tabs.switch_to_prev();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SwitchTab { index } => {
                if *index < self.tabs.tabs.len() {
                    self.tabs.active_index = *index;
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            _ => None,
        }
    }

    fn handle_marking_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::ToggleMark { location } => {
                self.marking.toggle(location.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkAll => {
                let entries = self.active_pane().entries.clone();
                self.marking.mark_all(&entries);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UnmarkAll => {
                self.marking.unmark_all();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkPattern { pattern } => {
                let entries = self.active_pane().entries.clone();
                self.marking.mark_pattern(&entries, pattern);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkRange { start, end } => {
                let entries = self.active_pane().entries.clone();
                self.marking.mark_range(&entries, *start, *end);
                self.ui.range_marking_start = None;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::InvertMarks => {
                let entries = self.active_pane().entries.clone();
                self.marking.invert_marks(&entries);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::EnterRangeMarkingMode => {
                let cursor = self.active_pane().cursor;
                self.ui.range_marking_start = Some(cursor);
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }

    fn handle_job_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        use std::time::SystemTime;

        match transition {
            Transition::EnqueueJob { spec } => {
                self.jobs.enqueue(spec.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::StartNextJob => {
                if self.jobs.can_start_job() {
                    if let Some(spec) = self.jobs.pop_next_job() {
                        self.jobs.start_job(spec.clone());
                        Some(StateUpdateResult::with_job(spec))
                    } else {
                        Some(StateUpdateResult::none())
                    }
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::JobStarted { job_id } => {
                if let Some(job) = self.jobs.active.get_mut(job_id) {
                    job.state = crate::job::ExecutionState::Running;
                    job.started_at = Some(SystemTime::now());
                }
                
                let log_entry = self.background_jobs.get_job(*job_id).map(|bg_job| {
                    let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                    format!("{} [Job {}] [Tab {}] {}: Started", 
                        timestamp, bg_job.id.short_id, bg_job.tab_id + 1, bg_job.name)
                });
                
                if let Some(log) = log_entry {
                    self.background_jobs.mark_job_running(*job_id);
                    Some(StateUpdateResult {
                        jobs_to_start: Vec::new(),
                        jobs_to_cancel: Vec::new(),
                        completed_jobs: Vec::new(),
                        failed_jobs: Vec::new(),
                        cancelled_jobs: Vec::new(),
                        started_jobs: vec![*job_id],
                        task_panel_logs: vec![log],
                        panes_to_refresh: Vec::new(),
                        ui_changed: true,
                    })
                } else {
                    Some(StateUpdateResult::with_ui_change())
                }
            }
            Transition::UpdateJobProgress { job_id, progress } => {
                self.jobs.update_progress(*job_id, *progress);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdateJobProgressWithDetail { job_id, progress, progress_message, operation_detail } => {
                self.jobs.update_progress(*job_id, *progress);
                let needs_running = self.background_jobs.get_job(*job_id)
                    .map(|j| j.status == crate::job::JobStatus::Pending)
                    .unwrap_or(false);
                
                let job_progress = crate::job::JobProgress {
                    percent: *progress,
                    message: progress_message.clone(),
                    current_operation_detail: operation_detail.clone(),
                };
                self.background_jobs.update_progress(*job_id, job_progress);
                
                if needs_running {
                    self.background_jobs.mark_job_running(*job_id);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CompleteJob { job_id, result } => {
                let job_spec = self.jobs.active.get(job_id).map(|job| job.spec.clone());
                let log_entry = self.background_jobs.get_job(*job_id).map(|bg_job| {
                    let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                    let status_tag = match result {
                        crate::job::OpResult::Success(_) => "[OK]",
                        crate::job::OpResult::Failed(_) => "[FAIL]",
                        crate::job::OpResult::Cancelled => "[WARN]",
                    };
                    format!("{} [Job {}] [Tab {}] {}: Completed {}",
                        timestamp, bg_job.id.short_id, bg_job.tab_id + 1, bg_job.name, status_tag)
                });

                if let crate::job::OpResult::Failed(ref error_message) = result {
                    if let Some(ref spec) = job_spec {
                        let op_name = match &spec.kind {
                            crate::job::JobKind::ReadDirectory { .. } => "Read directory",
                            crate::job::JobKind::Copy { .. } => "Copy",
                            crate::job::JobKind::Move { .. } => "Move",
                            crate::job::JobKind::Delete { .. } => "Delete",
                            crate::job::JobKind::Mkdir { .. } => "Create directory",
                            crate::job::JobKind::Rename { .. } => "Rename",
                            crate::job::JobKind::CalculateSize { .. } => "Calculate size",
                            crate::job::JobKind::ExtractArchive { .. } => "Extract archive",
                            crate::job::JobKind::CreateArchive { .. } => "Create archive",
                            crate::job::JobKind::ExecuteCustomFunction { .. } => "Execute custom function",
                            crate::job::JobKind::Search { .. } => "Search",
                            crate::job::JobKind::LoadFileForViewer { .. } => "Load file for viewer",
                            crate::job::JobKind::PatternRename { .. } => "Pattern rename",
                            crate::job::JobKind::CompareFiles { .. } => "File comparison",
                            crate::job::JobKind::SplitFile { .. } => "File split",
                            crate::job::JobKind::JoinFiles { .. } => "File join",
                            crate::job::JobKind::CountDown { .. } => "Countdown",
                        };
                        let error_dialog = crate::model::Dialog::from_job_failure(op_name, error_message);
                        self.dialogs.push(error_dialog);
                    }
                }

                self.jobs.complete_job(*job_id, result.clone());

                if let Some(ref spec) = job_spec {
                    match &spec.kind {
                        crate::job::JobKind::ReadDirectory { location } => {
                            if let crate::job::OpResult::Success(crate::job::SuccessData::DirectoryRead(entries)) = result {
                                self.cache.insert(location.clone(), entries.clone());
                            }
                        }
                        crate::job::JobKind::Copy { dest, .. } |
                        crate::job::JobKind::Move { dest, .. } |
                        crate::job::JobKind::ExtractArchive { dest, .. } => {
                            self.cache.invalidate(dest);
                        }
                        crate::job::JobKind::Delete { targets } => {
                            for target in targets {
                                if let Some(parent) = target.parent() {
                                    self.cache.invalidate(&parent);
                                }
                            }
                        }
                        crate::job::JobKind::Rename { from, .. } => {
                            if let Some(parent) = from.parent() {
                                self.cache.invalidate(&parent);
                            }
                        }
                        crate::job::JobKind::PatternRename { targets, .. } => {
                            for target in targets {
                                if let Some(parent) = target.parent() {
                                    self.cache.invalidate(&parent);
                                }
                            }
                        }
                        crate::job::JobKind::Mkdir { location } => {
                            if let Some(parent) = location.parent() {
                                self.cache.invalidate(&parent);
                            }
                        }
                        _ => {}
                    }
                }

                let mut result_obj = StateUpdateResult::with_ui_change();
                match result {
                    crate::job::OpResult::Success(_) => result_obj.completed_jobs.push(*job_id),
                    crate::job::OpResult::Failed(_) => result_obj.failed_jobs.push(*job_id),
                    crate::job::OpResult::Cancelled => result_obj.cancelled_jobs.push(*job_id),
                }

                if let Some(spec) = job_spec {
                    match &spec.kind {
                        crate::job::JobKind::ReadDirectory { location } => {
                            if let crate::job::OpResult::Success(crate::job::SuccessData::DirectoryRead(entries)) = result {
                                for tab in self.tabs.tabs.iter_mut() {
                                    if tab.left_pane.current_location == *location {
                                        tab.left_pane.entries = entries.clone();
                                        tab.left_pane.apply_sort();
                                        tab.left_pane.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                                    }
                                    if tab.right_pane.current_location == *location {
                                        tab.right_pane.entries = entries.clone();
                                        tab.right_pane.apply_sort();
                                        tab.right_pane.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::LoadFileForViewer { .. } => {
                            if let crate::job::OpResult::Success(crate::job::SuccessData::FileContents(contents)) = result {
                                let viewer_res = update_state(self, Transition::ViewerLoadComplete { contents: contents.clone() });
                                result_obj.ui_changed = viewer_res.ui_changed;
                            }
                        }
                        crate::job::JobKind::CompareFiles { .. } => {
                            if let crate::job::OpResult::Success(crate::job::SuccessData::ComparisonResult(diff)) = result {
                                let comp_res = update_state(self, Transition::ShowComparisonView { diff: diff.clone() });
                                result_obj.ui_changed = comp_res.ui_changed;
                            }
                        }
                        crate::job::JobKind::Copy { dest, .. } |
                        crate::job::JobKind::ExtractArchive { dest, .. } |
                        crate::job::JobKind::CreateArchive { dest, .. } |
                        crate::job::JobKind::SplitFile { dest_dir: dest, .. } => {
                            for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                if tab.left_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Left });
                                }
                                if tab.right_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Right });
                                }
                            }
                        }
                        crate::job::JobKind::JoinFiles { dest, .. } => {
                            if let Some(parent) = dest.parent() {
                                for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                    if tab.left_pane.current_location == parent {
                                        result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Left });
                                    }
                                    if tab.right_pane.current_location == parent {
                                        result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Right });
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::Move { sources, dest } => {
                            for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                if tab.left_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Left });
                                }
                                if tab.right_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Right });
                                }
                            }
                            for source in sources {
                                if let Some(parent) = source.parent() {
                                    for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                        if tab.left_pane.current_location == parent {
                                            result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Left });
                                        }
                                        if tab.right_pane.current_location == parent {
                                            result_obj.panes_to_refresh.push(PaneRefresh { tab_id: tab_idx, pane: crate::model::ActivePane::Right });
                                        }
                                    }
                                }
                            }
                            self.marking.unmark_all();
                        }
                        crate::job::JobKind::Delete { .. } |
                        crate::job::JobKind::Rename { .. } |
                        crate::job::JobKind::PatternRename { .. } |
                        crate::job::JobKind::Mkdir { .. } => {
                            result_obj.panes_to_refresh.push(PaneRefresh { tab_id: self.tabs.active_index, pane: self.ui.active_pane });
                            if let crate::job::JobKind::Delete { .. } = spec.kind {
                                self.marking.unmark_all();
                            }
                        }
                        crate::job::JobKind::CalculateSize { location } => {
                            if let crate::job::OpResult::Success(crate::job::SuccessData::SizeCalculated(size)) = result {
                                for tab in self.tabs.tabs.iter_mut() {
                                    if let Some(entry) = tab.left_pane.entries.iter_mut().find(|e| e.location == *location) {
                                        entry.calculated_size = Some(*size);
                                    }
                                    if let Some(entry) = tab.right_pane.entries.iter_mut().find(|e| e.location == *location) {
                                        entry.calculated_size = Some(*size);
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::ExecuteCustomFunction { command, .. } => {
                            let config_manager = crate::config::ConfigManager::new();
                            let config_path = config_manager.config_path().to_string_lossy().to_string();
                            
                            if command.contains(&config_path) {
                                if let crate::job::OpResult::Success(_) = result {
                                    let dialog = crate::model::Dialog::confirmation(
                                        "Configuration Editor Closed",
                                        "Reload configuration?"
                                    );
                                    self.dialogs.push(dialog);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(log) = log_entry {
                    result_obj.task_panel_logs.push(log);
                }

                match result {
                    crate::job::OpResult::Success(_) => self.background_jobs.mark_job_completed(*job_id),
                    crate::job::OpResult::Failed(e) => self.background_jobs.mark_job_failed(*job_id, e.clone()),
                    crate::job::OpResult::Cancelled => self.background_jobs.mark_job_cancelled(*job_id),
                }

                Some(result_obj)
            }
            Transition::CancelJob { job_id } => {
                if self.jobs.request_cancel(*job_id) {
                    Some(StateUpdateResult::with_cancel(*job_id))
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::AcknowledgeCancel { job_id } => {
                self.jobs.acknowledge_cancel(*job_id);
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }

    fn handle_ui_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::ChangeUIMode { mode } => {
                self.ui.mode = *mode;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePaneHeight { height } => {
                self.ui.layout.pane_height = *height;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowDialog { dialog } => {
                self.dialogs.push(dialog.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CloseDialog => {
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ToggleTaskPanel => {
                self.ui.layout.show_task_panel = !self.ui.layout.show_task_panel;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::IncreaseTaskPanelHeight => {
                if self.ui.layout.task_panel_height < 20 {
                    self.ui.layout.task_panel_height += 1;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::DecreaseTaskPanelHeight => {
                if self.ui.layout.task_panel_height > 3 {
                    self.ui.layout.task_panel_height -= 1;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ScrollTaskPanelUp => {
                if self.ui.layout.task_panel_scroll_offset > 0 {
                    self.ui.layout.task_panel_scroll_offset -= 1;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ScrollTaskPanelDown => {
                let total_items = self.jobs.queue.len() 
                    + self.jobs.active.len() 
                    + self.jobs.completed.len();
                
                if total_items > self.ui.layout.task_panel_height {
                    let max_scroll = total_items.saturating_sub(self.ui.layout.task_panel_height);
                    if self.ui.layout.task_panel_scroll_offset < max_scroll {
                        self.ui.layout.task_panel_scroll_offset += 1;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ConfirmDialog => {
                if let Some(dialog) = self.dialogs.current() {
                    match &dialog.content {
                        crate::model::DialogContent::Confirmation { .. } => {
                            let title = dialog.title.as_str();
                            if title == "Copy" {
                                let sources = if self.marking.count() > 0 {
                                    self.active_pane().entries.iter()
                                        .filter(|e| self.marking.is_marked(&e.location))
                                        .map(|e| e.location.clone())
                                        .collect()
                                } else if let Some(entry) = self.active_pane().current_entry() {
                                    vec![entry.location.clone()]
                                } else {
                                    vec![]
                                };
                                
                                if !sources.is_empty() {
                                    let dest = self.opposite_pane().current_location.clone();
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Copy { sources, dest });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Move" {
                                let sources = if self.marking.count() > 0 {
                                    self.active_pane().entries.iter()
                                        .filter(|e| self.marking.is_marked(&e.location))
                                        .map(|e| e.location.clone())
                                        .collect()
                                } else if let Some(entry) = self.active_pane().current_entry() {
                                    vec![entry.location.clone()]
                                } else {
                                    vec![]
                                };
                                
                                if !sources.is_empty() {
                                    let dest = self.opposite_pane().current_location.clone();
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Move { sources, dest });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Delete" {
                                let targets = if self.marking.count() > 0 {
                                    self.active_pane().entries.iter()
                                        .filter(|e| self.marking.is_marked(&e.location))
                                        .map(|e| e.location.clone())
                                        .collect()
                                } else if let Some(entry) = self.active_pane().current_entry() {
                                    vec![entry.location.clone()]
                                } else {
                                    vec![]
                                };
                                
                                if !targets.is_empty() {
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Delete { targets });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Configuration Editor Closed" {
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::ReloadConfig));
                            }
                        }
                        crate::model::DialogContent::Input { .. } => {
                            let title = dialog.title.as_str();
                            let input = self.dialogs.input_buffer.clone();
                            
                            if title == "Search" {
                                if !input.is_empty() {
                                    self.search.add_to_history(input.clone());
                                    if let Some(first_result) = self.search.current_result() {
                                        if let Some(index) = self.active_pane().entries.iter()
                                            .position(|e| e.location == first_result.location) {
                                            self.dialogs.pop();
                                            return Some(update_state(self, Transition::CursorJump {
                                                pane: self.ui.active_pane,
                                                position: index,
                                            }));
                                        }
                                    }
                                }
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::ChangeUIMode { mode: crate::model::UIMode::Normal }));
                            } else if title == "Rename" {
                                if let Some(entry) = self.active_pane().current_entry() {
                                    let from = entry.location.clone();
                                    let to = from.parent()
                                        .map(|parent| parent.join(&input))
                                        .unwrap_or_else(|| from.clone());
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Rename { from, to });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Create Directory" {
                                let current_location = self.active_pane().current_location.clone();
                                let new_dir_location = current_location.join(&input);
                                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Mkdir { location: new_dir_location });
                                self.dialogs.pop();
                                return Some(StateUpdateResult::with_job(job_spec));
                            } else if title == "Wildcard Marking" {
                                if !input.is_empty() {
                                    self.dialogs.pop();
                                    return Some(update_state(self, Transition::MarkPattern { pattern: input }));
                                }
                            } else if title == "Register Folder" {
                                if !input.is_empty() {
                                    self.dialogs.pop();
                                    return Some(update_state(self, Transition::RegisterCurrentFolder { name: input }));
                                }
                            } else if title == "File Mask Filter" {
                                self.dialogs.pop();
                                let mask = if input.is_empty() { None } else { Some(input) };
                                return Some(update_state(self, Transition::SetFileMask { pane: self.ui.active_pane, mask }));
                            }
                        }
                        crate::model::DialogContent::DriveSelection { drives, selected_index } => {
                            if let Some(drive) = drives.get(*selected_index) {
                                let location = crate::model::Location::Local(std::path::PathBuf::from(&drive.path));
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::ChangeLocation { pane, location }));
                            }
                        }
                        crate::model::DialogContent::RegisteredFolderSelector { selected_index, .. } => {
                            let folder_index = *selected_index;
                            if self.marking.count() > 0 {
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::MoveToRegisteredFolder { folder_index }));
                            } else {
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::NavigateToRegisteredFolder { folder_index }));
                            }
                        }
                        _ => {}
                    }
                }
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CancelDialog => {
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowContextMenu => {
                let dialog = crate::model::Dialog::context_menu();
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowDriveChangeDialog => {
                let drives = crate::volume_info::get_all_drives();
                let dialog = crate::model::Dialog::drive_selection(drives);
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowFileInfo => {
                if let Some(entry) = self.active_pane().current_entry() {
                    let dialog = crate::model::Dialog::file_info(entry);
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::ShowVersion => {
                let dialog = crate::model::Dialog::version();
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SaveLog => {
                let _ = self.log_manager.save_to_file();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::RotateHelpLanguage => {
                let current_lang = &self.config.help_language;
                let next_lang = crate::help_content::HelpContent::next_language(current_lang);
                self.config.help_language = next_lang.clone();
                
                if let Some(dialog) = self.dialogs.current_mut() {
                    if matches!(dialog.content, crate::model::DialogContent::Help { .. }) {
                        *dialog = crate::model::Dialog::help_with_language(&next_lang);
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowRegisteredFolderDialog => {
                let folders = self.registered_folders.folders.clone();
                let dialog = crate::model::Dialog::registered_folder_selector(folders);
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::RegisterCurrentFolder { name } => {
                let current_location = self.active_pane().current_location.clone();
                let path = current_location.display_path();
                let folder = crate::model::RegisteredFolder::new(name.clone(), path);
                self.registered_folders.add(folder);
                
                let save_path = crate::model::RegisteredFolderManager::default_path();
                let _ = self.registered_folders.save_to_file(&save_path);
                
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::NavigateToRegisteredFolder { folder_index } => {
                if let Some(folder) = self.registered_folders.folders.get(*folder_index) {
                    let expanded_path = self.registered_folders.expand_path(folder);
                    let location = crate::model::Location::Local(expanded_path);
                    
                    self.dialogs.pop();
                    return Some(update_state(self, Transition::ChangeLocation {
                        pane: self.ui.active_pane,
                        location,
                    }));
                }
                Some(StateUpdateResult::none())
            }
            Transition::MoveToRegisteredFolder { folder_index } => {
                if let Some(folder) = self.registered_folders.folders.get(*folder_index) {
                    let expanded_path = self.registered_folders.expand_path(folder);
                    let dest = crate::model::Location::Local(expanded_path);
                    
                    let sources = if self.marking.count() > 0 {
                        self.active_pane().entries.iter()
                            .filter(|e| self.marking.is_marked(&e.location))
                            .map(|e| e.location.clone())
                            .collect()
                    } else {
                        vec![]
                    };
                    
                    if !sources.is_empty() {
                        let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Move { sources, dest });
                        self.dialogs.pop();
                        return Some(StateUpdateResult::with_job(job_spec));
                    }
                }
                Some(StateUpdateResult::none())
            }
            Transition::LaunchConfigurationProgram => {
                let editor_command = self.config.editor_command.clone()
                    .unwrap_or_else(|| {
                        #[cfg(target_os = "windows")]
                        { "notepad".to_string() }
                        #[cfg(not(target_os = "windows"))]
                        {
                            std::env::var("EDITOR")
                                .or_else(|_| std::env::var("VISUAL"))
                                .unwrap_or_else(|_| "vi".to_string())
                        }
                    });
                
                let config_manager = crate::config::ConfigManager::new();
                let config_path = config_manager.config_path().to_string_lossy().to_string();
                let command = format!("{} \"{}\"", editor_command, config_path);
                
                let working_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."));
                
                let job_spec = JobSpec::new(crate::job::JobKind::ExecuteCustomFunction {
                    command,
                    working_dir: crate::model::Location::Local(working_dir),
                    pipe_to_action: None,
                    shell: None,
                });
                
                Some(StateUpdateResult::with_job(job_spec))
            }
            _ => None,
        }
    }

    fn handle_view_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::ChangeSortMode { pane, mode } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.sort_mode = *mode;
                pane_model.apply_sort();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ChangeDisplayMode { pane, mode } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.display_mode = *mode;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SetFileMask { pane, mask } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.file_mask = mask.clone();
                Some(StateUpdateResult::with_refresh(self.tabs.active_index, *pane))
            }
            Transition::ToggleHidden => {
                self.ui.show_hidden = !self.ui.show_hidden;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::Refresh { pane } | 
            Transition::RefreshAndClearMarks { pane } |
            Transition::RefreshNoClearMarks { pane } => {
                if let Transition::RefreshAndClearMarks { .. } = transition {
                    self.marking.unmark_all();
                }
                let tab = self.current_tab();
                let location = match pane {
                    crate::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                    crate::model::ActivePane::Right => tab.right_pane.current_location.clone(),
                };
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location });
                Some(StateUpdateResult::with_job(job_spec))
            }
            _ => None,
        }
    }

    fn handle_search_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::StartSearch { query } => {
                self.search.start_search(query.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdateSearchQuery { query } => {
                self.search.query = query.clone();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdateSearchResults { results } => {
                self.search.results = results.clone();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::NextSearchResult => {
                self.search.next_result();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::PrevSearchResult => {
                self.search.prev_result();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ClearSearch => {
                self.search.clear();
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }

    fn handle_viewer_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::OpenTextViewer { location } => {
                self.ui.mode = crate::model::UIMode::Viewer;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = crate::model::ViewerMode::Text;
                self.viewer = Some(viewer);
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer { location: location.clone() });
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenHexViewer { location } => {
                self.ui.mode = crate::model::UIMode::Viewer;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = crate::model::ViewerMode::Hex;
                self.viewer = Some(viewer);
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer { location: location.clone() });
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::CloseViewer => {
                self.ui.mode = crate::model::UIMode::Normal;
                self.viewer = None;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerLoadComplete { contents } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.set_contents(contents.clone());
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerCycleEncoding => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.cycle_encoding();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollDown { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.scroll_down(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollUp => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.scroll_up();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerPageDown { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.page_down(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerPageUp { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.page_up(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerJumpToTop => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.jump_to_top();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerJumpToBottom => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.jump_to_bottom(20); // TODO: Get viewport height
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerMoveToLineStart => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.move_to_line_start();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerMoveToLineEnd { viewport_width } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.move_to_line_end(*viewport_width);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerStartSearch { query } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.start_search(query.to_string());
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFindNext => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.find_next();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFindPrev => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.find_prev();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerClearSearch => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.clear_search();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }

    fn handle_advanced_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::SyncPanes => {
                let active_location = self.active_pane().current_location.clone();
                let opposite_pane = self.ui.active_pane.opposite();
                
                let cached_entries = self.cache.get(&active_location);
                
                let tab = self.current_tab_mut();
                let opposite_pane_model = match opposite_pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                
                tab.history.push(opposite_pane, opposite_pane_model.current_location.clone());
                
                opposite_pane_model.current_location = active_location.clone();
                opposite_pane_model.cursor = 0;
                opposite_pane_model.scroll_offset = 0;
                
                if let Some(entries) = cached_entries {
                    opposite_pane_model.entries = entries;
                    opposite_pane_model.apply_sort();
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location: active_location });
                    Some(StateUpdateResult::with_job(job_spec))
                }
            }
            Transition::SwapPanes => {
                let tab = self.current_tab_mut();
                
                // Swap locations only (Requirement 41.5: cursors stay with panes)
                std::mem::swap(&mut tab.left_pane.current_location, &mut tab.right_pane.current_location);
                
                tab.history.swap_panes();
                
                let left_location = tab.left_pane.current_location.clone();
                let right_location = tab.right_pane.current_location.clone();
                
                let left_cached = self.cache.get(&left_location);
                let right_cached = self.cache.get(&right_location);
                
                let tab = self.current_tab_mut();
                
                let left_needs_job = if let Some(entries) = left_cached {
                    tab.left_pane.entries = entries;
                    tab.left_pane.apply_sort();
                    false
                } else {
                    true
                };
                
                let right_needs_job = if let Some(entries) = right_cached {
                    tab.right_pane.entries = entries;
                    tab.right_pane.apply_sort();
                    false
                } else {
                    true
                };
                
                let mut result = StateUpdateResult::with_ui_change();
                
                if left_needs_job {
                    result.jobs_to_start.push(JobSpec::new(crate::job::JobKind::ReadDirectory { 
                        location: left_location 
                    }));
                }
                
                if right_needs_job {
                    result.jobs_to_start.push(JobSpec::new(crate::job::JobKind::ReadDirectory { 
                        location: right_location 
                    }));
                }
                
                Some(result)
            }
            Transition::CompareFiles { left, right } => {
                let job_spec = JobSpec::new(crate::job::JobKind::CompareFiles { 
                    left: left.clone(), 
                    right: right.clone() 
                });
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowComparisonView { diff } => {
                let dialog = crate::model::Dialog {
                    title: "File Comparison".to_string(),
                    content: crate::model::DialogContent::ComparisonView { 
                        diff: diff.clone(),
                        scroll_offset: 0,
                    },
                };
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CloseComparisonView => {
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowSplitJoinDialog => {
                let dialog = crate::model::Dialog::split_join_dialog();
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ExecuteFileSplit { source, dest_dir, chunk_size } => {
                let job_spec = JobSpec::new(crate::job::JobKind::SplitFile { 
                    source: source.clone(), 
                    dest_dir: dest_dir.clone(), 
                    chunk_size: *chunk_size 
                });
                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ExecuteFileJoin { parts, dest } => {
                let job_spec = JobSpec::new(crate::job::JobKind::JoinFiles { 
                    parts: parts.clone(), 
                    dest: dest.clone() 
                });
                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowPatternRenameDialog => {
                let filenames = if self.marking.count() > 0 {
                    self.active_pane().entries.iter()
                        .filter(|e| self.marking.is_marked(&e.location))
                        .map(|e| e.name.clone())
                        .collect()
                } else if let Some(entry) = self.active_pane().current_entry() {
                    vec![entry.name.clone()]
                } else {
                    vec![]
                };
                
                if !filenames.is_empty() {
                    let dialog = crate::model::Dialog::pattern_rename(String::new(), vec![]);
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::UpdatePatternRenamePattern { pattern } => {
                let filenames = if self.marking.count() > 0 {
                    self.active_pane().entries.iter()
                        .filter(|e| self.marking.is_marked(&e.location))
                        .map(|e| e.name.clone())
                        .collect()
                } else if let Some(entry) = self.active_pane().current_entry() {
                    vec![entry.name.clone()]
                } else {
                    vec![]
                };
                
                let preview = crate::pattern_rename::generate_preview(&filenames, pattern);
                if let Some(dialog) = self.dialogs.current_mut() {
                    match &mut dialog.content {
                        crate::model::DialogContent::PatternRename { pattern: p, preview: pr, .. } => {
                            *p = pattern.clone();
                            *pr = preview;
                        }
                        _ => {}
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ExecutePatternRename { pattern, targets } => {
                let job_spec = JobSpec::new(crate::job::JobKind::PatternRename {
                    targets: targets.clone(),
                    pattern: pattern.clone(),
                });
                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            _ => None,
        }
    }

    fn handle_job_management_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::CreateBackgroundJob { spec, name, description } => {
                let tab = self.current_tab();
                let tab_name = format!("{}|{}",
                    tab.left_pane.current_location.display_path(),
                    tab.right_pane.current_location.display_path()
                );
                let tab_id = self.tabs.active_index;

                self.background_jobs.start_job(
                    name.clone(),
                    description.clone(),
                    tab_id,
                    tab_name,
                    spec.clone(),
                );
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CreateAndStartCountDownJob { spec, name, description } |
            Transition::CreateAndStartFileJob { spec, name, description } => {
                let tab = self.current_tab();
                let tab_name = format!("{}|{}",
                    tab.left_pane.current_location.display_path(),
                    tab.right_pane.current_location.display_path()
                );
                let tab_id = self.tabs.active_index;

                let bg_job_id = self.background_jobs.start_job(
                    name.clone(),
                    description.clone(),
                    tab_id,
                    tab_name,
                    spec.clone(),
                );

                self.jobs.start_job(spec.clone());

                let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                let log_msg = format!(
                    "{} [Job {}] [Tab {}] {}: Started",
                    timestamp,
                    bg_job_id.short_id,
                    tab_id + 1,
                    name
                );

                Some(StateUpdateResult {
                    jobs_to_start: vec![spec.clone()],
                    jobs_to_cancel: Vec::new(),
                    completed_jobs: Vec::new(),
                    failed_jobs: Vec::new(),
                    cancelled_jobs: Vec::new(),
                    started_jobs: Vec::new(),
                    task_panel_logs: vec![log_msg],
                    panes_to_refresh: Vec::new(),
                    ui_changed: true,
                })
            }
            Transition::CreatePendingFileJob { spec, name, description: _ } => {
                // Create job spec WITHOUT starting it yet
                // Job will be started after conflict detection (or after dialog confirmation)
                debug!("CreatePendingFileJob: {:?} (will start after conflict check)", spec.kind);
                
                let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                let log_msg = format!(
                    "{} [Pending] {}: Waiting for conflict check",
                    timestamp,
                    name
                );

                Some(StateUpdateResult {
                    jobs_to_start: vec![spec.clone()],
                    jobs_to_cancel: Vec::new(),
                    completed_jobs: Vec::new(),
                    failed_jobs: Vec::new(),
                    cancelled_jobs: Vec::new(),
                    started_jobs: Vec::new(),
                    task_panel_logs: vec![log_msg],
                    panes_to_refresh: Vec::new(),
                    ui_changed: true,
                })
            }
            _ => None,
        }
    }
}

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
    SyncPanes,
    SwapPanes,
    
    // Tab management
    CreateTab,
    CloseTab { index: usize },
    NextTab,
    PrevTab,
    SwitchTab { index: usize },
    
    // File marking
    ToggleMark { location: crate::model::Location },
    MarkAll,
    UnmarkAll,
    MarkPattern { pattern: String },
    MarkRange { start: usize, end: usize },
    InvertMarks,
    EnterRangeMarkingMode,
    
    // Background jobs
    EnqueueJob { spec: JobSpec },
    StartNextJob,
    JobStarted { job_id: JobId },
    UpdateJobProgress { job_id: JobId, progress: f64 },
    UpdateJobProgressWithDetail { 
        job_id: JobId, 
        progress: f64, 
        progress_message: String, 
        operation_detail: String 
    },
    CompleteJob { job_id: JobId, result: crate::job::OpResult },
    CancelJob { job_id: JobId },
    AcknowledgeCancel { job_id: JobId },
    CreateBackgroundJob { spec: JobSpec, name: String, description: String },
    CreateAndStartFileJob { spec: JobSpec, name: String, description: String },
    CreatePendingFileJob { spec: JobSpec, name: String, description: String },
    CreateAndStartCountDownJob { spec: JobSpec, name: String, description: String },
    AddTaskPanelLog { message: String },
    
    // View settings
    ChangeSortMode { pane: crate::model::ActivePane, mode: crate::model::SortMode },
    ChangeDisplayMode { pane: crate::model::ActivePane, mode: crate::model::DisplayMode },
    SetFileMask { pane: crate::model::ActivePane, mask: Option<String> },
    ToggleHidden,
    Refresh { pane: crate::model::ActivePane },
    RefreshAndClearMarks { pane: crate::model::ActivePane },
    RefreshNoClearMarks { pane: crate::model::ActivePane },
    
    // UI state
    ChangeUIMode { mode: crate::model::UIMode },
    UpdatePaneHeight { height: usize },
    ShowDialog { dialog: crate::model::Dialog },
    CloseDialog,
    UpdateDialogInput { input: String },
    ConfirmDialog,
    CancelDialog,
    ToggleTaskPanel,
    IncreaseTaskPanelHeight,
    DecreaseTaskPanelHeight,
    ScrollTaskPanelUp,
    ScrollTaskPanelDown,
    ShowContextMenu,
    ShowDriveChangeDialog,
    ShowFileInfo,
    ShowVersion,
    SaveLog,
    RotateHelpLanguage,
    LaunchConfigurationProgram,
    ShowRegisteredFolderDialog,
    RegisterCurrentFolder { name: String },
    NavigateToRegisteredFolder { folder_index: usize },
    MoveToRegisteredFolder { folder_index: usize },
    
    // Configuration
    ReloadConfig,
    UpdateConfig { config: Box<AppConfig> },
    
    // Search
    StartSearch { query: String },
    UpdateSearchQuery { query: String },
    UpdateSearchResults { results: Vec<crate::model::FileEntry> },
    NextSearchResult,
    PrevSearchResult,
    ClearSearch,
    
    // Viewer
    OpenTextViewer { location: crate::model::Location },
    OpenHexViewer { location: crate::model::Location },
    CloseViewer,
    ViewerLoadComplete { contents: Vec<u8> },
    ViewerCycleEncoding,
    ViewerScrollDown { viewport_height: usize },
    ViewerScrollUp,
    ViewerPageDown { viewport_height: usize },
    ViewerPageUp { viewport_height: usize },
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
    /// Completed job IDs (for logging)
    pub completed_jobs: Vec<JobId>,
    /// Failed job IDs (for logging)
    pub failed_jobs: Vec<JobId>,
    /// Cancelled job IDs (for logging)
    pub cancelled_jobs: Vec<JobId>,
    /// Started job IDs (for logging)
    pub started_jobs: Vec<JobId>,
    /// Log messages for task panel
    pub task_panel_logs: Vec<String>,
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
            completed_jobs: Vec::new(),
            failed_jobs: Vec::new(),
            cancelled_jobs: Vec::new(),
            started_jobs: Vec::new(),
            task_panel_logs: Vec::new(),
            panes_to_refresh: Vec::new(),
            ui_changed: false,
        }
    }

    /// Create a result that starts a job
    pub fn with_job(job: JobSpec) -> Self {
        Self {
            jobs_to_start: vec![job],
            jobs_to_cancel: Vec::new(),
            completed_jobs: Vec::new(),
            failed_jobs: Vec::new(),
            cancelled_jobs: Vec::new(),
            started_jobs: Vec::new(),
            task_panel_logs: Vec::new(),
            panes_to_refresh: Vec::new(),
            ui_changed: true,
        }
    }

    /// Create a result that only triggers UI update
    pub fn with_ui_change() -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: Vec::new(),
            completed_jobs: Vec::new(),
            failed_jobs: Vec::new(),
            cancelled_jobs: Vec::new(),
            started_jobs: Vec::new(),
            task_panel_logs: Vec::new(),
            panes_to_refresh: Vec::new(),
            ui_changed: true,
        }
    }

    /// Create a result that refreshes a specific pane
    pub fn with_refresh(tab_id: usize, pane: crate::model::ActivePane) -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: Vec::new(),
            completed_jobs: Vec::new(),
            failed_jobs: Vec::new(),
            cancelled_jobs: Vec::new(),
            started_jobs: Vec::new(),
            task_panel_logs: Vec::new(),
            panes_to_refresh: vec![PaneRefresh { tab_id, pane }],
            ui_changed: true,
        }
    }

    /// Create a result that cancels a job
    pub fn with_cancel(job_id: JobId) -> Self {
        Self {
            jobs_to_start: Vec::new(),
            jobs_to_cancel: vec![job_id],
            completed_jobs: Vec::new(),
            failed_jobs: Vec::new(),
            cancelled_jobs: Vec::new(),
            started_jobs: Vec::new(),
            task_panel_logs: Vec::new(),
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
    if let Some(result) = state.handle_navigation_transition(&transition) {
        return result;
    }
    
    if let Some(result) = state.handle_tab_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_marking_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_job_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_job_management_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_ui_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_view_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_search_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_viewer_transition(&transition) {
        return result;
    }

    if let Some(result) = state.handle_advanced_transition(&transition) {
        return result;
    }

    match transition {
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
        
        Transition::ReloadConfig => {
            let config_manager = crate::config::ConfigManager::new();
            if let Ok(new_config) = config_manager.load_config() {
                state.config = new_config;
            }
            StateUpdateResult::with_ui_change()
        }
        
        Transition::UpdateConfig { config } => {
            state.jobs.max_parallel = config.worker_pool_size;
            state.config = *config;
            StateUpdateResult::with_ui_change()
        }
        
        Transition::Quit => {
            StateUpdateResult::none()
        }
        
        Transition::ExitAndChangeDirectory => {
            StateUpdateResult::none()
        }

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
        state.active_pane_mut().entries = entries;
        
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
        
        state.marking.toggle(Location::Local(PathBuf::from("/test/file1.txt")));
        state.marking.toggle(Location::Local(PathBuf::from("/test/file2.txt")));
        assert_eq!(state.marking.count(), 2);
        
        let result = update_state(&mut state, Transition::UnmarkAll);
        assert!(result.ui_changed);
        assert_eq!(state.marking.count(), 0);
    }

    #[test]
    fn test_mark_pattern_transition() {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            FileEntry {
                name: "test.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/test.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "other.doc".to_string(),
                location: Location::Local(PathBuf::from("/test/other.doc")),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        state.active_pane_mut().entries = entries;
        
        let result = update_state(&mut state, Transition::MarkPattern { pattern: "*.txt".to_string() });
        assert!(result.ui_changed);
        assert_eq!(state.marking.count(), 1);
        assert!(state.marking.is_marked(&Location::Local(PathBuf::from("/test/test.txt"))));
    }

    #[test]
    fn test_mark_range_transition() {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            FileEntry {
                name: "f1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/f1.txt")),
                size: 10,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "f2.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/f2.txt")),
                size: 10,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "f3.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/f3.txt")),
                size: 10,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        state.active_pane_mut().entries = entries;
        
        let result = update_state(&mut state, Transition::MarkRange { start: 0, end: 1 });
        assert!(result.ui_changed);
        assert_eq!(state.marking.count(), 2);
    }

    #[test]
    fn test_invert_marks_transition() {
        use crate::model::{Location, FileEntry};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            FileEntry {
                name: "f1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/f1.txt")),
                size: 10,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "f2.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/f2.txt")),
                size: 10,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        state.active_pane_mut().entries = entries;
        
        state.marking.toggle(Location::Local(PathBuf::from("/test/f1.txt")));
        assert_eq!(state.marking.count(), 1);
        
        let result = update_state(&mut state, Transition::InvertMarks);
        assert!(result.ui_changed);
        assert_eq!(state.marking.count(), 1);
        assert!(state.marking.is_marked(&Location::Local(PathBuf::from("/test/f2.txt"))));
    }

    #[test]
    fn test_enter_range_marking_mode_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        state.active_pane_mut().cursor = 5;
        
        let result = update_state(&mut state, Transition::EnterRangeMarkingMode);
        assert!(result.ui_changed);
        assert_eq!(state.ui.range_marking_start, Some(5));
    }

    #[test]
    fn test_cursor_move_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some dummy entries so cursor has somewhere to move
        state.active_pane_mut().entries = vec![
            crate::model::FileEntry::dummy("f1"),
            crate::model::FileEntry::dummy("f2"),
            crate::model::FileEntry::dummy("f3"),
        ];
        
        assert_eq!(state.active_pane().cursor, 0);
        
        let result = update_state(&mut state, Transition::CursorMove { pane: ActivePane::Left, delta: 1 });
        assert!(result.ui_changed);
        assert_eq!(state.active_pane().cursor, 1);
        
        update_state(&mut state, Transition::CursorMove { pane: ActivePane::Left, delta: 1 });
        assert_eq!(state.active_pane().cursor, 2);
        
        // Should clamp to last entry
        update_state(&mut state, Transition::CursorMove { pane: ActivePane::Left, delta: 1 });
        assert_eq!(state.active_pane().cursor, 2);
        
        // Should clamp to 0
        update_state(&mut state, Transition::CursorMove { pane: ActivePane::Left, delta: -10 });
        assert_eq!(state.active_pane().cursor, 0);
    }

    #[test]
    fn test_cursor_jump_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        state.active_pane_mut().entries = vec![
            crate::model::FileEntry::dummy("f1"),
            crate::model::FileEntry::dummy("f2"),
            crate::model::FileEntry::dummy("f3"),
        ];
        
        let result = update_state(&mut state, Transition::CursorJump { pane: ActivePane::Left, position: 2 });
        assert!(result.ui_changed);
        assert_eq!(state.active_pane().cursor, 2);
        
        // Should clamp
        update_state(&mut state, Transition::CursorJump { pane: ActivePane::Left, position: 100 });
        assert_eq!(state.active_pane().cursor, 2);
    }

    #[test]
    fn test_change_location_with_cache_hit() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let loc = Location::Local(PathBuf::from("/test"));
        let entries = vec![crate::model::FileEntry::dummy("file")];
        state.cache.insert(loc.clone(), entries);
        
        let result = update_state(&mut state, Transition::ChangeLocation { pane: ActivePane::Left, location: loc.clone() });
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 0);
        assert_eq!(state.active_pane().current_location, loc);
        assert_eq!(state.active_pane().entries.len(), 1);
    }

    #[test]
    fn test_change_location_with_cache_miss() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let loc = Location::Local(PathBuf::from("/new_place"));
        
        let result = update_state(&mut state, Transition::ChangeLocation { pane: ActivePane::Left, location: loc.clone() });
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        assert!(matches!(result.jobs_to_start[0].kind, crate::job::JobKind::ReadDirectory { .. }));
        assert_eq!(state.active_pane().current_location, loc);
    }

    #[test]
    fn test_enqueue_job_transition() {
        use crate::job::{JobSpec, JobKind};
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let job_spec = JobSpec::new(JobKind::CountDown { duration_secs: 1, start_value: 1 });
        
        assert_eq!(state.jobs.queue.len(), 0);
        
        let result = update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        assert!(result.ui_changed);
        assert_eq!(state.jobs.queue.len(), 1);
    }

    #[test]
    fn test_start_next_job_transition() {
        use crate::job::{JobSpec, JobKind};
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let job_spec = JobSpec::new(JobKind::CountDown { duration_secs: 1, start_value: 1 });
        state.jobs.enqueue(job_spec);
        
        assert_eq!(state.jobs.queue.len(), 1);
        assert_eq!(state.jobs.active.len(), 0);
        
        let result = update_state(&mut state, Transition::StartNextJob);
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        assert_eq!(state.jobs.queue.len(), 0);
        assert_eq!(state.jobs.active.len(), 1);
    }

    #[test]
    fn test_job_started_transition() {
        use crate::job::{JobSpec, JobKind, ExecutionState};
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let spec = JobSpec::new(JobKind::CountDown { duration_secs: 1, start_value: 1 });
        let job_id = spec.id;
        state.jobs.start_job(spec);
        
        assert!(matches!(state.jobs.active.get(&job_id).unwrap().state, ExecutionState::Pending));
        
        let result = update_state(&mut state, Transition::JobStarted { job_id });
        assert!(result.ui_changed);
        assert!(matches!(state.jobs.active.get(&job_id).unwrap().state, ExecutionState::Running));
    }

    #[test]
    fn test_complete_job_transition() {
        use crate::job::{JobSpec, JobKind, OpResult, SuccessData};
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let spec = JobSpec::new(JobKind::CountDown { duration_secs: 1, start_value: 1 });
        let job_id = spec.id;
        state.jobs.start_job(spec);
        
        let result = update_state(&mut state, Transition::CompleteJob { 
            job_id, 
            result: OpResult::Success(SuccessData::None) 
        });
        
        assert!(result.ui_changed);
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }

    #[test]
    fn test_cancel_job_transition() {
        use crate::job::{JobSpec, JobKind};
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let spec = JobSpec::new(JobKind::CountDown { duration_secs: 5, start_value: 5 });
        let job_id = spec.id;
        state.jobs.start_job(spec);
        
        let result = update_state(&mut state, Transition::CancelJob { job_id });
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_cancel.len(), 1);
        assert_eq!(result.jobs_to_cancel[0], job_id);
        assert!(state.jobs.active.get(&job_id).unwrap().cancel_requested);
    }

    #[test]
    fn test_sync_panes_transition() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let loc = Location::Local(PathBuf::from("/source"));
        state.current_tab_mut().left_pane.current_location = loc.clone();
        state.ui.active_pane = ActivePane::Left;
        
        let result = update_state(&mut state, Transition::SyncPanes);
        
        assert!(result.ui_changed);
        assert_eq!(state.current_tab().right_pane.current_location, loc);
        // Should trigger directory read for right pane (if not cached)
        assert_eq!(result.jobs_to_start.len(), 1);
    }

    #[test]
    fn test_swap_panes_transition() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let loc1 = Location::Local(PathBuf::from("/loc1"));
        let loc2 = Location::Local(PathBuf::from("/loc2"));
        
        state.current_tab_mut().left_pane.current_location = loc1.clone();
        state.current_tab_mut().right_pane.current_location = loc2.clone();
        
        let result = update_state(&mut state, Transition::SwapPanes);
        assert!(result.ui_changed);
        
        assert_eq!(state.current_tab().left_pane.current_location, loc2);
        assert_eq!(state.current_tab().right_pane.current_location, loc1);
    }

    #[test]
    fn test_change_ui_mode_transition() {
        use crate::model::UIMode;
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        assert_eq!(state.ui.mode, UIMode::Normal);
        
        let result = update_state(&mut state, Transition::ChangeUIMode { mode: UIMode::Dialog });
        assert!(result.ui_changed);
        assert_eq!(state.ui.mode, UIMode::Dialog);
    }

    #[test]
    fn test_show_dialog_transition() {
        use crate::model::Dialog;
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let dialog = Dialog::confirmation("Title", "Message");
        
        let result = update_state(&mut state, Transition::ShowDialog { dialog });
        assert!(result.ui_changed);
        assert!(state.dialogs.current().is_some());
        assert_eq!(state.dialogs.current().unwrap().title, "Title");
    }

    #[test]
    fn test_close_dialog_transition() {
        use crate::model::Dialog;
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        state.dialogs.push(Dialog::confirmation("Title", "Message"));
        assert!(state.dialogs.current().is_some());
        
        let result = update_state(&mut state, Transition::CloseDialog);
        assert!(result.ui_changed);
        assert!(state.dialogs.current().is_none());
    }
}
