# TWF Rust Port - Complete Specification Overview

## Table of Contents
1. [Executive Summary](#executive-summary)
2. [Architectural Analysis](#architectural-analysis)
3. [Internal Structure](#internal-structure)
4. [Implementation Guide](#implementation-guide)
5. [Design Principles](#design-principles)
6. [Technology Stack](#technology-stack)
7. [Development Roadmap](#development-roadmap)

## Executive Summary

The TWF Rust port represents a complete re-architecture of the existing C#/.NET dual-pane file manager to leverage Rust's performance, memory safety, and cross-platform capabilities. The port maintains all existing functionality while adopting a cleaner, more maintainable architecture centered around a single application state.

### Key Innovation: Single AppState Pattern
Rather than distributing state across multiple "manager" objects as in the original .NET implementation, the Rust port consolidates all mutable state into a single `AppState` struct. This eliminates consistency issues and simplifies reasoning about application behavior.

### Architecture Layers
```
Side Effects (adapters) → Pure Transition Functions → AppState → UI Rendering
```

## Architectural Analysis

### Core Architecture: Single AppState Pattern

#### Central Principle
There is exactly ONE `AppState` struct that owns ALL mutable state in the application.

#### State Classification
- **Pure State** (Stored in AppState):
  - Filesystem state (directory entries, metadata)
  - View state (selection, scroll, sort, display modes)
  - Navigation history (back/forward stacks per pane)
  - Configuration state (all settings)
  - Search state (queries, results, history)
  - Marking state (which files are marked)

- **Pure Transition Logic** (Pure Functions):
  - Input processing (maps events to state transitions)
  - Operation orchestration (defines operation state machines)
  - Job lifecycle logic (defines job state transitions)
  - Archive virtualization logic (transforms archive ops to state changes)
  - UI rendering logic (transforms state to UI representation)

- **Side Effects** (External Interactions):
  - Filesystem I/O: Reading directories, file operations
  - Terminal I/O: Rendering to screen, reading input
  - Background Threads: Long-running operations
  - File Watching: Monitoring for external changes

### State Transition Pattern
All state changes happen through a single update loop that applies transitions to the central state:

```rust
pub fn update_state(state: &mut AppState, transition: Transition) {
    match transition {
        Transition::NavigateDown(delta) => {
            // Update selection based on delta
        }
        Transition::ChangeDirectory(pane, path) => {
            // Update directory path for specified pane
        }
        // ... other transitions
    }
}
```

### Runtime Context Separation
External resources and connection handles are separated from the pure state:

```rust
pub struct RuntimeContext {
    pub backends: BackendHandles,
    pub job_handles: JobRuntimeHandles,
    pub event_sender: Sender<AppEvent>,
    pub event_receiver: Receiver<AppEvent>,
}

pub struct BackendHandles {
    pub ssh_sessions: HashMap<String, SSHTunnel>,
    pub cloud_providers: CloudProviders,
    pub archive_handles: HashMap<PathBuf, ArchiveHandle>,
}

pub struct JobRuntimeHandles {
    pub handles: HashMap<JobId, JoinHandle<OpResult>>,
    pub progress_channels: HashMap<JobId, Sender<ProgressUpdate>>,
}

// AppState contains only pure data:
pub struct AppState {
    // ... all state fields without connection handles
    pub backends: BackendState,  // Pure state about connections, not actual handles
    pub filesystem: FilesystemModel,  // Contains Location enum for different backends
    pub jobs: JobManager,  // Contains pure job state, not runtime handles
}

pub struct PaneModel {
    pub current_location: Location,  // Local, SSH, Cloud, or Archive
    pub entries: Vec<FileEntry>,
    pub sort_mode: SortMode,
    pub display_mode: DisplayMode,
    pub file_mask: String,
}

pub enum Location {
    Local(PathBuf),
    Ssh { host: String, path: PathBuf },
    Cloud { provider: String, path: PathBuf },
    Archive { archive_path: PathBuf, inner_path: PathBuf },
}

pub struct Job {
    pub id: JobId,
    pub op_type: JobType,
    pub status: JobStatus,
    pub progress: f64,
    pub source_paths: Vec<PathBuf>,
    pub dest_path: PathBuf,
    pub start_time: SystemTime,
    pub estimated_remaining: Option<Duration>,
}
```

## Internal Structure

### Structured Decomposition
The AppState is decomposed into meaningful sub-structures to prevent the "God object" anti-pattern:

- **Domain State**: FilesystemModel, JobManager, SearchModel, MarkingModel, NavigationHistory
- **UI State**: UIState, DialogStack
- **Backend State** (Pure Data): BackendState
- **Configuration**: AppConfig

### Runtime Context Separation
External resources are separated into RuntimeContext:

- **Runtime Resources**: BackendHandles (SSH sessions, cloud providers, archive handles)
- **Event Channels**: For communication between background tasks and AppState
- **Connection Handles**: Actual resource handles that require special lifetimes

### Extension Strategy
The structure supports adding new backends (SSH, Cloud, Archive) without restructuring the core AppState:

- SSH support extends BackendState::ssh_connections (pure state) and RuntimeContext::backends::ssh_sessions (actual handles)
- Cloud storage extends BackendState::cloud_providers (pure state) and RuntimeContext::backends::cloud_providers (actual handles)
- Archive support extends BackendState::archive_sessions (pure state) and RuntimeContext::backends::archive_handles (actual handles)

### Ownership Strategy
Clear ownership hierarchy prevents borrow checker conflicts and cyclic dependencies:

- Parent structs own child structs (no circular references)
- References flow downward: AppState → sub-structs → data
- RuntimeContext holds actual connection handles, AppState holds pure state
- Shared data lives in common parent structures
- Background tasks communicate via event channels, not direct state access

## Implementation Guide

### Project Structure
```
twf-rs/
├── src/
│   ├── main.rs              # Application entry point
│   ├── app.rs               # Main application loop and state management
│   ├── state/               # AppState definition and related types
│   │   ├── mod.rs
│   │   ├── app_state.rs     # The central AppState struct
│   │   ├── transitions.rs   # State transition types
│   │   └── types.rs         # Supporting types (enums, etc.)
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
```

### Core Data Structures

#### AppState Structure
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

pub struct FilesystemModel {
    pub left_pane: PaneModel,
    pub right_pane: PaneModel,
    pub cache: DirectoryCache,
}

pub struct PaneModel {
    pub current_location: Location,  // Local, SSH, Cloud, or Archive
    pub entries: Vec<FileEntry>,
    pub sort_mode: SortMode,
    pub display_mode: DisplayMode,
    pub file_mask: String,
}

pub enum Location {
    Local(PathBuf),
    Ssh { host: String, path: PathBuf },
    Cloud { provider: String, path: PathBuf },
    Archive { archive_path: PathBuf, inner_path: PathBuf },
}

pub struct JobManager {
    pub active_jobs: Vec<Job>,
    pub completed_jobs: Vec<Job>,
    pub queue: JobQueue,
}

pub struct SearchModel {
    pub query: String,
    pub results: Vec<FileEntry>,
    pub history: Vec<String>,
    pub current_index: Option<usize>,
}

pub struct MarkingModel {
    pub marked_files: HashSet<PathBuf>,
    pub marked_count: usize,
    pub marked_size: u64,
}

pub struct NavigationHistory {
    pub left_stack: Vec<PathBuf>,
    pub right_stack: Vec<PathBuf>,
    pub left_pos: usize,
    pub right_pos: usize,
}

pub struct UIState {
    pub active_pane: ActivePane,
    pub selection: SelectionState,
    pub scroll: ScrollState,
    pub mode: UIMode,
    pub layout: LayoutState,
}

pub struct SelectionState {
    pub left_cursor: usize,
    pub right_cursor: usize,
    pub visual_start: Option<usize>,
}

pub struct ScrollState {
    pub left_offset: usize,
    pub right_offset: usize,
}

pub struct DialogStack {
    pub stack: Vec<Dialog>,
    pub input_buffer: String,
}

pub struct BackendState {
    // Pure state about backend connections
    pub ssh_connections: HashMap<String, ConnectionStatus>,
    pub cloud_providers: HashMap<String, ProviderStatus>,
    pub archive_sessions: HashMap<PathBuf, ArchiveStatus>,
}

pub struct AppConfig {
    pub display: DisplayConfig,
    pub key_bindings: KeyBindings,
    pub file_operations: FileOpConfig,
    pub search: SearchConfig,
}
```

#### Transition Types
```rust
pub enum Transition {
    // Navigation
    ChangeDirectory(ActivePane, PathBuf),
    NavigateUp,
    NavigateDown(isize),
    SwitchPane,
    
    // File Operations
    StartCopyOperation(Vec<PathBuf>, PathBuf),
    StartMoveOperation(Vec<PathBuf>, PathBuf),
    StartDeleteOperation(Vec<PathBuf>),
    
    // View Operations
    ChangeSortMode(ActivePane, SortMode),
    ChangeDisplayMode(ActivePane, DisplayMode),
    ToggleMarkFile(PathBuf),
    
    // Search Operations
    StartSearch(String),
    UpdateSearchResults(Vec<FileEntry>),
    
    // Job Operations
    StartJob(Job),
    UpdateJobProgress(u64, f64),
    CompleteJob(u64),
    
    // UI Operations
    ChangeUIMode(UIMode),
    ShowDialog(DialogState),
    CloseDialog,
}
```

## Design Principles

### 1. Immutability Where Possible
- State transitions create new state rather than mutating existing state
- Functions are pure and have no side effects
- Side effects are isolated in adapter modules

### 2. Clear Separation of Concerns
- Business logic in transition functions
- State management in update_state function
- Side effects in adapter modules
- UI rendering in UI modules

### 3. Functional Programming Benefits
- Pure functions are easier to test and reason about
- Clear separation between business logic and side effects
- Predictable application behavior

### 4. Performance Considerations
- Avoid unnecessary cloning of large data structures
- Use references where possible
- Implement efficient algorithms for common operations
- Cache expensive computations when appropriate

### 5. Error Handling
- Use Result types consistently
- Handle errors at the boundary between pure functions and side effects
- Provide user-friendly error messages in UI

## Technology Stack

### Core Dependencies
- **Terminal UI**: `ratatui` for terminal-based interface
- **Async Runtime**: `tokio` for background operations
- **Configuration**: `serde_json` for JSON configuration
- **File Operations**: Standard library with async extensions
- **Testing**: Built-in Rust testing framework
- **CLI Parsing**: `clap` for command-line argument parsing

### Key Libraries
- `crossterm`: Cross-platform terminal manipulation
- `serde`: Serialization/deserialization framework
- `tokio`: Asynchronous runtime
- `ratatui`: Terminal user interface
- `thiserror`: Error handling utilities
- `anyhow`: High-level error handling

## Development Roadmap

### Phase 1: Foundation (Weeks 1-2)
- Set up project structure
- Implement basic AppState structure
- Create transition types
- Implement basic input handling
- Set up UI rendering with ratatui

### Phase 2: Core Functionality (Weeks 3-4)
- Implement filesystem adapter
- Add directory navigation
- Implement basic file operations (copy, move, delete)
- Add selection and marking functionality

### Phase 3: Advanced Features (Weeks 5-6)
- Implement search functionality
- Add job management system
- Implement configuration loading/saving
- Add archive support

### Phase 4: Polish & Testing (Weeks 7-8)
- Comprehensive testing
- Performance optimization
- UI polish and accessibility
- Cross-platform testing
- Documentation

### Risk Mitigation Strategies
1. **Performance**: Single-threaded state updates prevent race conditions
2. **Complexity**: Clear architectural boundaries prevent feature creep
3. **Migration**: Architecture designed to accommodate existing feature set
4. **Reliability**: Pure functions are inherently testable

## Expected Outcomes

### Performance Improvements
- Faster directory loading and file operations
- More responsive UI with reduced latency
- Better memory utilization

### Reliability Enhancements
- Elimination of memory-related crashes
- Improved error handling and recovery
- More robust file operation handling

### Cross-Platform Support
- Native performance on all platforms
- Consistent behavior across operating systems
- Unified codebase maintenance

## Conclusion

The single AppState architecture provides a clean, maintainable foundation for the Rust port of TWF. By focusing on pure functions and clear state management, the implementation will be both performant and reliable while maintaining all existing functionality. This approach leverages Rust's strengths while avoiding common pitfalls of complex state management systems.

The architecture eliminates the complexity of the original .NET implementation's multiple "managers" while preserving all functionality. The clear separation between pure logic and side effects makes the codebase more testable and maintainable, while the single source of truth for state eliminates consistency issues that plagued the original architecture.