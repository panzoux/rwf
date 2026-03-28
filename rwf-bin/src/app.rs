//! Application main loop
//!
//! This module implements the main event loop with rendering at 30+ FPS.

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};
use rwf_lib::{AppState, Transition, KeyBindings, action_to_transitions, WorkerPool, process_pending_events};
use rwf_lib::backend::{LocalFilesystemBackend, ZipArchiveHandler};
use std::io::Stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};

use crate::performance::PerformanceMetrics;
use crate::ui::render_ui;
use crate::ui::task_panel::TaskPanel;

/// Target frame rate (30 FPS)
const TARGET_FPS: u64 = 30;
const FRAME_DURATION: Duration = Duration::from_millis(1000 / TARGET_FPS);

/// Maximum time to wait for state changes (16ms for 60 FPS responsiveness)
const MAX_RENDER_DELAY: Duration = Duration::from_millis(16);

/// Application runner
pub struct App {
    state: AppState,
    key_bindings: KeyBindings,
    should_quit: bool,
    should_exit_and_cd: bool,
    metrics: PerformanceMetrics,
    last_metrics_log: Instant,
    worker_pool: Option<WorkerPool>,
    last_key_press: Option<(String, Instant, bool)>, // (key, time, is_repeating)
    task_panel: TaskPanel,
}

impl App {
    /// Create a new application instance with cwd flag
    pub fn with_cwd_flag(state: AppState, _cwd_flag: bool) -> Self {
        // Create worker pool with filesystem backend and archive handler
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(ZipArchiveHandler::new());
        let worker_pool = WorkerPool::new(state.config.worker_pool_size, backend, archive_handler);
        
        Self {
            state,
            key_bindings: KeyBindings::default(),
            should_quit: false,
            should_exit_and_cd: false,
            metrics: PerformanceMetrics::new(),
            last_metrics_log: Instant::now(),
            worker_pool: Some(worker_pool),
            last_key_press: None,
            task_panel: TaskPanel::new(),
        }
    }
    
    /// Create a new application instance with custom key bindings
    #[allow(dead_code)]
    pub fn with_key_bindings(state: AppState, key_bindings: KeyBindings) -> Self {
        // Create worker pool with filesystem backend and archive handler
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(ZipArchiveHandler::new());
        let worker_pool = WorkerPool::new(state.config.worker_pool_size, backend, archive_handler);
        
        Self {
            state,
            key_bindings,
            should_quit: false,
            should_exit_and_cd: false,
            metrics: PerformanceMetrics::new(),
            last_metrics_log: Instant::now(),
            worker_pool: Some(worker_pool),
            last_key_press: None,
            task_panel: TaskPanel::new(),
        }
    }

    /// Trigger initial directory reads for all panes in all tabs
    fn trigger_initial_directory_reads(&mut self) {
        info!("Triggering initial directory reads for all panes");
        
        let worker_pool = self.worker_pool.as_ref().expect("Worker pool should exist");
        
        // For each tab, trigger directory reads for both panes
        for tab_index in 0..self.state.tabs.tabs.len() {
            let tab = &self.state.tabs.tabs[tab_index];
            
            // Left pane
            let left_location = tab.left_pane.current_location.clone();
            let left_job = rwf_lib::job::JobSpec::new(
                rwf_lib::job::JobKind::ReadDirectory {
                    location: left_location.clone(),
                }
            );
            info!("Submitting initial read for left pane of tab {}: {}", tab_index, left_location.display_path());
            
            // Enqueue and start the job (it already has a unique UUID)
            self.state.jobs.enqueue(left_job.clone());
            self.state.jobs.start_job(left_job.clone());
            worker_pool.submit_job(left_job);
            
            // Right pane
            let right_location = tab.right_pane.current_location.clone();
            let right_job = rwf_lib::job::JobSpec::new(
                rwf_lib::job::JobKind::ReadDirectory {
                    location: right_location.clone(),
                }
            );
            info!("Submitting initial read for right pane of tab {}: {}", tab_index, right_location.display_path());
            
            // Enqueue and start the job (it already has a unique UUID)
            self.state.jobs.enqueue(right_job.clone());
            self.state.jobs.start_job(right_job.clone());
            worker_pool.submit_job(right_job);
        }
        
        info!("Initial directory read jobs submitted");
    }
    
    /// Get the directory to output on exit (current active pane directory)
    fn get_exit_directory(&self) -> String {
        let active_pane = self.state.active_pane();
        active_pane.current_location.display_path()
    }
    
    /// Check if directory should be output (for use after run() completes)
    pub fn should_output_directory(&self) -> bool {
        self.should_exit_and_cd
    }
    
    /// Get the exit directory (for use after run() completes)
    pub fn get_exit_directory_public(&self) -> String {
        self.get_exit_directory()
    }

    /// Run the main application loop
    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        info!("Starting main application loop");

        // Trigger initial directory reads for both panes
        self.trigger_initial_directory_reads();

        let mut last_render = Instant::now();

        loop {
            // Process any pending job events from worker pool
            let event_results = if let Some(ref mut pool) = self.worker_pool {
                let results = process_pending_events(pool, &mut self.state);
                if !results.is_empty() {
                    info!("Processed {} job events", results.len());
                    for (idx, result) in results.iter().enumerate() {
                        debug!("Event result {}: jobs_to_start={}, ui_changed={}, completed={}, failed={}, cancelled={}, started={}, logs={}",
                            idx, 
                            result.jobs_to_start.len(), 
                            result.ui_changed,
                            result.completed_jobs.len(),
                            result.failed_jobs.len(),
                            result.cancelled_jobs.len(),
                            result.started_jobs.len(),
                            result.task_panel_logs.len()
                        );
                    }
                    
                    // Add task panel log entries from state transitions
                    for result in &results {
                        // Add logs from state transitions (this is the ONLY place logs should be added)
                        for log_msg in &result.task_panel_logs {
                            self.task_panel.add_pending_log(log_msg.clone());
                        }
                    }
                    
                    // Process pane refreshes from completed jobs (Copy/Move/Delete/etc.)
                    for result in &results {
                        for refresh in &result.panes_to_refresh {
                            debug!("Refreshing pane {:?} in tab {}", refresh.pane, refresh.tab_id);
                            
                            // Get the location to refresh
                            let location = {
                                let tab = &self.state.tabs.tabs[refresh.tab_id];
                                match refresh.pane {
                                    rwf_lib::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                                    rwf_lib::model::ActivePane::Right => tab.right_pane.current_location.clone(),
                                }
                            };
                            
                            // Create and submit ReadDirectory job (async, non-blocking)
                            let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ReadDirectory { location });
                            self.state.jobs.start_job(job_spec.clone());
                            if let Some(ref pool) = self.worker_pool {
                                pool.submit_job(job_spec);
                                debug!("Submitted ReadDirectory job for pane refresh");
                            }
                        }
                    }
                }
                results
            } else {
                Vec::new()
            };
            let events_processed = !event_results.is_empty();
            
            // Submit any new jobs that were created by state transitions
            // Note: Jobs are already started in state.rs StartNextJob handler
            // Here we just submit them to the worker pool for execution
            if let Some(ref pool) = self.worker_pool {
                for result in &event_results {
                    for job_spec in &result.jobs_to_start {
                        debug!("App: Submitting job to worker pool job_id={:?} kind={:?}", job_spec.id, job_spec.kind);
                        pool.submit_job(job_spec.clone());
                    }
                }
            }

            // Update spinner animation (throttled to config interval for resource efficiency)
            // File manager doesn't need frequent UI updates - conserve CPU for file operations
            static mut SPINNER_LAST_UPDATE: Option<Instant> = None;
            unsafe {
                let spinner_interval = Duration::from_millis(
                    self.state.config.job_manager.task_panel_refresh_interval_ms
                );
                let should_tick = match SPINNER_LAST_UPDATE {
                    Some(last) => last.elapsed() >= spinner_interval,
                    None => true,
                };
                if should_tick {
                    self.task_panel.tick();
                    SPINNER_LAST_UPDATE = Some(Instant::now());
                }
            }

            // Process pending task panel logs before rendering
            self.task_panel.process_pending_logs(self.state.config.job_manager.max_task_panel_log_lines);

            // Cleanup expired jobs every 1 second (accurate timing)
            static mut LAST_CLEANUP_CHECK: Option<Instant> = None;
            unsafe {
                let check_interval = Duration::from_secs(1);
                let should_check = match LAST_CLEANUP_CHECK {
                    Some(last) => last.elapsed() >= check_interval,
                    None => true,
                };
                if should_check {
                    let cleaned = self.state.background_jobs.cleanup_expired_jobs();
                    if cleaned > 0 {
                        debug!("Cleaned up {} expired jobs", cleaned);
                    }
                    LAST_CLEANUP_CHECK = Some(Instant::now());
                }
            }

            // Calculate time since last render
            let elapsed = last_render.elapsed();
            let time_until_next_frame = FRAME_DURATION.saturating_sub(elapsed);

            // Handle input events with timeout
            if self.handle_events(time_until_next_frame.min(MAX_RENDER_DELAY))? {
                // State changed, render immediately
                self.render(terminal)?;
                last_render = Instant::now();
            } else if events_processed {
                // Job events were processed, render to show updates
                self.render(terminal)?;
                last_render = Instant::now();
            } else if elapsed >= FRAME_DURATION {
                // Time for next frame
                self.render(terminal)?;
                last_render = Instant::now();
            }

            // Log performance metrics every 5 seconds at TRACE level (not DEBUG to reduce noise)
            if self.last_metrics_log.elapsed() >= Duration::from_secs(5) {
                trace!("Performance: {}", self.metrics.summary());

                // Check for performance warnings (still log warnings at WARN level)
                let warnings = self.metrics.check_warnings();
                for warning in warnings {
                    warn!("Performance warning: {}", warning);
                }

                self.last_metrics_log = Instant::now();
            }

            // Check if we should quit
            if self.should_quit {
                info!("Application quit requested");
                
                // Log final performance metrics
                info!("Final performance: {}", self.metrics.summary());

                // Save session state before quitting
                if let Err(e) = self.state.save_session() {
                    tracing::error!("Failed to save session: {}", e);
                } else {
                    info!("Session state saved successfully");
                }

                // Cancel all active jobs before shutdown (so they don't block shutdown)
                debug!("Cancelling all active jobs before shutdown...");
                let active_job_ids: Vec<_> = self.state.background_jobs.get_active_jobs()
                    .map(|j| j.id.uuid)
                    .collect();
                for job_id in &active_job_ids {
                    self.state.background_jobs.cancel_job(*job_id);
                    debug!("Cancelled job {:?}", job_id);
                }
                debug!("Cancelled {} active jobs", active_job_ids.len());

                // Shutdown worker pool
                if let Some(pool) = self.worker_pool.take() {
                    info!("Shutting down worker pool...");
                    pool.shutdown().await;
                    info!("Worker pool shut down");
                }

                break;
            }
        }

        Ok(())
    }

    /// Handle input events with timeout
    /// Returns true if state changed
    fn handle_events(&mut self, timeout: Duration) -> Result<bool> {
        // Poll for events with timeout
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events, ignore release and repeat
                // This prevents duplicate processing when OS sends both Press and Release events
                if key.kind == crossterm::event::KeyEventKind::Press {
                    debug!("Key event: {:?}", key);
                    return Ok(self.handle_key_event(key));
                }
            }
        }

        Ok(false)
    }

    /// Handle a key event
    /// Returns true if state changed
    /// Processes input within 16ms for responsive feedback
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let start = Instant::now();
        
        debug!("Key event: {:?}", key);
        
        // Key repeat logic with proper state tracking:
        // - First press: immediate (no delay)
        // - Same key within 300ms: ignore (debounce)
        // - Same key after 300ms: enter "repeat mode"
        // - In repeat mode: accept every 50ms
        let key_string = rwf_lib::input::format_key_event(&key);
        let now = Instant::now();
        
        if let Some((last_key, last_time, is_repeating)) = &self.last_key_press {
            if last_key == &key_string {
                let elapsed = now.duration_since(*last_time);
                
                if *is_repeating {
                    // Already in repeat mode: use repeat rate (50ms)
                    let repeat_threshold = Duration::from_millis(self.state.config.key_repeat_rate_ms as u64);
                    if elapsed < repeat_threshold {
                        debug!("Key repeat ignored (repeat mode): {} (elapsed: {:?}, threshold: {:?})", 
                               key_string, elapsed, repeat_threshold);
                        return false;
                    }
                    // Accept this repeat - UPDATE last_time so next repeat is 50ms from now
                    debug!("Key repeat accepted (repeat mode): {} (elapsed: {:?})", key_string, elapsed);
                    self.last_key_press = Some((key_string.clone(), now, true));
                } else {
                    // Not yet repeating: check if we should enter repeat mode
                    let initial_delay = Duration::from_millis(self.state.config.key_repeat_delay_ms as u64);
                    if elapsed < initial_delay {
                        debug!("Key repeat ignored (initial delay): {} (elapsed: {:?}, threshold: {:?})", 
                               key_string, elapsed, initial_delay);
                        return false;
                    }
                    // Enter repeat mode - UPDATE last_time so first repeat is 50ms from now
                    debug!("Entering repeat mode for key: {} (elapsed: {:?})", key_string, elapsed);
                    self.last_key_press = Some((key_string.clone(), now, true));
                }
            } else {
                // Different key, reset to initial state
                debug!("Different key pressed: {} (previous: {})", key_string, last_key);
                self.last_key_press = Some((key_string.clone(), now, false));
            }
        } else {
            // First key press ever
            debug!("First key press: {}", key_string);
            self.last_key_press = Some((key_string.clone(), now, false));
        }

        // Special handling for dialog input
        if let Some(dialog) = self.state.dialogs.current_mut() {
            debug!("Dialog is open, handling dialog input");
            // Handle dialog input centrally
            match crate::ui::dialog::handle_dialog_input(dialog, key) {
                crate::ui::dialog::DialogAction::Cancel => {
                    // Close dialog
                    debug!("Dialog action: Cancel");
                    self.state.dialogs.pop();

                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);

                    return true;
                }
                crate::ui::dialog::DialogAction::Confirm => {
                    // Process dialog confirmation
                    debug!("Dialog action: Confirm");
                    // Check dialog type
                    match &dialog.content {
                        rwf_lib::model::dialog::DialogContent::JobManager { focused_field, selected_index } => {
                            let focused_field_copy = *focused_field;
                            let selected_index_copy = *selected_index;

                            if focused_field_copy == 2 {
                                // Terminate Job button focused - cancel the selected job
                                debug!("Terminate Job button activated (focused_field={})", focused_field_copy);
                                let jobs: Vec<rwf_lib::job::BackgroundJob> = self.state.background_jobs.get_all_jobs()
                                    .cloned()
                                    .collect();
                                if let Some(job) = jobs.get(selected_index_copy) {
                                    let job_short_id = job.id.short_id;
                                    let job_name = job.name.clone();
                                    self.state.background_jobs.cancel_job(job.id.uuid);
                                    debug!("Job {} cancelled", job_short_id);
                                    
                                    // Add task panel log for cancellation
                                    let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                                    let log_msg = format!(
                                        "{} [Job #{}] {}: Cancelled [WARN]",
                                        timestamp,
                                        job_short_id,
                                        job_name
                                    );
                                    self.task_panel.add_pending_log(log_msg);
                                }
                                // Don't close dialog, let user see the cancellation
                            } else {
                                // Close button or other - just close dialog
                                self.state.dialogs.pop();
                                debug!("Dialog popped (Close button or other)");
                            }
                        }
                        rwf_lib::model::dialog::DialogContent::CloseTabWithActiveJob { tab_index, job_ids, .. } => {
                            // Cancel all jobs in the tab and close it
                            debug!("CloseTabWithActiveJob confirmed: tab_index={}, job_ids={:?}", tab_index, job_ids);

                            // Extract tab_index to avoid borrow issues
                            let tab_index_copy = *tab_index;

                            // Collect job info for logging BEFORE cancelling
                            let jobs_to_cancel_info: Vec<_> = self.state.background_jobs.get_all_jobs()
                                .filter(|j| j.tab_id == tab_index_copy && j.is_active())
                                .map(|j| (j.id.uuid, j.id.short_id, j.name.clone()))
                                .collect();

                            debug!("Found {} active jobs in tab {} to cancel", jobs_to_cancel_info.len(), tab_index_copy + 1);

                            // Close dialog first to release borrow
                            self.state.dialogs.pop();

                            // Cancel all active jobs in this tab
                            let mut cancelled_count = 0;
                            for (job_id, short_id, job_name) in &jobs_to_cancel_info {
                                self.state.background_jobs.cancel_job(*job_id);
                                debug!("Job {} ({}) cancelled for tab close", short_id, job_name);

                                // Add task panel log for cancellation
                                let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                                let log_msg = format!(
                                    "{} [Job #{}] [Tab {}] {}: Cancelled [WARN]",
                                    timestamp,
                                    short_id,
                                    tab_index_copy + 1,
                                    job_name
                                );
                                debug!("Adding cancellation log: {}", log_msg);
                                self.task_panel.add_pending_log(log_msg.clone());
                                cancelled_count += 1;
                            }

                            debug!("Cancelled {} jobs total, added {} logs", cancelled_count, cancelled_count);
                            debug!("Task panel: {} log entries, {} pending", 
                                self.task_panel.log_count(), 
                                self.task_panel.pending_log_count());

                            // Close the tab
                            let _ = rwf_lib::state::update_state(&mut self.state, rwf_lib::Transition::CloseTab { index: tab_index_copy });
                            debug!("Tab {} closed after job cancellation", tab_index_copy + 1);
                        }
                        _ => {
                            if let Some(job_spec) = crate::ui::dialog::process_dialog_confirmation(&mut self.state) {
                                // Submit job to worker pool (compression/extraction)
                                if let Some(ref pool) = self.worker_pool {
                                    debug!("Submitting compression/extraction job: {:?}", job_spec.kind);
                                    self.state.jobs.start_job(job_spec.clone());
                                    pool.submit_job(job_spec);
                                }
                                // Close dialog after successful job submission
                                self.state.dialogs.pop();
                                debug!("Dialog popped after confirmation");
                            } else {
                                // Close dialog (other dialogs)
                                self.state.dialogs.pop();
                            }
                        }
                    }

                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);

                    return true;
                }
                crate::ui::dialog::DialogAction::None => {
                    // Input was consumed by dialog, no further action
                    debug!("Dialog action: None (input consumed)");
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);

                    return true;
                }
                crate::ui::dialog::DialogAction::NextField => {
                    // Move focus to next field in dialog
                    debug!("Dialog action: NextField");
                    // Focus cycling is handled in dialog content
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);
                    return true;
                }
                crate::ui::dialog::DialogAction::PrevField => {
                    // Move focus to previous field in dialog
                    debug!("Dialog action: PrevField");
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);
                    return true;
                }
                _ => {
                    // Other dialog actions (navigation, text input) are handled internally
                    debug!("Dialog action: Other");
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);

                    return true;
                }
            }
        }

        debug!("No dialog open, mapping key to action");
        // Map key event to action using KeyBindings
        if let Some(action) = self.key_bindings.map_key(&key) {
            debug!("Key mapped to action: {:?}", action);
            // Check if we're waiting for next key in sequence
            if action == rwf_lib::Action::PendingSequence {
                debug!("Waiting for next key in sequence: {:?}", self.key_bindings.get_pending_sequence());
                
                let elapsed = start.elapsed();
                self.metrics.record_input_time(elapsed);
                
                return true; // UI needs to show pending sequence indicator
            }
            
            // Convert action to transitions
            let transitions = action_to_transitions(&self.state, &action);
            
            // Apply all transitions
            let mut state_changed = false;
            for transition in transitions {
                // Check for quit transition
                if matches!(transition, Transition::Quit) {
                    self.should_quit = true;
                    
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);
                    
                    return true;
                }
                
                // Check for exit and change directory transition
                if matches!(transition, Transition::ExitAndChangeDirectory) {
                    self.should_exit_and_cd = true;
                    self.should_quit = true;
                    
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);
                    
                    return true;
                }
                
                let result = rwf_lib::state::update_state(&mut self.state, transition);
                
                // Submit any jobs that were created
                if let Some(ref pool) = self.worker_pool {
                    for job_spec in result.jobs_to_start {
                        debug!("Submitting job: {:?}", job_spec.kind);
                        // Add job to active jobs map before submitting to worker pool
                        self.state.jobs.start_job(job_spec.clone());
                        pool.submit_job(job_spec);
                    }
                }
                
                state_changed = state_changed || result.ui_changed;
            }
            
            let elapsed = start.elapsed();
            self.metrics.record_input_time(elapsed);
            
            if elapsed > Duration::from_millis(16) {
                warn!("Input processing took {:?} (exceeds 16ms target)", elapsed);
            }
            
            return state_changed;
        }
        
        // Clear any pending sequence on unrecognized key
        if self.key_bindings.has_pending_sequence() {
            self.key_bindings.clear_pending_sequence();
            
            let elapsed = start.elapsed();
            self.metrics.record_input_time(elapsed);
            
            return true;
        }
        
        let elapsed = start.elapsed();
        self.metrics.record_input_time(elapsed);
        
        false
    }

    /// Render the UI
    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        self.metrics.start_frame();
        
        // Calculate pane height before rendering
        let terminal_size = terminal.size()?;
        let tab_bar_height = if self.state.ui.layout.show_tab_bar { 1 } else { 0 };
        let top_separator_height = 1;
        let filename_line_height = 1; // Always shown
        let task_panel_height = if self.state.ui.layout.show_task_panel { 
            self.state.ui.layout.task_panel_height as u16
        } else { 
            0 
        };
        let status_bar_height = if self.state.ui.layout.show_status_bar { 1 } else { 0 };
        
        // Pane height = total height - (tab bar + top separator + filename line + task panel + status bar + borders)
        let pane_height = terminal_size.height
            .saturating_sub(tab_bar_height)
            .saturating_sub(top_separator_height)
            .saturating_sub(filename_line_height)
            .saturating_sub(task_panel_height)
            .saturating_sub(status_bar_height)
            .saturating_sub(2) as usize; // Subtract 2 for pane borders
        
        // Update pane height in state if it changed
        if self.state.ui.layout.pane_height != pane_height {
            let _ = rwf_lib::state::update_state(&mut self.state, Transition::UpdatePaneHeight { height: pane_height });
        }
        
        terminal.draw(|frame| {
            render_ui(frame, &self.state, &self.task_panel);
        })?;
        
        self.metrics.end_frame();

        Ok(())
    }
}
