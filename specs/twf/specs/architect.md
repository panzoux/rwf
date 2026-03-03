# TWF Rust Port - Architectural Analysis

## Fundamental Nature

TWF is fundamentally a **state synchronization system** that maintains consistency between multiple views of hierarchical filesystem state. At its core, it's a bidirectional data binding system where:

- Filesystem state drives UI representation
- User input drives filesystem mutations
- Multiple synchronized views share a consistent state model
- Background operations modify state asynchronously while maintaining UI responsiveness

The application operates as a **reactive state machine** where filesystem events, user inputs, and background job completions trigger state transitions that propagate through the system.

## Core Architecture: Single AppState Pattern

### Central Principle
There is exactly ONE `AppState` struct that owns ALL mutable state in the application.

### State Classification

#### Pure State (Stored in AppState)
- **R1**: Filesystem state (directory entries, metadata)
- **R2**: View state (selection, scroll, sort, display modes)
- **R4**: Navigation history (back/forward stacks per pane)
- **R7**: Configuration state (all settings)
- **R8**: Search state (queries, results, history)
- **R11**: Marking state (which files are marked)

#### Pure Transition Logic (Pure Functions)
- **R3**: Input processing (maps events to state transitions)
- **R5**: Operation orchestration (defines operation state machines)
- **R6**: Job lifecycle logic (defines job state transitions)
- **R9**: Archive virtualization logic (transforms archive ops to state changes)
- **R10**: UI rendering logic (transforms state to UI representation)

#### Side Effects (External Interactions)
- **Filesystem I/O**: Reading directories, file operations
- **Terminal I/O**: Rendering to screen, reading input
- **Background Threads**: Long-running operations
- **File Watching**: Monitoring for external changes
- **Network Access**: If any (e.g., remote filesystems)

## Single AppState Structure

```rust
pub struct AppState {
    // R1: Filesystem State
    pub left_pane_entries: Vec<FileEntry>,
    pub right_pane_entries: Vec<FileEntry>,
    pub current_left_dir: PathBuf,
    pub current_right_dir: PathBuf,
    
    // R2: View State
    pub left_selection: usize,
    pub right_selection: usize,
    pub left_scroll_offset: usize,
    pub right_scroll_offset: usize,
    pub left_sort_mode: SortMode,
    pub right_sort_mode: SortMode,
    pub left_display_mode: DisplayMode,
    pub right_display_mode: DisplayMode,
    
    // R4: Navigation History
    pub left_history: Vec<PathBuf>,
    pub right_history: Vec<PathBuf>,
    pub left_history_pos: usize,
    pub right_history_pos: usize,
    
    // R7: Configuration
    pub config: Config,
    
    // R8: Search State
    pub search_query: String,
    pub search_results: Vec<FileEntry>,
    pub search_history: Vec<String>,
    
    // R11: Marking State
    pub marked_files: HashSet<PathBuf>,
    
    // R6: Job State (pure state of jobs)
    pub active_jobs: Vec<Job>,
    pub job_queue: Vec<PendingJob>,
    
    // UI State
    pub active_pane: ActivePane,
    pub ui_mode: UIMode,
    pub current_dialog: Option<DialogState>,
}
```

## Transition Functions (Pure Logic)

### Input Handler
```rust
pub fn handle_input(app_state: &mut AppState, event: KeyEvent) -> Vec<Transition> {
    match app_state.ui_mode {
        UIMode::Normal => handle_normal_mode(app_state, event),
        UIMode::Search => handle_search_mode(app_state, event),
        // ...
    }
}

fn handle_normal_mode(app_state: &AppState, event: KeyEvent) -> Vec<Transition> {
    match lookup_keybinding(&app_state.config.key_bindings, event) {
        Some(Action::NavigateUp) => vec![Transition::MoveSelection(-1)],
        Some(Action::NavigateDown) => vec![Transition::MoveSelection(1)],
        Some(Action::SwitchPane) => vec![Transition::ToggleActivePane],
        // ...
        None => vec![],  // No transition
    }
}
```

### File Operation Orchestrator
```rust
pub fn start_copy_operation(app_state: &AppState, sources: &[PathBuf], dest: &Path) -> Vec<Transition> {
    if app_state.active_jobs.len() >= app_state.config.max_concurrent_jobs {
        return vec![Transition::QueueOperation(Operation::Copy(sources.to_vec(), dest.to_path_buf()))];
    }
    
    vec![
        Transition::StartJob(Job::new_copy(sources, dest)),
        Transition::AddToActiveJobs(JobId::new()),
    ]
}
```

### Search Processor
```rust
pub fn execute_search(app_state: &AppState, query: &str) -> Vec<Transition> {
    let current_dir = match app_state.active_pane {
        ActivePane::Left => &app_state.current_left_dir,
        ActivePane::Right => &app_state.current_right_dir,
    };
    
    let results = perform_search(current_dir, query, &app_state.config.search);
    
    vec![Transition::UpdateSearchResults(results)]
}
```

## Side Effect Handlers

### Filesystem Adapter
```rust
pub struct FilesystemAdapter;

impl FilesystemAdapter {
    pub fn read_directory(&self, path: &Path) -> Result<Vec<FileEntry>, FsError> {
        // Side effect: reads from filesystem
    }
    
    pub fn copy_files(&self, sources: &[Path], dest: &Path) -> Result<(), FsError> {
        // Side effect: performs file I/O
    }
}
```

### Terminal Adapter
```rust
pub struct TerminalAdapter {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalAdapter {
    pub fn render(&mut self, app_state: &AppState) -> Result<(), RenderError> {
        // Side effect: writes to terminal
    }
    
    pub fn poll_event(&mut self) -> Result<Option<Event>, TermError> {
        // Side effect: reads from terminal
    }
}
```

### Background Executor
```rust
pub struct BackgroundExecutor;

impl BackgroundExecutor {
    pub fn execute_operation(&self, op: Operation) -> JoinHandle<OpResult> {
        // Side effect: spawns background thread
        tokio::spawn(async move { perform_operation(op).await })
    }
}
```

## State Transition Pattern

All state changes happen through a single update loop:

```rust
pub fn update_app(mut app_state: AppState, transition: Transition) -> AppState {
    match transition {
        Transition::MoveSelection(delta) => {
            let current_selection = match app_state.active_pane {
                ActivePane::Left => &mut app_state.left_selection,
                ActivePane::Right => &mut app_state.right_selection,
            };
            
            let entries_len = match app_state.active_pane {
                ActivePane::Left => app_state.left_pane_entries.len(),
                ActivePane::Right => app_state.right_pane_entries.len(),
            };
            
            if entries_len > 0 {
                *current_selection = ((*current_selection as isize + delta) as usize)
                    .min(entries_len - 1)
                    .max(0);
            }
        }
        Transition::ChangeDirectory(pane, path) => {
            match pane {
                ActivePane::Left => app_state.current_left_dir = path,
                ActivePane::Right => app_state.current_right_dir = path,
            }
        }
        // ... other transitions
    }
    app_state
}
```

## Dependency Structure

```
Side Effects (external boundaries)
    ↓
Pure Transition Functions (business logic)
    ↓
AppState (single source of truth)
    ↓
UI Rendering (view transformation)
```

This architecture ensures:
- Single source of truth for all state
- Pure functions for all business logic
- Clear separation of side effects
- Deterministic state transitions
- Easy testing of business logic
- Predictable state evolution

## .NET Artifacts To Reconsider

### 1. Heavy Object-Oriented Decomposition
The original design shows excessive class proliferation (MainController, multiple "managers"). Rust prefers data-oriented design with functions operating on data structures.

### 2. Dependency Injection Container Pattern
The constructor in MainController takes many dependencies. Rust typically uses module-level state or explicit parameter passing rather than DI containers.

### 3. Async/Await for I/O Bound Operations
While async is appropriate, the original design may overuse it. Rust's async should be reserved for truly concurrent operations, not all I/O.

### 4. Event-Based Architecture Complexity
The original has many event handlers and callbacks. Rust's ownership model often makes direct state mutation clearer than complex event systems.

### 5. Terminal.Gui Framework Assumptions
The UI architecture assumes a particular GUI framework. Rust's terminal UI landscape (ratatui, cursive) has different idioms that should drive the architecture.

### 6. Configuration as Central Authority
Original design treats configuration as a provider that other components pull from. Rust idioms favor configuration being passed as parameters or using a centralized state system.

### 7. Logging as Cross-Cutting Concern
Heavy use of ILogger throughout the codebase. Rust typically uses tracing or simpler logging approaches without injecting loggers everywhere.

### 8. Caching as Separate Component
Cache management appears as a separate concern. In Rust, caching is often implemented as part of the data structure itself rather than a separate "cache service".

### 9. Multiple State Owners
The original architecture distributes state across multiple "managers". The new architecture consolidates all state into a single AppState to eliminate consistency issues and simplify reasoning about application state.