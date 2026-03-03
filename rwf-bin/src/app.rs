//! Application main loop
//!
//! This module implements the main event loop with rendering at 30+ FPS.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use rwf_lib::{AppState, Transition, KeyBindings, action_to_transitions};
use std::io::Stdout;
use std::time::{Duration, Instant};
use tracing::{debug, info};

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
}

impl App {
    /// Create a new application instance
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            key_bindings: KeyBindings::default(),
            should_quit: false,
        }
    }
    
    /// Create a new application instance with custom key bindings
    pub fn with_key_bindings(state: AppState, key_bindings: KeyBindings) -> Self {
        Self {
            state,
            key_bindings,
            should_quit: false,
        }
    }

    /// Run the main application loop
    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        info!("Starting main application loop");

        let mut last_render = Instant::now();

        loop {
            // Calculate time since last render
            let elapsed = last_render.elapsed();
            let time_until_next_frame = FRAME_DURATION.saturating_sub(elapsed);

            // Handle input events with timeout
            if self.handle_events(time_until_next_frame.min(MAX_RENDER_DELAY))? {
                // State changed, render immediately
                self.render(terminal)?;
                last_render = Instant::now();
            } else if elapsed >= FRAME_DURATION {
                // Time for next frame
                self.render(terminal)?;
                last_render = Instant::now();
            }

            // Check if we should quit
            if self.should_quit {
                info!("Application quit requested");
                
                // Save session state before quitting
                if let Err(e) = self.state.save_session() {
                    tracing::error!("Failed to save session: {}", e);
                } else {
                    info!("Session state saved successfully");
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
                debug!("Key event: {:?}", key);
                return Ok(self.handle_key_event(key));
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
        
        // Map key event to action using KeyBindings
        if let Some(action) = self.key_bindings.map_key(&key) {
            // Check if we're waiting for next key in sequence
            if action == rwf_lib::Action::PendingSequence {
                debug!("Waiting for next key in sequence: {:?}", self.key_bindings.get_pending_sequence());
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
                    return true;
                }
                
                let result = rwf_lib::state::update_state(&mut self.state, transition);
                
                // TODO: Handle jobs_to_start, jobs_to_cancel, panes_to_refresh
                
                state_changed = state_changed || result.ui_changed;
            }
            
            let elapsed = start.elapsed();
            if elapsed > Duration::from_millis(16) {
                debug!("Input processing took {:?} (exceeds 16ms target)", elapsed);
            }
            
            return state_changed;
        }
        
        // Clear any pending sequence on unrecognized key
        if self.key_bindings.has_pending_sequence() {
            self.key_bindings.clear_pending_sequence();
            return true;
        }
        
        false
    }

    /// Render the UI
    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        terminal.draw(|frame| {
            render_ui(frame, &self.state);
        })?;

        Ok(())
    }
}
