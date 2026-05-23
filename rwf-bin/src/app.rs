//! Application main loop
//!
//! This module implements a truly event-driven main loop with 0% idle CPU usage.

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};
use rwf_lib::{AppState, Transition, KeyBindings, WorkerPool, process_pending_events};
use rwf_lib::backend::{LocalFilesystemBackend, ZipArchiveHandler};
use rwf_lib::job::{JobSpec, JobKind};
use rwf_lib::model::dialog::ConflictPair;
use std::io::Stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, debug, info};

use crate::ui::render_ui;
use crate::ui::task_panel::TaskPanel;

/// Application runner
pub struct App {
    state: AppState,
    key_bindings: KeyBindings,
    should_quit: bool,
    should_exit_and_cd: bool,
    worker_pool: Option<WorkerPool<LocalFilesystemBackend, ZipArchiveHandler>>,
    last_key_press: Option<(String, Instant, bool)>, // (key, time, is_repeating)
    task_panel: TaskPanel,
    last_spinner_update: Option<Instant>,
    last_cleanup_check: Option<Instant>,
    pending_conflict_job: Option<(JobSpec, Vec<ConflictPair>, String, String)>,
    pending_job_submission: Vec<JobSpec>,
    // Search control fields
    last_search_input_time: Option<Instant>,
    search_dirty: bool,
}

impl App {
    pub fn with_cwd_flag(state: AppState, _cwd_flag: bool) -> Self {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(ZipArchiveHandler::new());
        let worker_pool = WorkerPool::new(state.config.worker_pool_size, backend, archive_handler);
        let mut task_panel = TaskPanel::new();
        let migemo_status = if state.search.is_migemo_dict_loaded() {
            state.search.migemo_dict_path().map_or("OK".to_string(), |p| format!("OK ({})", p))
        } else {
            "Available (No Dictionary)".to_string()
        };
        task_panel.add_log(format!("LogLevel: {:?} | Migemo: {}", state.config.log_level, migemo_status), crate::ui::task_panel::LogLevel::Info);

        Self {
            state, key_bindings: KeyBindings::default(), should_quit: false, should_exit_and_cd: false,
            worker_pool: Some(worker_pool),
            last_key_press: None, task_panel, last_spinner_update: None, last_cleanup_check: None,
            pending_conflict_job: None, pending_job_submission: Vec::new(),
            last_search_input_time: None, search_dirty: false,
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
                            if refresh.pane == rwf_lib::model::ActivePane::Left {
                                self.state.tabs.tabs[tab_idx].left_pane.is_loading = true;
                                self.state.tabs.tabs[tab_idx].left_pane.active_job_id = Some(job.id);
                            } else {
                                self.state.tabs.tabs[tab_idx].right_pane.is_loading = true;
                                self.state.tabs.tabs[tab_idx].right_pane.active_job_id = Some(job.id);
                            }
                            self.state.jobs.start_job(job.clone());
                            pool.submit_job(job);
                        }
                        for job_spec in &result.jobs_to_start { pool.submit_job(job_spec.clone()); }
                    }
                }
            }

            // 2. Process pending job submissions (from transitions)
            let pending_jobs: Vec<JobSpec> = self.pending_job_submission.drain(..).collect();
            if !pending_jobs.is_empty() { ui_needs_update = true; }
            for job_spec in pending_jobs {
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
                                self.pending_conflict_job = Some((job_spec, conflicts.clone(), job_name.clone(), job_name));
                                self.state.dialogs.push(rwf_lib::model::Dialog::file_conflict(conflicts, 0, self.state.config.text_input.edit_mode));
                            }
                        }
                        _ => {
                            tracing::info!("[AppLoop] Submitting pending job: id={:?}, kind={:?}", job_spec.id, job_spec.kind);
                            self.state.jobs.start_job(job_spec.clone());
                            pool.submit_job(job_spec);
                        },
                    }
                }
            }

            // 3. Process logs
            if self.task_panel.pending_log_count() > 0 {
                self.task_panel.process_pending_logs(self.state.config.job_manager.max_task_panel_log_lines);
                ui_needs_update = true;
            }

            // 4. Intelligent Ticking (Spinner)
            if self.has_active_jobs() {
                let interval = Duration::from_millis(self.state.config.job_manager.task_panel_refresh_interval_ms);
                if self.last_spinner_update.map_or(true, |l| l.elapsed() >= interval) {
                    self.task_panel.tick();
                    self.last_spinner_update = Some(Instant::now());
                    ui_needs_update = true;
                }
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

            if self.has_active_jobs() {
                let interval = Duration::from_millis(self.state.config.job_manager.task_panel_refresh_interval_ms);
                if let Some(last_tick) = self.last_spinner_update {
                    next_wakeup = next_wakeup.min(interval.saturating_sub(last_tick.elapsed()));
                } else {
                    next_wakeup = next_wakeup.min(Duration::from_millis(0));
                }
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
                if let Event::Key(key) = ev {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        if self.handle_key_event(key) { any_event = true; }
                    }
                }
                
                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
            }
        }
        Ok(any_event)
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let key_string = rwf_lib::input::format_key_event(&key);
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
            match crate::ui::dialog::handle_dialog_input(dialog, key) {
                crate::ui::dialog::DialogAction::Cancel => {
                    if let rwf_lib::model::dialog::DialogContent::FileConflict { .. } = &dialog.content { self.pending_conflict_job = None; }
                    self.state.dialogs.pop();
                    return true;
                }
                crate::ui::dialog::DialogAction::Confirm => {
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
                                let bg_job_id = self.state.background_jobs.start_job(job_name.clone(), job_desc, tab_id, tab_name, final_job.clone());
                                self.state.jobs.start_job(final_job.clone());
                                if let Some(ref pool) = self.worker_pool { pool.submit_job(final_job); }
                            }
                        }
                        _ => {
                            if let Some(job_spec) = crate::ui::dialog::process_dialog_confirmation(&mut self.state) {
                                self.state.jobs.start_job(job_spec.clone());
                                if let Some(ref pool) = self.worker_pool { pool.submit_job(job_spec); }
                            }
                        }
                    }
                    if should_pop { self.state.dialogs.pop(); }
                    return true;
                }
                _ => return true,
            }
        }

        // 2. Search mode handling
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

        // 3. Normal key handling (transitions)
        if let Some(action) = self.key_bindings.map_key(&key) {
            let transitions = rwf_lib::input::action_to_transitions(&self.state, &action);
            let mut state_changed = false;
            for tr in transitions {
                if matches!(tr, Transition::Quit) { self.should_quit = true; return true; }
                if matches!(tr, Transition::ExitAndChangeDirectory) { self.should_exit_and_cd = true; self.should_quit = true; return true; }
                let result = rwf_lib::state::update_state(&mut self.state, tr);
                for job_spec in result.jobs_to_start {
                    self.pending_job_submission.push(job_spec);
                }
                state_changed = state_changed || result.ui_changed;
            }
            return state_changed;
        }
        false
    }

    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let size = terminal.size()?;
        let tab_h = if self.state.ui.layout.show_tab_bar { 1 } else { 0 };
        let task_h = if self.state.ui.layout.show_task_panel { self.state.ui.layout.task_panel_height as u16 } else { 0 };
        let stat_h = if self.state.ui.layout.show_status_bar { 1 } else { 0 };
        let pane_h = size.height.saturating_sub(tab_h + task_h + stat_h + 4) as usize;
        
        if self.state.ui.layout.pane_height != pane_h {
            let _ = rwf_lib::state::update_state(&mut self.state, Transition::UpdatePaneHeight { height: pane_h });
        }
        terminal.draw(|f| render_ui(f, &self.state, &self.task_panel))?;
        Ok(())
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
