//! Application state management
//!
//! This module defines the central AppState structure and the Transition enum
//! for explicit state changes following the AppState pattern.

#![allow(clippy::unwrap_used)] // TODO(M6): ratchet — see plan/quality_overhaul.md

mod handlers;

use crate::job::{BackgroundJobManager, JobId, JobManager, JobSpec};
use crate::log_manager::LogManager;
use crate::model::{
    DialogStack, DirectoryCache, NavigationStateCache, SearchModel, TabManager, UIState,
    ViewerState,
};
use std::time::Duration;

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
    /// Job ID of the active background viewer search (for cancel on new search)
    pub viewer_search_job_id: Option<crate::job::JobId>,
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
    /// Staging: logs produced by dialog-confirmation built-in actions (drained by app.rs each frame)
    pub pending_confirmation_logs: Vec<String>,
    /// Staging: set true when a dialog confirmation triggered ReloadConfig (app.rs reloads keybindings)
    pub confirmation_needs_keybinding_reload: bool,
    /// Pending custom function awaiting $I user input; set when the Input dialog is pushed,
    /// consumed and cleared when that Input dialog confirms.
    pub pending_custom_function_input: Option<crate::model::dialog::CustomFunction>,
    /// Set by process_dialog_confirmation when it pushes a new dialog; tells app.rs not to
    /// pop the current dialog (it was replaced by the newly pushed one).
    pub suppress_next_dialog_pop: bool,
    /// Leap Navigation state; Some while UIMode::Leap is active.
    pub leap: Option<crate::model::LeapState>,
}

impl AppState {
    fn resolve_editor(config: &AppConfig) -> String {
        config.editor_command.clone().unwrap_or_else(|| {
            #[cfg(target_os = "windows")]
            {
                "notepad".to_string()
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::env::var("EDITOR")
                    .or_else(|_| std::env::var("VISUAL"))
                    .unwrap_or_else(|_| "vi".to_string())
            }
        })
    }

    /// Build a job that opens `file_path` in the configured editor.
    /// If `TerminalEditor` is set, returns `SuspendAndRun` (app suspends, editor owns terminal).
    /// Otherwise returns `SpawnProcess` via `Editor` (GUI editor).
    /// `wait_for_exit`: when true, the job doesn't complete until the editor closes
    /// (needed so callers can react to editor-closed, e.g. the config reload prompt).
    fn editor_job(
        config: &AppConfig,
        file_path: String,
        wait_for_exit: bool,
    ) -> crate::job::JobKind {
        if let Some(ref cmd) = config.terminal_editor {
            // Terminal editor: suspend rwf, run synchronously, then resume.
            // On Windows use cmd /c so .bat/.cmd wrappers (e.g. vim.bat) work.
            #[cfg(target_os = "windows")]
            {
                let mut args = vec!["/c".to_string()];
                args.extend(cmd.split_whitespace().map(str::to_string));
                args.push(file_path);
                return crate::job::JobKind::SuspendAndRun {
                    program: "cmd".to_string(),
                    args,
                };
            }
            #[cfg(not(target_os = "windows"))]
            {
                let mut parts = cmd.split_whitespace();
                let program = parts.next().unwrap_or("vi").to_string();
                let mut args: Vec<String> = parts.map(str::to_string).collect();
                args.push(file_path);
                return crate::job::JobKind::SuspendAndRun { program, args };
            }
        }
        // GUI editor: launch via cmd (Windows) / directly (Unix), no shell string parsing.
        let cmd = Self::resolve_editor(config);
        #[cfg(target_os = "windows")]
        {
            let mut args = vec!["/c".to_string()];
            args.extend(cmd.split_whitespace().map(str::to_string));
            args.push(file_path);
            crate::job::JobKind::SpawnProcess {
                program: "cmd".to_string(),
                args,
                wait: wait_for_exit,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut parts = cmd.split_whitespace();
            let program = parts.next().unwrap_or("vi").to_string();
            let mut args: Vec<String> = parts.map(str::to_string).collect();
            args.push(file_path);
            crate::job::JobKind::SpawnProcess {
                program,
                args,
                wait: wait_for_exit,
            }
        }
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
        let log_path =
            if config.log_save_path.starts_with('/') || config.log_save_path.contains(':') {
                // Absolute path — collect components to normalise separators
                std::path::Path::new(&config.log_save_path)
                    .components()
                    .collect::<std::path::PathBuf>()
            } else {
                // Relative: join each component individually so '/' in the value doesn't leak through
                let rel: std::path::PathBuf = std::path::Path::new(&config.log_save_path)
                    .components()
                    .collect();
                crate::logging::default_log_dir()
                    .parent()
                    .unwrap()
                    .join(rel)
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
        let (extension_associations, ext_result) =
            config_manager.load_extension_associations_with_result();

        let custom_fn_path = config_manager.custom_functions_path().to_path_buf();
        let custom_fn_dir = custom_fn_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let (custom_functions, custom_fn_result) =
            match crate::model::dialog::load_custom_functions(&custom_fn_path) {
                Ok(fns) if !fns.is_empty() || custom_fn_path.exists() => {
                    let result = if custom_fn_path.exists() {
                        crate::config::ConfigLoadResult::ok(custom_fn_path)
                    } else {
                        crate::config::ConfigLoadResult::skipped(custom_fn_path, "file not found")
                    };
                    (fns, result)
                }
                Ok(fns) => (
                    fns,
                    crate::config::ConfigLoadResult::skipped(custom_fn_path, "file not found"),
                ),
                Err(e) => (
                    Vec::new(),
                    crate::config::ConfigLoadResult::error(custom_fn_path, e.to_string()),
                ),
            };

        let context_menu_result =
            crate::config::ConfigManager::validate_json_file(config_manager.context_menu_path());

        // Validate any menu_*.json files in the same directory as custom_functions.json
        let mut menu_file_results: Vec<crate::config::ConfigLoadResult> = Vec::new();
        if custom_fn_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(custom_fn_dir) {
                let mut menu_paths: Vec<std::path::PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("menu_") && n.ends_with(".json"))
                            .unwrap_or(false)
                    })
                    .collect();
                menu_paths.sort();
                for path in menu_paths {
                    menu_file_results.push(crate::config::ConfigManager::validate_json_file(&path));
                }
            }
        }

        let mut config_load_results = vec![ext_result, custom_fn_result, context_menu_result];
        config_load_results.extend(menu_file_results);

        Self {
            tabs: TabManager::new(),
            jobs: JobManager::new(config.worker_pool_size),
            background_jobs: BackgroundJobManager::new(
                config.job_manager.max_simultaneous_jobs,
                Duration::from_secs(config.job_manager.job_retention_period_secs),
            ),
            search,
            ui: UIState::new(),
            dialogs: DialogStack::new(),
            registered_folders,
            cache: DirectoryCache::new(Duration::from_secs(30)),
            navigation_cache: NavigationStateCache::new(),
            viewer: None,
            viewer_job_id: None,
            viewer_search_job_id: None,
            viewer_search_input: String::new(),
            viewer_command_input: String::new(),
            log_manager,
            config,
            last_tab_created: None,
            extension_associations,
            custom_functions,
            config_load_results,
            pending_confirmation_logs: Vec::new(),
            confirmation_needs_keybinding_reload: false,
            pending_custom_function_input: None,
            suppress_next_dialog_pop: false,
            leap: None,
        }
    }

    /// Move the current viewer state into the active tab's `tab_viewer` slot
    /// and reset AppState to "no viewer". Called before switching away from a tab.
    fn save_viewer_to_current_tab(&mut self) {
        let idx = self.tabs.active_index;
        let tv = &mut self.tabs.tabs[idx].tab_viewer;
        tv.viewer = self.viewer.take();
        tv.viewer_job_id = self.viewer_job_id.take();
        tv.viewer_search_job_id = self.viewer_search_job_id.take();
        tv.viewer_layout = self.ui.layout.viewer_layout;
        tv.viewer_preferred_layout = self.ui.layout.viewer_preferred_layout;
        tv.viewer_anchor_pane = self.ui.layout.viewer_anchor_pane;
        tv.viewer_was_focused = matches!(
            self.ui.mode,
            crate::model::UIMode::Viewer
                | crate::model::UIMode::ViewerSearch
                | crate::model::UIMode::ViewerCommand
        );
        tv.viewer_search_input = std::mem::take(&mut self.viewer_search_input);
        tv.viewer_command_input = std::mem::take(&mut self.viewer_command_input);

        // Reset global viewer fields to default "no viewer" state.
        self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
        self.ui.layout.viewer_preferred_layout = crate::model::ViewerLayout::FullScreen;
        if matches!(
            self.ui.mode,
            crate::model::UIMode::Viewer
                | crate::model::UIMode::ViewerSearch
                | crate::model::UIMode::ViewerCommand
        ) {
            self.ui.mode = crate::model::UIMode::Normal;
        }
    }

    /// Restore viewer state from the newly active tab's `tab_viewer` slot into AppState.
    /// Called after switching to a new tab.
    fn restore_viewer_from_tab(&mut self) {
        let idx = self.tabs.active_index;
        let tv = &mut self.tabs.tabs[idx].tab_viewer;
        self.viewer = tv.viewer.take();
        self.viewer_job_id = tv.viewer_job_id.take();
        self.viewer_search_job_id = tv.viewer_search_job_id.take();
        self.ui.layout.viewer_layout = tv.viewer_layout;
        self.ui.layout.viewer_preferred_layout = tv.viewer_preferred_layout;
        self.ui.layout.viewer_anchor_pane = tv.viewer_anchor_pane;
        self.viewer_search_input = std::mem::take(&mut tv.viewer_search_input);
        self.viewer_command_input = std::mem::take(&mut tv.viewer_command_input);
        let was_focused = tv.viewer_was_focused;

        // Reset the slot to default so it's clean for next time.
        *tv = crate::model::TabViewerState::default();

        if was_focused && self.viewer.is_some() {
            self.ui.mode = crate::model::UIMode::Viewer;
        }
    }

    /// Cancel the previous search, start a background ViewerSearch job, and return the result.
    fn start_viewer_search_background(&mut self, query: &str) -> StateUpdateResult {
        use crate::job::{JobKind, JobSpec};
        use crate::model::viewer::hex_query_has_pattern;

        let viewer = match self.viewer.as_mut() {
            Some(v) => v,
            None => return StateUpdateResult::with_ui_change(),
        };

        viewer.search_query = Some(query.to_string());
        viewer.search_matches.clear();
        viewer.search_match_index = None;
        viewer.address_query = None;
        viewer.is_searching = false;

        if query.is_empty() {
            return StateUpdateResult::with_ui_change();
        }

        let is_hex = viewer.mode == crate::model::ViewerMode::Hex;
        let location = viewer.location.clone();
        let encoding = viewer.encoding;
        let case_sensitive = viewer.case_sensitive;
        let threshold = (self.config.viewer_large_file_threshold_mb as usize) * 1024 * 1024;

        if is_hex {
            // Apply address jump immediately (no I/O).
            if let Some(ref buf) = self.viewer.as_ref().unwrap().buffer {
                let file_size = buf.bytes.len();
                self.viewer
                    .as_mut()
                    .unwrap()
                    .hex_apply_address_jump(query, file_size);
            }
            // If there's no byte pattern to scan, we're done.
            if !hex_query_has_pattern(query) {
                return StateUpdateResult::with_ui_change();
            }
        }

        let migemo_pat = if !is_hex {
            self.search.get_migemo_regex(query, case_sensitive)
        } else {
            None
        };

        self.viewer.as_mut().unwrap().is_searching = true;
        let job = JobSpec::new(JobKind::ViewerSearch {
            location,
            migemo_pattern: migemo_pat,
            query: query.to_string(),
            is_hex_mode: is_hex,
            encoding,
            case_sensitive,
            large_file_threshold: threshold,
        });
        self.viewer_search_job_id = Some(job.id);
        let mut result = StateUpdateResult::with_ui_change();
        result.jobs_to_start.push(job);
        result
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
                    pane_model
                        .update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
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
                let total_items =
                    self.jobs.queue.len() + self.jobs.active.len() + self.jobs.completed.len();

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
                        crate::model::DialogContent::Confirmation(_) => {
                            let title = dialog.title.as_str();
                            if title == "Copy" {
                                let sources: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries
                                            .iter()
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
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Copy {
                                            sources,
                                            dest,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Move" {
                                let sources: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries
                                            .iter()
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
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Move {
                                            sources,
                                            dest,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Delete" {
                                let targets: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries
                                            .iter()
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
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Delete {
                                            targets,
                                        });
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
                                        if let Some(index) = self
                                            .active_pane()
                                            .entries
                                            .iter()
                                            .position(|e| e.location == first_result.location)
                                        {
                                            self.dialogs.pop();
                                            return Some(update_state(
                                                self,
                                                Transition::CursorJump {
                                                    pane: self.ui.active_pane,
                                                    position: index,
                                                },
                                            ));
                                        }
                                    }
                                }
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::ChangeUIMode {
                                        mode: crate::model::UIMode::Normal,
                                    },
                                ));
                            } else if title == "Create Directory" {
                                let current_location = self.active_pane().current_location.clone();
                                let new_dir_location = current_location.join(&input);
                                let job_spec =
                                    crate::job::JobSpec::new(crate::job::JobKind::Mkdir {
                                        location: new_dir_location,
                                    });
                                self.dialogs.pop();
                                return Some(StateUpdateResult::with_job(job_spec));
                            } else if title == "Wildcard Marking" {
                                if !input.is_empty() {
                                    self.dialogs.pop();
                                    return Some(update_state(
                                        self,
                                        Transition::MarkPattern { pattern: input },
                                    ));
                                }
                            } else if title == "Register Folder" {
                                if !input.is_empty() {
                                    self.dialogs.pop();
                                    let path = self.active_pane().current_location.display_path();
                                    return Some(update_state(
                                        self,
                                        Transition::RegisterCurrentFolder { name: input, path },
                                    ));
                                }
                            } else if title == "File Mask Filter" {
                                let mask = if input.is_empty() { None } else { Some(input) };
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::SetFileMask { pane, mask },
                                ));
                            }
                        }
                        crate::model::DialogContent::DeleteConfirm(
                            crate::model::dialog::DeleteConfirmDialog { targets, .. },
                        ) => {
                            let jobs_targets: Vec<_> =
                                targets.iter().map(|(loc, _)| loc.clone()).collect();
                            if !jobs_targets.is_empty() {
                                let job_spec =
                                    crate::job::JobSpec::new(crate::job::JobKind::Delete {
                                        targets: jobs_targets,
                                    });
                                self.dialogs.pop();
                                return Some(StateUpdateResult::with_job(job_spec));
                            }
                        }
                        crate::model::DialogContent::SimpleRename(_) => {
                            let new_name = self.dialogs.input_buffer.clone();
                            if !new_name.is_empty() {
                                if let Some(entry) = self.active_pane().current_entry() {
                                    let from = entry.location.clone();
                                    let to = from
                                        .parent()
                                        .unwrap_or_else(|| {
                                            self.active_pane().current_location.clone()
                                        })
                                        .join(&new_name);
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Rename {
                                            from,
                                            to,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            }
                        }
                        crate::model::DialogContent::FileMask(
                            crate::model::dialog::FileMaskDialog { input, .. },
                        ) => {
                            let mask = if input.is_empty() {
                                None
                            } else {
                                Some(input.clone())
                            };
                            let pane = self.ui.active_pane;
                            self.dialogs.pop();
                            return Some(update_state(
                                self,
                                Transition::SetFileMask { pane, mask },
                            ));
                        }
                        crate::model::DialogContent::DriveSelection(
                            crate::model::dialog::DriveSelectionDialog {
                                drives,
                                selected_index,
                                filter,
                            },
                        ) => {
                            let lower = filter.to_lowercase();
                            let filtered: Vec<&crate::model::dialog::DriveInfo> =
                                if filter.is_empty() {
                                    drives.iter().collect()
                                } else {
                                    drives
                                        .iter()
                                        .filter(|d| {
                                            d.display_label().to_lowercase().contains(&lower)
                                                || d.path.to_lowercase().contains(&lower)
                                        })
                                        .collect()
                                };
                            if let Some(drive) = filtered.get(*selected_index) {
                                let location = crate::model::Location::Local(
                                    std::path::PathBuf::from(&drive.path),
                                );
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::ChangeLocation { pane, location },
                                ));
                            }
                        }
                        crate::model::DialogContent::RegisteredFolderSelector(
                            crate::model::RegisteredFolderSelectorContent {
                                selected_index, ..
                            },
                        ) => {
                            let folder_index = *selected_index;
                            if self.active_pane().marking.count() > 0 {
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::MoveToRegisteredFolder { folder_index },
                                ));
                            } else {
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::NavigateToRegisteredFolder { folder_index },
                                ));
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
                    tracing::info!(
                        "No custom functions loaded (custom_functions.json missing or empty)"
                    );
                    None
                } else {
                    let dialog = crate::model::Dialog::custom_function_selector(functions);
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                }
            }
            Transition::InvokeCustomFunctionByName { name } => {
                let func = self
                    .custom_functions
                    .iter()
                    .find(|f| f.name == *name)
                    .cloned();
                if let Some(func) = func {
                    if func.is_menu() {
                        let title = func.name.clone();
                        let items = func.menu_items().to_vec();
                        self.dialogs
                            .push(crate::model::Dialog::custom_function_menu(title, items));
                        Some(StateUpdateResult::with_ui_change())
                    } else if let Some(_cmd) = func.get_command() {
                        let expander = crate::macro_expander::MacroExpander::new();
                        let command = expander
                            .expand(self, &func)
                            .unwrap_or_else(|_| func.get_command().unwrap_or("").replace("$I", ""));
                        let working_dir = self.active_pane().current_location.clone();
                        let shell = func.get_shell().map(|s| s.to_string());
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::ExecuteCustomFunction {
                                command,
                                working_dir,
                                pipe_to_action: func.pipe_to_action.clone(),
                                shell,
                            });
                        Some(StateUpdateResult::with_job(job_spec))
                    } else {
                        None
                    }
                } else {
                    tracing::warn!("InvokeCustomFunctionByName: no function named {:?}", name);
                    None
                }
            }
            Transition::ExecuteAssociation {
                command,
                working_dir,
                shell,
            } => {
                let job_spec =
                    crate::job::JobSpec::new(crate::job::JobKind::ExecuteCustomFunction {
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
                    let (ls, _) = tab
                        .history
                        .stack_and_pos(crate::model::ui::ActivePane::Left);
                    let (rs, _) = tab
                        .history
                        .stack_and_pos(crate::model::ui::ActivePane::Right);
                    (
                        ls.to_vec(),
                        rs.to_vec(),
                        tab.left_pane.current_location.clone(),
                        tab.right_pane.current_location.clone(),
                    )
                };
                let mut nw_roots: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();
                for loc in left_stack
                    .iter()
                    .chain(right_stack.iter())
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
                    if let crate::model::DialogContent::Help(crate::model::HelpDialog {
                        ref mut language,
                        ref mut entries,
                        ..
                    }) = dialog.content
                    {
                        *language = next_lang.clone();
                        let descriptions =
                            crate::help_content::ActionDescriptions::load(&next_lang);
                        *entries = crate::help_content::build_help_entries(
                            &self.config.key_bindings,
                            &descriptions,
                            &self.custom_functions,
                            self.config.help_show_unbound,
                            &self.config,
                        );
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
                let job_spec =
                    crate::job::JobSpec::new(crate::job::JobKind::CollectJumpCandidates {
                        root: root.clone(),
                        include_files: false,
                        max_results: self.config.jump_nav.jump_path_max_results,
                        max_depth: self.config.jump_nav.jump_path_max_depth,
                    });
                let job_id = job_spec.id;
                let mut dialog = crate::model::Dialog::jump_to_path(root, fast_candidates);
                if let crate::model::dialog::DialogContent::JumpToPath(
                    crate::model::dialog::JumpToPathDialog { loading_job_id, .. },
                ) = &mut dialog.content
                {
                    *loading_job_id = Some(job_id);
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowJumpToFileDialog => {
                let root = self.active_pane().current_location.display_path();
                let fast_candidates = collect_jump_file_fast_candidates(self);
                let job_spec =
                    crate::job::JobSpec::new(crate::job::JobKind::CollectJumpCandidates {
                        root: root.clone(),
                        include_files: true,
                        max_results: self.config.jump_nav.jump_file_max_results,
                        max_depth: self.config.jump_nav.jump_file_max_depth,
                    });
                let job_id = job_spec.id;
                let mut dialog = crate::model::Dialog::jump_to_file(root, fast_candidates);
                if let crate::model::dialog::DialogContent::JumpToFile(
                    crate::model::dialog::JumpToFileDialog { loading_job_id, .. },
                ) = &mut dialog.content
                {
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
                    return Some(update_state(
                        self,
                        Transition::ChangeLocation {
                            pane: self.ui.active_pane,
                            location,
                        },
                    ));
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
                            pane.entries
                                .iter()
                                .filter(|e| pane.marking.is_marked(&e.location))
                                .map(|e| e.location.clone())
                                .collect()
                        } else {
                            vec![]
                        }
                    };

                    if !sources.is_empty() {
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::Move { sources, dest });
                        self.dialogs.pop();
                        return Some(StateUpdateResult::with_job(job_spec));
                    }
                }
                Some(StateUpdateResult::none())
            }
            Transition::EditConfigFile => {
                let config_manager = crate::config::ConfigManager::new();
                let config_path = config_manager.config_path().to_string_lossy().to_string();
                let job_spec = JobSpec::new(Self::editor_job(&self.config, config_path, true));
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenWithEditor { path } => {
                let job_spec = JobSpec::new(Self::editor_job(&self.config, path.clone(), false));
                Some(StateUpdateResult::with_job(job_spec))
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
    CursorMove {
        pane: crate::model::ActivePane,
        delta: isize,
    },
    CursorJump {
        pane: crate::model::ActivePane,
        position: usize,
    },
    ChangeLocation {
        pane: crate::model::ActivePane,
        location: crate::model::Location,
    },
    NavigateUp {
        pane: crate::model::ActivePane,
    },
    NavigateHistory {
        pane: crate::model::ActivePane,
        direction: HistoryDirection,
    },
    NavigateToHistoryIndex {
        pane: crate::model::ActivePane,
        index: usize,
    },
    SwitchPane,
    SyncPanes,
    SwapPanes,

    // Tab management
    CreateTab,
    CloseTab {
        index: usize,
    },
    NextTab,
    PrevTab,
    SwitchTab {
        index: usize,
    },

    // File marking
    ToggleMark {
        location: crate::model::Location,
    },
    MarkAll,
    UnmarkAll,
    MarkPattern {
        pattern: String,
    },
    MarkRange {
        start: usize,
        end: usize,
    },
    InvertMarks,
    EnterRangeMarkingMode,

    // UI Events
    PaneRefreshed {
        tab_id: usize,
        pane: crate::model::ActivePane,
    },

    // Background jobs
    EnqueueJob {
        spec: JobSpec,
    },
    StartNextJob,
    JobStarted {
        job_id: JobId,
    },
    UpdateJobProgress {
        job_id: JobId,
        progress: f64,
    },
    UpdateJobProgressWithDetail {
        job_id: JobId,
        progress: f64,
        progress_message: String,
        operation_detail: String,
    },
    CompleteJob {
        job_id: JobId,
        result: crate::job::OpResult,
    },
    CancelJob {
        job_id: JobId,
    },
    AcknowledgeCancel {
        job_id: JobId,
    },
    CreateBackgroundJob {
        spec: JobSpec,
        name: String,
        description: String,
    },
    CreateAndStartFileJob {
        spec: JobSpec,
        name: String,
        description: String,
    },
    CreatePendingFileJob {
        spec: JobSpec,
        name: String,
        description: String,
    },
    CreateAndStartCountDownJob {
        spec: JobSpec,
        name: String,
        description: String,
    },
    AddTaskPanelLog {
        message: String,
    },

    // View settings
    ChangeSortMode {
        pane: crate::model::ActivePane,
        mode: crate::model::SortMode,
    },
    ChangeSortOrder {
        pane: crate::model::ActivePane,
        order: crate::model::SortOrder,
    },
    ChangeDisplayMode {
        pane: crate::model::ActivePane,
        mode: crate::model::DisplayMode,
    },
    SetFileMask {
        pane: crate::model::ActivePane,
        mask: Option<String>,
    },
    ToggleHidden,
    Refresh {
        pane: crate::model::ActivePane,
    },
    RefreshAndClearMarks {
        pane: crate::model::ActivePane,
    },
    RefreshNoClearMarks {
        pane: crate::model::ActivePane,
    },

    // UI state
    ChangeUIMode {
        mode: crate::model::UIMode,
    },
    UpdatePaneHeight {
        height: usize,
    },
    UpdatePaneWidth {
        width: usize,
    },
    ShowDialog {
        dialog: crate::model::Dialog,
    },
    CloseDialog,
    UpdateDialogInput {
        input: String,
    },
    ConfirmDialog,
    CancelDialog,
    ToggleTaskPanel,
    IncreaseTaskPanelHeight,
    DecreaseTaskPanelHeight,
    ScrollTaskPanelUp,
    ScrollTaskPanelDown,
    ShowContextMenu,
    ShowCustomFunctionsDialog,
    /// Invoke a custom function (or menu) by name, resolved from state.custom_functions at runtime.
    InvokeCustomFunctionByName {
        name: String,
    },
    /// Execute a command from a file-type extension association (Phase 6.2)
    ExecuteAssociation {
        command: String,
        working_dir: crate::model::Location,
        shell: Option<String>,
    },
    ShowDriveChangeDialog,
    ShowFileInfo,
    ShowVersion,
    SaveLog,
    RotateHelpLanguage,
    EditConfigFile,
    OpenWithEditor {
        path: String,
    },
    ShowRegisteredFolderDialog,
    RegisterCurrentFolder {
        name: String,
        path: String,
    },
    ShowJumpToPathDialog,
    ShowJumpToFileDialog,
    NavigateToRegisteredFolder {
        folder_index: usize,
    },
    MoveToRegisteredFolder {
        folder_index: usize,
    },

    // Configuration
    ReloadConfig,
    UpdateConfig {
        config: Box<AppConfig>,
    },

    // Search
    StartSearch {
        query: String,
    },
    UpdateSearchQuery {
        query: String,
    },
    UpdateSearchResults {
        results: Vec<crate::model::FileEntry>,
    },
    NextSearchResult,
    PrevSearchResult,
    ClearSearch,

    // Viewer
    OpenTextViewer {
        location: crate::model::Location,
    },
    OpenHexViewer {
        location: crate::model::Location,
    },
    OpenSideBySideViewer {
        location: crate::model::Location,
        mode: crate::model::ViewerMode,
    },
    /// Reload viewer content with an explicit mode. Used for auto-preview:
    /// cursor moves in SideBySide file pane update the viewer live.
    ReloadViewer {
        location: crate::model::Location,
        mode: crate::model::ViewerMode,
    },
    CloseViewer,
    ViewerSwitchLayout {
        layout: crate::model::ViewerLayout,
    },
    ViewerReady {
        buffer: crate::model::viewer::ViewerBuffer,
        encoding: crate::model::viewer::TextEncoding,
    },
    ViewerLoadComplete {
        contents: Vec<u8>,
    },
    ViewerSearchComplete {
        job_id: crate::job::JobId,
        matches: Vec<(usize, usize, usize)>,
    },
    ViewerCycleEncoding,
    ViewerToggleMode,
    ViewerScrollDown {
        viewport_height: usize,
    },
    ViewerScrollUp,
    ViewerPageDown {
        viewport_height: usize,
    },
    ViewerPageUp {
        viewport_height: usize,
    },
    ViewerJumpToTop,
    ViewerJumpToBottom {
        viewport_height: usize,
    },
    ViewerJumpToLine {
        line_idx: usize,
        viewport_height: usize,
    },
    ViewerMoveToLineStart,
    ViewerMoveToLineEnd {
        viewport_width: usize,
    },
    ViewerStartSearch {
        query: String,
    },
    ViewerFindNext,
    ViewerFindPrev,
    ViewerClearSearch,
    ViewerToggleCaseSensitive,
    ViewerScrollLeft {
        cols: usize,
    },
    ViewerScrollRight {
        cols: usize,
    },
    ViewerFastScrollUp {
        lines: usize,
    },
    ViewerFastScrollDown {
        lines: usize,
        viewport_height: usize,
    },

    // Pattern rename operations
    ShowPatternRenameDialog,
    UpdatePatternRenameFields {
        find: String,
        replace: String,
        use_regex: bool,
        case_sensitive: bool,
    },
    ExecutePatternRename {
        find: String,
        replace: String,
        use_regex: bool,
        case_sensitive: bool,
        targets: Vec<crate::model::Location>,
    },

    // File comparison and split/join operations
    CompareFiles {
        left: crate::model::Location,
        right: crate::model::Location,
    },
    ShowComparisonView {
        diff: crate::job::FileDiff,
    },
    CloseComparisonView,
    ShowSplitJoinDialog,
    ExecuteFileSplit {
        source: crate::model::Location,
        dest_dir: crate::model::Location,
        chunk_size: u64,
    },
    ExecuteFileJoin {
        parts: Vec<crate::model::Location>,
        dest: crate::model::Location,
    },

    // Application control
    Quit,
    ExitAndChangeDirectory,

    // Leap Navigation
    EnterLeap {
        root_dir: std::path::PathBuf,
        root_cursor: usize,
    },
    LeapApplyFilter {
        filtered_entries: Vec<crate::model::FileEntry>,
        cursor: usize,
    },
    LeapUpdateLastValid {
        buffer: String,
    },
    LeapGoParent,
    LeapClearLocal,
    LeapClearAll,
    LeapConfirm,
    LeapCancel,
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
    /// Signal to app.rs to reload keybindings from file (set by ReloadConfig)
    pub reload_keybindings: bool,
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
            reload_keybindings: false,
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
            reload_keybindings: false,
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
            reload_keybindings: false,
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
            reload_keybindings: false,
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
            reload_keybindings: false,
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
            let old_migemo = state.config.search.dict_path.clone();

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
            let (custom_fns, custom_fn_result) =
                match crate::model::dialog::load_custom_functions(&custom_fn_path) {
                    Ok(fns) => {
                        let result = if custom_fn_path.exists() {
                            crate::config::ConfigLoadResult::ok(custom_fn_path)
                        } else {
                            crate::config::ConfigLoadResult::skipped(
                                custom_fn_path,
                                "file not found",
                            )
                        };
                        (fns, result)
                    }
                    Err(e) => (
                        Vec::new(),
                        crate::config::ConfigLoadResult::error(custom_fn_path, e.to_string()),
                    ),
                };
            state.custom_functions = custom_fns;

            let context_menu_result = crate::config::ConfigManager::validate_json_file(
                config_manager.context_menu_path(),
            );

            // Scan menu_*.json files in the same directory as custom_functions.json
            let custom_fn_dir = config_manager
                .custom_functions_path()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let mut menu_file_results: Vec<crate::config::ConfigLoadResult> = Vec::new();
            if custom_fn_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&custom_fn_dir) {
                    let mut menu_paths: Vec<std::path::PathBuf> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| n.starts_with("menu_") && n.ends_with(".json"))
                                .unwrap_or(false)
                        })
                        .collect();
                    menu_paths.sort();
                    for path in menu_paths {
                        menu_file_results
                            .push(crate::config::ConfigManager::validate_json_file(&path));
                    }
                }
            }

            // Preserve first 2 slots (config.json [0] and keybindings.json [1]).
            // keybindings.json is reloaded in app.rs before this transition, so [1] is already current.
            let prev_results: Vec<_> = state.config_load_results.drain(..2).collect();
            state.config_load_results = prev_results;
            state.config_load_results[0] = config_result;
            state
                .config_load_results
                .extend([ext_result, custom_fn_result, context_menu_result]);
            state.config_load_results.extend(menu_file_results);

            // Build feedback messages
            use crate::config::ConfigLoadStatus;
            let mut messages: Vec<String> = Vec::new();
            messages.push("Configuration reloaded:".to_string());

            // Show status for each config file
            for r in &state.config_load_results {
                let filename = r
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| r.path.to_string_lossy().into_owned());
                let line = match &r.status {
                    ConfigLoadStatus::Ok => format!("  [OK]      {}", filename),
                    ConfigLoadStatus::Default(why) => format!("  [OK]      {} ({})", filename, why),
                    ConfigLoadStatus::Skipped(why) => format!("  [Skipped] {} ({})", filename, why),
                    ConfigLoadStatus::Error(detail) => {
                        format!("  [NG]      {} — {}", filename, detail)
                    }
                };
                messages.push(line);
            }

            // Restart-required notice
            let mut restart_items: Vec<&str> = Vec::new();
            if state.config.worker_pool_size != old_workers {
                restart_items.push("worker thread count");
            }
            if state.config.search.dict_path != old_migemo {
                restart_items.push("Migemo dictionary path");
            }
            if !restart_items.is_empty() {
                messages.push(format!(
                    "  Note: restart required to apply: {}.",
                    restart_items.join(", ")
                ));
            }

            let mut result = StateUpdateResult::with_ui_change();
            result.task_panel_logs = messages;
            result.reload_keybindings = true;
            result
        }

        Transition::UpdateConfig { config } => {
            state.jobs.max_parallel = config.worker_pool_size;
            state.config = *config;
            StateUpdateResult::with_ui_change()
        }

        Transition::Quit => StateUpdateResult::none(),

        Transition::ExitAndChangeDirectory => StateUpdateResult::none(),

        // Leap Navigation
        Transition::EnterLeap {
            root_dir,
            root_cursor,
        } => {
            state.leap = Some(crate::model::LeapState::new(root_dir, root_cursor));
            state.ui.mode = crate::model::UIMode::Leap;
            StateUpdateResult::none()
        }

        Transition::LeapApplyFilter {
            filtered_entries,
            cursor,
        } => {
            let pane = state.active_pane_mut();
            pane.entries = filtered_entries;
            pane.cursor = cursor;
            let height = state.ui.layout.pane_height;
            state.active_pane_mut().update_scroll(height, 3);
            StateUpdateResult::none()
        }

        Transition::LeapUpdateLastValid { buffer } => {
            if let Some(ref mut leap) = state.leap {
                leap.last_valid_buffer = buffer;
            }
            StateUpdateResult::none()
        }

        Transition::LeapGoParent => {
            if let Some(ref mut leap) = state.leap {
                leap.go_parent();
            }
            StateUpdateResult::none()
        }

        Transition::LeapClearLocal => {
            if let Some(ref mut leap) = state.leap {
                leap.clear_local();
            }
            StateUpdateResult::none()
        }

        Transition::LeapClearAll => {
            if let Some(ref mut leap) = state.leap {
                leap.clear_all();
            }
            StateUpdateResult::none()
        }

        Transition::LeapConfirm => {
            state.leap = None;
            state.ui.mode = crate::model::UIMode::Normal;
            // Remember which entry was selected so we can restore the cursor after
            // apply_current_filter() expands entries back to the full list.
            let selected_location = state
                .active_pane()
                .current_entry()
                .map(|e| e.location.clone());
            let pane = state.active_pane_mut();
            pane.apply_current_filter();
            if let Some(loc) = selected_location {
                if let Some(idx) = pane.entries.iter().position(|e| e.location == loc) {
                    pane.cursor = idx;
                }
            }
            StateUpdateResult::none()
        }

        Transition::LeapCancel => {
            if let Some(leap) = state.leap.take() {
                state.ui.mode = crate::model::UIMode::Normal;
                // Directory restoration handled by caller (app.rs navigates to root_dir first).
                let pane = state.active_pane_mut();
                pane.cursor = leap.root_cursor;
                pane.apply_current_filter();
            }
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
        let p = state
            .registered_folders
            .expand_path(folder)
            .to_string_lossy()
            .into_owned();
        if !p.is_empty() && seen.insert(p.clone()) {
            candidates.push(p);
        }
    }

    // 3. Navigation history (both panes, current tab)
    let tab = state.current_tab();
    let (left_stack, _) = tab
        .history
        .stack_and_pos(crate::model::ui::ActivePane::Left);
    let (right_stack, _) = tab
        .history
        .stack_and_pos(crate::model::ui::ActivePane::Right);
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
        if entry.name == ".." {
            continue;
        }
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

        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&location));

        let result = update_state(
            &mut state,
            Transition::ToggleMark {
                location: location.clone(),
            },
        );
        assert!(result.ui_changed);
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&location));

        let result = update_state(
            &mut state,
            Transition::ToggleMark {
                location: location.clone(),
            },
        );
        assert!(result.ui_changed);
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&location));
    }

    #[test]
    fn test_mark_all_transition() {
        use crate::model::{FileEntry, Location};
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
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

        state
            .current_tab_mut()
            .left_pane
            .marking
            .toggle(Location::Local(PathBuf::from("/test/file1.txt")));
        state
            .current_tab_mut()
            .left_pane
            .marking
            .toggle(Location::Local(PathBuf::from("/test/file2.txt")));
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);

        let result = update_state(&mut state, Transition::UnmarkAll);
        assert!(result.ui_changed);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 0);
    }

    #[test]
    fn test_mark_pattern_transition() {
        use crate::model::{FileEntry, Location};
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
            },
        ];
        state.active_pane_mut().entries = entries;

        let result = update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "*.txt".to_string(),
            },
        );
        assert!(result.ui_changed);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/test.txt"))));
    }

    #[test]
    fn test_mark_range_transition() {
        use crate::model::{FileEntry, Location};
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
            },
        ];
        state.active_pane_mut().entries = entries;

        let result = update_state(&mut state, Transition::MarkRange { start: 0, end: 1 });
        assert!(result.ui_changed);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
    }

    #[test]
    fn test_invert_marks_transition() {
        use crate::model::{FileEntry, Location};
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
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
                is_symlink: false,
                link_target: None,
                link_kind: None,
            },
        ];
        state.active_pane_mut().entries = entries;

        state
            .current_tab_mut()
            .left_pane
            .marking
            .toggle(Location::Local(PathBuf::from("/test/f1.txt")));
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);

        let result = update_state(&mut state, Transition::InvertMarks);
        assert!(result.ui_changed);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/f2.txt"))));
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

        let result = update_state(
            &mut state,
            Transition::CursorMove {
                pane: ActivePane::Left,
                delta: 1,
            },
        );
        assert!(result.ui_changed);
        assert_eq!(state.active_pane().cursor, 1);

        update_state(
            &mut state,
            Transition::CursorMove {
                pane: ActivePane::Left,
                delta: 1,
            },
        );
        assert_eq!(state.active_pane().cursor, 2);

        // Should clamp to last entry
        update_state(
            &mut state,
            Transition::CursorMove {
                pane: ActivePane::Left,
                delta: 1,
            },
        );
        assert_eq!(state.active_pane().cursor, 2);

        // Should clamp to 0
        update_state(
            &mut state,
            Transition::CursorMove {
                pane: ActivePane::Left,
                delta: -10,
            },
        );
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

        let result = update_state(
            &mut state,
            Transition::CursorJump {
                pane: ActivePane::Left,
                position: 2,
            },
        );
        assert!(result.ui_changed);
        assert_eq!(state.active_pane().cursor, 2);

        // Should clamp
        update_state(
            &mut state,
            Transition::CursorJump {
                pane: ActivePane::Left,
                position: 100,
            },
        );
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

        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: loc.clone(),
            },
        );

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

        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: loc.clone(),
            },
        );

        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        assert!(matches!(
            result.jobs_to_start[0].kind,
            crate::job::JobKind::ReadDirectory { .. }
        ));
        assert_eq!(state.active_pane().current_location, loc);
    }

    #[test]
    fn test_enqueue_job_transition() {
        use crate::job::{JobKind, JobSpec};
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        let job_spec = JobSpec::new(JobKind::CountDown {
            duration_secs: 1,
            start_value: 1,
        });

        assert_eq!(state.jobs.queue.len(), 0);

        let result = update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        assert!(result.ui_changed);
        assert_eq!(state.jobs.queue.len(), 1);
    }

    #[test]
    fn test_start_next_job_transition() {
        use crate::job::{JobKind, JobSpec};
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        let job_spec = JobSpec::new(JobKind::CountDown {
            duration_secs: 1,
            start_value: 1,
        });
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
        use crate::job::{ExecutionState, JobKind, JobSpec};
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        let spec = JobSpec::new(JobKind::CountDown {
            duration_secs: 1,
            start_value: 1,
        });
        let job_id = spec.id;
        state.jobs.start_job(spec);

        assert!(matches!(
            state.jobs.active.get(&job_id).unwrap().state,
            ExecutionState::Pending
        ));

        let result = update_state(&mut state, Transition::JobStarted { job_id });
        assert!(result.ui_changed);
        assert!(matches!(
            state.jobs.active.get(&job_id).unwrap().state,
            ExecutionState::Running
        ));
    }

    #[test]
    fn test_complete_job_transition() {
        use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        let spec = JobSpec::new(JobKind::CountDown {
            duration_secs: 1,
            start_value: 1,
        });
        let job_id = spec.id;
        state.jobs.start_job(spec);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        assert!(result.ui_changed);
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }

    #[test]
    fn test_cancel_job_transition() {
        use crate::job::{JobKind, JobSpec};
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        let spec = JobSpec::new(JobKind::CountDown {
            duration_secs: 5,
            start_value: 5,
        });
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

        let result = update_state(
            &mut state,
            Transition::ChangeUIMode {
                mode: UIMode::Dialog,
            },
        );
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
