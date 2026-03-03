# TWF Rust Port - Internal AppState Structure

## Overview

This document describes the internal structure of the single `AppState` for the TWF Rust port. The design maintains a unified state while preventing the "God object" anti-pattern through clear separation of concerns and well-defined sub-structures.

## Section A: Proposed AppState Structure

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

// External Runtime Context (Contains Actual Handles)
pub struct RuntimeContext {
    pub backends: BackendHandles,
    pub job_handles: JobRuntimeHandles,
    pub event_sender: Sender<AppEvent>,
    pub event_receiver: Receiver<AppEvent>,
}

pub struct BackendState {
    // Pure state about backend connections
    pub ssh_connections: HashMap<String, ConnectionStatus>,
    pub cloud_providers: HashMap<String, ProviderStatus>,
    pub archive_sessions: HashMap<PathBuf, ArchiveStatus>,
}

pub struct BackendHandles {
    // Actual connection handles (not in AppState)
    pub ssh_sessions: HashMap<String, SSHTunnel>,
    pub cloud_providers: CloudProviders,
    pub archive_handles: HashMap<PathBuf, ArchiveHandle>,
}

pub struct FilesystemModel {
    pub left_pane: PaneModel,
    pub right_pane: PaneModel,
    pub cache: DirectoryCache,
}

pub struct PaneModel {
    pub current_location: Location,
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

pub struct JobRuntimeHandles {
    pub handles: HashMap<JobId, JoinHandle<OpResult>>,
    pub progress_channels: HashMap<JobId, Sender<ProgressUpdate>>,
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

pub struct Dialog {
    pub title: String,
    pub content: DialogContent,
    pub pending_action: Option<PendingAction>,
}

#[derive(Debug, Clone)]
pub enum DialogContent {
    Confirmation { message: String },
    Input { prompt: String, default_value: String },
    FileOperation { operation: String, progress: Option<f64> },
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    ConfirmCopy { sources: Vec<PathBuf>, destination: PathBuf },
    ConfirmMove { sources: Vec<PathBuf>, destination: PathBuf },
    ConfirmDelete { paths: Vec<PathBuf> },
    // Add other actions as needed
}

pub struct BackendHandles {
    pub filesystem: FilesystemBackend,
    pub ssh_sessions: HashMap<String, SSHTunnel>,
    pub cloud_providers: CloudProviders,
    pub archive_handles: HashMap<PathBuf, ArchiveHandle>,
}

pub struct AppConfig {
    pub display: DisplayConfig,
    pub key_bindings: KeyBindings,
    pub file_operations: FileOpConfig,
    pub search: SearchConfig,
}
```

## Section B: Sub-Struct Responsibilities

### FilesystemModel
- **Data owned**: Directory entries, locations, sorting/display modes, cache
- **Invariants**: Entry list must match actual directory contents (within cache TTL); current location must be valid; location type must match backend availability
- **Behavior**: Contains methods for sorting, filtering, and cache management; handles virtual filesystem integration

### JobManager
- **Data owned**: Active/completed jobs (pure state), queue, statistics
- **Invariants**: Job IDs must be unique; queue must not contain completed jobs; job state must be consistent
- **Behavior**: Contains methods for job lifecycle management, progress tracking; does NOT contain actual handles or runtime resources

### PaneModel
- **Data owned**: Current location (Local/SSH/Cloud/Archive), entries, display settings
- **Invariants**: Entries must correspond to current location; location type must be supported
- **Behavior**: Contains methods for location-specific operations and entry management

### SearchModel
- **Data owned**: Query string, results, search history, current position
- **Invariants**: Results must match current query; current index must be valid if present
- **Behavior**: Contains methods for search execution and result navigation

### MarkingModel
- **Data owned**: Marked file paths, aggregate counts/sizes
- **Invariants**: Aggregate values must match marked file set; no duplicates in marked_files
- **Behavior**: Contains methods for marking operations and aggregate updates

### NavigationHistory
- **Data owned**: Back/forward stacks for each pane
- **Invariants**: Position must be valid within stack bounds; forward history cleared on navigation
- **Behavior**: Contains methods for navigation and history management

### UIState
- **Data owned**: Active pane, cursor positions, scroll offsets, UI mode, layout
- **Invariants**: Cursor positions must be valid for current entries; scroll must keep cursor visible
- **Behavior**: Contains methods for UI state transitions

### DialogStack
- **Data owned**: Active dialogs, input buffer
- **Invariants**: Dialog stack must be consistent; input buffer must be valid UTF-8
- **Behavior**: Contains methods for dialog management and input handling

### BackendState
- **Data owned**: Connection status, provider states, session metadata (pure data)
- **Invariants**: Connection states must be consistent; no dangling references
- **Behavior**: Contains methods for state management and status queries

### BackendHandles (in RuntimeContext)
- **Data owned**: Backend connections, session handles, resource managers
- **Invariants**: Handles must be valid; resources must be properly managed
- **Behavior**: Contains methods for backend operations and resource cleanup

### AppConfig
- **Data owned**: All configuration values
- **Invariants**: Values must be within valid ranges; no conflicting settings
- **Behavior**: Contains methods for validation and updates

## Section C: Extension Strategy (SSH/Cloud/Archive)

### Adding SSH Support
- Extend `BackendState::ssh_connections` with new connection status
- Extend `RuntimeContext::backends::ssh_sessions` with new connection handles
- Add SSH-specific paths to `FilesystemModel::entries` (virtual entries)
- No changes to core AppState structure needed
- SSH operations route through backend handles via RuntimeContext

### Adding Cloud Storage
- Extend `BackendState::cloud_providers` with new provider states
- Extend `RuntimeContext::backends::cloud_providers` with new provider types
- Cloud paths appear as virtual entries in `FilesystemModel`
- Authentication state stored in provider-specific structs
- No changes to core AppState structure needed

### Adding Archive Support
- Extend `BackendState::archive_sessions` with new archive status
- Extend `RuntimeContext::backends::archive_handles` with new archive sessions
- Archive contents appear as virtual entries in `FilesystemModel`
- Archive operations route through archive handles via RuntimeContext
- No changes to core AppState structure needed

### Extension Principles
- New backends add to `BackendState` (pure data) and `RuntimeContext::backends` (handles) without changing other fields
- Virtual filesystem entries integrate seamlessly with existing `FilesystemModel`
- UI rendering abstracts over backend differences
- Configuration extends with new sections without affecting core structure

## Section D: Borrow & Ownership Strategy

### Preventing Cyclic Dependencies
- Parent structs own child structs (no circular references)
- References flow downward: AppState → sub-structs → data
- Callbacks use weak references or closures that don't capture self
- Event system uses external channels to avoid self-references
- RuntimeContext holds actual handles, AppState holds pure state

### Managing Borrow Checker Conflicts
- Use `RefCell`/`Rc` only for shared read-only configuration data
- Prefer immutable snapshots for rendering: `render(&app_state.ui, &app_state.filesystem)`
- Use &mut references for updates: `update(&mut app_state, transition)`
- Separate read and write operations in update functions
- Use interior mutability sparingly and only where necessary

### Avoiding State Duplication
- Each piece of data has exactly one owner in the hierarchy
- Views/computed values are derived on-demand rather than stored
- Shared data lives in common parent (e.g., config in AppConfig)
- References are used for read-only access to parent data

### Memory Layout Optimization
- Group frequently accessed fields together
- Use appropriate data structures for access patterns
- Consider cache line alignment for performance-critical paths
- Minimize indirection for hot paths

### AppState & Runtime Separation
- AppState is pure data structure (Send + Sync + Clone friendly)
- RuntimeContext holds actual connection handles and resources
- Transitions update AppState via &mut references
- Effects operate on RuntimeContext
- Event loop coordinates between AppState and RuntimeContext

## Section E: Testability Strategy

### Pure Transition Testing
- Transitions operate on state: `transition(&mut app_state, event) -> Vec<Effects>`
- Update functions modify state in-place: `update(&mut app_state, transition)`
- Test transitions with known inputs and expected state changes
- Mock backend handles for filesystem-independent tests

### Isolating Infrastructure Dependencies
- Backend handles live in `RuntimeContext`, not in AppState
- Filesystem operations go through `RuntimeContext::backends::filesystem` abstraction
- Real filesystem access only occurs in integration tests
- Unit tests use in-memory filesystem mocks

### Job Update Pattern
- Job progress updates come through events: `AppEvent::JobProgress`
- Events are processed as transitions: `Event → Transition → State Update`
- No direct state mutation from background tasks
- Maintains state consistency and avoids race conditions

### Mocking Strategy
- `MockFilesystemBackend` for filesystem operations
- `MockSSHTunnel` for SSH operations  
- `MockArchiveHandle` for archive operations
- `RuntimeContext` with mock backends for integration tests
- Configuration can be hardcoded for tests

### Test Scenarios
- **Unit Tests**: Individual transition functions with mock state
- **Integration Tests**: End-to-end workflows with real backends
- **Regression Tests**: Common user workflows and edge cases
- **Performance Tests**: Large directory operations and concurrent jobs

### State Validation
- Invariant checking functions: `validate_app_state(&app_state) -> ValidationResult`
- Pre/post-condition checks in update functions
- Automated property-based testing for state transitions
- Snapshot testing for UI rendering output

This structure maintains a single mutable AppState while preventing the God object anti-pattern through clear separation of concerns, well-defined invariants, and extensibility for new backend types without restructuring the core state.