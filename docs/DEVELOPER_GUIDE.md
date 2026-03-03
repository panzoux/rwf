# Two-Pane File Manager - Developer Guide

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Design Patterns](#design-patterns)
3. [State Management](#state-management)
4. [Job System Integration](#job-system-integration)
5. [Extension Points](#extension-points)
6. [Building and Testing](#building-and-testing)
7. [Contributing Guidelines](#contributing-guidelines)

## Architecture Overview

The Two-Pane File Manager follows a clean architecture with strict separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                         UI Thread                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Input Handler│→ │   AppState   │→ │   Renderer   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         ↓                  ↓                                 │
│  ┌──────────────┐  ┌──────────────┐                        │
│  │  Transition  │  │  JobManager  │                        │
│  └──────────────┘  └──────────────┘                        │
└─────────────────────────────────────────────────────────────┘
                           ↓ JobSpec
┌─────────────────────────────────────────────────────────────┐
│                    rwf Worker Pool                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  FIFO Queue  │→ │  Worker 1-N  │→ │  JobExecutor │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                              ↓               │
│                                       ┌──────────────┐       │
│                                       │   Backends   │       │
│                                       └──────────────┘       │
└─────────────────────────────────────────────────────────────┘
                           ↑ JobEvent
```

### Core Principles

1. **Never Block the UI Thread**: All file I/O operations execute as Jobs in the rwf Worker Pool
2. **Explicit State Transitions**: All state changes occur through the `Transition` enum
3. **Pure State Logic**: State transformations are pure functions returning `StateUpdateResult`
4. **Event-Driven Architecture**: `JobEvent`s flow from Worker Pool to UI thread via channels
5. **FIFO Job Ordering**: Strict first-in-first-out job execution
6. **Cooperative Cancellation**: Jobs check cancellation tokens periodically
7. **Separation of Concerns**: Clear boundaries between state, side effects, and rendering

### Directory Structure

```
two-pane-fm/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── core/
│   │   ├── state.rs            # AppState and core data structures
│   │   ├── transition.rs       # Transition enum and state update logic
│   │   ├── job.rs              # Job types and JobManager
│   │   └── config.rs           # Configuration structures
│   ├── event/
│   │   ├── input.rs            # Input handling and key mapping
│   │   └── job_event.rs        # Job event types
│   ├── backend/
│   │   ├── filesystem.rs       # Filesystem backend trait
│   │   ├── local.rs            # Local filesystem implementation
│   │   ├── archive.rs          # Archive handler
│   │   └── ssh.rs              # SSH backend (future)
│   ├── executor/
│   │   └── job_executor.rs     # Job execution logic
│   ├── ui/
│   │   ├── renderer.rs         # Main UI rendering
│   │   ├── pane.rs             # Pane rendering
│   │   ├── dialog.rs           # Dialog rendering
│   │   ├── status.rs           # Status bar rendering
│   │   └── task_panel.rs       # Task panel rendering
│   └── util/
│       ├── format.rs           # Formatting utilities
│       └── cache.rs            # Directory cache
├── docs/
│   ├── USER_GUIDE.md           # User documentation
│   ├── DEVELOPER_GUIDE.md      # This file
│   └── API_REFERENCE.md        # API documentation
├── tests/
│   ├── integration/            # Integration tests
│   └── property/               # Property-based tests
└── Cargo.toml
```

## Design Patterns

### 1. AppState Pattern

The `AppState` structure is the single source of truth for all application state:

```rust
pub struct AppState {
    // Domain State
    pub tabs: TabManager,
    pub jobs: JobManager,
    pub search: SearchModel,
    pub marking: MarkingModel,
    
    // UI State
    pub ui: UIState,
    pub dialogs: DialogStack,
    
    // Backend State (Pure Data)
    pub backends: BackendStatus,
    
    // Configuration
    pub config: AppConfig,
}
```

**Key characteristics:**
- Immutable from outside the state update function
- All mutations go through `update_state()`
- Serializable for session persistence
- No direct I/O or side effects

### 2. Transition Pattern

All state changes are represented as explicit `Transition` enum values:

```rust
pub enum Transition {
    CursorMove { pane: ActivePane, delta: isize },
    ChangeLocation { pane: ActivePane, location: Location },
    EnqueueJob { spec: JobSpec },
    CompleteJob { job_id: JobId, result: OpResult },
    // ... many more variants
}
```

**Benefits:**
- Testable: Easy to write unit tests for each transition
- Debuggable: Can log all state changes
- Replayable: Can record and replay user sessions
- Predictable: No hidden state mutations

### 3. Pure State Functions

The `update_state()` function is pure and deterministic:

```rust
pub fn update_state(state: &mut AppState, transition: Transition) -> StateUpdateResult {
    match transition {
        Transition::CursorMove { pane, delta } => {
            // Pure logic to update cursor position
            // Returns StateUpdateResult with side effects
        }
        // ... handle all transitions
    }
}
```

**StateUpdateResult** describes side effects without executing them:

```rust
pub struct StateUpdateResult {
    pub started_jobs: Vec<JobSpec>,      // Jobs to submit to worker pool
    pub ui_changed: bool,                // Whether UI needs redraw
    pub needs_refresh: Vec<ActivePane>,  // Panes that need refresh
}
```

### 4. Backend Abstraction

The `FilesystemBackend` trait abstracts different storage backends:

```rust
#[async_trait]
pub trait FilesystemBackend: Send + Sync {
    async fn read_directory(&self, location: &Location) -> Result<Vec<FileEntry>, FsError>;
    async fn copy_files(&self, sources: &[Location], dest: &Location, cancel_token: &CancellationToken) -> Result<(), FsError>;
    async fn move_files(&self, sources: &[Location], dest: &Location, cancel_token: &CancellationToken) -> Result<(), FsError>;
    async fn delete_files(&self, locations: &[Location], cancel_token: &CancellationToken) -> Result<(), FsError>;
    async fn create_directory(&self, location: &Location) -> Result<(), FsError>;
    async fn rename_file(&self, from: &Location, to: &Location) -> Result<(), FsError>;
    async fn calculate_size(&self, location: &Location, cancel_token: &CancellationToken) -> Result<u64, FsError>;
}
```

**Implementations:**
- `LocalFilesystemBackend`: Local filesystem operations
- `ArchiveHandler`: Archive browsing and extraction
- `SshBackend`: SSH/SFTP operations (future)
- `CloudBackend`: Cloud storage (future)

### 5. Location Abstraction

The `Location` enum provides a unified way to represent different path types:

```rust
pub enum Location {
    Local(PathBuf),
    Ssh { host: String, port: u16, path: PathBuf },
    Cloud { provider: String, bucket: String, path: PathBuf },
    Archive { archive_path: Box<Location>, inner_path: PathBuf },
}
```

This allows seamless navigation between local files, archives, remote servers, and cloud storage.

## State Management

### State Update Flow

1. **Input Event** → User presses a key
2. **Input Handler** → Maps `KeyEvent` to `Transition`
3. **State Update** → `update_state()` applies transition
4. **Side Effects** → Returns `StateUpdateResult` with jobs to start
5. **Job Submission** → Jobs are enqueued to rwf Worker Pool
6. **Job Execution** → Workers execute jobs asynchronously
7. **Job Events** → Workers send `JobEvent`s back to UI thread
8. **Event Processing** → Events are converted to new `Transition`s
9. **Loop** → Back to step 3

### Example: Directory Navigation

```rust
// 1. User presses Enter on a directory
let key_event = KeyEvent { code: KeyCode::Enter, ... };

// 2. Input handler maps to transition
let transition = Transition::ChangeLocation {
    pane: ActivePane::Left,
    location: Location::Local(PathBuf::from("/home/user/documents")),
};

// 3. State update function processes transition
let result = update_state(&mut app_state, transition);
// result.started_jobs contains JobSpec for ReadDirectory

// 4. Main loop submits job to worker pool
for job_spec in result.started_jobs {
    worker_pool.submit(job_spec);
}

// 5. Worker executes job
let entries = local_backend.read_directory(&location).await?;

// 6. Worker sends completion event
event_sender.send(JobEvent::Completed {
    job_id,
    result: OpResult::Success(SuccessData::DirectoryRead(entries)),
});

// 7. UI thread receives event and converts to transition
let transition = Transition::CompleteJob { job_id, result };

// 8. State update processes completion
let result = update_state(&mut app_state, transition);
// Updates pane with loaded entries
```

### State Persistence

Session state is automatically persisted:

```rust
pub struct SessionState {
    pub tabs: Vec<TabState>,
    pub active_tab_index: usize,
    pub marked_locations: HashSet<Location>,
}

impl SessionState {
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    pub fn load(path: &Path) -> Result<Self, Error> {
        let json = std::fs::read_to_string(path)?;
        let state = serde_json::from_str(&json)?;
        Ok(state)
    }
}
```

Saved on:
- Normal exit (Q)
- Exit and change directory (Shift+Q)
- Periodic auto-save (every 30 seconds)

Restored on:
- Application startup (if `session_persistence` is enabled)

## Job System Integration

### rwf Worker Pool

The application uses the Reactive Worker Framework (rwf) for asynchronous operations:

```rust
use rwf::{WorkerPool, Job, JobSpec};

// Initialize worker pool
let worker_pool = WorkerPool::new(config.worker_pool_size);

// Create job specification
let job_spec = JobSpec::new(JobKind::Copy {
    sources: vec![source_location],
    dest: dest_location,
});

// Submit to FIFO queue
worker_pool.submit(job_spec);
```

### Job Lifecycle

```
Queued → Running → Completed
   ↓        ↓           ↓
   ↓    Cancelling → Cancelled
   ↓
Cancelled (before start)
```

### Job Types

All file operations are implemented as jobs:

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
    ExecuteCustomFunction { command: String, working_dir: Location, pipe_to_action: Option<PipeToAction> },
    Search { location: Location, pattern: String, recursive: bool },
}
```

### Job Events

Workers communicate progress via events:

```rust
pub enum JobEvent {
    Started { job_id: JobId },
    Progress { job_id: JobId, progress: f64 },
    Completed { job_id: JobId, result: OpResult },
}
```

### Cooperative Cancellation

Jobs check cancellation tokens periodically:

```rust
async fn copy_file(&self, src: &Path, dest: &Path, cancel_token: &CancellationToken) -> Result<(), FsError> {
    let mut src_file = tokio::fs::File::open(src).await?;
    let mut dest_file = tokio::fs::File::create(dest).await?;
    let mut buffer = vec![0u8; self.buffer_size];
    
    loop {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Err(FsError::Cancelled);
        }
        
        let n = src_file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        
        dest_file.write_all(&buffer[..n]).await?;
    }
    
    Ok(())
}
```

### FIFO Queue Guarantees

The job queue maintains strict FIFO ordering:

1. Jobs are enqueued in the order they are submitted
2. Workers pick jobs from the front of the queue
3. No job priority or reordering
4. Ensures predictable behavior for users

### Job Manager

The `JobManager` tracks all jobs:

```rust
pub struct JobManager {
    pub queue: VecDeque<JobSpec>,           // FIFO queue
    pub active: HashMap<JobId, Job>,        // Currently running
    pub completed: VecDeque<JobResult>,     // Recently completed
    pub max_parallel: usize,                // Worker pool size
    pub next_id: u64,                       // Job ID counter
}
```

**Key methods:**
- `enqueue()`: Add job to queue
- `can_start_job()`: Check if worker is available
- `pop_next_job()`: Get next job from queue
- `start_job()`: Mark job as running
- `update_progress()`: Update job progress
- `complete_job()`: Mark job as completed
- `request_cancel()`: Request job cancellation
- `acknowledge_cancel()`: Confirm job cancelled

## Extension Points

### 1. Adding New Backends

To add a new storage backend (e.g., FTP, WebDAV):

```rust
pub struct FtpBackend {
    connection_pool: FtpConnectionPool,
}

#[async_trait]
impl FilesystemBackend for FtpBackend {
    async fn read_directory(&self, location: &Location) -> Result<Vec<FileEntry>, FsError> {
        match location {
            Location::Ftp { host, path } => {
                let conn = self.connection_pool.get(host).await?;
                let entries = conn.list(path).await?;
                // Convert to FileEntry
                Ok(entries)
            }
            _ => Err(FsError::InvalidBackend),
        }
    }
    
    // Implement other methods...
}
```

Then register in the backend router:

```rust
pub struct BackendRouter {
    local: Arc<LocalFilesystemBackend>,
    archive: Arc<ArchiveHandler>,
    ftp: Arc<FtpBackend>,
}

impl BackendRouter {
    pub fn get_backend(&self, location: &Location) -> Arc<dyn FilesystemBackend> {
        match location {
            Location::Local(_) => self.local.clone(),
            Location::Archive { .. } => self.archive.clone(),
            Location::Ftp { .. } => self.ftp.clone(),
            // ...
        }
    }
}
```

### 2. Adding New Job Types

To add a new job type:

1. Add variant to `JobKind` enum:

```rust
pub enum JobKind {
    // ... existing variants
    CompressImage {
        source: Location,
        dest: Location,
        quality: u8,
    },
}
```

2. Implement execution in `JobExecutor`:

```rust
impl JobExecutor {
    pub async fn execute(&self, spec: JobSpec) {
        let result = match &spec.kind {
            // ... existing cases
            JobKind::CompressImage { source, dest, quality } => {
                self.execute_compress_image(source, dest, *quality, &spec).await
            }
        };
        
        // Send completion event
        let _ = self.event_sender.send(JobEvent::Completed { job_id: spec.id, result });
    }
    
    async fn execute_compress_image(
        &self,
        source: &Location,
        dest: &Location,
        quality: u8,
        spec: &JobSpec,
    ) -> OpResult {
        // Implementation
    }
}
```

3. Add transition to trigger the job:

```rust
pub enum Transition {
    // ... existing variants
    CompressImage {
        source: Location,
        dest: Location,
        quality: u8,
    },
}
```

4. Handle in `update_state()`:

```rust
pub fn update_state(state: &mut AppState, transition: Transition) -> StateUpdateResult {
    match transition {
        // ... existing cases
        Transition::CompressImage { source, dest, quality } => {
            let job_spec = JobSpec::new(JobKind::CompressImage { source, dest, quality });
            StateUpdateResult::with_job(job_spec)
        }
    }
}
```

### 3. Adding New UI Components

To add a new dialog or UI component:

1. Define the dialog content:

```rust
pub enum DialogContent {
    // ... existing variants
    ImagePreview {
        location: Location,
        image_data: Vec<u8>,
        zoom: f32,
    },
}
```

2. Implement rendering:

```rust
impl Renderer {
    fn render_dialog(&self, dialog: &Dialog, frame: &mut Frame) {
        match &dialog.content {
            // ... existing cases
            DialogContent::ImagePreview { location, image_data, zoom } => {
                self.render_image_preview(location, image_data, *zoom, frame);
            }
        }
    }
    
    fn render_image_preview(&self, location: &Location, data: &[u8], zoom: f32, frame: &mut Frame) {
        // Render image using ratatui or external library
    }
}
```

3. Add input handling:

```rust
fn handle_dialog_mode(state: &AppState, event: KeyEvent) -> Vec<Transition> {
    if let Some(dialog) = state.dialogs.current() {
        match &dialog.content {
            // ... existing cases
            DialogContent::ImagePreview { .. } => {
                match event.code {
                    KeyCode::Char('+') => vec![Transition::ZoomIn],
                    KeyCode::Char('-') => vec![Transition::ZoomOut],
                    KeyCode::Escape => vec![Transition::CloseDialog],
                    _ => vec![],
                }
            }
        }
    } else {
        vec![]
    }
}
```

### 4. Adding Custom Macro Expansions

To add new macros for custom functions:

```rust
impl CustomFunctionExpander {
    pub fn expand(&self, command: &str, context: &ExpansionContext) -> String {
        let mut result = command.to_string();
        
        // Existing macros: $P, $O, $F, etc.
        
        // Add new macro: $D (current date)
        if result.contains("$D") {
            let date = chrono::Local::now().format("%Y-%m-%d").to_string();
            result = result.replace("$D", &date);
        }
        
        // Add new macro: $T (current time)
        if result.contains("$T") {
            let time = chrono::Local::now().format("%H:%M:%S").to_string();
            result = result.replace("$T", &time);
        }
        
        // Add new macro: $S (selected file size)
        if result.contains("$S") {
            if let Some(entry) = context.cursor_entry {
                result = result.replace("$S", &entry.size.to_string());
            }
        }
        
        result
    }
}
```

### 5. Adding Archive Format Support

To add support for new archive formats:

```rust
impl ArchiveHandler {
    pub fn new() -> Self {
        Self {
            supported_formats: vec![
                "zip".to_string(),
                "tar".to_string(),
                "gz".to_string(),
                "7z".to_string(),
                "rar".to_string(),  // New format
            ],
        }
    }
    
    pub async fn read_archive(&self, location: &Location) -> Result<Vec<FileEntry>, FsError> {
        match location {
            Location::Archive { archive_path, inner_path } => {
                match archive_path.as_ref() {
                    Location::Local(path) => {
                        match path.extension().and_then(|s| s.to_str()) {
                            Some("zip") => self.read_zip(path, inner_path).await,
                            Some("rar") => self.read_rar(path, inner_path).await,  // New handler
                            _ => Err(FsError::Unknown("Unsupported archive format".to_string())),
                        }
                    }
                    _ => Err(FsError::InvalidBackend),
                }
            }
            _ => Err(FsError::InvalidBackend),
        }
    }
    
    async fn read_rar(&self, archive_path: &Path, inner_path: &Path) -> Result<Vec<FileEntry>, FsError> {
        // Implementation using unrar library
    }
}
```

## Building and Testing

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# With specific features
cargo build --features "ssh,cloud"
```

### Running Tests

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Integration tests
cargo test --test '*'

# Property-based tests
cargo test --test property

# With logging
RUST_LOG=debug cargo test

# Specific test
cargo test test_cursor_movement
```

### Property-Based Testing

The project uses property-based testing extensively:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_cursor_never_out_of_bounds(
        initial_cursor in 0usize..100,
        delta in -50isize..50,
        entry_count in 1usize..100,
    ) {
        let mut state = create_test_state(entry_count);
        state.active_pane_mut().cursor = initial_cursor.min(entry_count - 1);
        
        let result = update_state(&mut state, Transition::CursorMove {
            pane: ActivePane::Left,
            delta,
        });
        
        let cursor = state.active_pane().cursor;
        prop_assert!(cursor < entry_count);
    }
}
```

### Benchmarking

```bash
# Run benchmarks
cargo bench

# Specific benchmark
cargo bench --bench state_updates

# With profiling
cargo bench --bench state_updates -- --profile-time=5
```

### Code Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage
```

### Linting and Formatting

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy

# Clippy with all warnings
cargo clippy -- -W clippy::all
```

## Contributing Guidelines

### Code Style

1. **Follow Rust conventions**: Use `rustfmt` and `clippy`
2. **Descriptive names**: Use clear, descriptive variable and function names
3. **Documentation**: Document all public APIs with doc comments
4. **Error handling**: Use `Result` types, avoid panics in library code
5. **Testing**: Write tests for all new functionality

### Commit Messages

Follow conventional commits:

```
feat: Add SSH backend support
fix: Correct cursor position after delete
docs: Update developer guide with backend info
test: Add property tests for job cancellation
refactor: Extract dialog rendering to separate module
```

### Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make changes and commit
4. Write tests for new functionality
5. Ensure all tests pass: `cargo test`
6. Run formatting: `cargo fmt`
7. Run clippy: `cargo clippy`
8. Push to your fork
9. Create a pull request with description

### Testing Requirements

All PRs must include:
- Unit tests for new functions
- Integration tests for new features
- Property-based tests for core logic
- Documentation updates

### Performance Considerations

1. **Never block UI thread**: All I/O must be async
2. **Minimize allocations**: Reuse buffers where possible
3. **Efficient rendering**: Only redraw changed regions
4. **Cache wisely**: Balance memory usage vs. performance
5. **Profile before optimizing**: Use benchmarks to guide optimization

### Security Considerations

1. **Validate all input**: Especially file paths and user commands
2. **Sanitize shell commands**: Prevent command injection
3. **Check permissions**: Verify file access before operations
4. **Limit resource usage**: Prevent DoS via large files or deep recursion
5. **Secure temporary files**: Use secure temp file creation

### Debugging Tips

1. **Enable logging**: Set `RUST_LOG=debug` or `RUST_LOG=trace`
2. **Use debugger**: `rust-gdb` or `rust-lldb`
3. **Print state**: Add debug prints in `update_state()`
4. **Record transitions**: Log all transitions for replay
5. **Check job events**: Monitor job event channel

### Common Pitfalls

1. **Blocking UI thread**: Never call blocking I/O on UI thread
2. **Forgetting cancellation**: Always check cancel token in loops
3. **State inconsistency**: Only mutate state in `update_state()`
4. **Resource leaks**: Ensure proper cleanup in Drop implementations
5. **Race conditions**: Use proper synchronization for shared state

## Advanced Topics

### Custom Rendering

To customize rendering, implement the `Renderer` trait:

```rust
pub trait Renderer {
    fn render(&self, state: &AppState, frame: &mut Frame);
    fn render_pane(&self, pane: &PaneModel, area: Rect, frame: &mut Frame);
    fn render_status_bar(&self, state: &AppState, area: Rect, frame: &mut Frame);
    fn render_task_panel(&self, jobs: &JobManager, area: Rect, frame: &mut Frame);
}
```

### Plugin System (Future)

Planned plugin architecture:

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn on_load(&mut self, context: &mut PluginContext);
    fn on_transition(&mut self, transition: &Transition, state: &AppState);
    fn on_render(&mut self, state: &AppState, frame: &mut Frame);
}
```

### Scripting Support (Future)

Planned Lua scripting integration:

```lua
-- Custom function in Lua
function on_file_select(file)
    if file.extension == "rs" then
        run_command("cargo check --file " .. file.path)
    end
end
```

---

For user documentation, see [USER_GUIDE.md](USER_GUIDE.md).
For API documentation, see [API_REFERENCE.md](API_REFERENCE.md).
