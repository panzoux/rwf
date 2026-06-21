# Job Handling in RWF

## Quick Reference

| Task | Key Binding | File Location |
|------|-------------|---------------|
| Start CountDownJob | `9` | `rwf-lib/src/input/mod.rs` |
| Open Job Manager | `Alt+J` | `rwf-bin/src/ui/dialog/job_manager.rs` |
| Cancel Job | `C` (in dialog) | `rwf-lib/src/job/background_job_manager.rs` |
| Toggle Task Panel | `T` | `rwf-bin/src/ui/task_panel.rs` |
| Quit (cancel all jobs) | `q` | `rwf-bin/src/app.rs` |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ AppState                                                      │
│  ┌────────────────────┐    ┌────────────────────────────┐   │
│  │ BackgroundJobManager│    │ WorkerPool                 │   │
│  │ - jobs: HashMap    │    │ - Executes jobs            │   │
│  │ - cleanup_queue    │◄───│ - Sends JobEvents          │   │
│  │ - UI state         │    │                            │   │
│  └────────────────────┘    └────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
         ▲                                       │
         │ BackgroundJobEvent                    │ JobEvent
         │                                       ▼
┌──────────────────────────────────────────────────────────────┐
│ UI (renders progress, status)    JobExecutor (matches JobKind)│
└──────────────────────────────────────────────────────────────┘
```

**Key Concepts**:
1. **BackgroundJobManager** - Tracks UI state, schedules cleanup
2. **WorkerPool** - Executes jobs asynchronously
3. **JobEvent** - Communication channel (worker → UI)
4. **Cleanup Queue** - Priority queue for timed job removal

---

## Core Types

### JobKind (What to Execute)

**File**: `rwf-lib/src/job.rs`

```rust
pub enum JobKind {
    CountDown { duration_secs: u32, start_value: u32 },
    Copy { sources: Vec<Location>, dest: Location },
    Move { sources: Vec<Location>, dest: Location },
    Delete { targets: Vec<Location> },
    ReadDirectory { location: Location },
    Mkdir { location: Location },
    Rename { from: Location, to: Location },
    CalculateSize { location: Location },
    ExtractArchive { archive: Location, dest: Location },
    CreateArchive { sources: Vec<Location>, dest: Location, original_size: u64 },
    ExecuteCustomFunction { command: String, working_dir: Location, ... },
    Search { location: Location, pattern: String, recursive: bool },
}
```

### JobStatus (Current State)

**File**: `rwf-lib/src/job/background_job_manager.rs`

```rust
pub enum JobStatus { Pending, Running, Completed, Failed, Cancelled }
```

### OpResult (Execution Result)

**File**: `rwf-lib/src/job.rs`

```rust
pub enum OpResult {
    Success(SuccessData),
    Failed(String),      // Error message
    Cancelled,
}

pub enum SuccessData {
    DirectoryRead(Vec<FileEntry>),
    SizeCalculated(u64),
    SearchResults(Vec<FileEntry>),
    CustomFunctionOutput(String),
    FileContents(Vec<u8>),
    ComparisonResult(FileDiff),
    None,
}
```

### JobSpec (Job Specification)

**File**: `rwf-lib/src/job.rs`

```rust
pub struct JobSpec {
    pub id: JobId,                    // Unique identifier
    pub kind: JobKind,                // What to execute
    pub created_at: SystemTime,
    pub cancel_token: CancellationToken,
}
```

### BackgroundJob (UI Tracking)

**File**: `rwf-lib/src/job/background_job_manager.rs`

```rust
pub struct BackgroundJob {
    pub id: BackgroundJobId,              // UI identifier (contains JobId + short_id)
    pub name: String,                      // Display name
    pub status: JobStatus,
    pub progress_percent: f64,             // 0.0 to 100.0
    pub progress_message: String,
    pub current_operation_detail: String,
    pub start_time: SystemTime,
    pub end_time: Option<SystemTime>,
    pub tab_id: usize,                     // Which tab
    pub tab_name: String,                  // Tab path
}
```

**Note**: `JobId` is for internal tracking, `BackgroundJobId` is for UI display.

### JobEvent (Worker → UI Communication)

**File**: `rwf-lib/src/worker_pool.rs`

```rust
pub enum JobEvent {
    Started(JobId),
    Progress(JobId, f64),
    ProgressWithDetail(JobId, f64, String, String),
    Completed(JobId, SuccessData),
    Failed(JobId, String),
    Cancelled(JobId),
}
```

### Location (File Path)

**File**: `rwf-lib/src/model/location.rs`

```rust
pub enum Location {
    Local(PathBuf),
    Archive { archive_path: PathBuf, inner_path: PathBuf },
    // ... network locations
}
```

---

## Job Lifecycle

```
User presses "9"
  ↓
Action::CountDownJob → Transition::CreateAndStartCountDownJob
  ↓
update_state(): Creates BackgroundJob (Pending) + JobSpec
  ↓
WorkerPool receives JobSpec → executes job
  ↓
JobEvent::Started → Status: Pending → Running
  ↓
JobEvent::ProgressWithDetail (every 1s) → UI updates
  ↓
Job completes → JobEvent::Completed
  ↓
mark_job_completed(): Status → Completed, schedules cleanup (now + 5s)
  ↓
[5-6 seconds later]
  ↓
cleanup_expired_jobs(): Removes job from HashMap
```

---

## Adding New Job Types

### Step 1: Add JobKind Variant

**File**: `rwf-lib/src/job.rs`

```rust
pub enum JobKind {
    // ... existing ...
    MyNewJob { param1: String, param2: u32 },
}
```

### Step 2: Implement Executor

**File**: `rwf-lib/src/job/job_executor.rs`

```rust
async fn execute_my_new_job(&self, param1: String, param2: u32, spec: &JobSpec) -> OpResult {
    for step in 0..param2 {
        if spec.cancel_token.is_cancelled() { return OpResult::Cancelled; }
        
        let progress = step as f64 / param2 as f64;
        let _ = self.event_sender.send(JobEvent::ProgressWithDetail(
            spec.id, progress, format!("Step {}/{}", step, param2), param1.clone()
        ));
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    OpResult::Success(SuccessData::None)
}
```

### Step 3: Add to Dispatch

**File**: `rwf-lib/src/job/job_executor.rs`

```rust
let result = match &spec.kind {
    // ... existing ...
    JobKind::MyNewJob { param1, param2 } => {
        self.execute_my_new_job(param1.clone(), *param2, &spec).await
    }
};
```

### Step 4: Add State Transition

**File**: `rwf-lib/src/state.rs`

```rust
Transition::CreateMyNewJob { param1, param2 } => {
    let job_spec = JobSpec::new(JobKind::MyNewJob { param1: param1.clone(), param2 });
    state.background_jobs.start_job(
        format!("MyNewJob {}", param1), "Description",
        state.tabs.active_index, tab_name, job_spec.clone()
    );
    StateUpdateResult { jobs_to_start: vec![job_spec], ..StateUpdateResult::with_ui_change() }
}
```

### Step 5: Add Key Binding

**File**: `rwf-lib/src/input/mod.rs`

```rust
// Add to Action enum
MyNewJob,

// Add key binding
normal_mode.insert("F1".to_string(), Action::MyNewJob);

// Add transition mapping
Action::MyNewJob => vec![Transition::CreateMyNewJob { param1: "default".to_string(), param2: 10 }]
```

---

## File Operation Patterns

### Copy (Multiple Sources, Error Accumulation)

```rust
let total = sources.len();
let mut errors = Vec::new();

for (i, source) in sources.iter().enumerate() {
    if spec.cancel_token.is_cancelled() { return OpResult::Cancelled; }
    
    let progress = i as f64 / total as f64;
    let _ = self.event_sender.send(JobEvent::ProgressWithDetail(
        spec.id, progress, format!("Copying {}/{}", i+1, total), source.display_path()
    ));
    
    if let Err(e) = self.backend.copy_file(source, &dest, &spec.cancel_token).await {
        errors.push(format!("Failed to copy {}: {}", source.display_path(), e));
    }
}

if errors.is_empty() { OpResult::Success(SuccessData::None) }
else { OpResult::Failed(errors.join("\n")) }
```

### Delete (Skip Locked Files)

```rust
let total = targets.len();
let mut skipped = 0;

for (i, target) in targets.iter().enumerate() {
    if spec.cancel_token.is_cancelled() { return OpResult::Cancelled; }
    
    match self.backend.delete_file(target, &spec.cancel_token).await {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::Locked => skipped += 1,
        Err(e) => return OpResult::Failed(e.to_string()),
    }
}

OpResult::Success(SuccessData::None)
```

### Search (Streaming Results)

```rust
let mut results = Vec::new();
let mut searched = 0;

for entry in walkdir::WalkDir::new(location) {
    if spec.cancel_token.is_cancelled() { return OpResult::Cancelled; }
    searched += 1;
    
    let entry = match entry { Ok(e) => e, Err(_) => continue };
    if pattern.matches(entry.path()) { results.push(entry); }
}

OpResult::Success(SuccessData::SearchResults(results))
```

---

## Critical Patterns

### Cancellation Check (Required in All Jobs)

```rust
// At start of each iteration
if spec.cancel_token.is_cancelled() { return OpResult::Cancelled; }

// During long operations (e.g., sleep)
tokio::select! {
    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
    _ = async {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if spec.cancel_token.is_cancelled() { break; }
        }
    } => { return OpResult::Cancelled; }
}
```

### Progress Reporting

```rust
// Count-based: files_processed / total_files
let progress = files_processed as f64 / total_files as f64;

// Byte-based: bytes_processed / total_bytes
let progress = bytes_processed as f64 / total_bytes as f64;

// Send event
let _ = self.event_sender.send(JobEvent::ProgressWithDetail(
    job_id, progress,
    format!("Processing {}/{}", processed, total),
    current_item_name
));
```

### Error Handling

```rust
// Continue on error, report at end
let mut errors = Vec::new();
for item in items {
    match process(item) {
        Ok(_) => {}
        Err(e) => errors.push(e.to_string()),
    }
}
if errors.is_empty() { OpResult::Success(SuccessData::None) }
else { OpResult::Failed(errors.join("\n")) }
```

### Cleanup Scheduling (Automatic on Completion)

```rust
// In mark_job_completed(), mark_job_failed(), mark_job_cancelled():
let expiry_time = Instant::now() + self.cleanup_delay;
self.cleanup_queue.push(JobExpiry { expiry_time, job_id });
```

---

## User Controls

### Task Panel

| Key | Action |
|-----|--------|
| `T` | Toggle visibility |
| `Alt+Up/Down` | Scroll |
| `Ctrl+Up/Down` | Resize |

### Job Manager Dialog

| Key | Action |
|-----|--------|
| `Alt+J` | Open dialog |
| `Tab/Shift+Tab` | Cycle focus |
| `Up/Down` | Navigate list |
| `C` | Cancel selected job |
| `Enter` | Activate button |
| `Escape` | Close dialog |

---

## Cancelling Jobs

### Method 1: Job Manager Dialog
1. `Alt+J` to open
2. Select job with `Up/Down`
3. Press `C` or `Tab` → `Enter` on "Terminate Job"

### Method 2: Tab Close
1. `Ctrl+W` on tab with active jobs
2. Confirm dialog appears
3. `Enter` to confirm (cancels all jobs in tab)

### Method 3: Quit
- Press `q` → all active jobs automatically cancelled → app exits immediately

---

## Task Panel Logs

### Format
```
[HH:MM:SS] [Job #N] [Tab N] JobName: Message [TAG]
```

### Tags
| Tag | Color | Meaning |
|-----|-------|---------|
| `[OK]` | Green | Success |
| `[FAIL]` | Red | Error |
| `[WARN]` | Yellow | Cancelled |

### Example
```
[13:30:00] [Job #1] [Tab 1] CountDownJob 180: Started
[13:30:01] [Job #1] [Tab 1] CountDownJob 180: Countdown: 179/180 seconds
[13:30:05] [Job #1] [Tab 1] CountDownJob 180: Cancelled [WARN]
```

---

## Configuration

**File**: `config.json`

```json
{
  "JobManager": {
    "MaxSimultaneousJobs": 4,
    "JobRetentionPeriodSecs": 5,
    "MaxTaskPanelLogLines": 1000,
    "TaskPanelHeight": 10
  }
}
```

| Setting | Default | Description |
|---------|---------|-------------|
| `MaxSimultaneousJobs` | 4 | Concurrent job limit |
| `JobRetentionPeriodSecs` | 5 | Keep completed jobs for N seconds |
| `MaxTaskPanelLogLines` | 1000 | Max logs in memory |

---

## Debugging

### Enable Debug Logging
```bash
# PowerShell
$env:RUST_LOG="rwf=debug"
cargo run
```

### Key Log Messages

| Message | Meaning |
|---------|---------|
| `CreateAndStartCountDownJob: Creating...` | Job created |
| `Found X active jobs in tab N to cancel` | Multi-job cancellation |
| `Cleaned up X expired jobs` | Cleanup ran |
| `Cancelling all active jobs before shutdown...` | Quit with active jobs |

### Troubleshooting

| Problem | Check |
|---------|-------|
| Jobs don't disappear | Look for "Cleaned up X expired jobs" |
| App freezes on quit | Look for "Cancelling all active jobs..." |
| Only one log for multi-job cancel | Check "Found X active jobs" and "Cancelled X jobs total" |

---

## Common Pitfalls

### 1. Forgetting Cancellation Check
```rust
// WRONG: Job can't be cancelled
for i in 0..100 { do_work(); }

// RIGHT: Check every iteration
for i in 0..100 {
    if spec.cancel_token.is_cancelled() { return OpResult::Cancelled; }
    do_work();
}
```

### 2. Not Scheduling Cleanup
```rust
// WRONG: Job stays forever
pub fn mark_job_completed(&mut self, job_id: JobId) {
    job.status = JobStatus::Completed;
}

// RIGHT: Schedule cleanup
pub fn mark_job_completed(&mut self, job_id: JobId) {
    job.status = JobStatus::Completed;
    let expiry_time = Instant::now() + self.cleanup_delay;
    self.cleanup_queue.push(JobExpiry { expiry_time, job_id });
}
```

### 3. Using Wrong ID Type
- `JobId` - Internal tracking (worker pool, events)
- `BackgroundJobId` - UI display (has short_id for display)

### 4. Not Reporting Progress
```rust
// WRONG: UI shows 0% forever
for i in 0..100 { do_work(); }

// RIGHT: Report progress
for i in 0..100 {
    let progress = i as f64 / 100 as f64;
    let _ = self.event_sender.send(JobEvent::Progress(job_id, progress));
    do_work();
}
```

### 5. Blocking Async Executor
```rust
// WRONG: Blocks executor
std::thread::sleep(Duration::from_secs(1));

// RIGHT: Async sleep
tokio::time::sleep(Duration::from_secs(1)).await;
```

### 6. Forgetting to Set `active_job_id` on the Pane

When dispatching a `ReadDirectory` job via `.with_requesting_pane()`, you **must** also store
the job ID on the pane before returning. The `CompleteJob::ReadDirectory` handler checks
`pane.active_job_id == Some(job_id)` to reject stale results; if `active_job_id` is `None`
or a previous ID, the completion is silently discarded and `is_loading` stays `true` forever.

```rust
// WRONG: pane stuck loading even after job completes
pane_model.is_loading = true;
let job_spec = JobSpec::new(JobKind::ReadDirectory { location })
    .with_requesting_pane(tab_id, pane);
Some(StateUpdateResult::with_job(job_spec))

// RIGHT: completion handler can find and accept the result
pane_model.is_loading = true;
let job_spec = JobSpec::new(JobKind::ReadDirectory { location })
    .with_requesting_pane(tab_id, pane);
pane_model.active_job_id = Some(job_spec.id);  // ← required
Some(StateUpdateResult::with_job(job_spec))
```

Every place that sets `pane_model.is_loading = true` with a `ReadDirectory` job must also
set `pane_model.active_job_id`.

### 7. Dialog Confirm Handlers Must Forward Jobs from `update_state`

When `process_dialog_confirmation` calls `update_state()` with a transition that may enqueue
a job (e.g. any navigation transition), the returned `StateUpdateResult` must be forwarded to
the app loop. Calling `update_state()` and returning `None` silently drops the job — the pane
gets `is_loading = true` but nothing ever runs to clear it.

```rust
// WRONG: job is dropped, pane stuck loading
rwf_lib::state::update_state(state, Transition::NavigateToHistoryIndex { pane, index });
return None;

// RIGHT: forward the job so the app loop submits it
let result = rwf_lib::state::update_state(state, Transition::NavigateToHistoryIndex { pane, index });
return result.jobs_to_start.into_iter().next();
```

Reference implementation: `DriveSelection` confirm handler in
`rwf-bin/src/ui/dialog/mod.rs`.

---

## Testing

### Unit Test Pattern

```rust
#[tokio::test]
async fn test_my_new_job() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let executor = JobExecutor::new(backend, archive_handler, tx);
    
    let spec = JobSpec::new(JobKind::MyNewJob { param1: "test".to_string(), param2: 5 });
    executor.execute(spec).await;
    
    // Check events were sent
    assert!(matches!(rx.recv().await, Some(JobEvent::Started(_))));
    // ... check progress events ...
    assert!(matches!(rx.recv().await, Some(JobEvent::Completed(_, _))));
}
```

### Testing Cancellation

```rust
#[tokio::test]
async fn test_cancellation() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let executor = JobExecutor::new(backend, archive_handler, tx);
    
    let mut spec = JobSpec::new(JobKind::MyNewJob { param1: "test".to_string(), param2: 100 });
    spec.cancel_token.cancel();  // Cancel before execution
    
    executor.execute(spec).await;
    
    assert!(matches!(rx.recv().await, Some(JobEvent::Cancelled(_))));
}
```

### Testing Checklist

- [ ] Job starts (JobEvent::Started sent)
- [ ] Progress reported (JobEvent::ProgressWithDetail sent)
- [ ] Completion handled (JobEvent::Completed/Failed/Cancelled sent)
- [ ] Cancellation works (job exits early when cancelled)
- [ ] Errors reported (JobEvent::Failed with error message)
- [ ] Cleanup scheduled (job removed after retention period)

---

## Performance

### Concurrency Limits

| Setting | Recommended | Notes |
|---------|-------------|-------|
| `MaxSimultaneousJobs` | 4 | Increase for I/O-bound jobs, decrease for CPU-bound |
| `JobRetentionPeriodSecs` | 5 | Increase if users need to see completed jobs longer |

### Memory Usage

- Each `BackgroundJob`: ~200 bytes
- Cleanup queue: ~50 bytes per pending cleanup
- Log entries: ~100 bytes per entry (max 1000 = 100KB)

### When Cleanup Frees Memory

Jobs are removed from `jobs: HashMap` when:
1. Job completes/fails/cancels → scheduled for cleanup
2. `expiry_time <= now` → removed by `cleanup_expired_jobs()`
3. Rust's garbage collector frees memory

---

## Code Locations

| Component | File |
|-----------|------|
| JobKind, JobSpec, OpResult | `rwf-lib/src/job.rs` |
| BackgroundJob, cleanup | `rwf-lib/src/job/background_job_manager.rs` |
| Job execution | `rwf-lib/src/job/job_executor.rs` |
| Worker pool, JobEvent | `rwf-lib/src/worker_pool.rs` |
| State transitions | `rwf-lib/src/state.rs` |
| Main loop, cleanup check | `rwf-bin/src/app.rs` |
| Job Manager dialog | `rwf-bin/src/ui/dialog/job_manager.rs` |
| Task panel | `rwf-bin/src/ui/task_panel.rs` |
| Key bindings | `rwf-lib/src/input/mod.rs` |

---

## Glossary

| Term | Definition |
|------|------------|
| **Job** | Unit of work (e.g., copy files, countdown) |
| **JobSpec** | Job specification (what to execute + metadata) |
| **JobKind** | Type of job (Copy, Delete, CountDown, etc.) |
| **JobId** | Unique identifier for internal tracking |
| **BackgroundJob** | UI tracking structure (status, progress, etc.) |
| **BackgroundJobId** | UI identifier (contains JobId + display short_id) |
| **JobEvent** | Message from worker to UI (Started, Progress, Completed, etc.) |
| **WorkerPool** | Executes jobs asynchronously |
| **BackgroundJobManager** | Tracks UI state, schedules cleanup |
| **Transition** | State change operation (CreateJob, CancelJob, etc.) |
| **OpResult** | Job execution result (Success, Failed, Cancelled) |
| **SuccessData** | Data returned on successful completion |
| **Location** | File path (Local, Archive, Network) |
| **Cleanup Queue** | Priority queue for timed job removal |

---

## Implementation Details

### StateUpdateResult Structure

**File**: `rwf-lib/src/state.rs`

```rust
pub struct StateUpdateResult {
    pub jobs_to_start: Vec<JobSpec>,
    pub jobs_to_cancel: Vec<JobId>,
    pub completed_jobs: Vec<JobId>,
    pub failed_jobs: Vec<JobId>,
    pub cancelled_jobs: Vec<JobId>,
    pub started_jobs: Vec<JobId>,
    pub task_panel_logs: Vec<String>,
    pub panes_to_refresh: Vec<PaneRefresh>,
    pub ui_changed: bool,
}

impl StateUpdateResult {
    pub fn none() -> Self { /* No changes */ }
    pub fn with_ui_change() -> Self { /* ui_changed = true */ }
    pub fn with_refresh(tab_id: usize, pane: ActivePane) -> Self { /* targets specific pane */ }
    pub fn with_job(spec: JobSpec) -> Self { /* jobs_to_start = vec![spec] */ }
    pub fn with_cancel(job_id: JobId) -> Self { /* jobs_to_cancel = vec![job_id] */ }
}
```

**Usage**:
```rust
// No state changes
StateUpdateResult::none()

// UI needs refresh
StateUpdateResult::with_ui_change()

// Targeted pane refresh (prevents global redraws)
StateUpdateResult::with_refresh(tab_id, pane)

// Start a job
StateUpdateResult {
    jobs_to_start: vec![job_spec],
    ..StateUpdateResult::with_ui_change()
}
```

> **Warning**: `update_state()` returns `StateUpdateResult`. If you call it from a dialog
> confirm handler and discard the return value, any `jobs_to_start` inside are silently lost.
> Always capture the result and return `result.jobs_to_start.into_iter().next()` when the
> underlying transition may enqueue a job. See pitfall #7 below.

### Transition Enum (All Variants)

**File**: `rwf-lib/src/state.rs`

```rust
pub enum Transition {
    // Job creation
    CreateBackgroundJob { spec: JobSpec, name: String, description: String },
    CreateAndStartCountDownJob { spec: JobSpec, name: String, description: String },
    
    // Job lifecycle
    JobStarted { job_id: JobId },
    UpdateJobProgress { job_id: JobId, progress: f64 },
    UpdateJobProgressWithDetail { job_id: JobId, progress: f64, progress_message: String, operation_detail: String },
    CompleteJob { job_id: JobId, result: OpResult },
    CancelJob { job_id: JobId },
    AcknowledgeCancel { job_id: JobId },
    
    // Job queue management
    StartNextJob,
    
    // Tab management
    CloseTab { index: usize },
    NextTab,
    PrevTab,
    SwitchTab { index: usize },
    
    // UI state
    ToggleTaskPanel,
    IncreaseTaskPanelHeight,
    DecreaseTaskPanelHeight,
    ScrollTaskPanelUp,
    ScrollTaskPanelDown,
    
    // ... more variants as needed
}
```

**When to Create New Variant**:
- New state change pattern → Create new variant
- Similar to existing → Reuse existing with different parameters
- Only data differs → Add fields to existing variant

### CancellationToken API

**File**: `rwf-lib/src/job/cancellation_token.rs` (or similar)

```rust
#[derive(Clone)]
pub struct CancellationToken {
    // Internal implementation (Arc-based flag)
}

impl CancellationToken {
    pub fn new() -> Self { /* Creates non-cancelled token */ }
    pub fn is_cancelled(&self) -> bool { /* Check if cancelled */ }
    pub fn cancel(&self) { /* Set cancelled flag */ }
}
```

**Usage Pattern**:
```rust
// JobSpec creates token automatically
let spec = JobSpec::new(JobKind::MyJob { ... });

// Check in job executor
if spec.cancel_token.is_cancelled() {
    return OpResult::Cancelled;
}

// Cancel from UI
state.background_jobs.cancel_job(job_id);  // Calls cancel_token.cancel() internally
```

### Backend Trait

**File**: `rwf-lib/src/backend/mod.rs`

```rust
#[async_trait]
pub trait Backend {
    async fn copy_file(&self, source: &Location, dest: &Location, cancel_token: &CancellationToken) -> Result<(), Error>;
    async fn move_file(&self, source: &Location, dest: &Location, cancel_token: &CancellationToken) -> Result<(), Error>;
    async fn delete_file(&self, target: &Location, cancel_token: &CancellationToken) -> Result<(), Error>;
    async fn read_directory(&self, location: &Location, cancel_token: &CancellationToken) -> Result<Vec<FileEntry>, Error>;
    async fn create_dir(&self, location: &Location, cancel_token: &CancellationToken) -> Result<(), Error>;
    async fn rename(&self, from: &Location, to: &Location, cancel_token: &CancellationToken) -> Result<(), Error>;
    // ... more methods as needed
}

pub enum ErrorKind {
    Locked,
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    // ... more error types
}
```

### JobSpec Constructor

**File**: `rwf-lib/src/job.rs`

```rust
impl JobSpec {
    pub fn new(kind: JobKind) -> Self {
        Self {
            id: JobId::new(),              // Generates new UUID
            kind,
            created_at: SystemTime::now(),
            cancel_token: CancellationToken::new(),
        }
    }
}
```

### BackgroundJobManager::start_job()

**File**: `rwf-lib/src/job/background_job_manager.rs`

```rust
pub fn start_job(
    &mut self,
    name: String,
    description: String,
    tab_id: usize,
    tab_name: String,
    job_spec: JobSpec,
) -> BackgroundJobId {
    let short_id = self.next_short_id;  // Sequential for display
    self.next_short_id += 1;
    
    let bg_job = BackgroundJob {
        id: BackgroundJobId { uuid: job_spec.id, short_id },
        name,
        description,
        status: JobStatus::Pending,
        progress_percent: 0.0,
        progress_message: String::new(),
        current_operation_detail: String::new(),
        start_time: SystemTime::now(),
        end_time: None,
        cancel_token: job_spec.cancel_token.clone(),
        tab_id,
        tab_name,
    };
    
    self.jobs.insert(job_spec.id, bg_job);
    BackgroundJobId { uuid: job_spec.id, short_id }
}
```

**Note**: `tab_name` typically comes from `state.current_tab().left_pane.current_location.display_path()`.

### JobExecutor Constructor

**File**: `rwf-lib/src/job/job_executor.rs`

```rust
impl JobExecutor {
    pub fn new(
        backend: Box<dyn Backend>,
        archive_handler: Arc<dyn ArchiveHandler>,
        event_sender: mpsc::UnboundedSender<JobEvent>,
    ) -> Self {
        Self { backend, archive_handler, event_sender }
    }
}
```

---

## Import Templates

### For New Job Type

**File**: `rwf-lib/src/job.rs`
```rust
// No additional imports needed (JobKind is defined here)
```

**File**: `rwf-lib/src/job/job_executor.rs`
```rust
use crate::job::{JobSpec, OpResult, SuccessData, CancellationToken};
use crate::backend::Backend;
use tokio::sync::mpsc;
use std::time::Duration;
```

**File**: `rwf-lib/src/state.rs`
```rust
use crate::job::{JobSpec, JobKind, StateUpdateResult};
use crate::model::dialog::Dialog;
```

**File**: `rwf-lib/src/input/mod.rs`
```rust
use crate::state::Transition;
```

---

## Compilation Order

Edit files in this sequence to avoid cascading errors:

1. **`rwf-lib/src/job.rs`** - Add JobKind variant
   - Run `cargo check`
   - Fix any errors

2. **`rwf-lib/src/job/job_executor.rs`** - Implement executor, add to dispatch
   - Run `cargo check`
   - Fix any errors

3. **`rwf-lib/src/state.rs`** - Add Transition variant, implement handler
   - Run `cargo check`
   - Fix any errors

4. **`rwf-lib/src/input/mod.rs`** - Add Action variant, key binding, transition mapping
   - Run `cargo check`
   - Fix any errors

5. **Full build** - `cargo build`
   - Fix any remaining errors

---

## Decision Trees

### Adding New Job Type

```
Need new job operation?
├─ YES → Add JobKind variant (job.rs)
│
├─ Does it need new state change logic?
│  ├─ YES → Add Transition variant (state.rs)
│  └─ NO → Reuse existing Transition
│
├─ Does user trigger it differently?
│  ├─ YES → Add Action variant (input/mod.rs)
│  └─ NO → Reuse existing Action
│
├─ Does it return new data type?
│  ├─ YES → Add SuccessData variant (job.rs)
│  └─ NO → Use SuccessData::None
│
└─ Verify: cargo check passes
```

### Error Handling Strategy

```
Operation can fail?
├─ Single file operation
│  ├─ Critical failure? → Return OpResult::Failed immediately
│  └─ Recoverable? → Continue, collect errors, report at end
│
├─ Multiple file operations
│  ├─ Continue on error? → Collect errors, report summary
│  └─ Stop on error? → Return first error immediately
│
└─ User cancellation? → Always check cancel_token, return OpResult::Cancelled
```

### Progress Reporting Strategy

```
Job has measurable progress?
├─ Count-based (files, items)? → progress = processed / total
├─ Byte-based (copy, download)? → progress = bytes_processed / total_bytes
├─ Time-based (countdown)? → progress = elapsed / total_duration
└─ Unknown total? → Use indeterminate progress or estimate
```

---

## Verification Checklist

After implementing new job type:

### Compilation
- [ ] `cargo check` passes with no errors
- [ ] No wildcard matches in JobKind dispatch (explicit matching)
- [ ] All imports resolve correctly

### Functionality
- [ ] Job can be started (via key binding or other trigger)
- [ ] Job appears in Job Manager dialog
- [ ] Progress updates are visible
- [ ] Job can be cancelled (via dialog or tab close)
- [ ] Cancelled job shows "[WARN]" in task panel
- [ ] Completed job shows "[OK]" in task panel
- [ ] Failed job shows "[FAIL]" in task panel

### Cleanup
- [ ] Cancelled job disappears after retention period (default 5-6 seconds)
- [ ] Completed job disappears after retention period
- [ ] Debug log shows "Cleaned up X expired jobs"

### Edge Cases
- [ ] Job cancels immediately if cancelled before start
- [ ] Multiple jobs can run concurrently (up to MaxSimultaneousJobs)
- [ ] App quits immediately even with active jobs

---

## Variable Origins

Common variables and where they come from:

| Variable | Origin | Example |
|----------|--------|---------|
| `state` | Function parameter | `fn update_state(state: &mut AppState, ...)` |
| `tab_name` | `state.current_tab().left_pane.current_location.display_path()` | `"C:\\Users\\user\\temp"` |
| `state.tabs.active_index` | TabManager field | `0` for first tab |
| `job_spec.id` | Generated by `JobSpec::new()` | `JobId(uuid)` |
| `short_id` | Sequential counter in BackgroundJobManager | `1, 2, 3, ...` |
| `cancel_token` | Created in `JobSpec::new()` | `CancellationToken::new()` |
| `backend` | Injected into JobExecutor | `Box<dyn Backend>` |

---

## Type Dependency Graph

```
JobKind ─┬─> JobSpec ──> JobExecutor ──> OpResult ──> JobEvent
         │      │           │               │            │
         │      │           │               │            └─> BackgroundJobManager
         │      │           │               │                      │
         │      │           │               └─> SuccessData        │
         │      │           │                                       │
         │      │           └─> CancellationToken <─────────────────┘
         │      │
         │      └─> BackgroundJob (via start_job())
         │
         └─> Location (used in variants)
```

**Key Relationships**:
- `JobKind` is contained in `JobSpec`
- `JobSpec` is executed by `JobExecutor`
- `JobExecutor` sends `JobEvent` to UI
- `BackgroundJobManager` tracks `BackgroundJob` for UI display
- `CancellationToken` is shared between `JobSpec` and `JobExecutor` for cancellation

---

## References

- **TWF Background Operations**: `specs/twf/BACKGROUND_OPERATIONS.md`
- **Dialog Design Spec**: `docs/DIALOG_DESIGN_SPEC.md`
- **Plan Document**: `plan/plan_job_dialog.md`
