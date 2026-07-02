//! Application main loop
//!
//! This module implements a truly event-driven main loop with 0% idle CPU usage.

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};
use rwf_lib::{AppState, Transition, KeyBindings, WorkerPool, process_pending_events};
use rwf_lib::backend::{LocalFilesystemBackend, MultiFormatArchiveHandler};
use rwf_lib::job::{JobSpec, JobKind};
use rwf_lib::model::dialog::ConflictPair;
use std::io::Stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::error;

use crate::ui::render_ui;
use crate::ui::task_panel::TaskPanel;

/// Application runner
pub struct App {
    state: AppState,
    key_bindings: KeyBindings,
    should_quit: bool,
    should_exit_and_cd: bool,
    worker_pool: Option<WorkerPool<LocalFilesystemBackend, MultiFormatArchiveHandler>>,
    last_key_press: Option<(String, Instant, bool)>, // (key, time, is_repeating)
    task_panel: TaskPanel,
    last_cleanup_check: Option<Instant>,
    pending_conflict_job: Option<(JobSpec, Vec<ConflictPair>, String, String)>,
    pending_job_submission: Vec<JobSpec>,
    // Search control fields
    last_search_input_time: Option<Instant>,
    search_dirty: bool,
    // Pattern rename debounce
    pattern_rename_dirty: bool,
    pattern_rename_last_changed: Option<Instant>,
    /// Cached (find, replace, use_regex, case_sensitive) waiting for debounce flush
    pattern_rename_pending: Option<(String, String, bool, bool)>,
    // Leap Navigation debounce
    leap_dirty: bool,
    last_leap_input_time: Option<Instant>,
}

impl App {
    pub fn with_state_and_keybindings(state: AppState, _cwd_flag: bool, key_bindings: KeyBindings) -> Self {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MultiFormatArchiveHandler::new());
        let worker_pool = WorkerPool::new(state.config.worker_pool_size, backend, archive_handler);
        let mut task_panel = TaskPanel::new();

        // Compact version info at startup
        for line in Self::build_version_info_compact(&state) {
            task_panel.add_log(line, crate::ui::task_panel::LogLevel::Info);
        }

        // Warn immediately in the task panel for any config file that failed to parse
        use rwf_lib::config::ConfigLoadStatus;
        for r in &state.config_load_results {
            if let ConfigLoadStatus::Error(detail) = &r.status {
                let path_str = r.path.to_string_lossy();
                task_panel.add_log(
                    format!("[NG] {}: {}", path_str, detail),
                    crate::ui::task_panel::LogLevel::Fail,
                );
            }
        }

        // Check keybindings.json for duplicate keys
        {
            let kb_path = rwf_lib::config::ConfigManager::new().keybindings_path().to_path_buf();
            for warning in rwf_lib::check_keybindings_duplicates(&kb_path) {
                tracing::warn!("{}", warning);
                task_panel.add_log(warning, crate::ui::task_panel::LogLevel::Warn);
            }
        }

        // Log verbose info to session log so it's always discoverable
        for line in Self::build_version_info_verbose(&state) {
            tracing::info!("{}", line);
        }

        task_panel.add_log("No active tasks".to_string(), crate::ui::task_panel::LogLevel::Info);

        let mut app = Self {
            state, key_bindings, should_quit: false, should_exit_and_cd: false,
            worker_pool: Some(worker_pool),
            last_key_press: None, task_panel, last_cleanup_check: None,
            pending_conflict_job: None, pending_job_submission: Vec::new(),
            last_search_input_time: None, search_dirty: false,
            pattern_rename_dirty: false, pattern_rename_last_changed: None, pattern_rename_pending: None,
            leap_dirty: false, last_leap_input_time: None,
        };
        app.state.config.key_bindings = app.key_bindings.clone();
        app
    }
    
    fn build_version_info_compact(state: &AppState) -> Vec<String> {
        let version = env!("CARGO_PKG_VERSION");
        let git_hash = env!("GIT_HASH");
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let log_path = state.log_manager.log_path().to_string_lossy().into_owned();
        let log_level = state.config.log_level.to_filter_string();
        let migemo = if state.search.is_migemo_dict_loaded() { "available" } else { "unavailable" };
        vec![
            format!("RWF v{} build {} | {}/{}", version, git_hash, os, arch),
            format!("Log [{}] {}", log_level, log_path),
            format!("Archives: ZIP, 7Z, TAR, TGZ, ISO | Migemo: {}", migemo),
        ]
    }

    fn build_version_info_verbose(state: &AppState) -> Vec<String> {
        let version = env!("CARGO_PKG_VERSION");
        let git_hash = env!("GIT_HASH");
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let log_path = state.log_manager.log_path().to_string_lossy().into_owned();
        let log_level = state.config.log_level.to_filter_string();

        let migemo_line = if state.search.is_migemo_dict_loaded() {
            state.search.migemo_dict_path()
                .map_or("available".to_string(), |p| p.to_string())
        } else {
            "unavailable".to_string()
        };

        let mut lines = vec![
            format!("RWF v{} build {} | {}/{}", version, git_hash, os, arch),
            format!("Log: {}", log_path),
            format!("LogLevel: {} | Migemo: {}", log_level, migemo_line),
            "Archives: ZIP, 7Z, TAR, TGZ / Extract only: ISO".to_string(),
            "Config files:".to_string(),
        ];

        use rwf_lib::config::ConfigLoadStatus;
        for r in &state.config_load_results {
            let path_str = r.path.to_string_lossy();
            match &r.status {
                ConfigLoadStatus::Ok =>
                    lines.push(format!(" [OK]      {}", path_str)),
                ConfigLoadStatus::Default(reason) =>
                    lines.push(format!(" [OK]      {}  ({})", path_str, reason)),
                ConfigLoadStatus::Skipped(reason) =>
                    lines.push(format!(" [Skipped] {}  ({})", path_str, reason)),
                ConfigLoadStatus::Error(detail) => {
                    lines.push(format!(" [NG]      {}", path_str));
                    lines.push(format!("           {}", detail));
                }
            }
        }
        lines
    }

    fn log_version_info(&mut self) {
        for line in Self::build_version_info_compact(&self.state) {
            self.task_panel.add_log(line, crate::ui::task_panel::LogLevel::Info);
        }
    }

    fn log_version_info_verbose(&mut self) {
        for line in Self::build_version_info_verbose(&self.state) {
            self.task_panel.add_log(line, crate::ui::task_panel::LogLevel::Info);
        }
    }

    fn trigger_initial_directory_reads(&mut self) {
        let worker_pool = self.worker_pool.as_ref().expect("Worker pool should exist");
        for tab_index in 0..self.state.tabs.tabs.len() {
            let tab_id = self.state.tabs.tabs[tab_index].id;
            let left_loc = self.state.tabs.tabs[tab_index].left_pane.current_location.clone();
            let right_loc = self.state.tabs.tabs[tab_index].right_pane.current_location.clone();

            let job_l = JobSpec::new(JobKind::ReadDirectory { location: left_loc })
                .with_requesting_pane(tab_id, rwf_lib::model::ActivePane::Left);
            self.state.tabs.tabs[tab_index].left_pane.is_loading = true;
            self.state.tabs.tabs[tab_index].left_pane.active_job_id = Some(job_l.id);
            self.state.jobs.start_job(job_l.clone());
            worker_pool.submit_job(job_l);

            let job_r = JobSpec::new(JobKind::ReadDirectory { location: right_loc })
                .with_requesting_pane(tab_id, rwf_lib::model::ActivePane::Right);
            self.state.tabs.tabs[tab_index].right_pane.is_loading = true;
            self.state.tabs.tabs[tab_index].right_pane.active_job_id = Some(job_r.id);
            self.state.jobs.start_job(job_r.clone());
            worker_pool.submit_job(job_r);
        }
    }
    
    pub fn should_output_directory(&self) -> bool { self.should_exit_and_cd }
    pub fn get_exit_directory_public(&self) -> String { self.state.active_pane().current_location.display_path() }

    fn has_active_jobs(&self) -> bool {
        let active = !self.state.jobs.active.is_empty();
        let background = self.state.background_jobs.get_active_jobs().next().is_some();
        if active || background {
            tracing::debug!("[AppLoop] has_active_jobs=true (active={}, background={})", active, background);
        } else {
            tracing::debug!("[AppLoop] has_active_jobs=false (active_count={})", self.state.jobs.active.len());
        }
        active || background
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        self.trigger_initial_directory_reads();
        
        // Initial render
        let mut ui_needs_update = true;

        loop {
            if ui_needs_update {
                self.render(terminal)?;
                ui_needs_update = false;
            }

            // 1. Process background events from workers
            let leap_active_pane_was_loading = if self.state.ui.mode == rwf_lib::model::UIMode::Leap {
                let ap = self.state.ui.active_pane;
                let tab = self.state.current_tab();
                match ap {
                    rwf_lib::model::ActivePane::Left  => tab.left_pane.is_loading,
                    rwf_lib::model::ActivePane::Right => tab.right_pane.is_loading,
                }
            } else { false };

            if let Some(ref mut pool) = self.worker_pool {
                let results = process_pending_events(pool, &mut self.state);
                if !results.is_empty() {
                    tracing::info!("[AppLoop] Processed {} events", results.len());
                    ui_needs_update = true;
                    for result in &results {
                        tracing::debug!("[AppLoop] Result: ui_changed={}, started_jobs={}", result.ui_changed, result.jobs_to_start.len());
                        for log_msg in &result.task_panel_logs { self.task_panel.add_pending_log(log_msg.clone()); }
                        for refresh in &result.panes_to_refresh {
                            let tab_idx = refresh.tab_id; // array index stored by file-op handlers
                            let tab_id = self.state.tabs.tabs[tab_idx].id;
                            let location = if refresh.pane == rwf_lib::model::ActivePane::Left {
                                self.state.tabs.tabs[tab_idx].left_pane.current_location.clone()
                            } else {
                                self.state.tabs.tabs[tab_idx].right_pane.current_location.clone()
                            };
                            let job = JobSpec::new(JobKind::ReadDirectory { location })
                                .with_requesting_pane(tab_id, refresh.pane);
                            // Keep showing old entries during refresh (no loading indicator)
                            if refresh.pane == rwf_lib::model::ActivePane::Left {
                                self.state.tabs.tabs[tab_idx].left_pane.active_job_id = Some(job.id);
                            } else {
                                self.state.tabs.tabs[tab_idx].right_pane.active_job_id = Some(job.id);
                            }
                            self.state.jobs.start_job(job.clone());
                            pool.submit_job(job);
                        }
                        for job_spec in &result.jobs_to_start { pool.submit_job(job_spec.clone()); }
                    }
                }
            }

            // Re-apply leap filter immediately after active pane finishes loading a directory.
            if leap_active_pane_was_loading && self.state.ui.mode == rwf_lib::model::UIMode::Leap {
                let ap = self.state.ui.active_pane;
                let tab = self.state.current_tab();
                let now_loading = match ap {
                    rwf_lib::model::ActivePane::Left  => tab.left_pane.is_loading,
                    rwf_lib::model::ActivePane::Right => tab.right_pane.is_loading,
                };
                if !now_loading {
                    self.perform_leap_filter();
                    self.leap_dirty = false;
                    self.last_leap_input_time = None;
                    ui_needs_update = true;
                }
            }

            // 2. Process pending job submissions (from transitions)
            let pending_jobs: Vec<JobSpec> = self.pending_job_submission.drain(..).collect();
            if !pending_jobs.is_empty() { ui_needs_update = true; }
            for job_spec in pending_jobs {
                // SuspendAndRun: hand the terminal to the editor, then resume.
                // Must happen on the main thread before any pool interaction.
                if let JobKind::SuspendAndRun { program, args } = &job_spec.kind {
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(
                        std::io::stdout(),
                        crossterm::terminal::LeaveAlternateScreen,
                    );
                    let _ = std::process::Command::new(program).args(args).status();
                    let _ = crossterm::terminal::enable_raw_mode();
                    let _ = crossterm::execute!(
                        std::io::stdout(),
                        crossterm::terminal::EnterAlternateScreen,
                    );
                    let _ = terminal.clear();
                    continue;
                }
                if let Some(ref pool) = self.worker_pool {
                    match &job_spec.kind {
                        JobKind::Copy { sources, dest } | JobKind::Move { sources, dest } => {
                            let conflicts = pool.detect_conflicts(sources, dest).await;
                            if conflicts.is_empty() {
                                let job_name = match &job_spec.kind { JobKind::Copy { sources, .. } => format!("Copy ({} files)", sources.len()), JobKind::Move { sources, .. } => format!("Move ({} files)", sources.len()), _ => "File Op".to_string() };
                                let bg_id = self.state.background_jobs.start_job(job_name.clone(), job_name.clone(), self.state.tabs.active_index, String::new(), job_spec.clone());
                                self.state.jobs.start_job(job_spec.clone());
                                self.task_panel.add_pending_log(format!("{} [Job {}] {}: Started", chrono::Local::now().format("[%H:%M:%S]"), bg_id.short_id, job_name));
                                pool.submit_job(job_spec);
                            } else {
                                let job_name = match &job_spec.kind { JobKind::Copy { sources, .. } => format!("Copy ({} files)", sources.len()), JobKind::Move { sources, .. } => format!("Move ({} files)", sources.len()), _ => "File Op".to_string() };
                                let op_name = match &job_spec.kind { JobKind::Move { .. } => "Move", _ => "Copy" };
                                self.pending_conflict_job = Some((job_spec, conflicts.clone(), job_name.clone(), job_name));
                                self.state.dialogs.push(rwf_lib::model::Dialog::file_conflict(conflicts, 0, self.state.config.text_input.edit_mode, op_name));
                            }
                        }
                        _ => {
                            tracing::info!("[AppLoop] Submitting pending job: id={:?}, kind={:?}", job_spec.id, job_spec.kind);
                            // For ReadDirectory jobs, track active_job_id so CompleteJob can validate ownership.
                            // ChangeLocation/NavigateHistory set is_loading=true but cannot set active_job_id
                            // (job is submitted here, not in the state handler), so we set it now.
                            if let JobKind::ReadDirectory { .. } = &job_spec.kind {
                                if let Some((req_tab_id, req_pane)) = job_spec.requesting_pane {
                                    if let Some(tab) = self.state.tabs.tabs.iter_mut().find(|t| t.id == req_tab_id) {
                                        match req_pane {
                                            rwf_lib::model::ActivePane::Left => tab.left_pane.active_job_id = Some(job_spec.id),
                                            rwf_lib::model::ActivePane::Right => tab.right_pane.active_job_id = Some(job_spec.id),
                                        }
                                    }
                                }
                            }
                            self.state.jobs.start_job(job_spec.clone());
                            pool.submit_job(job_spec);
                        },
                    }
                }
            }

            // 3. Process logs
            if self.task_panel.pending_log_count() > 0 {
                self.task_panel.process_pending_logs(self.state.config.job_manager.max_task_panel_log_lines);
                let h = self.state.ui.layout.task_panel_height;
                self.task_panel.scroll_to_end(h);
                ui_needs_update = true;
            }

            // 4. Redraw while jobs are active so the spinner (wall-clock based) animates
            if self.has_active_jobs() {
                ui_needs_update = true;
            }

            // 5. Search Mode Timer-Only Trigger
            if self.state.ui.mode == rwf_lib::model::UIMode::Search && self.search_dirty {
                if let Some(last_input) = self.last_search_input_time {
                    let debounce = Duration::from_millis(self.state.config.search.search_debounce_ms);
                    if last_input.elapsed() >= debounce {
                        self.perform_incremental_search();
                        self.search_dirty = false;
                        self.last_search_input_time = None;
                        ui_needs_update = true;
                    }
                }
            }

            // 5b. Pattern Rename Debounce Flush
            if self.pattern_rename_dirty {
                if let Some(last_changed) = self.pattern_rename_last_changed {
                    let debounce = Duration::from_millis(self.state.config.search.pattern_rename_debounce_ms);
                    if last_changed.elapsed() >= debounce {
                        if let Some((f, r, ur, cs)) = self.pattern_rename_pending.take() {
                            rwf_lib::state::update_state(
                                &mut self.state,
                                rwf_lib::state::Transition::UpdatePatternRenameFields {
                                    find: f, replace: r, use_regex: ur, case_sensitive: cs,
                                },
                            );
                        }
                        self.pattern_rename_dirty = false;
                        self.pattern_rename_last_changed = None;
                        ui_needs_update = true;
                    }
                }
            }

            // 5c. Leap Mode Debounce Flush
            if self.state.ui.mode == rwf_lib::model::UIMode::Leap && self.leap_dirty {
                if let Some(last_input) = self.last_leap_input_time {
                    let debounce_ms = self.state.config.jump_nav.leap_debounce_ms;
                    if last_input.elapsed() >= Duration::from_millis(debounce_ms) {
                        self.perform_leap_filter();
                        self.leap_dirty = false;
                        self.last_leap_input_time = None;
                        ui_needs_update = true;
                    }
                }
            }

            // 6. Cleanup
            if self.last_cleanup_check.map_or(true, |l| l.elapsed() >= Duration::from_secs(5)) {
                self.state.background_jobs.cleanup_expired_jobs();
                self.last_cleanup_check = Some(Instant::now());
            }

            // 7. Adaptive Sleep (Next Wakeup Calculation)
            let mut next_wakeup = Duration::from_secs(1); // Default safety poll

            if self.search_dirty {
                if let Some(last_input) = self.last_search_input_time {
                    let debounce = Duration::from_millis(self.state.config.search.search_debounce_ms);
                    next_wakeup = next_wakeup.min(debounce.saturating_sub(last_input.elapsed()));
                }
            }

            if self.pattern_rename_dirty {
                if let Some(last_changed) = self.pattern_rename_last_changed {
                    let debounce = Duration::from_millis(self.state.config.search.pattern_rename_debounce_ms);
                    next_wakeup = next_wakeup.min(debounce.saturating_sub(last_changed.elapsed()));
                }
            }

            if self.leap_dirty {
                if let Some(last_input) = self.last_leap_input_time {
                    let debounce_ms = self.state.config.jump_nav.leap_debounce_ms;
                    let debounce = Duration::from_millis(debounce_ms);
                    next_wakeup = next_wakeup.min(debounce.saturating_sub(last_input.elapsed()));
                }
            }

            // Poll frequently while any pane is still loading so completion events
            // are picked up promptly. Interval is configurable for slow machines.
            let any_pane_loading = self.state.tabs.tabs.iter()
                .any(|t| t.left_pane.is_loading || t.right_pane.is_loading);
            if any_pane_loading {
                let poll_ms = self.state.config.job_manager.loading_poll_interval_ms;
                next_wakeup = next_wakeup.min(Duration::from_millis(poll_ms));
            }

            // If UI needs update, render immediately without blocking
            if ui_needs_update {
                next_wakeup = Duration::from_millis(0);
            }

            tracing::debug!("[AppLoop] Adaptive poll timeout: {}ms", next_wakeup.as_millis());

            // Wait for events OR timeout
            if self.handle_events(next_wakeup)? {
                ui_needs_update = true;
            }

            if self.should_quit {
                if let Err(e) = self.state.save_session() { error!("Save session failed: {}", e); }
                let active_ids: Vec<_> = self.state.background_jobs.get_active_jobs().map(|j| j.id.uuid).collect();
                for id in active_ids { self.state.background_jobs.cancel_job(id); }
                if let Some(pool) = self.worker_pool.take() { pool.shutdown().await; }
                break;
            }
        }
        Ok(())
    }

    fn handle_events(&mut self, timeout: Duration) -> Result<bool> {
        let mut any_event = false;
        // Block until input OR timeout
        if event::poll(timeout)? {
            // Read ALL pending events to clear queue
            loop {
                let ev = event::read()?;
                match ev {
                    Event::Key(key) => {
                        if key.kind == crossterm::event::KeyEventKind::Press {
                            if self.handle_key_event(key) { any_event = true; }
                        }
                    }
                    Event::Resize(_, _) => {
                        // Recalculate task panel view on terminal resize
                        let h = self.state.ui.layout.task_panel_height;
                        self.task_panel.scroll_to_end(h);
                        any_event = true;
                    }
                    _ => {}
                }

                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
            }
        }
        Ok(any_event)
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> bool {
        // Ctrl+L: force full redraw (works in any mode)
        if key.code == crossterm::event::KeyCode::Char('l')
            && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
        {
            return true;
        }

        // Ctrl+W: emergency viewer escape (hardcoded, works regardless of mode or dialog state).
        // Cancels any in-progress viewer loading job and closes the viewer immediately.
        // Useful when the viewer appears stuck or unresponsive.
        if key.code == crossterm::event::KeyCode::Char('w')
            && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
            && self.state.viewer.is_some()
        {
            rwf_lib::state::update_state(&mut self.state, Transition::CloseViewer);
            return true;
        }

        let key_string = rwf_lib::input::format_key_event(&key);
        tracing::info!("[KEY] code={:?} modifiers={:?} kind={:?} formatted={:?}", key.code, key.modifiers, key.kind, key_string);
        let now = Instant::now();
        
        // Key repeat logic
        if let Some((last_key, last_time, is_repeating)) = &self.last_key_press {
            if last_key == &key_string {
                let elapsed = now.duration_since(*last_time);
                if *is_repeating {
                    if elapsed < Duration::from_millis(self.state.config.key_repeat_rate_ms as u64) { return false; }
                    self.last_key_press = Some((key_string.clone(), now, true));
                } else {
                    if elapsed < Duration::from_millis(self.state.config.key_repeat_delay_ms as u64) { return false; }
                    self.last_key_press = Some((key_string.clone(), now, true));
                }
            } else { self.last_key_press = Some((key_string.clone(), now, false)); }
        } else { self.last_key_press = Some((key_string.clone(), now, false)); }

        // 1. Dialog handling
        if let Some(dialog) = self.state.dialogs.current_mut() {
            tracing::info!("[KEY] consumed by dialog: {:?}", dialog.title);
            match crate::ui::dialog::handle_dialog_input(dialog, key, Some(&self.state.search)) {
                crate::ui::dialog::DialogAction::Cancel => {
                    if let rwf_lib::model::dialog::DialogContent::FileConflict { .. } = &dialog.content { self.pending_conflict_job = None; }
                    if let rwf_lib::model::dialog::DialogContent::PatternRename { .. } = &dialog.content {
                        self.pattern_rename_dirty = false;
                        self.pattern_rename_last_changed = None;
                        self.pattern_rename_pending = None;
                    }
                    let loading_job = match &dialog.content {
                        rwf_lib::model::dialog::DialogContent::JumpToFile { loading_job_id, .. } => *loading_job_id,
                        rwf_lib::model::dialog::DialogContent::JumpToPath { loading_job_id, .. } => *loading_job_id,
                        _ => None,
                    };
                    if let Some(job_id) = loading_job {
                        self.state.jobs.request_cancel(job_id);
                    }
                    let is_menu_dialog = matches!(&dialog.content, rwf_lib::model::dialog::DialogContent::CustomFunctionMenu { .. });
                    self.state.dialogs.pop();
                    // Esc on a CustomFunctionMenu closes the whole stack (not just the menu)
                    if is_menu_dialog {
                        if let Some(d) = self.state.dialogs.current() {
                            if matches!(&d.content, rwf_lib::model::dialog::DialogContent::CustomFunctionSelector { .. }) {
                                self.state.dialogs.pop();
                            }
                        }
                    }
                    return true;
                }
                crate::ui::dialog::DialogAction::Confirm => {
                    let is_function_menu = matches!(&dialog.content, rwf_lib::model::dialog::DialogContent::CustomFunctionMenu { .. });
                    let mut should_pop = true;
                    match &mut dialog.content {
                        rwf_lib::model::dialog::DialogContent::FileConflict { conflicts, current_index, decisions, .. } => {
                            if *current_index + 1 < conflicts.len() {
                                *current_index += 1;
                                dialog.update_file_conflict_title();
                                should_pop = false;
                            } else if let Some((job_spec, conflicts_list, job_name, job_desc)) = self.pending_conflict_job.take() {
                                let conflict_decisions: Vec<_> = conflicts_list.iter().zip(decisions.iter()).map(|(c, a)| rwf_lib::job::ConflictDecision {
                                    source: c.source_path.clone(), dest: c.dest_path.clone(),
                                    action: match a {
                                        rwf_lib::model::dialog::ConflictAction::Force => rwf_lib::job::ConflictAction::Force,
                                        rwf_lib::model::dialog::ConflictAction::OverwriteIfNewer => rwf_lib::job::ConflictAction::OverwriteIfNewer,
                                        rwf_lib::model::dialog::ConflictAction::Skip => rwf_lib::job::ConflictAction::Skip,
                                        rwf_lib::model::dialog::ConflictAction::Rename { new_name } => rwf_lib::job::ConflictAction::Rename { new_name: new_name.clone() },
                                    },
                                }).collect();

                                let mut final_job = job_spec.clone();
                                final_job.conflict_decisions = Some(conflict_decisions);
                                let tab_id = self.state.tabs.active_index;
                                let tab_name = format!("{}|{}", self.state.current_tab().left_pane.current_location.display_path(), self.state.current_tab().right_pane.current_location.display_path());
                                let _bg_job_id = self.state.background_jobs.start_job(job_name.clone(), job_desc, tab_id, tab_name, final_job.clone());
                                self.state.jobs.start_job(final_job.clone());
                                if let Some(ref pool) = self.worker_pool { pool.submit_job(final_job); }
                            }
                        }
                        _ => {
                            if let rwf_lib::model::dialog::DialogContent::PatternRename { .. } = &dialog.content {
                                self.pattern_rename_dirty = false;
                                self.pattern_rename_last_changed = None;
                                self.pattern_rename_pending = None;
                            }
                            let confirmed_job = crate::ui::dialog::process_dialog_confirmation(&mut self.state);
                            // If process_dialog_confirmation pushed a new dialog, don't pop the old one.
                            if self.state.suppress_next_dialog_pop {
                                self.state.suppress_next_dialog_pop = false;
                                should_pop = false;
                            }
                            // Drain staging logs and reload flag written by built-in menu actions
                            let conf_logs: Vec<String> = self.state.pending_confirmation_logs.drain(..).collect();
                            if !conf_logs.is_empty() {
                                for log in conf_logs {
                                    let level = if log.contains("[NG]") || log.contains("[FAIL]") {
                                        crate::ui::task_panel::LogLevel::Fail
                                    } else if log.contains("[Skipped]") {
                                        crate::ui::task_panel::LogLevel::Warn
                                    } else {
                                        crate::ui::task_panel::LogLevel::Info
                                    };
                                    self.task_panel.add_log(log, level);
                                }
                                let h = self.state.ui.layout.task_panel_height;
                                self.task_panel.scroll_to_end(h);
                            }
                            if self.state.confirmation_needs_keybinding_reload {
                                self.state.confirmation_needs_keybinding_reload = false;
                                let kb_path = rwf_lib::config::ConfigManager::new().keybindings_path().to_path_buf();
                                for warning in rwf_lib::check_keybindings_duplicates(&kb_path) {
                                    tracing::warn!("{}", warning);
                                    self.task_panel.add_log(warning, crate::ui::task_panel::LogLevel::Warn);
                                }
                                if let Ok(kb) = rwf_lib::input::KeyBindings::load_from_file(&kb_path) {
                                    self.key_bindings = kb.clone();
                                    self.state.config.key_bindings = kb;
                                }
                            }
                            if let Some(job_spec) = confirmed_job {
                                // For Delete jobs confirmed via dialog, register background job for task panel logs
                                if let JobKind::Delete { ref targets } = job_spec.kind {
                                    let job_name = crate::ui::dialog::delete_job_name(targets);
                                    let tab_id = self.state.tabs.active_index;
                                    let tab_name = format!("{}|{}",
                                        self.state.current_tab().left_pane.current_location.display_path(),
                                        self.state.current_tab().right_pane.current_location.display_path()
                                    );
                                    let bg_id = self.state.background_jobs.start_job(
                                        job_name.clone(), job_name.clone(), tab_id, tab_name, job_spec.clone()
                                    );
                                    self.task_panel.add_pending_log(format!(
                                        "{} [Job {}] [Tab {}] {}: Started",
                                        chrono::Local::now().format("[%H:%M:%S]"),
                                        bg_id.short_id, tab_id + 1, job_name
                                    ));
                                    let h = self.state.ui.layout.task_panel_height;
                                    self.task_panel.scroll_to_end(h);
                                }
                                self.state.jobs.start_job(job_spec.clone());
                                if let Some(ref pool) = self.worker_pool { pool.submit_job(job_spec); }
                            }
                        }
                    }
                    if should_pop { self.state.dialogs.pop(); }
                    // Confirming a menu item also closes the underlying CustomFunctionSelector
                    if is_function_menu {
                        if let Some(d) = self.state.dialogs.current() {
                            if matches!(&d.content, rwf_lib::model::dialog::DialogContent::CustomFunctionSelector { .. }) {
                                self.state.dialogs.pop();
                            }
                        }
                    }
                    return true;
                }
                crate::ui::dialog::DialogAction::DeleteSelected => {
                    if let Some(log_msg) = crate::ui::dialog::process_dialog_delete(&mut self.state) {
                        self.task_panel.add_log(log_msg, crate::ui::task_panel::LogLevel::Info);
                        let h = self.state.ui.layout.task_panel_height;
                        self.task_panel.scroll_to_end(h);
                    }
                    return true;
                }
                crate::ui::dialog::DialogAction::ConfirmAll => {
                    // Shift+Enter: all decisions already pushed by handle_file_conflict_input; submit job now
                    if let rwf_lib::model::dialog::DialogContent::FileConflict { decisions, .. } = &mut dialog.content {
                        if let Some((job_spec, conflicts_list, job_name, job_desc)) = self.pending_conflict_job.take() {
                            let conflict_decisions: Vec<_> = conflicts_list.iter().zip(decisions.iter()).map(|(c, a)| rwf_lib::job::ConflictDecision {
                                source: c.source_path.clone(), dest: c.dest_path.clone(),
                                action: match a {
                                    rwf_lib::model::dialog::ConflictAction::Force => rwf_lib::job::ConflictAction::Force,
                                    rwf_lib::model::dialog::ConflictAction::OverwriteIfNewer => rwf_lib::job::ConflictAction::OverwriteIfNewer,
                                    rwf_lib::model::dialog::ConflictAction::Skip => rwf_lib::job::ConflictAction::Skip,
                                    rwf_lib::model::dialog::ConflictAction::Rename { new_name } => rwf_lib::job::ConflictAction::Rename { new_name: new_name.clone() },
                                },
                            }).collect();
                            let mut final_job = job_spec.clone();
                            final_job.conflict_decisions = Some(conflict_decisions);
                            let tab_id = self.state.tabs.active_index;
                            let tab_name = format!("{}|{}", self.state.current_tab().left_pane.current_location.display_path(), self.state.current_tab().right_pane.current_location.display_path());
                            let _bg_job_id = self.state.background_jobs.start_job(job_name.clone(), job_desc, tab_id, tab_name, final_job.clone());
                            self.state.jobs.start_job(final_job.clone());
                            if let Some(ref pool) = self.worker_pool { pool.submit_job(final_job); }
                        }
                    }
                    self.state.dialogs.pop();
                    return true;
                }
                crate::ui::dialog::DialogAction::PatternChanged => {
                    // Stash current fields for debounced preview regeneration
                    if let Some(dialog) = self.state.dialogs.current() {
                        if let rwf_lib::model::dialog::DialogContent::PatternRename {
                            find, replace, use_regex, case_sensitive, ..
                        } = &dialog.content {
                            self.pattern_rename_pending = Some((find.clone(), replace.clone(), *use_regex, *case_sensitive));
                            self.pattern_rename_dirty = true;
                            self.pattern_rename_last_changed = Some(Instant::now());
                        }
                    }
                    return true;
                }
                crate::ui::dialog::DialogAction::RotateLanguage => {
                    rwf_lib::state::update_state(&mut self.state, rwf_lib::state::Transition::RotateHelpLanguage);
                    return true;
                }
                crate::ui::dialog::DialogAction::OpenMenu { title, items } => {
                    let menu_dialog = rwf_lib::model::dialog::Dialog::custom_function_menu(title, items);
                    self.state.dialogs.push(menu_dialog);
                    return true;
                }
                _ => return true,
            }
        }

        // 2. Viewer mode handling
        if self.state.ui.mode == rwf_lib::model::UIMode::Viewer
            || self.state.ui.mode == rwf_lib::model::UIMode::ViewerSearch
            || self.state.ui.mode == rwf_lib::model::UIMode::ViewerCommand
        {
            // 2.0 "v"/"V"/Tab — viewer layout cycling (handled before search/command sub-modes)
            if self.state.ui.mode == rwf_lib::model::UIMode::Viewer {
                use rwf_lib::model::ViewerLayout;
                let layout = self.state.ui.layout.viewer_layout;
                match key_string.as_str() {
                    "v" => {
                        match layout {
                            ViewerLayout::SideBySide => {
                                rwf_lib::state::update_state(&mut self.state,
                                    Transition::ViewerSwitchLayout { layout: ViewerLayout::FullScreen });
                            }
                            ViewerLayout::FullScreen => {
                                rwf_lib::state::update_state(&mut self.state, Transition::CloseViewer);
                            }
                        }
                        return true;
                    }
                    "V" => {
                        match layout {
                            ViewerLayout::FullScreen => {
                                rwf_lib::state::update_state(&mut self.state,
                                    Transition::ViewerSwitchLayout { layout: ViewerLayout::SideBySide });
                            }
                            ViewerLayout::SideBySide => {
                                rwf_lib::state::update_state(&mut self.state, Transition::CloseViewer);
                            }
                        }
                        return true;
                    }
                    "Tab" | "Shift+Tab" if layout == ViewerLayout::SideBySide => {
                        self.state.ui.mode = rwf_lib::model::UIMode::Normal;
                        self.refresh_sbs_preview();
                        return true;
                    }
                    _ => {}
                }
            }

            // 2a. ViewerSearch: typing builds the query and searches incrementally
            if self.state.ui.mode == rwf_lib::model::UIMode::ViewerSearch {
                use crossterm::event::{KeyCode, KeyModifiers};
                match key.code {
                    KeyCode::Esc => {
                        self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                        self.state.viewer_search_input.clear();
                        rwf_lib::state::update_state(&mut self.state, Transition::ViewerClearSearch);
                        return true;
                    }
                    KeyCode::Enter => {
                        // Commit — stay in Viewer mode with current results.
                        self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                        return true;
                    }
                    KeyCode::Backspace => {
                        self.state.viewer_search_input.pop();
                        let query = self.state.viewer_search_input.clone();
                        let result = rwf_lib::state::update_state(&mut self.state, Transition::ViewerStartSearch { query });
                        for job in result.jobs_to_start { self.pending_job_submission.push(job); }
                        return true;
                    }
                    // Ctrl+~ or Ctrl+^ toggles case sensitivity while typing
                    KeyCode::Char('~') | KeyCode::Char('^')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        rwf_lib::state::update_state(&mut self.state, Transition::ViewerToggleCaseSensitive);
                        return true;
                    }
                    KeyCode::Char(c) => {
                        self.state.viewer_search_input.push(c);
                        let query = self.state.viewer_search_input.clone();
                        let result = rwf_lib::state::update_state(&mut self.state, Transition::ViewerStartSearch { query });
                        for job in result.jobs_to_start { self.pending_job_submission.push(job); }
                        return true;
                    }
                    _ => return false,
                }
            }

            // 2b. ViewerCommand: line-jump (less-style "100g")
            if self.state.ui.mode == rwf_lib::model::UIMode::ViewerCommand {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Esc => {
                        self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                        self.state.viewer_command_input.clear();
                        return true;
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        self.state.viewer_command_input.push(c);
                        return true;
                    }
                    KeyCode::Backspace => {
                        self.state.viewer_command_input.pop();
                        if self.state.viewer_command_input.is_empty() {
                            self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                        }
                        return true;
                    }
                    KeyCode::Char('g') | KeyCode::Char('<') => {
                        let vp = self.state.ui.layout.pane_height;
                        let tr = if let Ok(n) = self.state.viewer_command_input.parse::<usize>() {
                            Transition::ViewerJumpToLine { line_idx: n.saturating_sub(1), viewport_height: vp }
                        } else {
                            Transition::ViewerJumpToTop
                        };
                        rwf_lib::state::update_state(&mut self.state, tr);
                        self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                        self.state.viewer_command_input.clear();
                        return true;
                    }
                    KeyCode::Char('G') | KeyCode::Char('>') => {
                        let vp = self.state.ui.layout.pane_height;
                        let tr = if let Ok(n) = self.state.viewer_command_input.parse::<usize>() {
                            Transition::ViewerJumpToLine { line_idx: n.saturating_sub(1), viewport_height: vp }
                        } else {
                            Transition::ViewerJumpToBottom { viewport_height: vp }
                        };
                        rwf_lib::state::update_state(&mut self.state, tr);
                        self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                        self.state.viewer_command_input.clear();
                        return true;
                    }
                    _ => return false,
                }
            }

            // 2c. Normal viewer actions
            let vp_height = self.state.ui.layout.pane_height;

            // Digit or ':' enters command mode.
            if self.state.ui.mode == rwf_lib::model::UIMode::Viewer {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        self.state.viewer_command_input.clear();
                        self.state.viewer_command_input.push(c);
                        self.state.ui.mode = rwf_lib::model::UIMode::ViewerCommand;
                        return true;
                    }
                    KeyCode::Char(':') => {
                        self.state.viewer_command_input.clear();
                        self.state.ui.mode = rwf_lib::model::UIMode::ViewerCommand;
                        return true;
                    }
                    _ => {}
                }
            }

            // Remaining normal viewer actions (keybinding lookup)
            let vp_height = vp_height;
            if let Some(action) = self.key_bindings.viewer_mode.get(&key_string).cloned() {
                use rwf_lib::input::Action;
                let tr = match action {
                    Action::ViewerClose => Some(Transition::CloseViewer),
                    Action::ViewerToggleHexMode => Some(Transition::ViewerToggleMode),
                    Action::ViewerScrollDown => Some(Transition::ViewerScrollDown { viewport_height: vp_height }),
                    Action::ViewerScrollUp => Some(Transition::ViewerScrollUp),
                    Action::ViewerPageDown => Some(Transition::ViewerPageDown { viewport_height: vp_height }),
                    Action::ViewerPageUp => Some(Transition::ViewerPageUp { viewport_height: vp_height }),
                    Action::ViewerGoToTop => Some(Transition::ViewerJumpToTop),
                    Action::ViewerGoToBottom => Some(Transition::ViewerJumpToBottom { viewport_height: vp_height }),
                    Action::ViewerClearSearch => Some(Transition::ViewerClearSearch),
                    Action::ViewerCycleEncoding => Some(Transition::ViewerCycleEncoding),
                    Action::ViewerBeginSearch => {
                        if let Some(ref mut viewer) = self.state.viewer {
                            viewer.search_forward = true;
                        }
                        self.state.ui.mode = rwf_lib::model::UIMode::ViewerSearch;
                        self.state.viewer_search_input.clear();
                        return true;
                    }
                    Action::ViewerBeginSearchBackward => {
                        if let Some(ref mut viewer) = self.state.viewer {
                            viewer.search_forward = false;
                        }
                        self.state.ui.mode = rwf_lib::model::UIMode::ViewerSearch;
                        self.state.viewer_search_input.clear();
                        return true;
                    }
                    Action::ViewerFindNext => Some(Transition::ViewerFindNext),
                    Action::ViewerFindPrev => Some(Transition::ViewerFindPrev),
                    Action::ViewerToggleCaseSensitive => Some(Transition::ViewerToggleCaseSensitive),
                    Action::ViewerScrollLeft  => Some(Transition::ViewerScrollLeft  { cols: 1 }),
                    Action::ViewerScrollRight => Some(Transition::ViewerScrollRight { cols: 1 }),
                    Action::ViewerFastScrollLeft  => Some(Transition::ViewerScrollLeft  { cols: 10 }),
                    Action::ViewerFastScrollRight => Some(Transition::ViewerScrollRight { cols: 10 }),
                    Action::ViewerFastScrollUp   => Some(Transition::ViewerFastScrollUp   { lines: 10 }),
                    Action::ViewerFastScrollDown => Some(Transition::ViewerFastScrollDown { lines: 10, viewport_height: vp_height }),
                    _ => None,
                };
                if let Some(t) = tr {
                    rwf_lib::state::update_state(&mut self.state, t);
                    return true;
                }
            }
            return false;
        }

        // 3. Search mode handling
        if self.state.ui.mode == rwf_lib::model::UIMode::Search {
            use crossterm::event::KeyCode;
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.state.ui.mode = rwf_lib::model::UIMode::Normal;
                    if key.code == KeyCode::Esc { self.state.search.clear(); }
                    self.search_dirty = false;
                    self.last_search_input_time = None;
                    return true;
                }
                KeyCode::Backspace | KeyCode::Char(_) => {
                    if key.code == KeyCode::Backspace { self.state.search.query.pop(); }
                    else if let KeyCode::Char(c) = key.code { self.state.search.query.push(c); }
                    self.last_search_input_time = Some(Instant::now());
                    self.search_dirty = true;
                    return true;
                }
                KeyCode::Down | KeyCode::Up => {
                    let pane = self.state.active_pane();
                    let query = &self.state.search.query;
                    let target = if key.code == KeyCode::Down {
                        self.state.search.find_next_index(&pane.entries, pane.cursor + 1, query)
                    } else {
                        self.state.search.find_prev_index(&pane.entries, pane.cursor.saturating_sub(1), query)
                    };
                    if let Some(idx) = target {
                        self.state.active_pane_mut().cursor = idx;
                        let height = self.state.ui.layout.pane_height;
                        self.state.active_pane_mut().update_scroll(height, 3);
                    }
                    return true;
                }
                _ => {}
            }
        }

        // 3.7 Leap mode handling
        if self.state.ui.mode == rwf_lib::model::UIMode::Leap {
            use crossterm::event::KeyCode;
            use rwf_lib::input::Action;

            if let Some(action) = self.key_bindings.lookup_leap_action(&key_string) {
                match action {
                    Action::LeapConfirm => {
                        rwf_lib::state::update_state(&mut self.state, rwf_lib::state::Transition::LeapConfirm);
                        self.leap_dirty = false;
                        self.last_leap_input_time = None;
                        return true;
                    }
                    Action::LeapCancel => {
                        if let Some(leap) = self.state.leap.as_ref().cloned() {
                            let loc = rwf_lib::model::Location::Local(leap.root_dir.clone());
                            let pane = self.state.ui.active_pane;
                            let job_result = rwf_lib::state::update_state(
                                &mut self.state,
                                rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
                            );
                            for job_spec in job_result.jobs_to_start {
                                self.pending_job_submission.push(job_spec);
                            }
                        }
                        rwf_lib::state::update_state(&mut self.state, rwf_lib::state::Transition::LeapCancel);
                        self.leap_dirty = false;
                        self.last_leap_input_time = None;
                        return true;
                    }
                    Action::LeapGoDeeperOrOpen => {
                        let entry = self.state.active_pane().current_entry().cloned();
                        if let Some(entry) = entry {
                            if entry.is_dir {
                                let new_dir = self.state.active_pane()
                                    .current_location.path()
                                    .map(|p| p.join(&entry.name))
                                    .unwrap_or_default();
                                if let Some(ref mut l) = self.state.leap {
                                    l.push_separator(new_dir.clone());
                                    l.last_valid_buffer = l.buffer.clone();
                                }
                                let loc = rwf_lib::model::Location::Local(new_dir);
                                let pane = self.state.ui.active_pane;
                                let job_result = rwf_lib::state::update_state(
                                    &mut self.state,
                                    rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
                                );
                                for job_spec in job_result.jobs_to_start {
                                    self.pending_job_submission.push(job_spec);
                                }
                                self.leap_dirty = false;
                                self.last_leap_input_time = None;
                            } else {
                                // Select file and exit leap
                                rwf_lib::state::update_state(&mut self.state, rwf_lib::state::Transition::LeapConfirm);
                                self.leap_dirty = false;
                                self.last_leap_input_time = None;
                            }
                        }
                        return true;
                    }
                    Action::LeapOpenFile => {
                        let entry = self.state.active_pane().current_entry().cloned();
                        if let Some(entry) = entry {
                            if entry.is_dir {
                                let new_dir = self.state.active_pane()
                                    .current_location.path()
                                    .map(|p| p.join(&entry.name))
                                    .unwrap_or_default();
                                if let Some(ref mut l) = self.state.leap {
                                    l.push_separator(new_dir.clone());
                                    l.last_valid_buffer = l.buffer.clone();
                                }
                                let loc = rwf_lib::model::Location::Local(new_dir);
                                let pane = self.state.ui.active_pane;
                                let job_result = rwf_lib::state::update_state(
                                    &mut self.state,
                                    rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
                                );
                                for job_spec in job_result.jobs_to_start {
                                    self.pending_job_submission.push(job_spec);
                                }
                                self.leap_dirty = false;
                                self.last_leap_input_time = None;
                            } else {
                                // Exit leap then trigger EnterDirectory on the selected file
                                rwf_lib::state::update_state(&mut self.state, rwf_lib::state::Transition::LeapConfirm);
                                self.leap_dirty = false;
                                self.last_leap_input_time = None;
                                let transitions = rwf_lib::input::action_to_transitions(&self.state, &Action::EnterDirectory);
                                for tr in transitions {
                                    let result = rwf_lib::state::update_state(&mut self.state, tr);
                                    for job_spec in result.jobs_to_start {
                                        self.pending_job_submission.push(job_spec);
                                    }
                                }
                            }
                        }
                        return true;
                    }
                    Action::LeapGoParent => {
                        let has_depth = self.state.leap.as_ref()
                            .map_or(false, |l| l.buffer.contains('/'));
                        if has_depth {
                            rwf_lib::state::update_state(&mut self.state, rwf_lib::state::Transition::LeapGoParent);
                            let parent = self.state.active_pane()
                                .current_location.path()
                                .and_then(|p| p.parent())
                                .map(|p| p.to_path_buf());
                            if let Some(parent) = parent {
                                let loc = rwf_lib::model::Location::Local(parent);
                                let pane = self.state.ui.active_pane;
                                let job_result = rwf_lib::state::update_state(
                                    &mut self.state,
                                    rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
                                );
                                for job_spec in job_result.jobs_to_start {
                                    self.pending_job_submission.push(job_spec);
                                }
                            }
                        }
                        return true;
                    }
                    Action::LeapCursorUp => {
                        let pane = self.state.active_pane_mut();
                        if pane.cursor > 0 { pane.cursor -= 1; }
                        let h = self.state.ui.layout.pane_height;
                        self.state.active_pane_mut().update_scroll(h, 3);
                        return true;
                    }
                    Action::LeapCursorDown => {
                        let len = self.state.active_pane().entries.len();
                        let pane = self.state.active_pane_mut();
                        if pane.cursor + 1 < len { pane.cursor += 1; }
                        let h = self.state.ui.layout.pane_height;
                        self.state.active_pane_mut().update_scroll(h, 3);
                        return true;
                    }
                    Action::LeapClearLocal => {
                        if let Some(ref mut l) = self.state.leap { l.clear_local(); }
                        self.leap_dirty = true;
                        self.last_leap_input_time = Some(Instant::now());
                        return true;
                    }
                    Action::LeapClearAll => {
                        let root = self.state.leap.as_ref().map(|l| l.root_dir.clone());
                        if let Some(ref mut l) = self.state.leap { l.clear_all(); }
                        if let Some(root) = root {
                            let loc = rwf_lib::model::Location::Local(root);
                            let pane = self.state.ui.active_pane;
                            let job_result = rwf_lib::state::update_state(
                                &mut self.state,
                                rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
                            );
                            for job_spec in job_result.jobs_to_start {
                                self.pending_job_submission.push(job_spec);
                            }
                        }
                        self.leap_dirty = true;
                        self.last_leap_input_time = Some(Instant::now());
                        return true;
                    }
                    _ => {}
                }
            }

            // Backspace and character input
            match key.code {
                KeyCode::Backspace => {
                    let result = self.state.leap.as_mut()
                        .map(|l| l.backspace())
                        .unwrap_or(rwf_lib::model::BackspaceResult::Empty);
                    match result {
                        rwf_lib::model::BackspaceResult::GoToParent => {
                            let parent = self.state.active_pane()
                                .current_location.path()
                                .and_then(|p| p.parent())
                                .map(|p| p.to_path_buf());
                            if let Some(parent) = parent {
                                let loc = rwf_lib::model::Location::Local(parent);
                                let pane = self.state.ui.active_pane;
                                let job_result = rwf_lib::state::update_state(
                                    &mut self.state,
                                    rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
                                );
                                for job_spec in job_result.jobs_to_start {
                                    self.pending_job_submission.push(job_spec);
                                }
                            }
                            self.leap_dirty = true;
                            self.last_leap_input_time = Some(Instant::now());
                        }
                        rwf_lib::model::BackspaceResult::PopChar => {
                            self.leap_dirty = true;
                            self.last_leap_input_time = Some(Instant::now());
                        }
                        rwf_lib::model::BackspaceResult::Empty => {}
                    }
                    return true;
                }
                KeyCode::Char(c) => {
                    if let Some(ref mut l) = self.state.leap { l.push_char(c); }
                    self.leap_dirty = true;
                    self.last_leap_input_time = Some(Instant::now());
                    return true;
                }
                _ => {}
            }
            return true; // consume all unhandled keys in leap mode
        }

        // 3.5 SideBySide focus: Tab toggles focus to viewer; Esc closes SideBySide.
        if self.state.ui.mode == rwf_lib::model::UIMode::Normal
            && self.state.viewer.is_some()
            && self.state.ui.layout.viewer_layout == rwf_lib::model::ViewerLayout::SideBySide
        {
            match key_string.as_str() {
                "Tab" | "Shift+Tab" => {
                    self.state.ui.mode = rwf_lib::model::UIMode::Viewer;
                    return true;
                }
                "Escape" => {
                    // Close the SideBySide viewer; restore both file panes.
                    rwf_lib::state::update_state(&mut self.state, Transition::CloseViewer);
                    return true;
                }
                _ => {}
            }
        }

        // 3.6 Viewer open/cycle: "v" and "V" in normal mode
        if self.state.ui.mode == rwf_lib::model::UIMode::Normal
            && (key_string == "v" || key_string == "V")
        {
            use rwf_lib::model::ViewerLayout;
            if self.state.viewer.is_some() {
                // Viewer already open: "v" closes (FullScreen) or switches to FullScreen (SideBySide).
                // "V" closes (SideBySide) or switches to SideBySide (FullScreen).
                let layout = self.state.ui.layout.viewer_layout;
                if key_string == "v" {
                    match layout {
                        ViewerLayout::SideBySide => {
                            rwf_lib::state::update_state(&mut self.state,
                                Transition::ViewerSwitchLayout { layout: ViewerLayout::FullScreen });
                        }
                        ViewerLayout::FullScreen => {
                            rwf_lib::state::update_state(&mut self.state, Transition::CloseViewer);
                        }
                    }
                } else {
                    match layout {
                        ViewerLayout::FullScreen => {
                            rwf_lib::state::update_state(&mut self.state,
                                Transition::ViewerSwitchLayout { layout: ViewerLayout::SideBySide });
                        }
                        ViewerLayout::SideBySide => {
                            rwf_lib::state::update_state(&mut self.state, Transition::CloseViewer);
                        }
                    }
                }
            } else {
                // No viewer: open one. "v" uses preferred layout; "V" forces SideBySide.
                // Binary files open in Hex mode by default; everything else in Text mode.
                if let Some(entry) = self.state.active_pane().current_entry().cloned() {
                    if !entry.is_dir {
                        let location = entry.location.clone();
                        let mode = Self::default_viewer_mode(&location);
                        let is_sbs = key_string == "V"; // "v" = always FullScreen; "V" = always SideBySide
                        let tr = if is_sbs {
                            Transition::OpenSideBySideViewer { location, mode }
                        } else {
                            match mode {
                                rwf_lib::model::ViewerMode::Hex  => Transition::OpenHexViewer { location },
                                rwf_lib::model::ViewerMode::Text => Transition::OpenTextViewer { location },
                            }
                        };
                        let result = rwf_lib::state::update_state(&mut self.state, tr);
                        for job_spec in result.jobs_to_start {
                            self.pending_job_submission.push(job_spec);
                        }
                    }
                }
            }
            return true;
        }

        // 4. Normal key handling (transitions)
        if let Some(action) = self.key_bindings.map_key(&key) {
            use rwf_lib::input::Action;
            // In SideBySide mode, pane-switching is disabled: the anchored file pane
            // stays on its side for the duration of the SideBySide session.
            if self.state.viewer.is_some()
                && self.state.ui.layout.viewer_layout == rwf_lib::model::ViewerLayout::SideBySide
                && matches!(action, Action::SwitchPane | Action::SwitchToLeftPane | Action::SwitchToRightPane)
            {
                return false;
            }
            tracing::info!("[KEY] action={:?}", action);
            // EnterLeap: enter leap navigation mode
            if action == Action::EnterLeap {
                if self.state.config.jump_nav.leap_enabled {
                    let root_dir = self.state.active_pane().current_location.path()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_default();
                    let root_cursor = self.state.active_pane().cursor;
                    rwf_lib::state::update_state(
                        &mut self.state,
                        rwf_lib::state::Transition::EnterLeap { root_dir, root_cursor },
                    );
                }
                return true;
            }
            // ShowVersionInfo / ShowVersionInfoVerbose: write to task panel, no state change
            if action == rwf_lib::input::Action::ShowVersionInfo {
                self.log_version_info();
                return true;
            }
            if action == rwf_lib::input::Action::ShowVersionInfoVerbose {
                self.log_version_info_verbose();
                return true;
            }
            // Task panel log scroll — operates on the widget, not app state
            if action == Action::ScrollTaskPanelUp {
                self.task_panel.scroll_up();
                return true;
            }
            if action == Action::ScrollTaskPanelDown {
                let h = self.state.ui.layout.task_panel_height;
                self.task_panel.scroll_down(h);
                return true;
            }
            let is_reload_config = action == rwf_lib::input::Action::ReloadConfig;
            // Reload keybindings BEFORE the ReloadConfig transition so the state-generated
            // log includes the updated keybindings status.
            if is_reload_config {
                let kb_path = rwf_lib::config::ConfigManager::new().keybindings_path().to_path_buf();
                let kb_exists = kb_path.exists();
                for warning in rwf_lib::check_keybindings_duplicates(&kb_path) {
                    tracing::warn!("{}", warning);
                    self.task_panel.add_log(warning, crate::ui::task_panel::LogLevel::Warn);
                }
                let (new_kb, kb_result) = match rwf_lib::input::KeyBindings::load_from_file(&kb_path) {
                    Ok(kb) => {
                        tracing::info!("Keybindings reloaded from {:?}", kb_path);
                        (kb, rwf_lib::config::ConfigLoadResult::ok(kb_path))
                    }
                    Err(e) => {
                        let result = if kb_exists {
                            tracing::warn!("Failed to reload {:?}, using built-in defaults: {:?}", kb_path, e);
                            rwf_lib::config::ConfigLoadResult::error(kb_path, e.to_string())
                        } else {
                            tracing::info!("Keybindings file not found at {:?}, using built-in defaults", kb_path);
                            rwf_lib::config::ConfigLoadResult::default_fallback(kb_path, "built-in defaults")
                        };
                        (rwf_lib::KeyBindings::default(), result)
                    }
                };
                self.key_bindings = new_kb.clone();
                self.state.config.key_bindings = new_kb;
                if self.state.config_load_results.len() > 1 {
                    self.state.config_load_results[1] = kb_result;
                }
            }
            let transitions = rwf_lib::input::action_to_transitions(&self.state, &action);
            tracing::info!("[KEY] transitions={}", transitions.len());
            let mut state_changed = false;
            for tr in transitions {
                if matches!(tr, Transition::Quit) { self.should_quit = true; return true; }
                if matches!(tr, Transition::ExitAndChangeDirectory) { self.should_exit_and_cd = true; self.should_quit = true; return true; }
                let result = rwf_lib::state::update_state(&mut self.state, tr);
                for job_spec in result.jobs_to_start {
                    self.pending_job_submission.push(job_spec);
                }
                if !result.task_panel_logs.is_empty() {
                    for log_msg in result.task_panel_logs {
                        let level = if log_msg.contains("[NG]") || log_msg.contains("[FAIL]") {
                            crate::ui::task_panel::LogLevel::Fail
                        } else if log_msg.contains("[WARN]") {
                            crate::ui::task_panel::LogLevel::Warn
                        } else {
                            crate::ui::task_panel::LogLevel::Info
                        };
                        self.task_panel.add_log(log_msg, level);
                    }
                    let h = self.state.ui.layout.task_panel_height;
                    self.task_panel.scroll_to_end(h);
                    state_changed = true;
                }
                state_changed = state_changed || result.ui_changed;
            }

            if self.refresh_sbs_preview() { state_changed = true; }
            return state_changed;
        }
        tracing::info!("[KEY] no action mapped for {:?}", key_string);
        false
    }

    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let size = terminal.size()?;
        let tab_h = if self.state.ui.layout.show_tab_bar { 1 } else { 0 };
        let task_h = if self.state.ui.layout.show_task_panel { self.state.ui.layout.task_panel_height as u16 } else { 0 };
        let pane_h = size.height.saturating_sub(tab_h + task_h + 4) as usize;
        let pane_w = size.width as usize;

        if self.state.ui.layout.pane_height != pane_h {
            let _ = rwf_lib::state::update_state(&mut self.state, Transition::UpdatePaneHeight { height: pane_h });
        }
        if self.state.ui.layout.pane_width != pane_w {
            let _ = rwf_lib::state::update_state(&mut self.state, Transition::UpdatePaneWidth { width: pane_w });
        }
        terminal.draw(|f| render_ui(f, &self.state, &self.task_panel))?;
        Ok(())
    }

    /// Called whenever the cursor might have moved in SideBySide file-pane mode.
    /// For files: reloads the viewer if the location changed.
    /// Directory preview counts are computed inline in render_ui (no state needed).
    /// Returns true if a state change requires a redraw.
    fn refresh_sbs_preview(&mut self) -> bool {
        if self.state.viewer.is_none()
            || self.state.ui.layout.viewer_layout != rwf_lib::model::ViewerLayout::SideBySide
            || self.state.ui.mode != rwf_lib::model::UIMode::Normal
        {
            return false;
        }

        let anchor = self.state.ui.layout.viewer_anchor_pane;
        let entry = {
            let tab = self.state.current_tab();
            match anchor {
                rwf_lib::model::ActivePane::Left  => tab.left_pane.current_entry().cloned(),
                rwf_lib::model::ActivePane::Right => tab.right_pane.current_entry().cloned(),
            }
        };

        let Some(entry) = entry else { return false; };

        if entry.is_dir {
            // Dir preview is rendered inline in render_ui — just signal a redraw.
            true
        } else {
            // File: reload viewer if the location changed, auto-selecting mode by extension.
            let new_loc = entry.location.clone();
            let current_loc = self.state.viewer.as_ref().map(|v| v.location.clone());
            if current_loc.as_ref() == Some(&new_loc) {
                return false;
            }
            let mode = Self::default_viewer_mode(&new_loc);
            let result = rwf_lib::state::update_state(
                &mut self.state, Transition::ReloadViewer { location: new_loc, mode },
            );
            for job_spec in result.jobs_to_start {
                self.pending_job_submission.push(job_spec);
            }
            true
        }
    }

    /// Choose the default viewer mode for a file based on its extension.
    /// Binary/media files open in Hex mode; everything else opens in Text mode.
    fn default_viewer_mode(location: &rwf_lib::model::Location) -> rwf_lib::model::ViewerMode {
        let ext = std::path::Path::new(&location.display_path())
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        let is_binary = matches!(ext.as_str(),
            "mp4" | "mp3" | "avi" | "mov" | "mkv" | "wmv" | "flv" | "m4v" | "m4a" |
            "aac" | "flac" | "wav" | "ogg" | "opus" | "wma" |
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico" |
            "exe" | "dll" | "so" | "dylib" | "pdb" | "lib" | "a" |
            "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "zst" |
            "pdf" | "db" | "sqlite" | "sqlite3" | "iso" | "img" | "dmg"
        );
        if is_binary { rwf_lib::model::ViewerMode::Hex } else { rwf_lib::model::ViewerMode::Text }
    }

    fn perform_leap_filter(&mut self) {
        let leap = match self.state.leap.as_ref() {
            Some(l) => l.clone(),
            None    => return,
        };

        let local_filter = leap.local_filter().to_string();
        let raw_entries  = self.state.active_pane().raw_entries.clone();

        let (filtered, cursor) = rwf_lib::leap_filter::apply_leap_filter(
            &raw_entries,
            &local_filter,
            &self.state.search,
        );

        let filtered_entries: Vec<rwf_lib::model::FileEntry> =
            filtered.into_iter().cloned().collect();

        if filtered_entries.is_empty() && !local_filter.is_empty() {
            match self.state.config.jump_nav.no_match_feedback {
                rwf_lib::config::NoMatchFeedback::TaskPanel => {
                    let valid = self.state.leap.as_ref()
                        .map(|l| l.last_valid_buffer.clone())
                        .unwrap_or_default();
                    let removed: String = leap.buffer.trim_start_matches(valid.as_str()).to_string();
                    if let Some(ref mut l) = self.state.leap {
                        l.buffer = valid;
                    }
                    self.task_panel.add_log(
                        format!("Leap: no match — removed \"{}\"", removed),
                        crate::ui::task_panel::LogLevel::Warn,
                    );
                    // Re-run with the restored buffer
                    self.perform_leap_filter();
                }
                rwf_lib::config::NoMatchFeedback::Inline => {
                    rwf_lib::state::update_state(
                        &mut self.state,
                        rwf_lib::state::Transition::LeapApplyFilter {
                            filtered_entries: Vec::new(),
                            cursor: 0,
                        },
                    );
                }
            }
            return;
        }

        // Single-directory auto-enter
        if filtered_entries.len() == 1 && filtered_entries[0].is_dir && !local_filter.is_empty() {
            let dir_name = filtered_entries[0].name.clone();
            let new_dir = self.state.active_pane()
                .current_location.path()
                .map(|p| p.join(&dir_name))
                .unwrap_or_default();
            if let Some(ref mut l) = self.state.leap {
                l.push_separator(new_dir.clone());
                l.last_valid_buffer = l.buffer.clone();
            }
            let loc = rwf_lib::model::Location::Local(new_dir);
            let pane = self.state.ui.active_pane;
            let job_result = rwf_lib::state::update_state(
                &mut self.state,
                rwf_lib::state::Transition::ChangeLocation { pane, location: loc },
            );
            for job_spec in job_result.jobs_to_start {
                self.pending_job_submission.push(job_spec);
            }
            return;
        }

        // Update last_valid_buffer when we have matches
        if !filtered_entries.is_empty() {
            let buf = self.state.leap.as_ref().map(|l| l.buffer.clone()).unwrap_or_default();
            rwf_lib::state::update_state(
                &mut self.state,
                rwf_lib::state::Transition::LeapUpdateLastValid { buffer: buf },
            );
        }

        rwf_lib::state::update_state(
            &mut self.state,
            rwf_lib::state::Transition::LeapApplyFilter { filtered_entries, cursor },
        );
    }

    fn perform_incremental_search(&mut self) {
        let query = self.state.search.query.clone();
        if query.is_empty() { return; }
        let pane = self.state.active_pane();
        if let Some(m) = self.state.search.find_next_index(&pane.entries, pane.cursor, &query) {
            self.state.active_pane_mut().cursor = m;
            let h = self.state.ui.layout.pane_height;
            self.state.active_pane_mut().update_scroll(h, 3);
        }
    }
}
