# Job Manager Dialog & Task Panel Implementation Plan

## Document Status
- **Created**: 2026-03-14
- **Last Updated**: 2026-03-23
- **Version**: 2.0 (Revised for RWF Architecture)
- **Status**: Planning Phase
- **Estimated Effort**: 60-86 hours

---

# Part 0: Architecture Analysis & Design Decisions

## 0.1 TWF vs RWF Architecture Comparison

| Component | TWF (.NET/Terminal.Gui) | RWF (Rust/Ratatui) | Adaptation Strategy |
|-----------|-------------------------|--------------------|---------------------|
| **Job Status** | `JobStatus` enum (5 states) | `ExecutionState` (2 states) | Add `JobStatus` to `BackgroundJob` |
| **Event Model** | `event EventHandler` | `mpsc::UnboundedSender<JobEvent>` | Use channels, wrap in `BackgroundJob` |
| **Job Storage** | `ConcurrentDictionary` | `HashMap` + queue | Add `BackgroundJob` map to `JobManager` |
| **UI Framework** | Terminal.Gui (immediate mode) | Ratatui (retained mode) | State-based rendering |
| **Spinner** | `TaskStatusView.Tick()` | Not implemented | Add `tick()` to `TaskPanel` |
| **Dialog** | Modal `Dialog` class | Custom dialog stack | Use existing dialog infrastructure |

## 0.2 TWF Reference: Spinner Implementation

**TaskStatusView.cs (TWF):**
```csharp
// Configuration for busy spinner
private readonly string[] _spinnerFrames = { "|", "/", "-", "\\" };
private int _spinnerFrameIndex = 0;

public void Tick()
{
    _spinnerFrameIndex = (_spinnerFrameIndex + 1) % _spinnerFrames.Length;
}
```

**Usage in TabBarView:**
```csharp
// MainController updates spinner and passes to TabBarView
var spinner = taskStatusView.CurrentSpinnerFrame;
// Render spinner next to tab with active jobs
```

## 0.3 RWF Existing Structure

**Current Job System:**
```
rwf-lib/src/job.rs
├── JobId (UUID)
├── JobSpec (id, kind, created_at, cancel_token)
├── JobKind (enum of operation types)
├── Job (spec, state, progress, started_at)
├── ExecutionState (Running, Cancelling)
├── JobResult (id, kind, completed_at, result)
└── JobManager (queue, active, completed)

rwf-lib/src/job/job_executor.rs
└── JobExecutor::execute() - dispatches by JobKind
```

**Current UI Structure:**
```
rwf-bin/src/ui/
├── dialog/
│   ├── mod.rs          # Dialog handling, handle_dialog_input()
│   ├── compression.rs  # Compression dialog
│   └── extract_confirm.rs
├── task_panel.rs       # Current task panel (no state)
├── tab_bar.rs          # Tab bar with has_jobs indicator
└── ...
```

## 0.4 Key Design Decisions

### Decision 1: BackgroundJob as Wrapper Layer
Rather than replacing the existing `Job` struct, we add `BackgroundJob` as a **higher-level abstraction** that:
- Tracks user-visible metadata (name, description, tab info)
- Maintains `JobStatus` for UI display
- Wraps the internal `JobSpec`/`Job` for execution

### Decision 2: Dual Job Tracking
- **Internal**: `JobSpec` → `Job` → `JobResult` (existing, for worker pool)
- **External**: `BackgroundJob` (new, for UI display)

The `JobManager` maintains a mapping between internal job IDs and `BackgroundJob`.

### Decision 3: Event-Driven Updates
Use existing `JobEvent` channel, but add `BackgroundJobManager` that:
- Listens to `JobEvent::Started/Progress/Completed/Failed/Cancelled`
- Updates corresponding `BackgroundJob` state
- Provides query interface for UI (get active jobs, etc.)

### Decision 4: Spinner Animation in Main Loop
Following TWF's pattern:
- `TaskPanel` has `spinner_frames` and `spinner_index`
- `tick()` method advances the spinner
- Main loop calls `task_panel.tick()` each frame

---

# Part 1: TWF Reference Analysis

## 1.1 BackgroundJob Model (TWF)

```csharp
public class BackgroundJob
{
    public Guid Id { get; } = Guid.NewGuid();
    public int ShortId { get; } = Interlocked.Increment(ref _nextShortId);

    public string Name { get; set; } = string.Empty;
    public string Description { get; set; } = string.Empty;
    public JobStatus Status { get; set; } = JobStatus.Pending;
    public double ProgressPercent { get; set; }
    public string ProgressMessage { get; set; } = string.Empty;
    public string CurrentOperationDetail { get; set; } = string.Empty;
    public DateTime CurrentFileStartTime { get; set; } = DateTime.MinValue;
    public DateTime StartTime { get; set; } = DateTime.Now;
    public DateTime? EndTime { get; set; }
    public CancellationTokenSource CancellationTokenSource { get; set; }

    // Context info
    public int TabId { get; set; } = -1;
    public string TabName { get; set; } = string.Empty;

    public bool IsActive => Status == JobStatus.Pending || Status == JobStatus.Running;
}

public enum JobStatus { Pending, Running, Completed, Failed, Cancelled }
```

## 1.2 JobManager (TWF)

```csharp
public class JobManager
{
    private readonly ConcurrentDictionary<Guid, BackgroundJob> _jobs;
    private readonly SemaphoreSlim _concurrencySemaphore; // max_parallel jobs
    private int _updateIntervalMs = 300; // throttle UI updates

    public event EventHandler<BackgroundJob>? JobStarted;
    public event EventHandler<BackgroundJob>? JobUpdated;
    public event EventHandler<BackgroundJob>? JobCompleted;

    public BackgroundJob StartJob(string name, string description, int tabId, string tabName,
                                   Func<BackgroundJob, CancellationToken, IProgress<JobProgress>, Task> action)
    {
        // Creates job, enqueues, runs async with semaphore limiting
        // Progress reporting throttled to updateIntervalMs
    }

    public void CancelJob(Guid jobId);
    public IEnumerable<BackgroundJob> GetActiveJobs();
    public IEnumerable<BackgroundJob> GetAllJobs();
    public bool IsTabBusy(int tabId);
    public int GetActiveJobCount(int tabId);
}
```

## 1.3 TaskStatusView (TWF Task Panel)

**Features:**
- Located at bottom of main window
- Collapsed: 1 line (most recent log)
- Expanded: scrollable log history (default 10 lines)
- Log format: `[HH:MM:SS] [Job #N] [Tab N] JobName: Message [TAG]`
- Tags: `[OK]` (green), `[FAIL]` (red), `[WARN]` (yellow)
- Memory buffer: max 1000 lines
- Disk logging: flushes to `%APPDATA%/TWF/twf_tasks.log`
- Log rotation: keeps last N files
- Spinner animation for busy indication

**Key Bindings:**
- `Ctrl+L` - Toggle expand/collapse
- `Ctrl+Up/Down` - Resize panel height
- `Alt+Up/Down` - Scroll up/down
- `Ctrl+J` - Show Job Manager dialog

## 1.4 JobManagerDialog (TWF)

**Dimensions:** 64x24 fixed

**Layout:**
```
┌──────────────────────────────────────────────────────────────┐
│ Background Jobs                                              │
│                                                              │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ [>#1] [R] Copy: file1.txt... - 45%                       │ │ ← Job List (8 lines)
│ │ [ #2] [R] Delete: old.log... - 20%                       │ │
│ │ [ #3] [P] Move: Waiting... -                             │ │
│ │ [ #4] [F] Copy: failed.txt... - 0%                       │ │
│ │ [ #5] [X] Delete: Cancelled... -                         │ │
│ │                                                          │ │
│ │                                                          │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│ ──────────────────────────────────────────────────────────── │ ← Separator
│ Selected Job Details:                                        │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Job ID: a1b2c3d4-e5f6-7890-abcd-ef1234567890             │ │
│ │ Started: 18:22:15                                        │ │ ← Detail View (10 lines)
│ │ Status: Running                                          │ │
│ │ Progress: Copying file1.txt...                           │ │
│ │ Current File: C:\temp\very_long_filename...txt           │ │
│ │ Files: 45/100                                            │ │
│ │ Bytes: 10.5 MB / 25.0 MB                                 │ │
│ │                                                          │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│                                                              │
│                    [Close]  [Cancel Job]                     │ ← Buttons
└──────────────────────────────────────────────────────────────┘
```

**Features:**
- Timer-based refresh (500ms default)
- Smart filename truncation (preserves extension)
- Selection changes update detail view
- Cancel Job button shows confirmation dialog
- Status chars: R=Running, P=Pending, C=Completed, F=Failed, X=Cancelled

## 1.5 Tab Busy Indicators (TWF)

```
Tab Bar:
[1:temp* | 2:rwf// | 3:DSK* | 4:docs*]
              ^^
              2 slashes = 2 active jobs in tab 2
```

- Animated spinner (`|`, `/`, `-`, `\`) cycling
- Number of spinners = number of active jobs

---

# Part 2: Visual Mockups

## Mockup 1: Task Panel (Collapsed - 1 line)
```
┌─────────────────────────────────────────────────────────────┐
│ [1:temp* | 2:rwf]                                           │ ← Tab bar
│ C:\Users\user\temp                C:\Users\user\source\rwf  │ ← Path line
│ (C:)                              (C:) 3 Files 14.96 KB     │ ← Volume line
│ [file list...]                    [file list...]            │
│                                                             │
│ [QUEUED] Reading C:\Users\user\temp                         │ ← Task Panel (1 line)
└─────────────────────────────────────────────────────────────┘
```

## Mockup 2: Task Panel (Expanded - 10 lines)
```
┌─────────────────────────────────────────────────────────────┐
│ [1:temp* | 2:rwf]                                           │
│ C:\Users\user\temp                C:\Users\user\source\rwf  │
│ (C:)                              (C:) 3 Files 14.96 KB     │
│ [file list...]                    [file list...]            │
│                                                             │
│ [2026-03-14 18:22:15] [Job #1] [Tab 1] Copy: Starting...   │ ← Task Panel
│ [2026-03-14 18:22:16] [Job #1] [Tab 1] Copy: 5/100 files   │   (expanded,
│ [2026-03-14 18:22:17] [Job #1] [Tab 1] Copy: file1.txt [OK]│    10 lines)
│ [2026-03-14 18:22:18] [Job #1] [Tab 1] Copy: file2.txt [OK]│
│ [2026-03-14 18:22:19] [Job #2] [Tab 2] Delete: Starting... │
│ [2026-03-14 18:22:20] [Job #2] [Tab 2] Delete: old.log [OK]│
│ [2026-03-14 18:22:21] [Job #1] [Tab 1] Copy: file3.txt [OK]│
│ [2026-03-14 18:22:22] [Job #1] [Tab 1] Copy: Completed [OK]│
│ [2026-03-14 18:22:23] [Job #2] [Tab 2] Delete: Completed [OK]│
│ [2026-03-14 18:22:24] [Job #3] [Tab 1] Move: Starting...   │
└─────────────────────────────────────────────────────────────┘
```

## Mockup 3: Job Manager Dialog (64x24)
```
┌──────────────────────────────────────────────────────────────┐
│ Background Jobs                                              │
│                                                              │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ [>#1] [R] Copy: file1.txt... - 45%                       │ │ ← Job List
│ │ [ #2] [R] Delete: old.log... - 20%                       │ │   (8 lines)
│ │ [ #3] [P] Move: Waiting... -                             │ │
│ │ [ #4] [F] Copy: failed.txt... - 0%                       │ │
│ │ [ #5] [X] Delete: Cancelled... -                         │ │
│ │                                                          │ │
│ │                                                          │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│ ──────────────────────────────────────────────────────────── │ ← Separator
│ Selected Job Details:                                        │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Job ID: a1b2c3d4-e5f6-7890-abcd-ef1234567890             │ │
│ │ Started: 18:22:15                                        │ │ ← Detail View
│ │ Status: Running                                          │ │   (10 lines)
│ │ Progress: Copying file1.txt...                           │ │
│ │ Current File: C:\temp\very_long_filename...txt           │ │
│ │ Files: 45/100                                            │ │
│ │ Bytes: 10.5 MB / 25.0 MB                                 │ │
│ │                                                          │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│                                                              │
│                    [Close]  [Cancel Job]                     │ ← Buttons
└──────────────────────────────────────────────────────────────┘
```

## Mockup 4: Tab Busy Indicators
```
Tab Bar Line:
┌─────────────────────────────────────────────────────────────┐
│ [1:temp* | 2:rwf// | 3:DSK* | 4:docs* | 5:training... |misc*│
│                          ^^                                  │
│                          2 active jobs in tab 2              │
└─────────────────────────────────────────────────────────────┘
```

---

# Part 3: Implementation Phases (RWF-Adapted)

## Phase 1: Core Job Management Infrastructure

### 1.1 BackgroundJob Model
**File:** `rwf-lib/src/job/background_job.rs` (NEW)

```rust
use std::time::SystemTime;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Unique identifier for a background job (display purposes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackgroundJobId {
    pub uuid: Uuid,      // Internal tracking
    pub short_id: u32,   // Sequential for display (Job #N)
}

/// Job status for UI display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Background job with user-visible metadata
pub struct BackgroundJob {
    pub id: BackgroundJobId,
    pub name: String,
    pub description: String,
    pub status: JobStatus,
    pub progress_percent: f64,
    pub progress_message: String,
    pub current_operation_detail: String,  // Current file being processed
    pub start_time: SystemTime,
    pub end_time: Option<SystemTime>,
    pub cancel_token: CancellationToken,
    pub tab_id: usize,
    pub tab_name: String,
}

impl BackgroundJob {
    pub fn is_active(&self) -> bool {
        matches!(self.status, JobStatus::Pending | JobStatus::Running)
    }
}
```

### 1.2 BackgroundJobManager
**File:** `rwf-lib/src/job/background_job_manager.rs` (NEW)

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;
use crate::job::{JobSpec, JobEvent, JobManager};

pub struct BackgroundJobManager {
    /// Map from internal JobId to BackgroundJob
    jobs: HashMap<crate::job::JobId, BackgroundJob>,
    /// Semaphore for concurrency limiting
    semaphore: Arc<Semaphore>,
    /// Sequential ID counter for display
    next_short_id: u32,
    /// Event channel for job updates
    event_tx: mpsc::UnboundedSender<BackgroundJobEvent>,
    event_rx: mpsc::UnboundedReceiver<BackgroundJobEvent>,
}

/// Background job events for UI
pub enum BackgroundJobEvent {
    Started(BackgroundJob),
    Updated(BackgroundJob),
    Completed(BackgroundJob),
    Failed(BackgroundJob, String),
    Cancelled(BackgroundJob),
}

impl BackgroundJobManager {
    pub fn new(max_parallel: usize) -> Self;
    
    /// Start a new background job
    pub fn start_job(
        &mut self,
        name: String,
        description: String,
        tab_id: usize,
        tab_name: String,
        job_spec: JobSpec,
    ) -> BackgroundJobId;
    
    /// Cancel a job by ID
    pub fn cancel_job(&mut self, job_id: crate::job::JobId);
    
    /// Get all active jobs
    pub fn get_active_jobs(&self) -> impl Iterator<Item = &BackgroundJob>;
    
    /// Get all jobs (active + completed)
    pub fn get_all_jobs(&self) -> impl Iterator<Item = &BackgroundJob>;
    
    /// Check if a tab has active jobs
    pub fn is_tab_busy(&self, tab_id: usize) -> bool;
    
    /// Get count of active jobs for a tab
    pub fn get_active_job_count(&self, tab_id: usize) -> usize;
    
    /// Process internal JobEvent and update BackgroundJob state
    pub fn process_job_event(&mut self, event: &JobEvent);
    
    /// Get next background job event for UI
    pub fn poll_background_event(&mut self) -> Option<BackgroundJobEvent>;
}
```

### 1.3 JobProgress Update
**File:** `rwf-lib/src/job/mod.rs` (MODIFY)

```rust
/// Progress update for jobs
pub struct JobProgress {
    pub percent: f64,
    pub message: String,
    pub current_operation_detail: String,  // NEW: Current file name
}
```

**Tasks:**
- [ ] Create `BackgroundJob` struct with all fields
- [ ] Create `JobStatus` enum
- [ ] Create `BackgroundJobManager` with semaphore-based concurrency
- [ ] Implement job start/cancel/get methods
- [ ] Implement `process_job_event()` to sync with internal `JobEvent`
- [ ] Implement event channel for UI updates
- [ ] Write unit tests for job lifecycle

---

## Phase 2: Task Panel (TaskStatusView)

### 2.1 Task Panel State
**File:** `rwf-bin/src/ui/task_panel.rs` (MODIFY - add state)

```rust
use std::collections::VecDeque;
use std::time::SystemTime;

/// Log entry with timestamp and level
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub message: String,
    pub level: LogLevel,
}

/// Log level for colored tags
pub enum LogLevel {
    Info,    // Normal text
    Ok,      // [OK] - Green
    Fail,    // [FAIL] - Red
    Warn,    // [WARN] - Yellow
}

/// Task panel with state
pub struct TaskPanel {
    /// In-memory log buffer
    log_entries: Vec<LogEntry>,
    /// Pending logs from background thread
    pending_logs: Vec<String>,
    /// Panel expansion state
    is_expanded: bool,
    /// Expanded height in lines
    expanded_height: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Spinner animation frames (TWF reference)
    spinner_frames: [&'static str; 4],  // "|", "/", "-", "\\"
    /// Current spinner frame index
    spinner_index: usize,
}

impl TaskPanel {
    pub fn new() -> Self;
    
    /// Add a log entry
    pub fn add_log(&mut self, message: String, level: LogLevel);
    
    /// Add a pending log (thread-safe)
    pub fn add_pending_log(&mut self, message: String);
    
    /// Process pending logs
    pub fn process_pending_logs(&mut self, max_lines: usize);
    
    /// Scroll up
    pub fn scroll_up(&mut self);
    
    /// Scroll down
    pub fn scroll_down(&mut self);
    
    /// Scroll to end
    pub fn scroll_to_end(&mut self);
    
    /// Toggle expand/collapse
    pub fn toggle_expanded(&mut self);
    
    /// Resize panel (up)
    pub fn resize_up(&mut self, max_height: usize);
    
    /// Resize panel (down)
    pub fn resize_down(&mut self, min_height: usize);
    
    /// Advance spinner animation (TWF Tick() equivalent)
    pub fn tick(&mut self);
    
    /// Get current spinner frame
    pub fn current_spinner(&self) -> &'static str;
}
```

### 2.2 Log Format
```
[HH:MM:SS] [Job #N] [Tab N] JobName: Message [TAG]

Examples:
[18:22:15] [Job #1] [Tab 1] Copy: Starting...
[18:22:16] [Job #1] [Tab 1] Copy: file1.txt [OK]
[18:22:17] [Job #2] [Tab 2] Delete: failed [FAIL]
[18:22:18] [Job #3] [Tab 1] Move: cancelled [WARN]
```

### 2.3 Log Persistence
- Memory buffer: max 1000 lines (configurable)
- Disk logging: flush to `%APPDATA%/rwf/rwf_tasks.log`
- Log rotation: keep last N files (configurable)
- On exit: flush all remaining logs

**Tasks:**
- [ ] Convert `task_panel.rs` from function-only to stateful module
- [ ] Implement `TaskPanel` struct with all fields
- [ ] Implement `add_log()` with `LogLevel`
- [ ] Implement `process_pending_logs()` with max lines
- [ ] Implement scroll methods (up/down/end)
- [ ] Implement expand/collapse toggle
- [ ] Implement resize methods
- [ ] Implement `tick()` for spinner animation
- [ ] Implement log persistence (memory + disk)
- [ ] Implement log rotation
- [ ] Write unit tests for log buffering

---

## Phase 3: Job Manager Dialog

### 3.1 Dialog Structure
**File:** `rwf-bin/src/ui/dialog/job_manager.rs` (NEW)

```rust
use ratatui::Frame;
use rwf_lib::job::background_job_manager::BackgroundJobManager;

pub struct JobManagerDialog {
    /// Reference to job manager (borrowed from AppState)
    job_manager: *mut BackgroundJobManager,
    /// Cached job list for display
    jobs_list: Vec<BackgroundJob>,
    /// Selected job index
    selected_index: usize,
    /// Refresh timer counter
    refresh_counter: u32,
    /// Refresh interval (frames between refreshes)
    refresh_interval: u32,  // 500ms ≈ 15 frames at 30 FPS
}

impl JobManagerDialog {
    pub fn new(job_manager: &mut BackgroundJobManager) -> Self;
    
    /// Handle dialog input
    pub fn handle_input(&mut self, key: crossterm::event::KeyEvent) -> DialogAction;
    
    /// Render dialog
    pub fn render(&mut self, frame: &mut Frame, area: ratatui::layout::Rect);
    
    /// Refresh job list from manager
    fn refresh_list(&mut self);
    
    /// Update detail view for selected job
    fn update_detail_view(&self) -> String;
    
    /// Cancel selected job with confirmation
    fn cancel_selected_job(&mut self) -> bool;
}
```

### 3.2 Layout
- Dialog dimensions: 64x24 fixed
- Job list: 8 lines (with scrollbar if needed)
- Separator: 1 line
- Detail view: 10 lines (read-only text view)
- Buttons: 1 line (Close, Cancel Job)

### 3.3 Job List Format
```
[#{short_id}] [{status_char}] {truncated_name} - {percent}%

Status chars:
R = Running
P = Pending
C = Completed
F = Failed
X = Cancelled

Example: [>#1] [R] Copy: very_long_filename...txt - 45%
           [ #2] [R] Delete: old.log - 20%
```

### 3.4 Detail View Content
```
Job ID: a1b2c3d4-e5f6-7890-abcd-ef1234567890
Started: 18:22:15
Status: Running
Progress: Copying file1.txt...
Current File: C:\temp\very_long_filename...txt
Files: 45/100
Bytes: 10.5 MB / 25.0 MB
```

### 3.5 Features
- Timer-based refresh (500ms, configurable)
- Smart filename truncation (preserve extension)
- Selection change updates detail view
- Cancel Job shows confirmation dialog
- Close button closes dialog

**Tasks:**
- [ ] Create `JobManagerDialog` struct
- [ ] Implement job list rendering with status chars
- [ ] Implement detail view rendering
- [ ] Implement timer-based refresh (frame counter)
- [ ] Implement selection handling
- [ ] Implement Cancel Job with confirmation
- [ ] Implement smart filename truncation
- [ ] Write integration tests

---

## Phase 4: Tab Busy Indicators

### 4.1 Tab Bar Enhancement
**File:** `rwf-bin/src/ui/tab_bar.rs` (MODIFY)

```rust
// Current implementation already has has_jobs check
// Add spinner animation support

// For each tab:
let job_count = background_job_manager.get_active_job_count(tab_index);
let spinner = if job_count > 0 {
    task_panel.current_spinner()  // "|", "/", "-", or "\\"
} else { "" };

// Render spinners based on job count
// Option 1: Single spinner cycling
// Option 2: Multiple spinners (one per job) - TWF style
```

**Current RWF Implementation:**
```rust
// In tab_bar.rs already:
let has_jobs = state.jobs.active.values().any(|job| {
    matches_tab_location(job, &tab.left_pane.current_location)
        || matches_tab_location(job, &tab.right_pane.current_location)
});

// Format tab label with pane paths
let label = if has_jobs {
    format!(" [~{}:{}~] ", idx + 1, active_marker)  // ~ indicates busy
} else if is_active {
    format!(" [{}:{}] ", idx + 1, active_marker)
} else {
    format!(" {}:{} ", idx + 1, active_marker)
};
```

**Enhancement:**
- Replace `~` with actual spinner character
- Support multiple spinners for multiple jobs (TWF style: `//` = 2 jobs)

**Tasks:**
- [ ] Add spinner character to tab bar rendering
- [ ] Integrate with `TaskPanel::current_spinner()`
- [ ] Support multiple spinners per job count
- [ ] Test with multiple concurrent jobs

---

## Phase 5: Integration with Existing Job System

### 5.1 Job Executor Updates
**File:** `rwf-lib/src/job/job_executor.rs` (MODIFY)

The existing `JobExecutor` already sends `JobEvent` updates. We need to:
1. Ensure `JobEvent::Progress` includes `current_operation_detail`
2. Ensure `BackgroundJobManager` listens to these events

**Progress Reporting by Job Type:**

**Copy Operations:**
```rust
// Per-file update
let _ = event_sender.send(JobEvent::Progress(
    job_id,
    progress,
    Some(format!("Count: {}/{} - Current: {}", i + 1, total_files, filename)),
));
```

**Move Operations:**
```rust
// Same as copy
```

**Delete Operations:**
```rust
// Per-file update
let _ = event_sender.send(JobEvent::Progress(
    job_id,
    progress,
    Some(format!("Deleted: {}", filename)),
));
```

**Directory Size:**
```rust
// Progress callback already exists in calculate_directory_size_with_progress
// Just ensure it sends JobEvent::Progress
```

**Tasks:**
- [ ] Modify `execute_copy()` to report per-file progress with detail
- [ ] Modify `execute_move()` to report per-file progress with detail
- [ ] Modify `execute_delete()` to report per-file progress with detail
- [ ] Modify `execute_calculate_size()` to use existing progress callback
- [ ] Ensure `BackgroundJobManager::process_job_event()` handles all events
- [ ] Test progress reporting accuracy

---

## Phase 6: Configuration

### 6.1 Config Entries
**File:** `rwf-lib/src/config.rs` (MODIFY)

```rust
pub struct AppConfig {
    // ... existing fields ...
    
    // Job Manager settings
    pub max_simultaneous_jobs: usize,           // Default: 4
    pub update_interval_ms: u64,                // Default: 300, min: 100
    
    // Task Panel settings
    pub task_panel_default_height: usize,       // Default: 10
    pub task_panel_refresh_interval_ms: u64,    // Default: 500
    pub max_log_lines_in_memory: usize,         // Default: 1000
    pub log_save_path: String,                  // Default: "logs/session.log"
    pub max_log_files: usize,                   // Default: 5
    
    // Job Manager Dialog settings
    pub job_manager_refresh_interval_ms: u64,   // Default: 500
}
```

**Tasks:**
- [ ] Add job manager config fields to `AppConfig`
- [ ] Add config loading/saving
- [ ] Add default values
- [ ] Integrate with existing config system
- [ ] Write config tests

---

## Phase 7: Key Bindings

### 7.1 Default Key Bindings
**File:** `rwf-lib/src/input/mod.rs` (MODIFY)

```rust
// Job Management (TWF-compatible)
normal_mode.insert("Ctrl+J".to_string(), Action::ShowJobManager);
normal_mode.insert("Ctrl+L".to_string(), Action::ToggleTaskPanel);
normal_mode.insert("Ctrl+Up".to_string(), Action::ResizeTaskPanelUp);
normal_mode.insert("Ctrl+Down".to_string(), Action::ResizeTaskPanelDown);
normal_mode.insert("Alt+Up".to_string(), Action::ScrollTaskPanelUp);
normal_mode.insert("Alt+Down".to_string(), Action::ScrollTaskPanelDown);

// Alternative bindings (for terminals without Ctrl+Arrow)
normal_mode.insert("Alt+B".to_string(), Action::ScrollTaskPanelUp);
normal_mode.insert("Alt+F".to_string(), Action::ScrollTaskPanelDown);
normal_mode.insert("Alt+T".to_string(), Action::ToggleTaskPanel);
normal_mode.insert("Alt+U".to_string(), Action::ResizeTaskPanelUp);
normal_mode.insert("Alt+D".to_string(), Action::ResizeTaskPanelDown);
```

**Action Enum:**
```rust
pub enum Action {
    // ... existing ...
    ShowJobManager,
    ToggleTaskPanel,
    ResizeTaskPanelUp,
    ResizeTaskPanelDown,
    ScrollTaskPanelUp,
    ScrollTaskPanelDown,
}
```

**Tasks:**
- [ ] Add action variants to `Action` enum
- [ ] Add key bindings in `KeyBindings::default()`
- [ ] Implement action handlers in `app.rs`
- [ ] Test all key bindings

---

## Phase 8: UI Rendering

### 8.1 Task Panel Rendering
**File:** `rwf-bin/src/ui/task_panel.rs` (MODIFY)

```rust
pub fn render_task_panel(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    task_panel: &mut TaskPanel,
) {
    let height = if task_panel.is_expanded {
        task_panel.expanded_height
    } else {
        1
    };

    // Process pending logs before rendering
    task_panel.process_pending_logs(state.config.max_log_lines_in_memory);

    // Render log entries with colored tags
    // [OK] = Green, [FAIL] = Red, [WARN] = Yellow
    // Auto-scroll to bottom if already at bottom
}
```

### 8.2 Color Integration
Use config colors for:
- `ok_color` (Green) - for `[OK]` tags
- `warning_color` (Yellow) - for `[WARN]` tags
- `error_color` (Red) - for `[FAIL]` tags
- `foreground_color` / `background_color` - for normal text

### 8.3 Main Loop Integration
**File:** `rwf-bin/src/app.rs` (MODIFY)

```rust
// In main loop:
loop {
    // ... existing event processing ...
    
    // Update spinner animation
    if let Some(ref mut task_panel) = self.task_panel {
        task_panel.tick();
    }
    
    // Render UI
    self.render(terminal)?;
}
```

**Tasks:**
- [ ] Implement task panel rendering with state
- [ ] Implement collapsed/expanded modes
- [ ] Implement colored tag parsing
- [ ] Implement auto-scroll logic
- [ ] Integrate with color config
- [ ] Add `tick()` call to main loop
- [ ] Test rendering performance

---

## Phase 9: Testing

### 9.1 Unit Tests
- [ ] `BackgroundJob`: status transitions, `is_active()`
- [ ] `BackgroundJobManager`: start, cancel, get active jobs, concurrency limiting
- [ ] `TaskPanel`: add_log, scroll, persistence, tick
- [ ] Log formatting: tag parsing, color assignment

### 9.2 Integration Tests
- [ ] Copy files → verify job appears in Task Panel
- [ ] Cancel job → verify status changes to Cancelled
- [ ] Job completion → verify `[OK]` tag in log
- [ ] Job failure → verify `[FAIL]` tag in log
- [ ] Tab busy indicators → verify spinner count matches job count
- [ ] Job Manager dialog → verify job list and detail view

### 9.3 Performance Tests
- [ ] 100 concurrent jobs → verify throttling works
- [ ] 1000 log entries → verify memory buffer limit
- [ ] Log file rotation → verify old logs flushed correctly
- [ ] UI refresh rate → verify no lag with many jobs

---

# Part 4: Implementation Priority & Timeline

## Priority Order

1. **Phase 1** (Core Infrastructure) - **REQUIRED FIRST**
   - BackgroundJob model
   - BackgroundJobManager with concurrency control
   - Event channel integration

2. **Phase 5** (Integration with Existing Jobs) - **Can work alongside Phase 1**
   - Modify job executor progress reporting
   - Integrate with BackgroundJobManager

3. **Phase 2** (Task Panel) - **Visible progress tracking**
   - Log buffering and rendering
   - Persistence
   - Spinner animation

4. **Phase 3** (Job Manager Dialog) - **Advanced management**
   - Dialog UI
   - Job cancellation

5. **Phase 4** (Tab Indicators) - **Nice-to-have visual**
   - Spinner animation in tab bar
   - Multiple spinner support

6. **Phase 6-8** (Config, Keys, UI) - **Supporting features**
   - Configuration
   - Key bindings
   - Color integration
   - Main loop integration

7. **Phase 9** (Testing) - **Throughout development**
   - Unit tests
   - Integration tests
   - Performance tests

## Estimated Timeline

| Phase | Estimated Hours | Dependencies |
|-------|----------------|--------------|
| Phase 1: Core Infrastructure | 10-14 hours | None |
| Phase 5: Job Integration | 8-10 hours | Phase 1 |
| Phase 2: Task Panel | 12-16 hours | Phase 1 |
| Phase 3: Job Manager Dialog | 12-16 hours | Phase 1, 2 |
| Phase 4: Tab Indicators | 4-6 hours | Phase 1, 2 |
| Phase 6: Configuration | 4-6 hours | None |
| Phase 7: Key Bindings | 2-4 hours | None |
| Phase 8: UI Rendering | 8-10 hours | Phase 2, 3 |
| Phase 9: Testing | 10-14 hours | All phases |
| **TOTAL** | **70-96 hours** | |

**Realistic Timeline:** 2-3 weeks of focused development

---

# Part 5: Technical Decisions & Notes

## 5.1 Concurrency Model
- Use `tokio::sync::Semaphore` for limiting concurrent jobs
- Default: 4 parallel jobs (configurable)
- Jobs queue when semaphore is exhausted
- `BackgroundJobManager` wraps semaphore, not replaces existing `JobManager`

## 5.2 Dual Job Tracking
- **Internal**: `JobSpec` → `Job` → `JobResult` (worker pool)
- **External**: `BackgroundJob` (UI display)
- `BackgroundJobManager` maintains mapping between them
- `JobEvent` bridges internal and external

## 5.3 Progress Throttling
- Update UI every `update_interval_ms` (default: 300ms)
- Prevents UI lag from excessive updates
- Store latest progress, update UI on timer

## 5.4 Log Buffer Strategy
- In-memory: `Vec<LogEntry>` (max 1000 lines)
- Pending: `Vec<String>` for thread-safe writes
- Flush old logs to disk when buffer exceeds limit
- Append-only file with rotation

## 5.5 Cancellation Strategy
- Cooperative cancellation via `CancellationToken`
- Jobs check token periodically
- Cancelled jobs show `[WARN]` tag
- Cleanup partial operations on cancel

## 5.6 Error Handling
- Failed jobs show `[FAIL]` tag
- Store error message in `progress_message`
- Display error in Job Manager detail view

## 5.7 Tab Integration
- Each job tracks `tab_id` and `tab_name`
- Tab bar queries `background_job_manager.get_active_job_count(tab_id)`
- Spinner animation synchronized across all tabs via `TaskPanel::tick()`

## 5.8 Main Loop Integration
- `TaskPanel` state owned by `App` struct
- `tick()` called each frame (30 FPS)
- Dialog rendering uses existing dialog stack

---

# Part 6: Future Enhancements (Not in Initial Scope)

The following features are explicitly **out of scope** for this implementation:

- [ ] **Pause/resume jobs** - This is not a Job's scope. Job-handled jobs should be handled at each file operation (function) level.
- [ ] **Conflict resolution options (overwrite/skip/rename)** - File management apps don't need this complexity.
- [ ] **Job priority (high/normal/low)** - FIFO is sufficient.
- [ ] **Job dependencies (job B waits for job A)** - There should be no dependencies.
- [ ] **Retry failed jobs** - Disastrous for file operations.
- [ ] **Job templates (predefined operations)** - Not needed.
- [ ] **Export job log to file** - Job is always single operation.
- [ ] **Email/notification on job completion** - Not needed for TUI.
- [ ] **Job scheduling (run at specific time)** - Do it as requested (input).
- [ ] **Bandwidth throttling for copy/move** - Not needed.

---

# Appendix A: File Structure

```
rwf-lib/src/
├── job/
│   ├── mod.rs                      # JobProgress, JobId types (MODIFY)
│   ├── background_job.rs           # BackgroundJob struct, JobStatus enum (NEW)
│   ├── background_job_manager.rs   # BackgroundJobManager implementation (NEW)
│   └── job_executor.rs             # (existing) Modified for progress reporting
├── ui/
│   └── task_panel.rs               # TaskPanel state (MOVED from rwf-bin)
└── config.rs                       # AppConfig struct (MODIFY)

rwf-bin/src/
├── ui/
│   ├── dialog/
│   │   ├── mod.rs                  # (existing) Dialog handling
│   │   ├── job_manager.rs          # JobManagerDialog (NEW)
│   │   ├── compression.rs          # (existing)
│   │   └── extract_confirm.rs      # (existing)
│   ├── task_panel.rs               # (existing) Modified for state + rendering
│   └── tab_bar.rs                  # (existing) Modified for spinner
└── app.rs                          # (existing) Modified for task_panel state
```

---

# Appendix B: State Ownership

```
App struct (rwf-bin/src/app.rs)
├── state: AppState
│   ├── jobs: JobManager          # Internal job tracking (existing)
│   └── config: AppConfig
├── task_panel: TaskPanel         # NEW: Task panel state
└── background_job_manager: BackgroundJobManager  # NEW: Background job tracking

Dialog rendering
└── JobManagerDialog
    └── References background_job_manager (borrowed)
```

---

# Appendix C: Event Flow

```
JobExecutor
    │
    │ JobEvent::Started/Progress/Completed/Failed/Cancelled
    │
    ▼
BackgroundJobManager::process_job_event()
    │
    │ Updates BackgroundJob state
    │ Queues BackgroundJobEvent
    │
    ▼
TaskPanel::add_pending_log()
    │
    │ Processed in main loop
    │
    ▼
render_task_panel()
```

---

# Appendix D: Dialog Design Specification Updates

The `docs/DIALOG_DESIGN_SPEC.md` should be updated with:

## Part 6: Job Manager Dialog

### 6.1 Dialog Title
```
"Background Jobs"
```

### 6.2 Sections (In Tab Order)

| Order | Section | Type | Label | Content | Focus Field |
|-------|---------|------|-------|---------|-------------|
| 0 | Job List | List | N/A | 8 job entries | 0 |
| 1 | Detail View | Text | "Selected Job Details:" | Read-only text | N/A |
| 2 | Close Button | Button | N/A | `[*Close*]` (default) | 1 |
| 3 | Cancel Job Button | Button | N/A | `[Cancel Job]` | 2 |

### 6.3 Layout Constraints

```rust
pub fn get_job_manager_dialog_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Length(1),   // Title
        Constraint::Length(1),   // Spacing
        Constraint::Length(10),  // Job list: 8 items + borders
        Constraint::Length(1),   // Separator
        Constraint::Length(1),   // Detail label
        Constraint::Length(10),  // Detail view
        Constraint::Length(1),   // Spacing
        Constraint::Length(1),   // Buttons
    ]
}

pub fn calculate_job_manager_dialog_min_height() -> u16 {
    get_job_manager_dialog_constraints()
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum()
}
```

**Total Content Height:** 26 lines (calculated)
**Total Dialog Height:** 28 lines (26 content + 2 borders)

### 6.4 Job List Format

```
[#{short_id}] [{status_char}] {truncated_name} - {percent}%

Status chars:
R = Running
P = Pending
C = Completed
F = Failed
X = Cancelled

Selection indicator:
> = Selected item
  = Unselected

Example:
[>#1] [R] Copy: very_long_filename...txt - 45%
 [ #2] [R] Delete: old.log - 20%
 [ #3] [P] Move: Waiting... -
```

### 6.5 Key Bindings

| Key | Action | Scope |
|-----|--------|-------|
| Tab | Move focus (Job List → Close → Cancel) | All dialogs |
| Shift+Tab | Move focus reverse | All dialogs |
| Up | Move selection up in job list | Job list only |
| Down | Move selection down in job list | Job list only |
| Enter | Activate focused button | Buttons only |
| Escape | Close dialog | All dialogs |
| C | Cancel selected job (shortcut) | Job list only |

### 6.6 Complete Layout Mockup

```
┌──────────────────────────────────────────────┐  ← Line 1 (border + title)
│ Background Jobs                              │
│                                              │
│ ┌──────────────────────────────────────────┐ │  ← Lines 3-12 (job list)
│ │ [>#1] [R] Copy: file1.txt... - 45%       │ │
│ │ [ #2] [R] Delete: old.log... - 20%       │ │
│ │ [ #3] [P] Move: Waiting... -             │ │
│ │ [ #4] [F] Copy: failed.txt... - 0%       │ │
│ │ [ #5] [X] Delete: Cancelled... -         │ │
│ │                                          │ │
│ │                                          │ │
│ │                                          │ │
│ └──────────────────────────────────────────┘ │
│ ──────────────────────────────────────────── │  ← Line 13 (separator)
│ Selected Job Details:                        │  ← Line 14
│ ┌──────────────────────────────────────────┐ │  ← Lines 15-24 (detail)
│ │ Job ID: a1b2c3d4-e5f6-7890-abcd-ef123456 │ │
│ │ Started: 18:22:15                        │ │
│ │ Status: Running                          │ │
│ │ Progress: Copying file1.txt...           │ │
│ │ Current File: C:\temp\very_long_file...  │ │
│ │ Files: 45/100                            │ │
│ │ Bytes: 10.5 MB / 25.0 MB                 │ │
│ │                                          │ │
│ │                                          │ │
│ │                                          │ │
│ └──────────────────────────────────────────┘ │
│                                              │
│              [*Close*]  [Cancel Job]         │  ← Line 26 (buttons)
└──────────────────────────────────────────────┘  ← Lines 27-28 (bottom border)
```

**Total: 28 lines** (26 content + 2 borders)

---

# Appendix E: Lessons from TWF

## E.1 Spinner Implementation (TWF)

```csharp
// TaskStatusView.cs
private readonly string[] _spinnerFrames = { "|", "/", "-", "\\" };
private int _spinnerFrameIndex = 0;

public void Tick()
{
    _spinnerFrameIndex = (_spinnerFrameIndex + 1) % _spinnerFrames.Length;
}

public string CurrentSpinnerFrame => _spinnerFrames[_spinnerFrameIndex];
```

**RWF Adaptation:**
```rust
// task_panel.rs
pub struct TaskPanel {
    spinner_frames: [&'static str; 4],  // ["|", "/", "-", "\\"]
    spinner_index: usize,
}

impl TaskPanel {
    pub fn tick(&mut self) {
        self.spinner_index = (self.spinner_index + 1) % self.spinner_frames.len();
    }
    
    pub fn current_spinner(&self) -> &'static str {
        self.spinner_frames[self.spinner_index]
    }
}
```

## E.2 Log Format (TWF)

```csharp
// TaskStatusView.cs::AddLogEntry()
string idPrefix = shortId > 0 ? $"[Job #{shortId}]" : "";
string tabPrefix = job.TabId >= 0 ? $"[Tab {job.TabId+1}]" : "";
string prefix = $"{idPrefix}{tabPrefix}".Trim();

string message = $"{prefix}{job.Name}: {action}";

// Add status tag
if (job.Status == JobStatus.Completed) message += " [OK]";
else if (job.Status == JobStatus.Failed) message += " [FAIL]";
else if (job.Status == JobStatus.Cancelled) message += " [WARN]";
```

**RWF Adaptation:**
```rust
// task_panel.rs
fn format_log_entry(job: &BackgroundJob, action: &str) -> String {
    let timestamp = format_time(job.start_time);
    let job_id = format!("[Job #{}]", job.id.short_id);
    let tab_id = format!("[Tab {}]", job.tab_id + 1);
    let tag = match job.status {
        JobStatus::Completed => "[OK]",
        JobStatus::Failed => "[FAIL]",
        JobStatus::Cancelled => "[WARN]",
        _ => "",
    };
    
    format!(
        "[{}] {} {} {}: {} {}",
        timestamp, job_id, tab_id, job.name, action, tag
    )
}
```

## E.3 Filename Truncation (TWF)

```csharp
// JobManagerDialog.cs::RefreshList()
string truncatedName = CharacterWidthHelper.SmartTruncate(
    j.Name, 
    availableWidth, 
    _config.Display.Ellipsis
);
```

**RWF Adaptation:**
```rust
// job_manager.rs
use crate::ui::unicode_utils::smart_truncate;

let truncated_name = smart_truncate(&job.name, available_width, &config.ellipsis);
```

---

# Appendix F: Migration Notes

## F.1 Existing Code Compatibility

The existing `JobManager` and `JobExecutor` remain **unchanged** in their core functionality. The `BackgroundJobManager` is a **wrapper layer** that:
1. Listens to `JobEvent` from existing system
2. Maintains `BackgroundJob` for UI display
3. Provides query interface for UI components

## F.2 AppState Changes

```rust
// AppState (rwf-lib/src/state.rs)
pub struct AppState {
    pub jobs: JobManager,              // Existing
    pub background_jobs: BackgroundJobManager,  // NEW
    // ... other fields ...
}

// App (rwf-bin/src/app.rs)
pub struct App {
    state: AppState,
    task_panel: TaskPanel,             // NEW
    // ... other fields ...
}
```

## F.3 Main Loop Changes

```rust
// In app.rs::run()
loop {
    // Process job events
    let event_results = process_pending_events(pool, &mut self.state);
    
    // Update background job manager
    for result in &event_results {
        for event in &result.events {
            self.state.background_jobs.process_job_event(event);
        }
    }
    
    // Update spinner
    self.task_panel.tick();
    
    // Render
    self.render(terminal)?;
}
```

---

# Appendix G: Testing Strategy

## G.1 Unit Test Coverage

| Module | Test Cases |
|--------|-----------|
| `BackgroundJob` | `is_active()`, status transitions |
| `BackgroundJobManager` | start, cancel, get_active, concurrency |
| `TaskPanel` | add_log, scroll, tick, expand/collapse |
| `LogEntry` | formatting, color assignment |

## G.2 Integration Test Scenarios

1. **Copy Operation Flow**
   - Start copy job → Verify `BackgroundJob` created
   - Progress updates → Verify log entries added
   - Job completes → Verify `[OK]` tag

2. **Cancellation Flow**
   - Start job → Cancel via dialog → Verify `[WARN]` tag
   - Verify job status = Cancelled

3. **Tab Busy Indicators**
   - Start 2 jobs on tab 1 → Verify `//` in tab bar
   - Complete 1 job → Verify `/` in tab bar

## G.3 Performance Benchmarks

| Metric | Target | Measurement |
|--------|--------|-------------|
| Job startup latency | < 100ms | Time from submit to UI display |
| Progress update latency | < 300ms | Time from progress to UI update |
| Log buffer capacity | 1000 lines | Memory usage at max |
| Spinner frame rate | 30 FPS | Visual smoothness |
