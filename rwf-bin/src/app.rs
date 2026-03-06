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
use tracing::{debug, info, warn};

use crate::performance::PerformanceMetrics;
use crate::ui::render_ui;

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
    cwd_flag: bool, // Whether -cwd flag was provided
}

impl App {
    /// Create a new application instance
    pub fn new(state: AppState) -> Self {
        Self::with_cwd_flag(state, false)
    }
    
    /// Create a new application instance with cwd flag
    pub fn with_cwd_flag(state: AppState, cwd_flag: bool) -> Self {
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
            cwd_flag,
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
            cwd_flag: false,
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
                        debug!("Event result {}: jobs_to_start={}, ui_changed={}", 
                            idx, result.jobs_to_start.len(), result.ui_changed);
                    }
                }
                results
            } else {
                Vec::new()
            };
            let events_processed = !event_results.is_empty();
            
            // Submit any new jobs that were created by state transitions
            if let Some(ref pool) = self.worker_pool {
                for result in &event_results {
                    for job_spec in &result.jobs_to_start {
                        // Add job to active jobs map before submitting to worker pool
                        self.state.jobs.start_job(job_spec.clone());
                        pool.submit_job(job_spec.clone());
                    }
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

            // Log performance metrics every 5 seconds at DEBUG level
            if self.last_metrics_log.elapsed() >= Duration::from_secs(5) {
                debug!("Performance: {}", self.metrics.summary());
                
                // Check for performance warnings
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
        
        // Special handling for help dialog language rotation
        // **Validates: Requirements 48.3**
        if let Some(dialog) = self.state.dialogs.current() {
            if matches!(dialog.content, rwf_lib::DialogContent::Help { .. }) {
                // Check if 'L' key is pressed (case-insensitive)
                if matches!(key.code, crossterm::event::KeyCode::Char('l') | crossterm::event::KeyCode::Char('L')) 
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                    // Rotate help language
                    let result = rwf_lib::state::update_state(&mut self.state, Transition::RotateHelpLanguage);
                    
                    let elapsed = start.elapsed();
                    self.metrics.record_input_time(elapsed);
                    
                    return result.ui_changed;
                }
            }
        }
        
        // Map key event to action using KeyBindings
        if let Some(action) = self.key_bindings.map_key(&key) {
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
            render_ui(frame, &self.state);
        })?;
        
        self.metrics.end_frame();

        Ok(())
    }
}
