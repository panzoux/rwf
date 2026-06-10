# RWF: Reactive Worker Filemanager - API Reference

Complete API documentation for all public types, traits, and functions.

## Table of Contents

1. [Core State Types](#core-state-types)
2. [Transition System](#transition-system)
3. [Job System](#job-system)
4. [Backend Traits](#backend-traits)
5. [Configuration Schema](#configuration-schema)
6. [Location Types](#location-types)
7. [File Entry Types](#file-entry-types)

## Core State Types

### `AppState`

Central application state coordinating all components.

```rust
pub struct AppState {
    pub tabs: TabManager,
    pub jobs: JobManager,
    pub search: SearchModel,
    pub marking: MarkingModel,
    pub ui: UIState,
    pub dialogs: DialogStack,
    pub backends: BackendStatus,
    pub config: AppConfig,
}
```

**Methods:**
- `new(config: AppConfig) -> Self` - Create new state
- `current_tab(&self) -> &TabState` - Get active tab
- `current_tab_mut(&mut self) -> &mut TabState` - Get mutable active tab
- `active_pane(&self) -> &PaneModel` - Get active pane
- `active_pane_mut(&mut self) -> &mut PaneModel` - Get mutable active pane
- `opposite_pane(&self) -> &PaneModel` - Get opposite pane

### `TabManager`

Manages multiple tabs with independent pane states.

```rust
pub struct TabManager {
    pub tabs: Vec<TabState>,
    pub active_index: usize,
}
```

**Methods:**
- `new() -> Self` - Create with one initial tab
- `create_tab(&mut self) -> usize` - Create new tab, returns index
- `close_tab(&mut self, index: usize) -> bool` - Close tab (fails if last)
- `switch_to_next(&mut self)` - Switch to next tab (wraps)
- `switch_to_prev(&mut self)` - Switch to previous tab (wraps)
- `has_active_jobs(&self, job_manager: &JobManager) -> Vec<bool>` - Check tabs with active jobs

### `TabState`

State for a single tab.

```rust
pub struct TabState {
    pub id: usize,
    pub left_pane: PaneModel,
    pub right_pane: PaneModel,
    pub history: NavigationHistory,
}
```

**Methods:**
- `new(id: usize) -> Self` - Create tab with CWD in both panes

### `PaneModel`

State for a single pane.

```rust
pub struct PaneModel {
    pub current_location: Location,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub sort_mode: SortMode,
    pub display_mode: DisplayMode,
    pub file_mask: Option<String>,
}
```

**Methods:**
- `new(location: Location) -> Self` - Create pane at location
- `current_entry(&self) -> Option<&FileEntry>` - Get entry under cursor
- `marked_entries(&self) -> Vec<&FileEntry>` - Get all marked entries
- `apply_sort(&mut self)` - Sort entries by current sort mode
- `apply_filter(&mut self, mask: &str)` - Apply file mask filter

### `SortMode`

File sorting criteria.

```rust
pub enum SortMode {
    Name,       // Alphabetical by name
    Size,       // By file size
    Date,       // By modification date
    Extension,  // By file extension
}
```

### `DisplayMode`

Pane display format.

```rust
pub enum DisplayMode {
    Columns(u8),  // 1-8 column layout
    Detailed,     // Full metadata view
}
```

## Transition System

### `Transition`

Explicit state change operations.

```rust
pub enum Transition {
    // Navigation
    CursorMove { pane: ActivePane, delta: isize },
    CursorJump { pane: ActivePane, position: usize },
    ChangeLocation { pane: ActivePane, location: Location },
    NavigateUp { pane: ActivePane },
    NavigateHistory { pane: ActivePane, direction: HistoryDirection },
    SwitchPane,
    
    // Tab Management
    CreateTab,
    CloseTab { index: usize },
    SwitchTab { index: usize },
    NextTab,
    PrevTab,
    
    // Job Operations
    EnqueueJob { spec: JobSpec },
    StartNextJob,
    UpdateJobProgress { job_id: JobId, progress: f64 },
    CompleteJob { job_id: JobId, result: OpResult },
    CancelJob { job_id: JobId },
    AcknowledgeCancel { job_id: JobId },
    
    // View Operations
    ChangeSortMode { pane: ActivePane, mode: SortMode },
    ChangeDisplayMode { pane: ActivePane, mode: DisplayMode },
    SetFileMask { pane: ActivePane, mask: Option<String> },
    ToggleHidden,
    Refresh { pane: ActivePane },
    
    // Pane Operations
    SyncPanes,
    SwapPanes,
    
    // Marking Operations
    ToggleMark { location: Location },
    MarkAll,
    UnmarkAll,
    MarkPattern { pattern: String },
    MarkRange { start: usize, end: usize },
    InvertMarks,
    
    // UI Operations
    ShowDialog { dialog: Dialog },
    CloseDialog,
    ChangeUIMode { mode: UIMode },
    
    // Application Control
    Quit,
    ReloadConfig,
}
```

### `update_state`

Pure function that applies transitions to state.

```rust
pub fn update_state(
    state: &mut AppState,
    transition: Transition
) -> StateUpdateResult
```

**Parameters:**
- `state`: Mutable reference to application state
- `transition`: State change to apply

**Returns:** `StateUpdateResult` describing side effects

**Guarantees:**
- Deterministic: Same input always produces same output
- No I/O: Never performs file operations
- No blocking: Completes immediately

### `StateUpdateResult`

Describes side effects without executing them.

```rust
pub struct StateUpdateResult {
    pub started_jobs: Vec<JobSpec>,
    pub ui_changed: bool,
    pub needs_refresh: Vec<ActivePane>,
}
```

**Methods:**
- `none() -> Self` - No side effects
- `with_job(job: JobSpec) -> Self` - Single job to start
- `with_ui_change() -> Self` - UI needs redraw
- `with_refresh(pane: ActivePane) -> Self` - Pane needs refresh

## Job System

### `JobManager`

Manages job queue and execution state.

```rust
pub struct JobManager {
    pub queue: VecDeque<JobSpec>,
    pub active: HashMap<JobId, Job>,
    pub completed: VecDeque<JobResult>,
    pub max_parallel: usize,
    pub next_id: u64,
}
```

**Methods:**
- `new(max_parallel: usize) -> Self` - Create with worker pool size
- `enqueue(&mut self, spec: JobSpec) -> JobId` - Add job to queue
- `can_start_job(&self) -> bool` - Check if worker available
- `pop_next_job(&mut self) -> Option<JobSpec>` - Get next from queue (FIFO)
- `start_job(&mut self, spec: JobSpec)` - Mark job as running
- `update_progress(&mut self, job_id: JobId, progress: f64)` - Update progress
- `complete_job(&mut self, job_id: JobId, result: OpResult)` - Mark completed
- `request_cancel(&mut self, job_id: JobId) -> bool` - Request cancellation
- `acknowledge_cancel(&mut self, job_id: JobId)` - Confirm cancelled

### `JobSpec`

Specification for a job to execute.

```rust
pub struct JobSpec {
    pub id: JobId,
    pub kind: JobKind,
    pub created_at: SystemTime,
    pub cancel_token: CancellationToken,
}
```

**Methods:**
- `new(kind: JobKind) -> Self` - Create job spec (ID assigned by JobManager)

### `JobKind`

Types of jobs that can be executed.

```rust
pub enum JobKind {
    ReadDirectory { location: Location },
    Copy { sources: Vec<Location>, dest: Location },
    Move { sources: Vec<Location>, dest: Location },
    Delete { targets: Vec<Location> },
    Mkdir { location: Location },
    Rename { from: Location, to: Location },
    CalculateSize { location: Location },
    ExtractArchive { archive: Location, dest: Location },
    CreateArchive { sources: Vec<Location>, dest: Location },
    ExecuteCustomFunction {
        command: String,
        working_dir: Location,
        pipe_to_action: Option<PipeToAction>,
    },
    Search {
        location: Location,
        pattern: String,
        recursive: bool,
    },
}
```

### `JobEvent`

Events sent from workers to UI thread.

```rust
pub enum JobEvent {
    Started { job_id: JobId },
    Progress { job_id: JobId, progress: f64 },
    Completed { job_id: JobId, result: OpResult },
}
```

### `OpResult`

Result of a job execution.

```rust
pub enum OpResult {
    Success(SuccessData),
    Failed(String),
    Cancelled,
}
```

### `SuccessData`

Data returned by successful jobs.

```rust
pub enum SuccessData {
    DirectoryRead(Vec<FileEntry>),
    SizeCalculated(u64),
    CustomFunctionOutput(String),
    SearchResults(Vec<FileEntry>),
    None,
}
```

## Backend Traits

### `FilesystemBackend`

Trait for storage backend implementations.

```rust
#[async_trait]
pub trait FilesystemBackend: Send + Sync {
    async fn read_directory(
        &self,
        location: &Location
    ) -> Result<Vec<FileEntry>, FsError>;
    
    async fn copy_files(
        &self,
        sources: &[Location],
        dest: &Location,
        cancel_token: &CancellationToken
    ) -> Result<(), FsError>;
    
    async fn move_files(
        &self,
        sources: &[Location],
        dest: &Location,
        cancel_token: &CancellationToken
    ) -> Result<(), FsError>;
    
    async fn delete_files(
        &self,
        locations: &[Location],
        cancel_token: &CancellationToken
    ) -> Result<(), FsError>;
    
    async fn create_directory(
        &self,
        location: &Location
    ) -> Result<(), FsError>;
    
    async fn rename_file(
        &self,
        from: &Location,
        to: &Location
    ) -> Result<(), FsError>;
    
    async fn calculate_size(
        &self,
        location: &Location,
        cancel_token: &CancellationToken
    ) -> Result<u64, FsError>;
}
```

**Implementations:**
- `LocalFilesystemBackend` - Local filesystem operations
- `ArchiveHandler` - Archive browsing and extraction
- `SshBackend` - SSH/SFTP operations (future)
- `CloudBackend` - Cloud storage (future)

### `FsError`

Filesystem operation errors.

```rust
pub enum FsError {
    Io(std::io::Error),
    PermissionDenied,
    NotFound(String),
    InvalidBackend,
    Cancelled,
    Unknown(String),
}
```

## Configuration Schema

### `AppConfig`

Main application configuration.

```rust
pub struct AppConfig {
    pub display: DisplayConfig,
    pub key_bindings: KeyBindings,
    pub file_operations: FileOpConfig,
    pub search: SearchConfig,
    pub ui: UIConfig,
    pub worker_pool_size: usize,
    pub log_level: LogLevel,
    pub session_persistence: bool,
}
```

**Default:** TWF-compatible defaults

### `DisplayConfig`

Display and visual settings.

```rust
pub struct DisplayConfig {
    pub show_hidden: bool,
    pub show_system: bool,
    pub date_format: String,
    pub time_format: TimeFormat,
    pub cjk_width: u8,
    pub colors: ColorScheme,
}
```

### `ColorScheme`

Color configuration for all UI elements.

```rust
pub struct ColorScheme {
    pub foreground_color: String,
    pub background_color: String,
    pub highlight_foreground_color: String,
    pub highlight_background_color: String,
    pub marked_file_color: String,
    pub directory_color: String,
    pub pane_border_color: String,
    pub ok_color: String,
    pub warning_color: String,
    pub error_color: String,
    // ... and more
}
```

**Supported color formats:**
- Named: `"White"`, `"BrightCyan"`, etc.
- RGB: `"#RRGGBB"` (e.g., `"#FF5733"`)

### `KeyBindings`

Configurable key mappings.

```rust
pub struct KeyBindings {
    pub normal_mode: HashMap<String, Action>,
    pub search_mode: HashMap<String, Action>,
    pub dialog_mode: HashMap<String, Action>,
    pub viewer_mode: HashMap<String, Action>,
}
```

**Key format:** `"Ctrl+C"`, `"Shift+K"`, `"Alt+Left"`, `"F1"`, etc.

### `FileOpConfig`

File operation settings.

```rust
pub struct FileOpConfig {
    pub confirm_delete: bool,
    pub confirm_overwrite: bool,
    pub buffer_size: usize,
    pub preserve_timestamps: bool,
}
```

### `SearchConfig`

Search behavior settings.

```rust
pub struct SearchConfig {
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub use_migemo: bool,
    pub max_results: usize,
}
```

### `LogLevel`

Logging verbosity.

```rust
pub enum LogLevel {
    None,
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}
```

## Location Types

### `Location`

Abstract path representation supporting multiple backends.

```rust
pub enum Location {
    Local(PathBuf),
    Ssh {
        host: String,
        port: u16,
        path: PathBuf,
    },
    Cloud {
        provider: String,
        bucket: String,
        path: PathBuf,
    },
    Archive {
        archive_path: Box<Location>,
        inner_path: PathBuf,
    },
}
```

**Methods:**
- `display_path(&self) -> String` - Human-readable path
- `parent(&self) -> Option<Location>` - Parent directory
- `join(&self, component: &str) -> Location` - Append path component

**Examples:**
```rust
// Local path
let loc = Location::Local(PathBuf::from("/home/user/documents"));

// SSH path
let loc = Location::Ssh {
    host: "server.com".to_string(),
    port: 22,
    path: PathBuf::from("/var/www"),
};

// Archive path
let loc = Location::Archive {
    archive_path: Box::new(Location::Local(PathBuf::from("/data/backup.zip"))),
    inner_path: PathBuf::from("documents/report.pdf"),
};
```

## File Entry Types

### `FileEntry`

Represents a file or directory with metadata.

```rust
pub struct FileEntry {
    pub name: String,
    pub location: Location,
    pub size: u64,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub modified: SystemTime,
    pub marked: bool,
    pub calculated_size: Option<u64>,
}
```

**Methods:**
- `extension(&self) -> Option<&str>` - File extension
- `name_without_extension(&self) -> &str` - Name without extension
- `formatted_size(&self) -> String` - Human-readable size
- `formatted_date(&self) -> String` - Formatted modification date

### Custom Functions

### `CustomFunction`

User-defined command with macro expansion.

```rust
pub struct CustomFunction {
    pub name: String,
    pub key: Option<String>,
    pub command: String,
    pub shell: String,
    pub pipe_to_action: Option<PipeToAction>,
    pub description: Option<String>,
}
```

### `PipeToAction`

Action to perform with custom function output.

```rust
pub enum PipeToAction {
    JumpToPath,
    ExecuteFile,
    ExecuteFileWithEditor,
}
```

### Macro Expansion

**Available macros:**
- `$P` - Active pane path
- `$O` - Opposite pane path
- `$L` - Left pane path
- `$R` - Right pane path
- `$F` - Cursor file name
- `$W` - File name without extension
- `$E` - File extension
- `$M` - Marked files list
- `$*` - All files in pane
- `$I` - User input prompt
- `$V` - Selected text
- `$~` - Home directory
- `$#` - File count

### Registered Folders

### `RegisteredFolder`

Bookmarked directory with environment variable support.

```rust
pub struct RegisteredFolder {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
}
```

**Environment variable formats:**
- Unix: `$VAR`, `${VAR}`
- Windows: `%VAR%`, `$env:VAR`

## Utility Functions

### Formatting

```rust
pub fn format_size(bytes: u64) -> String
```
Formats byte size as human-readable string (B, KB, MB, GB, TB).

```rust
pub fn format_date(time: SystemTime) -> String
```
Formats timestamp according to config date format.

### Pattern Matching

```rust
pub fn wildcard_to_regex(pattern: &str) -> String
```
Converts wildcard pattern (* and ?) to regex.

```rust
pub fn matches_pattern(name: &str, pattern: &str) -> bool
```
Tests if filename matches wildcard pattern.

### Path Utilities

```rust
pub fn is_hidden(path: &Path) -> bool
```
Checks if file is hidden (platform-specific).

```rust
pub fn is_archive(filename: &str) -> bool
```
Checks if file is a supported archive format.

## Error Handling

All fallible operations return `Result<T, E>` types:

```rust
// File operations
Result<Vec<FileEntry>, FsError>
Result<(), FsError>

// Configuration
Result<AppConfig, ConfigError>

// Job execution
Result<OpResult, JobError>
```

**Error handling pattern:**
```rust
match backend.read_directory(&location).await {
    Ok(entries) => {
        // Process entries
    }
    Err(FsError::PermissionDenied) => {
        // Handle permission error
    }
    Err(FsError::NotFound(path)) => {
        // Handle not found
    }
    Err(e) => {
        // Handle other errors
    }
}
```

## Thread Safety

**UI Thread types:** Not `Send` or `Sync`
- `AppState` (mutable only on UI thread)
- `Renderer`
- `InputHandler`

**Worker Pool types:** `Send + Sync`
- `FilesystemBackend` implementations
- `JobSpec`
- `JobEvent`

**Synchronization:**
- Jobs communicate via `mpsc::UnboundedSender<JobEvent>`
- Cancellation via `CancellationToken` (atomic)
- No shared mutable state between threads

## Testing Utilities

### Property-Based Testing

```rust
use proptest::prelude::*;

// Generate arbitrary AppState
pub fn arb_app_state() -> impl Strategy<Value = AppState>

// Generate arbitrary Transition
pub fn arb_transition() -> impl Strategy<Value = Transition>

// Generate arbitrary Location
pub fn arb_location() -> impl Strategy<Value = Location>
```

### Test Helpers

```rust
pub fn create_test_state(entry_count: usize) -> AppState
pub fn create_test_entries(count: usize) -> Vec<FileEntry>
pub fn create_test_job_spec(kind: JobKind) -> JobSpec
```

---

For complete examples and usage patterns, see:
- [USER_GUIDE.md](USER_GUIDE.md) - End-user documentation
- [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md) - Architecture and patterns
