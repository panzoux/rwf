//! Application state management
//!
//! This module defines the central AppState structure and the Transition enum
//! for explicit state changes following the AppState pattern.

use crate::job::{JobManager, JobId, JobSpec, BackgroundJobManager};
use crate::model::{TabManager, SearchModel, UIState, DialogStack, DirectoryCache, ViewerState, NavigationStateCache};
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
    /// Job ID of the active viewer file-loading job (for cancel on close)
    pub viewer_job_id: Option<crate::job::JobId>,
    /// Search query being typed in ViewerSearch mode (not yet committed).
    pub viewer_search_input: String,
    /// Command being typed in ViewerCommand mode (line-jump digits).
    pub viewer_command_input: String,
    /// Session log manager
    pub log_manager: LogManager,
    /// Application configuration
    pub config: AppConfig,
    /// Last time a tab was created, for debouncing
    pub last_tab_created: Option<std::time::Instant>,
    /// File-type extension associations loaded from extension_associations.json
    pub extension_associations: Vec<crate::config::ExtensionAssociation>,
    /// Custom functions loaded from custom_functions.json
    pub custom_functions: Vec<crate::model::dialog::CustomFunction>,
    /// Load results for all config files, used by the verbose version info display
    pub config_load_results: Vec<crate::config::ConfigLoadResult>,
}

impl AppState {
    fn resolve_editor(config: &AppConfig) -> String {
        config.editor_command.clone().unwrap_or_else(|| {
            #[cfg(target_os = "windows")]
            { "notepad".to_string() }
            #[cfg(not(target_os = "windows"))]
            {
                std::env::var("EDITOR")
                    .or_else(|_| std::env::var("VISUAL"))
                    .unwrap_or_else(|_| "vi".to_string())
            }
        })
    }

    /// Build a SpawnProcess job that opens `file_path` in the configured editor.
    /// Splits `EditorCommand` on whitespace to support flags like "code --wait".
    fn editor_job(config: &AppConfig, file_path: String) -> crate::job::JobKind {
        let cmd = Self::resolve_editor(config);
        let mut parts = cmd.split_whitespace();
        let program = parts.next().unwrap_or("notepad").to_string();
        let mut args: Vec<String> = parts.map(str::to_string).collect();
        args.push(file_path);
        crate::job::JobKind::SpawnProcess { program, args }
    }

    pub fn new(config: AppConfig) -> Self {
        let mut registered_folders = crate::model::RegisteredFolderManager::new();
        // Try to load registered folders from default path
        let path = crate::model::RegisteredFolderManager::default_path();
        if let Err(e) = registered_folders.load_from_file(&path) {
            tracing::warn!("Failed to load registered folders: {}", e);
        }
        
        // Create log manager with configured settings.
        // Normalize path separators so display is consistent on Windows.
        let log_path = if config.log_save_path.starts_with('/') || config.log_save_path.contains(':') {
            // Absolute path — collect components to normalise separators
            std::path::Path::new(&config.log_save_path).components().collect::<std::path::PathBuf>()
        } else {
            // Relative: join each component individually so '/' in the value doesn't leak through
            let rel: std::path::PathBuf = std::path::Path::new(&config.log_save_path)
                .components()
                .collect();
            crate::logging::default_log_dir().parent().unwrap().join(rel)
        };
        
        let log_manager = LogManager::new(
            config.max_log_lines_in_memory,
            log_path,
            config.log_file_progress_threshold_ms,
        );
        
        let mut search = SearchModel::new();
        let dict_path = config.search.dict_path.as_deref();
        match search.load_migemo_dict_auto(dict_path) {
            Ok(_) => {
                search.use_migemo = true;
                tracing::info!("Migemo dictionary loaded successfully; migemo search enabled");
            }
            Err(e) => {
                tracing::warn!("Migemo dictionary load failed (Dictionary search feature will fallback to substring matching): {}", e);
            }
        }

        let config_manager = crate::config::ConfigManager::new();
        let (extension_associations, ext_result) = config_manager.load_extension_associations_with_result();

        let custom_fn_path = config_manager.custom_functions_path().to_path_buf();
        let (custom_functions, custom_fn_result) = match crate::model::dialog::load_custom_functions(&custom_fn_path) {
            Ok(fns) if !fns.is_empty() || custom_fn_path.exists() => {
                let result = if custom_fn_path.exists() {
                    crate::config::ConfigLoadResult::ok(custom_fn_path)
                } else {
                    crate::config::ConfigLoadResult::skipped(custom_fn_path, "file not found")
                };
                (fns, result)
            }
            Ok(fns) => (fns, crate::config::ConfigLoadResult::skipped(custom_fn_path, "file not found")),
            Err(e) => (Vec::new(), crate::config::ConfigLoadResult::error(custom_fn_path, e.to_string())),
        };

        let context_menu_result = crate::config::ConfigManager::validate_json_file(
            config_manager.context_menu_path()
        );

        let config_load_results = vec![ext_result, custom_fn_result, context_menu_result];

        Self {
            tabs: TabManager::new(),
            jobs: JobManager::new(config.worker_pool_size),
            background_jobs: BackgroundJobManager::new(
                config.job_manager.max_simultaneous_jobs,
                Duration::from_secs(config.job_manager.job_retention_period_secs)
            ),
            search,
            ui: UIState::new(),
            dialogs: DialogStack::new(),
            registered_folders,
            cache: DirectoryCache::new(Duration::from_secs(30)),
            navigation_cache: NavigationStateCache::new(),
            viewer: None,
            viewer_job_id: None,
            viewer_search_input: String::new(),
            viewer_command_input: String::new(),
            log_manager,
            config,
            last_tab_created: None,
            extension_associations,
            custom_functions,
            config_load_results,
        }
    }
    
    /// Move the current viewer state into the active tab's `tab_viewer` slot
    /// and reset AppState to "no viewer". Called before switching away from a tab.
    fn save_viewer_to_current_tab(&mut self) {
        let idx = self.tabs.active_index;
        let tv = &mut self.tabs.tabs[idx].tab_viewer;
        tv.viewer               = self.viewer.take();
        tv.viewer_job_id        = self.viewer_job_id.take();
        tv.viewer_layout        = self.ui.layout.viewer_layout;
        tv.viewer_preferred_layout = self.ui.layout.viewer_preferred_layout;
        tv.viewer_anchor_pane   = self.ui.layout.viewer_anchor_pane;
        tv.viewer_was_focused   = matches!(
            self.ui.mode,
            crate::model::UIMode::Viewer | crate::model::UIMode::ViewerSearch | crate::model::UIMode::ViewerCommand
        );
        tv.viewer_search_input  = std::mem::take(&mut self.viewer_search_input);
        tv.viewer_command_input = std::mem::take(&mut self.viewer_command_input);

        // Reset global viewer fields to default "no viewer" state.
        self.ui.layout.viewer_layout           = crate::model::ViewerLayout::FullScreen;
        self.ui.layout.viewer_preferred_layout = crate::model::ViewerLayout::FullScreen;
        if matches!(self.ui.mode, crate::model::UIMode::Viewer | crate::model::UIMode::ViewerSearch | crate::model::UIMode::ViewerCommand) {
            self.ui.mode = crate::model::UIMode::Normal;
        }
    }

    /// Restore viewer state from the newly active tab's `tab_viewer` slot into AppState.
    /// Called after switching to a new tab.
    fn restore_viewer_from_tab(&mut self) {
        let idx = self.tabs.active_index;
        let tv = &mut self.tabs.tabs[idx].tab_viewer;
        self.viewer               = tv.viewer.take();
        self.viewer_job_id        = tv.viewer_job_id.take();
        self.ui.layout.viewer_layout           = tv.viewer_layout;
        self.ui.layout.viewer_preferred_layout = tv.viewer_preferred_layout;
        self.ui.layout.viewer_anchor_pane      = tv.viewer_anchor_pane;
        self.viewer_search_input  = std::mem::take(&mut tv.viewer_search_input);
        self.viewer_command_input = std::mem::take(&mut tv.viewer_command_input);
        let was_focused = tv.viewer_was_focused;

        // Reset the slot to default so it's clean for next time.
        *tv = crate::model::TabViewerState::default();

        if was_focused && self.viewer.is_some() {
            self.ui.mode = crate::model::UIMode::Viewer;
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

    /// Clear marks on every pane in every tab
    pub fn unmark_all_panes(&mut self) {
        for tab in self.tabs.tabs.iter_mut() {
            tab.left_pane.marking.unmark_all();
            tab.right_pane.marking.unmark_all();
        }
    }

    /// Save current state to session storage
    pub fn save_session(&self) -> Result<(), crate::session::SessionError> {
        let session = crate::session::save_session(
            &self.tabs.tabs,
            self.tabs.active_index,
            self.ui.active_pane,
            &std::collections::HashSet::new(),
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

        // Prevent duplicate IDs: next_tab_id must be greater than all restored IDs
        self.tabs.update_next_id_after_restore();

        // Restore active tab index (ensure it's valid)
        if session.active_tab_index < self.tabs.tabs.len() {
            self.tabs.active_index = session.active_tab_index;
        } else {
            self.tabs.active_index = 0;
        }

        // Restore active pane
        self.ui.active_pane = session.active_pane.into();

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
                let tab_id = tab.id;
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
                    pane_model.is_loading = false;
                    pane_model.apply_sort();
                    if !pane_model.entries.is_empty() {
                        pane_model.cursor = pane_model.cursor.min(pane_model.entries.len() - 1);
                    } else {
                        pane_model.cursor = 0;
                        pane_model.scroll_offset = 0;
                    }
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    // Clear current entries and set loading state
                    pane_model.entries.clear();
                    pane_model.is_loading = true;
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;

                    let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location: location.clone() })
                        .with_requesting_pane(tab_id, *pane);
                    pane_model.active_job_id = Some(job_spec.id);
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
                        pane_model.is_loading = false;
                        pane_model.apply_sort();
                        Some(StateUpdateResult::with_ui_change())
                    } else {
                        pane_model.entries.clear();
                        pane_model.is_loading = true;
                        let tab_id = self.current_tab().id;
                        let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location })
                            .with_requesting_pane(tab_id, *pane);
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
                let now = std::time::Instant::now();
                if let Some(last) = self.last_tab_created {
                    if now.duration_since(last) < std::time::Duration::from_millis(300) {
                        return Some(StateUpdateResult::none());
                    }
                }
                self.last_tab_created = Some(now);
                
                let new_index = self.tabs.create_tab();
                // Get the stable ID of the new tab
                let tab_id = self.tabs.tabs[new_index].id;

                // Fetch locations and set loading state
                let left_loc = self.tabs.tabs[new_index].left_pane.current_location.clone();
                let right_loc = self.tabs.tabs[new_index].right_pane.current_location.clone();

                let job_left = JobSpec::new(crate::job::JobKind::ReadDirectory {
                    location: left_loc
                }).with_requesting_pane(tab_id, crate::model::ActivePane::Left);

                let job_right = JobSpec::new(crate::job::JobKind::ReadDirectory {
                    location: right_loc
                }).with_requesting_pane(tab_id, crate::model::ActivePane::Right);

                self.tabs.tabs[new_index].left_pane.is_loading = true;
                self.tabs.tabs[new_index].right_pane.is_loading = true;
                self.tabs.tabs[new_index].left_pane.active_job_id = Some(job_left.id);
                self.tabs.tabs[new_index].right_pane.active_job_id = Some(job_right.id);

                tracing::info!("[CreateTab] Created tab index={}, id={}", new_index, tab_id);

                self.tabs.active_index = new_index;

                let mut result = StateUpdateResult::with_ui_change();
                result.jobs_to_start.push(job_left);
                result.jobs_to_start.push(job_right);
                Some(result)
            }
            Transition::CloseTab { index } => {
                if *index >= self.tabs.tabs.len() {
                    return Some(StateUpdateResult::none());
                }

                let tab_id = self.tabs.tabs[*index].id;
                let is_active = *index == self.tabs.active_index;

                // Cancel any viewer job saved in the tab being closed.
                if is_active {
                    // Drop live viewer state (cancel job, reset fields).
                    if let Some(job_id) = self.viewer_job_id.take() {
                        self.jobs.request_cancel(job_id);
                    }
                    self.viewer = None;
                    self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                    if matches!(self.ui.mode, crate::model::UIMode::Viewer | crate::model::UIMode::ViewerSearch | crate::model::UIMode::ViewerCommand) {
                        self.ui.mode = crate::model::UIMode::Normal;
                    }
                } else if let Some(job_id) = self.tabs.tabs[*index].tab_viewer.viewer_job_id {
                    self.jobs.request_cancel(job_id);
                }

                // Collect and cancel all active jobs for this tab
                let active_jobs: Vec<crate::job::JobId> = self.background_jobs
                    .get_active_jobs()
                    .filter(|j| j.tab_id == tab_id)
                    .map(|j| j.id.uuid)
                    .collect();

                for id in &active_jobs {
                    self.background_jobs.cancel_job(*id);
                }

                if self.tabs.close_tab(*index) {
                    if self.tabs.active_index >= self.tabs.tabs.len() {
                        self.tabs.active_index = self.tabs.tabs.len().saturating_sub(1);
                    }
                    if is_active {
                        self.restore_viewer_from_tab();
                    }
                    let mut result = StateUpdateResult::with_ui_change();
                    result.jobs_to_cancel = active_jobs;
                    Some(result)
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::NextTab => {
                self.save_viewer_to_current_tab();
                self.tabs.switch_to_next();
                self.restore_viewer_from_tab();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::PrevTab => {
                self.save_viewer_to_current_tab();
                self.tabs.switch_to_prev();
                self.restore_viewer_from_tab();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SwitchTab { index } => {
                if *index < self.tabs.tabs.len() {
                    self.save_viewer_to_current_tab();
                    self.tabs.active_index = *index;
                    self.restore_viewer_from_tab();
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
                self.active_pane_mut().marking.toggle(location.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkAll => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut().marking.mark_all(&entries);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UnmarkAll => {
                self.active_pane_mut().marking.unmark_all();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkPattern { pattern } => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut().marking.mark_pattern(&entries, pattern);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkRange { start, end } => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut().marking.mark_range(&entries, *start, *end);
                self.ui.range_marking_start = None;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::InvertMarks => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut().marking.invert_marks(&entries);
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
                tracing::info!("[CompleteJob] Received completion event for job={:?}", job_id);
                let job_spec = self.jobs.active.get(job_id).map(|job| job.spec.clone());
                tracing::debug!("[CompleteJob] Processing job_id={:?}, has_spec={}, result_type={}", job_id, job_spec.is_some(), match result { crate::job::OpResult::Success(_) => "Success", crate::job::OpResult::Failed(_) => "Failed", crate::job::OpResult::Cancelled => "Cancelled" });
                
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
                            crate::job::JobKind::CollectJumpCandidates { .. } => "Collect jump candidates",
                            crate::job::JobKind::SpawnProcess { .. } => "Spawn process",
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
                    tracing::info!("[CompleteJob] Job spec kind={:?}, requesting_pane={:?}", spec.kind, spec.requesting_pane);
                    match &spec.kind {
                        crate::job::JobKind::ReadDirectory { location } => {
                            tracing::info!("[CompleteJob::ReadDirectory] location={}, requesting_pane={:?}, success={}", location.display_path(), spec.requesting_pane, matches!(result, crate::job::OpResult::Success(_)));
                            if let crate::job::OpResult::Success(crate::job::SuccessData::DirectoryRead(entries)) = result {
                                if let Some((requesting_tab_id, pane_side)) = spec.requesting_pane {
                                    tracing::info!("[CompleteJob::ReadDirectory] Looking up tab_id={}, current_tabs.len()={}", requesting_tab_id, self.tabs.tabs.len());
                                    for (idx, t) in self.tabs.tabs.iter().enumerate() {
                                        tracing::debug!("[CompleteJob::ReadDirectory] Tab[{}].id={}", idx, t.id);
                                    }
                                    
                                    if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.id == requesting_tab_id) {
                                        let pane = match pane_side {
                                            crate::model::ActivePane::Left => &mut tab.left_pane,
                                            crate::model::ActivePane::Right => &mut tab.right_pane,
                                        };

                                        // Verify job ownership
                                        if pane.active_job_id == Some(*job_id) {
                                            let pane_name = match pane_side { crate::model::ActivePane::Left => "Left", crate::model::ActivePane::Right => "Right" };
                                            tracing::info!("[CompleteJob::ReadDirectory] Found tab! Updating {} pane with {} entries", pane_name, entries.len());
                                            if pane.raw_entries != *entries {
                                                pane.raw_entries = entries.clone();
                                                pane.entries = entries.clone();
                                                pane.is_loading = false;
                                                pane.apply_sort();
                                                pane.apply_current_filter();
                                                pane.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                                                if let Some(name) = pane.pending_cursor_name.take() {
                                                    if let Some(pos) = pane.entries.iter().position(|e| e.name == name) {
                                                        pane.cursor = pos;
                                                        pane.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                                                    }
                                                }
                                                result_obj.ui_changed = true;
                                            } else {
                                                pane.is_loading = false;
                                                pane.pending_cursor_name = None;
                                            }
                                            pane.active_job_id = None; // Job complete
                                        } else {
                                            tracing::warn!("[CompleteJob::ReadDirectory] Stale job result (id={:?}, expected={:?}). Discarding.", job_id, pane.active_job_id);
                                        }
                                        } else {                                        tracing::warn!("[CompleteJob::ReadDirectory] Tab not found (likely closed)! tab_id={}, job_id={:?}. Cancelling job.", requesting_tab_id, job_id);
                                        self.background_jobs.cancel_job(*job_id);
                                    }
                                } else {
                                    // Fallback to old behavior
                                    tracing::warn!("[CompleteJob::ReadDirectory] Using fallback path - requesting_pane is None! location={}", location.display_path());
                                    result_obj.ui_changed = false;
                                    for tab in self.tabs.tabs.iter_mut() {
                                        if tab.left_pane.current_location == *location {
                                            tracing::debug!("[CompleteJob::ReadDirectory::Fallback] Updating left pane via fallback");
                                            tab.left_pane.raw_entries = entries.clone();
                                            tab.left_pane.entries = entries.clone();
                                            tab.left_pane.is_loading = false;
                                            tab.left_pane.apply_sort();
                                            tab.left_pane.apply_current_filter();
                                            tab.left_pane.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                                            result_obj.ui_changed = true;
                                        }
                                        if tab.right_pane.current_location == *location {
                                            tracing::debug!("[CompleteJob::ReadDirectory::Fallback] Updating right pane via fallback");
                                            tab.right_pane.raw_entries = entries.clone();
                                            tab.right_pane.entries = entries.clone();
                                            tab.right_pane.is_loading = false;
                                            tab.right_pane.apply_sort();
                                            tab.right_pane.apply_current_filter();
                                            tab.right_pane.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                                            result_obj.ui_changed = true;
                                        }
                                    }
                                }
                            } else {
                                // Reset loading state on failure/cancellation
                                if let Some((requesting_tab_id, pane_side)) = spec.requesting_pane {
                                    if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.id == requesting_tab_id) {
                                        let pane = match pane_side {
                                            crate::model::ActivePane::Left => &mut tab.left_pane,
                                            crate::model::ActivePane::Right => &mut tab.right_pane,
                                        };
                                        pane.is_loading = false;
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::LoadFileForViewer { .. } => {
                            // Buffer was already delivered via ViewerReady event.
                            // On final Completed just mark loading as done.
                            if let crate::job::OpResult::Success(_) = result {
                                if let Some(ref mut viewer) = self.viewer {
                                    viewer.is_loading = false;
                                }
                                result_obj.ui_changed = true;
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
                            self.unmark_all_panes();
                        }
                        crate::job::JobKind::Rename { from, to } => {
                            // In-memory update: no ReadDirectory needed for a single rename
                            if let crate::job::OpResult::Success(_) = result {
                                let new_name = to.path()
                                    .and_then(|p| p.file_name())
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                if !new_name.is_empty() {
                                    let pane_height = self.ui.layout.pane_height;
                                    let scroll_offset = self.config.ui.scroll_offset;
                                    for tab in self.tabs.tabs.iter_mut() {
                                        for pane in [&mut tab.left_pane, &mut tab.right_pane] {
                                            if let Some(e) = pane.raw_entries.iter_mut().find(|e| &e.location == from) {
                                                e.name = new_name.clone();
                                                e.location = to.clone();
                                            }
                                            pane.apply_sort();
                                            pane.apply_current_filter();
                                            pane.update_scroll(pane_height, scroll_offset);
                                        }
                                    }
                                    result_obj.ui_changed = true;
                                }
                            }
                        }
                        crate::job::JobKind::Delete { targets } => {
                            if let crate::job::OpResult::Success(_) = result {
                                // In-memory removal: remove deleted entries from all panes without
                                // triggering a full ReadDirectory (same approach as Rename).
                                let pane_height = self.ui.layout.pane_height;
                                let scroll_offset = self.config.ui.scroll_offset;
                                let mut any_changed = false;
                                for tab in self.tabs.tabs.iter_mut() {
                                    for pane in [&mut tab.left_pane, &mut tab.right_pane] {
                                        let before = pane.raw_entries.len();
                                        pane.raw_entries.retain(|e| !targets.contains(&e.location));
                                        if pane.raw_entries.len() != before {
                                            pane.apply_current_filter();
                                            pane.apply_sort();
                                            if pane.entries.is_empty() {
                                                pane.cursor = 0;
                                            } else {
                                                pane.cursor = pane.cursor.min(pane.entries.len() - 1);
                                            }
                                            pane.update_scroll(pane_height, scroll_offset);
                                            any_changed = true;
                                        }
                                    }
                                }
                                if any_changed { result_obj.ui_changed = true; }
                            } else {
                                result_obj.panes_to_refresh.push(PaneRefresh { tab_id: self.tabs.active_index, pane: self.ui.active_pane });
                            }
                            self.unmark_all_panes();
                        }
                        crate::job::JobKind::PatternRename { .. } |
                        crate::job::JobKind::Mkdir { .. } => {
                            result_obj.panes_to_refresh.push(PaneRefresh { tab_id: self.tabs.active_index, pane: self.ui.active_pane });
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
                            if let crate::job::OpResult::Success(_) = result {
                                let config_manager = crate::config::ConfigManager::new();
                                let config_path = config_manager.config_path().to_string_lossy().to_string();

                                if command.contains(&config_path) {
                                    let dialog = crate::model::Dialog::confirmation(
                                        "Configuration Editor Closed",
                                        "Reload configuration?"
                                    );
                                    self.dialogs.push(dialog);
                                } else {
                                    // External commands may change files in unknown ways.
                                    // Always refresh the active pane rather than requiring the user
                                    // to declare an explicit refresh scope in config.
                                    result_obj.panes_to_refresh.push(PaneRefresh {
                                        tab_id: self.tabs.active_index,
                                        pane: self.ui.active_pane,
                                    });
                                }
                            }
                        }
                        crate::job::JobKind::CollectJumpCandidates { include_files, .. } => {
                            if let crate::job::OpResult::Success(crate::job::SuccessData::JumpCandidates(new_candidates)) = result {
                                let include_files = *include_files;
                                let job_id_val = *job_id;
                                for dialog in self.dialogs.stack.iter_mut().rev() {
                                    let matched = match &dialog.content {
                                        crate::model::dialog::DialogContent::JumpToFile { loading_job_id, .. } => *loading_job_id == Some(job_id_val),
                                        crate::model::dialog::DialogContent::JumpToPath { loading_job_id, .. } => !include_files && *loading_job_id == Some(job_id_val),
                                        _ => false,
                                    };
                                    if matched {
                                        match &mut dialog.content {
                                            crate::model::dialog::DialogContent::JumpToFile { candidates, suggestions, loading_job_id, query, .. } => {
                                                let mut seen: std::collections::HashSet<String> = candidates.iter().cloned().collect();
                                                for c in new_candidates {
                                                    if seen.insert(c.clone()) {
                                                        candidates.push(c.clone());
                                                    }
                                                }
                                                *loading_job_id = None;
                                                *suggestions = crate::model::dialog::filter_jump_to_file_suggestions(candidates, query);
                                                result_obj.ui_changed = true;
                                            }
                                            crate::model::dialog::DialogContent::JumpToPath { candidates, suggestions, loading_job_id, query, .. } => {
                                                let mut seen: std::collections::HashSet<String> = candidates.iter().cloned().collect();
                                                for c in new_candidates {
                                                    if seen.insert(c.clone()) {
                                                        candidates.push(c.clone());
                                                    }
                                                }
                                                *loading_job_id = None;
                                                *suggestions = crate::model::dialog::filter_jump_to_path_suggestions(candidates, query);
                                                result_obj.ui_changed = true;
                                            }
                                            _ => {}
                                        }
                                        break;
                                    }
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
            Transition::NavigateToHistoryIndex { pane, index } => {
                let location = {
                    let tab = self.current_tab_mut();
                    tab.history.jump_to_index(*pane, *index)
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
                        pane_model.is_loading = false;
                        pane_model.apply_sort();
                        Some(StateUpdateResult::with_ui_change())
                    } else {
                        pane_model.entries.clear();
                        pane_model.is_loading = true;
                        let tab_id = self.current_tab().id;
                        let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location })
                            .with_requesting_pane(tab_id, *pane);
                        Some(StateUpdateResult::with_job(job_spec))
                    }
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            _ => None,
        }
    }

    fn handle_ui_transition(&mut self, transition: &Transition) -> Option<StateUpdateResult> {
        match transition {
            Transition::PaneRefreshed { tab_id, pane } => {
                if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.id == *tab_id) {
                    let pane_model = match pane {
                        crate::model::ActivePane::Left => &mut tab.left_pane,
                        crate::model::ActivePane::Right => &mut tab.right_pane,
                    };
                    pane_model.is_loading = false;
                    pane_model.apply_sort();
                    pane_model.apply_current_filter();
                    pane_model.update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::ChangeUIMode { mode } => {
                self.ui.mode = *mode;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePaneHeight { height } => {
                self.ui.layout.pane_height = *height;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePaneWidth { width } => {
                self.ui.layout.pane_width = *width;
                let content_w = width.saturating_sub(10);
                if let Some(ref mut viewer) = self.viewer {
                    viewer.content_width = content_w;
                }
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
                                let sources: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries.iter()
                                            .filter(|e| pane.marking.is_marked(&e.location))
                                            .map(|e| e.location.clone())
                                            .collect()
                                    } else if let Some(entry) = pane.current_entry() {
                                        vec![entry.location.clone()]
                                    } else {
                                        vec![]
                                    }
                                };

                                if !sources.is_empty() {
                                    let dest = self.opposite_pane().current_location.clone();
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Copy { sources, dest });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Move" {
                                let sources: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries.iter()
                                            .filter(|e| pane.marking.is_marked(&e.location))
                                            .map(|e| e.location.clone())
                                            .collect()
                                    } else if let Some(entry) = pane.current_entry() {
                                        vec![entry.location.clone()]
                                    } else {
                                        vec![]
                                    }
                                };

                                if !sources.is_empty() {
                                    let dest = self.opposite_pane().current_location.clone();
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Move { sources, dest });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Delete" {
                                let targets: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries.iter()
                                            .filter(|e| pane.marking.is_marked(&e.location))
                                            .map(|e| e.location.clone())
                                            .collect()
                                    } else if let Some(entry) = pane.current_entry() {
                                        vec![entry.location.clone()]
                                    } else {
                                        vec![]
                                    }
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
                                    let path = self.active_pane().current_location.display_path();
                                    return Some(update_state(self, Transition::RegisterCurrentFolder { name: input, path }));
                                }
                            } else if title == "File Mask Filter" {
                                let mask = if input.is_empty() { None } else { Some(input) };
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::SetFileMask { pane, mask }));
                            }
                        }
                        crate::model::DialogContent::DeleteConfirm { targets, .. } => {
                            let jobs_targets: Vec<_> = targets.iter().map(|(loc, _)| loc.clone()).collect();
                            if !jobs_targets.is_empty() {
                                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Delete { targets: jobs_targets });
                                self.dialogs.pop();
                                return Some(StateUpdateResult::with_job(job_spec));
                            }
                        }
                        crate::model::DialogContent::SimpleRename { .. } => {
                            let new_name = self.dialogs.input_buffer.clone();
                            if !new_name.is_empty() {
                                if let Some(entry) = self.active_pane().current_entry() {
                                    let from = entry.location.clone();
                                    let to = from.parent()
                                        .unwrap_or_else(|| self.active_pane().current_location.clone())
                                        .join(&new_name);
                                    let job_spec = crate::job::JobSpec::new(crate::job::JobKind::Rename { from, to });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            }
                        }
                        crate::model::DialogContent::FileMask { input, .. } => {
                            let mask = if input.is_empty() { None } else { Some(input.clone()) };
                            let pane = self.ui.active_pane;
                            self.dialogs.pop();
                            return Some(update_state(self, Transition::SetFileMask { pane, mask }));
                        }
                        crate::model::DialogContent::DriveSelection { drives, selected_index, filter } => {
                            let lower = filter.to_lowercase();
                            let filtered: Vec<&crate::model::dialog::DriveInfo> = if filter.is_empty() {
                                drives.iter().collect()
                            } else {
                                drives.iter().filter(|d| {
                                    d.display_label().to_lowercase().contains(&lower)
                                        || d.path.to_lowercase().contains(&lower)
                                }).collect()
                            };
                            if let Some(drive) = filtered.get(*selected_index) {
                                let location = crate::model::Location::Local(std::path::PathBuf::from(&drive.path));
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::ChangeLocation { pane, location }));
                            }
                        }
                        crate::model::DialogContent::RegisteredFolderSelector { selected_index, .. } => {
                            let folder_index = *selected_index;
                            if self.active_pane().marking.count() > 0 {
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
            Transition::ShowCustomFunctionsDialog => {
                let functions = self.custom_functions.clone();
                if functions.is_empty() {
                    tracing::info!("No custom functions loaded (custom_functions.json missing or empty)");
                    None
                } else {
                    let dialog = crate::model::Dialog::custom_function_selector(functions);
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                }
            }
            Transition::ExecuteAssociation { command, working_dir, shell } => {
                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::ExecuteCustomFunction {
                    command: command.clone(),
                    working_dir: working_dir.clone(),
                    pipe_to_action: None,
                    shell: shell.clone(),
                });
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowDriveChangeDialog => {
                let mut entries = Vec::new();

                // 1. Home directory
                if let Some(home) = dirs::home_dir() {
                    entries.push(crate::model::dialog::DriveInfo {
                        path: home.to_string_lossy().into_owned(),
                        label: "~ User Directory".to_string(),
                        drive_type: crate::model::dialog::DriveType::Local,
                        total_space: None,
                        free_space: None,
                    });
                }

                // 2. Network shares discovered from both panes' history
                let (left_stack, right_stack, cur_left, cur_right) = {
                    let tab = self.current_tab();
                    let (ls, _) = tab.history.stack_and_pos(crate::model::ui::ActivePane::Left);
                    let (rs, _) = tab.history.stack_and_pos(crate::model::ui::ActivePane::Right);
                    (ls.to_vec(), rs.to_vec(),
                     tab.left_pane.current_location.clone(),
                     tab.right_pane.current_location.clone())
                };
                let mut nw_roots: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                for loc in left_stack.iter().chain(right_stack.iter())
                                     .chain(std::iter::once(&cur_left))
                                     .chain(std::iter::once(&cur_right))
                {
                    if let Some(root) = get_share_root_from_location(loc) {
                        nw_roots.insert(root);
                    }
                }
                for root in &nw_roots {
                    entries.push(crate::model::dialog::DriveInfo {
                        path: root.clone(),
                        label: root.clone(),
                        drive_type: crate::model::dialog::DriveType::Network,
                        total_space: None,
                        free_space: None,
                    });
                }

                // 3. System drives
                entries.extend(crate::volume_info::get_all_drives());

                let dialog = crate::model::Dialog::drive_selection(entries, self.ui.active_pane);
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
            Transition::ShowJumpToPathDialog => {
                let root = self.active_pane().current_location.display_path();
                let fast_candidates = collect_jump_path_fast_candidates(self);
                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::CollectJumpCandidates {
                    root: root.clone(),
                    include_files: false,
                    max_results: self.config.jump_nav.jump_path_max_results,
                    max_depth: self.config.jump_nav.jump_path_max_depth,
                });
                let job_id = job_spec.id;
                let mut dialog = crate::model::Dialog::jump_to_path(root, fast_candidates);
                if let crate::model::dialog::DialogContent::JumpToPath { loading_job_id, .. } = &mut dialog.content {
                    *loading_job_id = Some(job_id);
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowJumpToFileDialog => {
                let root = self.active_pane().current_location.display_path();
                let fast_candidates = collect_jump_file_fast_candidates(self);
                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::CollectJumpCandidates {
                    root: root.clone(),
                    include_files: true,
                    max_results: self.config.jump_nav.jump_file_max_results,
                    max_depth: self.config.jump_nav.jump_file_max_depth,
                });
                let job_id = job_spec.id;
                let mut dialog = crate::model::Dialog::jump_to_file(root, fast_candidates);
                if let crate::model::dialog::DialogContent::JumpToFile { loading_job_id, .. } = &mut dialog.content {
                    *loading_job_id = Some(job_id);
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::RegisterCurrentFolder { name, path } => {
                let folder = crate::model::RegisteredFolder::new(name.clone(), path.clone());
                self.registered_folders.add(folder);

                let save_path = crate::model::RegisteredFolderManager::default_path();
                let _ = self.registered_folders.save_to_file(&save_path);

                let log = format!("[Folder] Registered \"{}\" → {}", name, path);
                let mut result = StateUpdateResult::with_ui_change();
                result.task_panel_logs.push(log);
                Some(result)
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

                    let sources: Vec<_> = {
                        let pane = self.active_pane();
                        if pane.marking.count() > 0 {
                            pane.entries.iter()
                                .filter(|e| pane.marking.is_marked(&e.location))
                                .map(|e| e.location.clone())
                                .collect()
                        } else {
                            vec![]
                        }
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
                let config_manager = crate::config::ConfigManager::new();
                let config_path = config_manager.config_path().to_string_lossy().to_string();
                let job_spec = JobSpec::new(Self::editor_job(
                    &self.config, config_path,
                ));
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenWithEditor { path } => {
                let job_spec = JobSpec::new(Self::editor_job(&self.config, path.clone()));
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
            Transition::ChangeSortOrder { pane, order } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.sort_order = *order;
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
                let pane_height = self.ui.layout.pane_height;
                let scroll_offset = self.config.ui.scroll_offset;
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.file_mask = mask.clone();
                pane_model.apply_current_filter();
                pane_model.update_scroll(pane_height, scroll_offset);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ToggleHidden => {
                self.ui.show_hidden = !self.ui.show_hidden;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::Refresh { pane } |
            Transition::RefreshAndClearMarks { pane } |
            Transition::RefreshNoClearMarks { pane } => {
                if let Transition::RefreshAndClearMarks { .. } = transition {
                    let cleared_pane = *pane;
                    let tab = self.current_tab_mut();
                    match cleared_pane {
                        crate::model::ActivePane::Left => tab.left_pane.marking.unmark_all(),
                        crate::model::ActivePane::Right => tab.right_pane.marking.unmark_all(),
                    }
                }
                let tab = self.current_tab_mut();
                let tab_id = tab.id;
                let (location, pane_model) = match pane {
                    crate::model::ActivePane::Left => (tab.left_pane.current_location.clone(), &mut tab.left_pane),
                    crate::model::ActivePane::Right => (tab.right_pane.current_location.clone(), &mut tab.right_pane),
                };
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location })
                    .with_requesting_pane(tab_id, *pane);
                pane_model.is_loading = true;
                pane_model.active_job_id = Some(job_spec.id);
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
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = crate::model::ViewerMode::Text;
                self.viewer = Some(viewer);
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(), index_lines: true,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenHexViewer { location } => {
                self.ui.mode = crate::model::UIMode::Viewer;
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = crate::model::ViewerMode::Hex;
                self.viewer = Some(viewer);
                // Hex mode doesn't need a line index — mmap only.
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(), index_lines: false,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ReloadViewer { location, mode } => {
                // Cancel the previous loading job.
                if let Some(job_id) = self.viewer_job_id.take() {
                    self.jobs.request_cancel(job_id);
                }
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = *mode;
                self.viewer = Some(viewer);
                self.viewer_search_input.clear();
                let index_lines = *mode == crate::model::ViewerMode::Text;
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(), index_lines,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenSideBySideViewer { location, mode } => {
                // File pane keeps focus; viewer appears alongside it.
                self.ui.mode = crate::model::UIMode::Normal;
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::SideBySide;
                self.ui.layout.viewer_preferred_layout = crate::model::ViewerLayout::SideBySide;
                // Pin the viewer to the opposite side of the current active pane.
                // This stays fixed for the duration of the SideBySide session.
                self.ui.layout.viewer_anchor_pane = self.ui.active_pane;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = *mode;
                self.viewer = Some(viewer);
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(),
                    index_lines: *mode == crate::model::ViewerMode::Text,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::CloseViewer => {
                if let Some(job_id) = self.viewer_job_id.take() {
                    self.jobs.request_cancel(job_id);
                }
                self.ui.mode = crate::model::UIMode::Normal;
                self.viewer = None;
                self.viewer_search_input.clear();
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerSwitchLayout { layout } => {
                match layout {
                    crate::model::ViewerLayout::FullScreen => {
                        // Viewer takes full focus; remember that user came from SideBySide.
                        self.ui.mode = crate::model::UIMode::Viewer;
                        self.ui.layout.viewer_preferred_layout = crate::model::ViewerLayout::SideBySide;
                    }
                    crate::model::ViewerLayout::SideBySide => {
                        // File pane gets focus.
                        self.ui.mode = crate::model::UIMode::Normal;
                    }
                }
                self.ui.layout.viewer_layout = *layout;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerReady { buffer, encoding } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.buffer = Some(buffer.clone());
                    // Only apply detected encoding on first arrival (encoding may have
                    // been manually changed by the user before the job completes).
                    if viewer.encoding == crate::model::viewer::TextEncoding::Utf8 {
                        viewer.encoding = *encoding;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerLoadComplete { contents } => {
                // Legacy path: used by tests and the ViewerLoadComplete transition.
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
            Transition::ViewerToggleMode => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.mode = match viewer.mode {
                        crate::model::ViewerMode::Text => crate::model::ViewerMode::Hex,
                        crate::model::ViewerMode::Hex => crate::model::ViewerMode::Text,
                    };
                    viewer.clear_search();
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
            Transition::ViewerJumpToBottom { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.jump_to_bottom(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerJumpToLine { line_idx, viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    let max = if viewer.mode == crate::model::ViewerMode::Hex {
                        viewer.hex_line_count()
                    } else {
                        viewer.line_count()
                    };
                    viewer.line_offset = (*line_idx).min(max.saturating_sub(*viewport_height));
                    viewer.column_offset = 0;
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
                    let migemo_pat = self.search.get_migemo_regex(query, viewer.case_sensitive);
                    viewer.start_search(query.to_string(), migemo_pat.as_deref());
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFindNext => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.find_next_in_dir();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFindPrev => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.find_prev_in_dir();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerClearSearch => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.clear_search();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerToggleCaseSensitive => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.case_sensitive = !viewer.case_sensitive;
                    // Re-run any active search under the new sensitivity.
                    if let Some(query) = viewer.search_query.clone() {
                        let migemo_pat = self.search.get_migemo_regex(&query, viewer.case_sensitive);
                        viewer.start_search(query, migemo_pat.as_deref());
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollLeft { cols } => {
                if let Some(ref mut viewer) = self.viewer { viewer.scroll_left(*cols); }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollRight { cols } => {
                if let Some(ref mut viewer) = self.viewer { viewer.scroll_right(*cols); }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFastScrollUp { lines } => {
                if let Some(ref mut viewer) = self.viewer { viewer.fast_scroll_up(*lines); }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFastScrollDown { lines, viewport_height } => {
                if let Some(ref mut viewer) = self.viewer { viewer.fast_scroll_down(*lines, *viewport_height); }
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
                let filenames: Vec<String> = {
                    let pane = self.active_pane();
                    if pane.entries.is_empty() {
                        return Some(StateUpdateResult::none());
                    }
                    if pane.marking.count() > 0 {
                        pane.entries.iter()
                            .filter(|e| pane.marking.is_marked(&e.location))
                            .map(|e| e.name.clone())
                            .collect()
                    } else {
                        pane.entries.iter().map(|e| e.name.clone()).collect()
                    }
                };
                // Pre-populate preview so the dialog opens at full size with all files visible
                let initial_preview = crate::pattern_rename::generate_preview(
                    &filenames, "", "", true, false,
                );
                let mut dialog = crate::model::Dialog::pattern_rename();
                if let crate::model::DialogContent::PatternRename { preview, .. } = &mut dialog.content {
                    *preview = initial_preview;
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePatternRenameFields { find, replace, use_regex, case_sensitive } => {
                let filenames: Vec<_> = {
                    let pane = self.active_pane();
                    if pane.marking.count() > 0 {
                        pane.entries.iter()
                            .filter(|e| pane.marking.is_marked(&e.location))
                            .map(|e| e.name.clone())
                            .collect()
                    } else {
                        pane.entries.iter().map(|e| e.name.clone()).collect()
                    }
                };

                let preview = crate::pattern_rename::generate_preview(
                    &filenames, find, replace, *use_regex, *case_sensitive,
                );
                if let Some(dialog) = self.dialogs.current_mut() {
                    if let crate::model::DialogContent::PatternRename {
                        find: f, replace: r, use_regex: ur, case_sensitive: cs, preview: pr, error_message: em, ..
                    } = &mut dialog.content {
                        *f = find.clone();
                        *r = replace.clone();
                        *ur = *use_regex;
                        *cs = *case_sensitive;
                        *pr = preview;
                        *em = None;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ExecutePatternRename { find, replace, use_regex, case_sensitive, targets } => {
                let job_spec = JobSpec::new(crate::job::JobKind::PatternRename {
                    targets: targets.clone(),
                    find: find.clone(),
                    replace: replace.clone(),
                    use_regex: *use_regex,
                    case_sensitive: *case_sensitive,
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
    NavigateToHistoryIndex { pane: crate::model::ActivePane, index: usize },
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
    
    // UI Events
    PaneRefreshed { tab_id: usize, pane: crate::model::ActivePane },
    
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
    ChangeSortOrder { pane: crate::model::ActivePane, order: crate::model::SortOrder },
    ChangeDisplayMode { pane: crate::model::ActivePane, mode: crate::model::DisplayMode },
    SetFileMask { pane: crate::model::ActivePane, mask: Option<String> },
    ToggleHidden,
    Refresh { pane: crate::model::ActivePane },
    RefreshAndClearMarks { pane: crate::model::ActivePane },
    RefreshNoClearMarks { pane: crate::model::ActivePane },
    
    // UI state
    ChangeUIMode { mode: crate::model::UIMode },
    UpdatePaneHeight { height: usize },
    UpdatePaneWidth { width: usize },
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
    ShowCustomFunctionsDialog,
    /// Execute a command from a file-type extension association (Phase 6.2)
    ExecuteAssociation { command: String, working_dir: crate::model::Location, shell: Option<String> },
    ShowDriveChangeDialog,
    ShowFileInfo,
    ShowVersion,
    SaveLog,
    RotateHelpLanguage,
    LaunchConfigurationProgram,
    OpenWithEditor { path: String },
    ShowRegisteredFolderDialog,
    RegisterCurrentFolder { name: String, path: String },
    ShowJumpToPathDialog,
    ShowJumpToFileDialog,
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
    OpenSideBySideViewer { location: crate::model::Location, mode: crate::model::ViewerMode },
    /// Reload viewer content with an explicit mode. Used for auto-preview:
    /// cursor moves in SideBySide file pane update the viewer live.
    ReloadViewer { location: crate::model::Location, mode: crate::model::ViewerMode },
    CloseViewer,
    ViewerSwitchLayout { layout: crate::model::ViewerLayout },
    ViewerReady { buffer: crate::model::viewer::ViewerBuffer, encoding: crate::model::viewer::TextEncoding },
    ViewerLoadComplete { contents: Vec<u8> },
    ViewerCycleEncoding,
    ViewerToggleMode,
    ViewerScrollDown { viewport_height: usize },
    ViewerScrollUp,
    ViewerPageDown { viewport_height: usize },
    ViewerPageUp { viewport_height: usize },
    ViewerJumpToTop,
    ViewerJumpToBottom { viewport_height: usize },
    ViewerJumpToLine { line_idx: usize, viewport_height: usize },
    ViewerMoveToLineStart,
    ViewerMoveToLineEnd { viewport_width: usize },
    ViewerStartSearch { query: String },
    ViewerFindNext,
    ViewerFindPrev,
    ViewerClearSearch,
    ViewerToggleCaseSensitive,
    ViewerScrollLeft { cols: usize },
    ViewerScrollRight { cols: usize },
    ViewerFastScrollUp { lines: usize },
    ViewerFastScrollDown { lines: usize, viewport_height: usize },
    
    // Pattern rename operations
    ShowPatternRenameDialog,
    UpdatePatternRenameFields { find: String, replace: String, use_regex: bool, case_sensitive: bool },
    ExecutePatternRename { find: String, replace: String, use_regex: bool, case_sensitive: bool, targets: Vec<crate::model::Location> },
    
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

            // Remember settings that require restart to take effect
            let old_workers = state.config.worker_pool_size;
            let old_migemo  = state.config.search.dict_path.clone();

            // Reload config.json
            let config_path = config_manager.config_path().to_path_buf();
            let config_result = match config_manager.load_config() {
                Ok(new_config) => {
                    state.config = new_config;
                    crate::config::ConfigLoadResult::ok(config_path)
                }
                Err(e) => {
                    let is_not_found = matches!(&e, crate::config::ConfigError::IoError(io) if io.kind() == std::io::ErrorKind::NotFound);
                    if is_not_found {
                        crate::config::ConfigLoadResult::skipped(config_path, "file not found")
                    } else {
                        crate::config::ConfigLoadResult::error(config_path, format!("{:?}", e))
                    }
                }
            };

            // Reload other config files
            let (ext_assocs, ext_result) = config_manager.load_extension_associations_with_result();
            state.extension_associations = ext_assocs;

            let custom_fn_path = config_manager.custom_functions_path().to_path_buf();
            let (custom_fns, custom_fn_result) = match crate::model::dialog::load_custom_functions(&custom_fn_path) {
                Ok(fns) => {
                    let result = if custom_fn_path.exists() {
                        crate::config::ConfigLoadResult::ok(custom_fn_path)
                    } else {
                        crate::config::ConfigLoadResult::skipped(custom_fn_path, "file not found")
                    };
                    (fns, result)
                }
                Err(e) => (Vec::new(), crate::config::ConfigLoadResult::error(custom_fn_path, e.to_string())),
            };
            state.custom_functions = custom_fns;

            let context_menu_result = crate::config::ConfigManager::validate_json_file(
                config_manager.context_menu_path()
            );

            // Preserve config.json and keybindings.json results at front (already populated at startup)
            let prev_results: Vec<_> = state.config_load_results.drain(..2).collect();
            state.config_load_results = prev_results;
            state.config_load_results[0] = config_result;
            // keybindings result (index 1) doesn't change on reload — keybindings.json is not reloaded at runtime
            state.config_load_results.extend([ext_result, custom_fn_result, context_menu_result]);

            // Build feedback messages
            use crate::config::ConfigLoadStatus;
            let mut messages: Vec<String> = Vec::new();
            messages.push("Configuration reloaded.".to_string());

            // Restart-required notice
            let mut restart_items: Vec<&str> = Vec::new();
            if state.config.worker_pool_size != old_workers {
                restart_items.push("worker thread count");
            }
            if state.config.search.dict_path != old_migemo {
                restart_items.push("Migemo dictionary path");
            }
            if !restart_items.is_empty() {
                messages.push(format!("  Restart required for: {}.", restart_items.join(", ")));
            }

            // [NG] errors
            for r in &state.config_load_results {
                if let ConfigLoadStatus::Error(detail) = &r.status {
                    messages.push(format!("  [NG] {}: {}", r.path.to_string_lossy(), detail));
                }
            }

            let mut result = StateUpdateResult::with_ui_change();
            result.task_panel_logs = messages;
            result
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

/// Collect directory candidates for the Jump to Path dialog.
///
/// Collect fast (instant) candidates for Jump to Directory dialog.
/// Returns current pane subdirs + registered folders + navigation history.
/// The disk walk is done asynchronously by a CollectJumpCandidates job.
fn collect_jump_path_fast_candidates(state: &AppState) -> Vec<String> {
    use std::collections::HashSet;

    let mut candidates: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1. Current pane subdirs
    let pane = state.active_pane();
    for entry in &pane.entries {
        if entry.is_dir && entry.name != ".." {
            let p = entry.location.display_path();
            if seen.insert(p.clone()) {
                candidates.push(p);
            }
        }
    }

    // 2. Registered folders (expanded)
    for folder in &state.registered_folders.folders {
        let p = state.registered_folders.expand_path(folder)
            .to_string_lossy()
            .into_owned();
        if !p.is_empty() && seen.insert(p.clone()) {
            candidates.push(p);
        }
    }

    // 3. Navigation history (both panes, current tab)
    let tab = state.current_tab();
    let (left_stack, _) = tab.history.stack_and_pos(crate::model::ui::ActivePane::Left);
    let (right_stack, _) = tab.history.stack_and_pos(crate::model::ui::ActivePane::Right);
    for loc in left_stack.iter().chain(right_stack.iter()) {
        let p = loc.display_path();
        if !p.is_empty() && seen.insert(p.clone()) {
            candidates.push(p);
        }
    }

    candidates
}

/// Collect fast (instant) candidates for Jump to File dialog.
/// Returns current pane items (files AND dirs). The disk walk is async.
fn collect_jump_file_fast_candidates(state: &AppState) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let pane = state.active_pane();
    for entry in &pane.entries {
        if entry.name == ".." { continue; }
        let p = entry.location.display_path();
        if seen.insert(p.clone()) {
            candidates.push(p);
        }
    }

    candidates
}

/// Extract `\\server\share` root from a `Location::Local` network path.
/// Returns `None` for non-network paths.
fn get_share_root_from_location(loc: &crate::model::Location) -> Option<String> {
    if let crate::model::Location::Local(path) = loc {
        let s = path.to_string_lossy();
        let normalized = s.replace('/', "\\");
        if normalized.starts_with("\\\\") {
            let clean = normalized.trim_start_matches('\\');
            let parts: Vec<&str> = clean.split('\\').filter(|p| !p.is_empty()).collect();
            return match parts.len() {
                0 => None,
                1 => Some(format!("\\\\{}", parts[0])),
                _ => Some(format!("\\\\{}\\{}", parts[0], parts[1])),
            };
        }
    }
    None
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
        
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&location));
        
        let result = update_state(&mut state, Transition::ToggleMark { location: location.clone() });
        assert!(result.ui_changed);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&location));
        
        let result = update_state(&mut state, Transition::ToggleMark { location: location.clone() });
        assert!(result.ui_changed);
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&location));
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
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
    }

    #[test]
    fn test_unmark_all_transition() {
        use crate::model::Location;
        use std::path::PathBuf;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        state.current_tab_mut().left_pane.marking.toggle(Location::Local(PathBuf::from("/test/file1.txt")));
        state.current_tab_mut().left_pane.marking.toggle(Location::Local(PathBuf::from("/test/file2.txt")));
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        
        let result = update_state(&mut state, Transition::UnmarkAll);
        assert!(result.ui_changed);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 0);
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
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/test.txt"))));
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
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
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
        
        state.current_tab_mut().left_pane.marking.toggle(Location::Local(PathBuf::from("/test/f1.txt")));
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        
        let result = update_state(&mut state, Transition::InvertMarks);
        assert!(result.ui_changed);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/f2.txt"))));
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
