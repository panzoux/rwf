# TWF Rust Port - Main Application Structure

## main.rs

```rust
mod app;
mod state;
mod transitions;
mod adapters;
mod ui;
mod config;

use crate::app::{run_application};
use crate::state::{AppState, AppConfig};
use crate::adapters::{FilesystemAdapter, TerminalAdapter, BackgroundExecutor};
use crate::config::ConfigLoader;

use std::sync::mpsc;
use std::thread;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    let mut terminal_adapter = TerminalAdapter::new()?;
    
    // Load configuration
    let config = ConfigLoader::load()?;
    
    // Initialize app state
    let mut app_state = AppState::new(config);
    
    // Initialize runtime context
    let (event_sender, event_receiver) = mpsc::channel();
    let mut runtime_context = RuntimeContext::new(event_sender);
    
    // Initialize adapters
    let filesystem_adapter = FilesystemAdapter::new();
    let background_executor = BackgroundExecutor::new(runtime_context.event_sender.clone());
    
    // Load initial directory contents
    if let Ok(entries) = filesystem_adapter.read_directory(&app_state.filesystem.left_pane.current_location).await {
        app_state.filesystem.left_pane.entries = entries;
    }
    
    if let Ok(entries) = filesystem_adapter.read_directory(&app_state.filesystem.right_pane.current_location).await {
        app_state.filesystem.right_pane.entries = entries;
    }
    
    // Run main application loop
    run_application(
        &mut app_state,
        &mut terminal_adapter,
        &mut runtime_context,
        &filesystem_adapter,
        &background_executor
    ).await?;
    
    Ok(())
}
```

## app.rs

```rust
use crate::state::{AppState, RuntimeContext, Transition, AppEvent};
use crate::adapters::{TerminalAdapter, FilesystemAdapter, BackgroundExecutor};

pub async fn run_application(
    app_state: &mut AppState,
    terminal_adapter: &mut TerminalAdapter,
    runtime_context: &mut RuntimeContext,
    filesystem_adapter: &FilesystemAdapter,
    background_executor: &BackgroundExecutor,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Render current state
        terminal_adapter.render(app_state)?;
        
        // Process any incoming events from background tasks
        process_background_events(app_state, runtime_context)?;
        
        // Poll for user input
        if let Some(event) = terminal_adapter.poll_event()? {
            match event {
                // Handle terminal events
                TerminalEvent::Key(key_event) => {
                    let transitions = handle_input(app_state, key_event);
                    for transition in transitions {
                        update_state(app_state, transition);
                    }
                }
                TerminalEvent::Resize(width, height) => {
                    // Handle resize
                }
                TerminalEvent::Quit => break,
            }
        }
        
        // Process any queued events
        process_queued_events(app_state, runtime_context)?;
        
        // Small delay to prevent excessive CPU usage
        tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
    }
    
    Ok(())
}

fn process_background_events(
    app_state: &mut AppState,
    runtime_context: &mut RuntimeContext,
) -> Result<(), Box<dyn std::error::Error>> {
    // Process events from background operations
    while let Ok(event) = runtime_context.event_receiver.try_recv() {
        match event {
            AppEvent::JobProgress { job_id, progress } => {
                update_state(app_state, Transition::UpdateJobProgress(job_id, progress));
            }
            AppEvent::JobCompleted { job_id, result } => {
                let result = update_state(app_state, Transition::CompleteJob(job_id, result));
                // Job handles are now managed in JobManager
                for completed_job_id in result.completed_jobs {
                    app_state.jobs.complete_job(completed_job_id);
                }
            }
            AppEvent::JobCancelled { job_id } => {
                // Handle job cancellation acknowledgment
                let result = update_state(app_state, Transition::CancelAcknowledged(job_id));
                // Job handles are now managed in JobManager
                for job_id in result.completed_jobs {
                    app_state.jobs.complete_job(job_id);
                }
            }
            AppEvent::SearchCompleted { query, results } => {
                update_state(app_state, Transition::UpdateSearchResults { query, results });
            }
        }
    }
    Ok(())
}

fn process_queued_jobs(
    app_state: &mut AppState,
    runtime_context: &mut RuntimeContext,
    background_executor: &BackgroundExecutor,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check for jobs whose dependencies are now satisfied
    app_state.jobs.check_dependencies();
    
    // Start next jobs if we're under the parallel limit
    while app_state.jobs.active.len() < app_state.jobs.max_parallel && !app_state.jobs.queue.is_empty() {
        let result = update_state(app_state, Transition::StartNextJob);
        
        // Execute any newly started jobs
        for job_spec in result.started_jobs {
            app_state.jobs.spawn_job(job_spec, background_executor);
        }
    }
    
    Ok(())
}

fn process_queued_events(
    app_state: &mut AppState,
    runtime_context: &mut RuntimeContext,
) -> Result<(), Box<dyn std::error::Error>> {
    // Process any other queued events
    Ok(())
}
```

## state/app_state.rs

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// All state structures are defined in design.md and referenced here
// This file contains the actual implementation of AppState methods
```

## transitions/input.rs

```rust
use crate::state::{AppState, Transition, ActivePane, UIMode};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_input(app_state: &AppState, event: KeyEvent) -> Vec<Transition> {
    match app_state.ui.mode {
        UIMode::Normal => handle_normal_mode(app_state, event),
        UIMode::Search => handle_search_mode(app_state, event),
        UIMode::Visual => handle_visual_mode(app_state, event),
        UIMode::Command => handle_command_mode(app_state, event),
        UIMode::Dialog => handle_dialog_mode(app_state, event),
    }
}

fn handle_normal_mode(app_state: &AppState, event: KeyEvent) -> Vec<Transition> {
    match event.code {
        KeyCode::Up | KeyCode::Char('k') => vec![Transition::NavigateDown(app_state.ui.active_pane, -1)],
        KeyCode::Down | KeyCode::Char('j') => vec![Transition::NavigateDown(app_state.ui.active_pane, 1)],
        KeyCode::Left | KeyCode::Char('h') => vec![Transition::SwitchPane],
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(selected) = app_state.current_selection() {
                if selected.is_dir {
                    vec![Transition::ChangeLocation(
                        app_state.ui.active_pane,
                        selected.location.clone()
                    )]
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        KeyCode::Char('g') => vec![Transition::NavigateUp(app_state.ui.active_pane)], // Go to parent directory
        KeyCode::Tab => vec![Transition::SwitchPane],
        KeyCode::Char('q') => vec![Transition::QuitApplication],
        _ => vec![],
    }
}

// Other mode handlers...
```

## adapters/filesystem.rs

```rust
use crate::state::{FileEntry, Location};
use std::path::Path;
use std::fs;
use std::time::SystemTime;

#[async_trait]
pub trait FilesystemBackend {
    async fn read_directory(&self, location: &Location) -> Result<Vec<FileEntry>, FsError>;
    async fn copy_files(&self, sources: &[Location], dest: &Location) -> Result<(), FsError>;
    async fn move_files(&self, sources: &[Location], dest: &Location) -> Result<(), FsError>;
    async fn delete_files(&self, locations: &[Location]) -> Result<(), FsError>;
    async fn create_directory(&self, location: &Location) -> Result<(), FsError>;
    async fn rename_file(&self, from: &Location, to: &Location) -> Result<(), FsError>;
}

pub struct LocalFilesystemBackend;

#[async_trait]
impl FilesystemBackend for LocalFilesystemBackend {
    async fn read_directory(&self, location: &Location) -> Result<Vec<FileEntry>, FsError> {
        match location {
            Location::Local(path) => {
                let mut entries = Vec::new();
                
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let metadata = entry.metadata()?;
                    
                    let file_entry = FileEntry {
                        name: entry.file_name().to_string_lossy().to_string(),
                        location: Location::Local(entry.path()), // This is the key fix
                        size: metadata.len(),
                        is_dir: metadata.file_type().is_dir(),
                        is_hidden: is_hidden(&entry.path()),
                        modified: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
                        marked: false,
                    };
                    
                    entries.push(file_entry);
                }
                
                Ok(entries)
            }
            _ => Err(FsError::InvalidBackend),
        }
    }
    
    // Other implementations...
    async fn copy_files(&self, sources: &[Location], dest: &Location) -> Result<(), FsError> {
        // Local filesystem implementation
        Ok(())
    }
    
    async fn move_files(&self, sources: &[Location], dest: &Location) -> Result<(), FsError> {
        // Local filesystem implementation
        Ok(())
    }
    
    async fn delete_files(&self, locations: &[Location]) -> Result<(), FsError> {
        // Local filesystem implementation
        Ok(())
    }
    
    async fn create_directory(&self, location: &Location) -> Result<(), FsError> {
        // Local filesystem implementation
        Ok(())
    }
    
    async fn rename_file(&self, from: &Location, to: &Location) -> Result<(), FsError> {
        // Local filesystem implementation
        Ok(())
    }
}

pub struct FilesystemAdapter {
    backends: HashMap<String, Box<dyn FilesystemBackend>>,
}

impl FilesystemAdapter {
    pub fn new() -> Self {
        let mut backends = HashMap::new();
        backends.insert("local".to_string(), Box::new(LocalFilesystemBackend));
        
        Self { backends }
    }
    
    pub async fn read_directory(&self, location: &Location) -> Result<Vec<FileEntry>, FsError> {
        match location {
            Location::Local(_) => {
                if let Some(backend) = self.backends.get("local") {
                    backend.read_directory(location).await
                } else {
                    Err(FsError::BackendNotFound)
                }
            }
            // Handle other location types when backends are implemented
            _ => Err(FsError::BackendNotImplemented),
        }
    }
}
```