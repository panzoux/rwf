# TWF Rust Port - Implementation Guide

## Overview

This guide provides detailed implementation instructions for the TWF Rust port, following the single AppState architecture with structured decomposition. All type definitions should be referenced from design.md as the single source of truth.

## Project Structure

```
twf-rs/
├── src/
│   ├── main.rs              # Application entry point
│   ├── app.rs               # Main application loop and state management
│   ├── state/               # AppState definition and related types
│   │   ├── mod.rs
│   │   ├── app_state.rs     # The central AppState struct (references design.md)
│   │   └── transitions.rs   # State transition types (references design.md)
│   ├── transitions/         # Pure transition functions
│   │   ├── mod.rs
│   │   ├── input.rs         # Input handling logic
│   │   ├── operations.rs    # File operation logic
│   │   ├── search.rs        # Search logic
│   │   └── navigation.rs    # Navigation logic
│   ├── adapters/            # Side effect handlers
│   │   ├── mod.rs
│   │   ├── filesystem.rs    # Filesystem operations
│   │   ├── terminal.rs      # Terminal I/O
│   │   └── background.rs    # Background operations
│   ├── ui/                  # UI rendering logic
│   │   ├── mod.rs
│   │   ├── renderer.rs      # Rendering functions
│   │   └── widgets/         # UI components
│   └── config/              # Configuration handling
│       ├── mod.rs
│       └── loader.rs        # Config loading/saving
├── Cargo.toml               # Dependencies and project metadata
└── README.md                # Project documentation
```

## Core Implementation

### 1. AppState Reference

The AppState structure is defined in design.md and should be referenced from there. The implementation follows the structured decomposition pattern:

```rust
pub struct AppState {
    // Domain State
    pub filesystem: FilesystemModel,
    pub jobs: JobManager,
    pub search: SearchModel,
    pub marking: MarkingModel,
    pub history: NavigationHistory,
    
    // UI State
    pub ui: UIState,
    pub dialogs: DialogStack,
    
    // Backend State (Pure Data, No Handles)
    pub backends: BackendState,
    
    // Configuration
    pub config: AppConfig,
}
```

### 2. State Update Function

The core state transformation function follows the structured approach:

```rust
pub fn update_state(state: &mut AppState, transition: Transition) {
    match transition {
        Transition::NavigateDown(pane, delta) => {
            let (cursor, entries) = match pane {
                ActivePane::Left => (&mut state.ui.selection.left_cursor, &state.filesystem.left_pane.entries),
                ActivePane::Right => (&mut state.ui.selection.right_cursor, &state.filesystem.right_pane.entries),
            };
            
            if !entries.is_empty() {
                let new_cursor = (*cursor as isize + delta).max(0).min(entries.len() as isize - 1) as usize;
                *cursor = new_cursor;
            }
        }
        
        Transition::NavigateUp(pane) => {
            // Navigate up means go to parent directory
            let current_location = match pane {
                ActivePane::Left => &state.filesystem.left_pane.current_location,
                ActivePane::Right => &state.filesystem.right_pane.current_location,
            };
            
            let parent_location = get_parent_location(current_location);
            if let Some(parent_loc) = parent_location {
                match pane {
                    ActivePane::Left => {
                        state.filesystem.left_pane.current_location = parent_loc;
                    }
                    ActivePane::Right => {
                        state.filesystem.right_pane.current_location = parent_loc;
                    }
                }
            }
        }
        
        Transition::SwitchPane => {
            state.ui.active_pane = match state.ui.active_pane {
                ActivePane::Left => ActivePane::Right,
                ActivePane::Right => ActivePane::Left,
            };
        }
        
        Transition::ChangeLocation(pane, location) => {
            match pane {
                ActivePane::Left => {
                    // Update history
                    state.history.left_stack.truncate(state.history.left_pos + 1);
                    state.history.left_stack.push(state.filesystem.left_pane.current_location.clone());
                    state.history.left_pos = state.history.left_stack.len() - 1;
                    
                    state.filesystem.left_pane.current_location = location;
                    // Entries will be updated by filesystem adapter
                }
                ActivePane::Right => {
                    state.history.right_stack.truncate(state.history.right_pos + 1);
                    state.history.right_stack.push(state.filesystem.right_pane.current_location.clone());
                    state.history.right_pos = state.history.right_stack.len() - 1;
                    
                    state.filesystem.right_pane.current_location = location;
                }
            }
        }
        
        Transition::ToggleMarkLocation(location) => {
            if state.marking.marked_locations.contains(&location) {
                state.marking.marked_locations.remove(&location);
            } else {
                state.marking.marked_locations.insert(location);
            }
            
            // Update the marked flag in the entries
            update_marked_entries(state);
        }
        
        Transition::MarkAll => {
            let entries = match state.ui.active_pane {
                ActivePane::Left => &state.filesystem.left_pane.entries,
                ActivePane::Right => &state.filesystem.right_pane.entries,
            };
            
            // Mark all entries in current pane
            for entry in entries {
                state.marking.marked_locations.insert(entry.location.clone());
            }
            
            update_marked_entries(state);
        }
        
        Transition::UnmarkAll => {
            let current_location = state.current_location().clone();
            // Remove all marked locations that are in current directory
            state.marking.marked_locations.retain(|loc| !is_same_or_subdir(loc, &current_location));
            update_marked_entries(state);
        }
        
        Transition::StartSearch(query) => {
            state.search.query = query;
            // Search results will be populated by search adapter
        }
        
        Transition::UpdateSearchResults(results) => {
            state.search.results = results;
        }
        
        Transition::ChangeUIMode(mode) => {
            state.ui.mode = mode;
        }
        
        Transition::ShowDialog(dialog) => {
            state.dialogs.stack.push(dialog);
            state.ui.mode = UIMode::Dialog;
        }
        
        Transition::CloseDialog => {
            if !state.dialogs.stack.is_empty() {
                state.dialogs.stack.pop();
            }
            if state.dialogs.stack.is_empty() {
                state.ui.mode = UIMode::Normal;
            } else {
                state.ui.mode = UIMode::Dialog;
            }
        }
        
        // Add other transition cases...
    }
}

fn get_parent_location(location: &Location) -> Option<Location> {
    match location {
        Location::Local(path) => {
            if let Some(parent) = path.parent() {
                Some(Location::Local(parent.to_path_buf()))
            } else {
                None // Already at root
            }
        }
        Location::Ssh { host, path } => {
            if let Some(parent) = path.parent() {
                Some(Location::Ssh { 
                    host: host.clone(), 
                    path: parent.to_path_buf() 
                })
            } else {
                None // Already at root
            }
        }
        Location::Cloud { provider, path } => {
            if let Some(parent) = path.parent() {
                Some(Location::Cloud { 
                    provider: provider.clone(), 
                    path: parent.to_path_buf() 
                })
            } else {
                None // Already at root
            }
        }
        Location::Archive { archive_path, inner_path } => {
            if let Some(parent) = inner_path.parent() {
                Some(Location::Archive { 
                    archive_path: archive_path.clone(), 
                    inner_path: parent.to_path_buf() 
                })
            } else {
                None // Already at root
            }
        }
    }
}

fn is_same_or_subdir(location: &Location, parent: &Location) -> bool {
    match (location, parent) {
        (Location::Local(loc_path), Location::Local(parent_path)) => {
            loc_path.starts_with(parent_path)
        }
        _ => false, // For simplicity, only handle local paths for now
    }
}

fn update_marked_entries(state: &mut AppState) {
    for entry in &mut state.filesystem.left_pane.entries {
        entry.marked = state.marking.marked_locations.contains(&entry.location);
    }
    
    for entry in &mut state.filesystem.right_pane.entries {
        entry.marked = state.marking.marked_locations.contains(&entry.location);
    }
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            filesystem: FilesystemModel {
                left_pane: PaneModel {
                    current_location: Location::Local(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))),
                    entries: Vec::new(),
                    sort_mode: SortMode::NameAsc,
                    display_mode: DisplayMode::Details,
                    file_mask: String::from("*"),
                },
                right_pane: PaneModel {
                    current_location: Location::Local(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))),
                    entries: Vec::new(),
                    sort_mode: SortMode::NameAsc,
                    display_mode: DisplayMode::Details,
                    file_mask: String::from("*"),
                },
                cache: DirectoryCache::new(),
            },
            
            jobs: JobManager {
                active_jobs: Vec::new(),
                completed_jobs: Vec::new(),
                queue: JobQueue::new(),
            },
            
            search: SearchModel {
                query: String::new(),
                results: Vec::new(),
                history: Vec::new(),
                current_index: None,
                case_sensitive: false,
                use_regex: false,
            },
            
            marking: MarkingModel {
                marked_locations: HashSet::new(),
                marked_count: 0,
                marked_size: 0,
            },
            
            history: NavigationHistory {
                left_stack: vec![],
                right_stack: vec![],
                left_pos: 0,
                right_pos: 0,
            },
            
            ui: UIState {
                active_pane: ActivePane::Left,
                selection: SelectionState {
                    left_cursor: 0,
                    right_cursor: 0,
                    visual_start: None,
                },
                scroll: ScrollState {
                    left_offset: 0,
                    right_offset: 0,
                },
                mode: UIMode::Normal,
                layout: LayoutState {
                    pane_split_ratio: 0.5,
                    show_status_bar: true,
                    show_task_panel: true,
                },
            },
            
            dialogs: DialogStack {
                stack: Vec::new(),
                input_buffer: String::new(),
            },
            
            backends: BackendState {
                ssh_connections: HashMap::new(),
                cloud_providers: HashMap::new(),
                archive_sessions: HashMap::new(),
            },
            
            config,
        }
    }
    
    pub fn current_selection(&self) -> Option<&FileEntry> {
        let entries = match self.ui.active_pane {
            ActivePane::Left => &self.filesystem.left_pane.entries,
            ActivePane::Right => &self.filesystem.right_pane.entries,
        };
        let cursor = match self.ui.active_pane {
            ActivePane::Left => self.ui.selection.left_cursor,
            ActivePane::Right => self.ui.selection.right_cursor,
        };
        entries.get(cursor)
    }
    
    pub fn current_location(&self) -> &Location {
        match self.ui.active_pane {
            ActivePane::Left => &self.filesystem.left_pane.current_location,
            ActivePane::Right => &self.filesystem.right_pane.current_location,
        }
    }
}
```

## Key Implementation Principles

### 1. Mutability with &mut References
- State transitions modify state in-place using &mut references for performance
- Functions operate on mutable state directly rather than returning new state
- Side effects are isolated in adapter modules
- Design principles emphasize pure logic despite mutable state updates

### 2. Runtime Context Separation
- External resources (SSH sessions, archive handles, cloud providers) are held in RuntimeContext, not AppState
- AppState contains only pure data structures without connection handles
- Background operations communicate with AppState through events
- Location-aware filesystem model supports Local, SSH, Cloud, and Archive backends
- Job runtime handles are kept in RuntimeContext, not in AppState
- Job progress updates flow: Background Task → Event Channel → AppState update

### 3. Event-Based Job Updates
- Background operations send events to update AppState
- No direct mutation of AppState from background tasks
- Maintains state consistency and avoids race conditions
- Example: JobProgress { job_id, progress } → Event Processing → AppState update

### 4. Clear Separation of Concerns
- Business logic in transition functions
- State management in update_state function
- Side effects in adapter modules
- UI rendering in UI modules

### 5. Runtime Context Separation
- External resources (SSH sessions, archive handles, cloud providers) are held in RuntimeContext, not AppState
- AppState contains only pure data structures without connection handles
- Background operations communicate with AppState through events
- Location-aware filesystem model supports Local, SSH, Cloud, and Archive backends
- Job runtime handles are kept in RuntimeContext, not in AppState
- Job progress updates flow: Background Task → Event Channel → AppState update

### 6. Error Handling
- Use Result types consistently
- Handle errors at the boundary between pure functions and side effects
- Provide user-friendly error messages in UI

### 7. Performance Considerations
- Avoid unnecessary cloning of large data structures
- Use references where possible
- Implement efficient algorithms for common operations
- Cache expensive computations when appropriate

### 8. Testing Strategy
- Pure transition functions are easily unit-testable
- Side effect adapters can be mocked for testing
- State transitions can be tested with known inputs and expected outputs
- Integration tests verify the complete flow