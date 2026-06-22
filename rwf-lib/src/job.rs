//! Job management for background file operations
//!
//! This module implements the JobManager and related types for managing
//! asynchronous file operations via the rwf Worker Pool.

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, Duration};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use crate::model::Location;

/// Unique identifier for a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Generate a new unique job ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Job specification submitted to the worker pool
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub id: JobId,
    pub kind: JobKind,
    pub created_at: SystemTime,
    pub cancel_token: CancellationToken,
    pub conflict_decisions: Option<Vec<ConflictDecision>>,
    pub requesting_pane: Option<(usize, crate::model::ActivePane)>,
}

/// Decision for resolving a file conflict
#[derive(Debug, Clone)]
pub struct ConflictDecision {
    pub source: Location,
    pub dest: Location,
    pub action: ConflictAction,
}

/// Action to take for a file conflict
#[derive(Debug, Clone)]
pub enum ConflictAction {
    Force,
    OverwriteIfNewer,
    Skip,
    Rename { new_name: String },
}

impl JobSpec {
    pub fn new(kind: JobKind) -> Self {
        Self {
            id: JobId::new(), // Generate unique UUID immediately
            kind,
            created_at: SystemTime::now(),
            cancel_token: CancellationToken::new(),
            conflict_decisions: None,
            requesting_pane: None,
        }
    }

    pub fn with_requesting_pane(mut self, tab_idx: usize, pane: crate::model::ActivePane) -> Self {
        self.requesting_pane = Some((tab_idx, pane));
        self
    }
}

/// Types of background jobs
#[derive(Debug, Clone, PartialEq)]
pub enum JobKind {
    ReadDirectory {
        location: Location,
    },
    Copy {
        sources: Vec<Location>,
        dest: Location,
    },
    Move {
        sources: Vec<Location>,
        dest: Location,
    },
    Delete {
        targets: Vec<Location>,
    },
    Mkdir {
        location: Location,
    },
    Rename {
        from: Location,
        to: Location,
    },
    CalculateSize {
        location: Location,
    },
    ExtractArchive {
        archive: Location,
        dest: Location,
    },
    CreateArchive {
        sources: Vec<Location>,
        dest: Location,
        original_size: u64,
    },
    ExecuteCustomFunction {
        command: String,
        working_dir: Location,
        pipe_to_action: Option<PipeToAction>,
        shell: Option<String>,
    },
    /// Spawn a program directly (no shell), avoiding cmd.exe quote-mangling.
    /// `program` is the executable name/path; `args` are its arguments.
    SpawnProcess {
        program: String,
        args: Vec<String>,
    },
    /// Run a terminal (TUI) program by suspending rwf, handing it the terminal,
    /// and resuming when the program exits. Intercepted in the app layer — never
    /// reaches the worker pool.
    SuspendAndRun {
        program: String,
        args: Vec<String>,
    },
    Search {
        location: Location,
        pattern: String,
        recursive: bool,
    },
    LoadFileForViewer {
        location: Location,
        /// true = text mode (build newline index); false = hex mode
        index_lines: bool,
        /// Files larger than this (in bytes) use Seekable mode instead of InMemory.
        large_file_threshold: usize,
    },
    /// Background search over the viewer's file (hex or text mode).
    ViewerSearch {
        location: Location,
        /// Pre-computed migemo regex (text mode only); None = plain query.
        migemo_pattern: Option<String>,
        query: String,
        is_hex_mode: bool,
        encoding: crate::model::viewer::TextEncoding,
        case_sensitive: bool,
        large_file_threshold: usize,
    },
    PatternRename {
        targets: Vec<Location>,
        find: String,
        replace: String,
        use_regex: bool,
        case_sensitive: bool,
    },
    CompareFiles {
        left: Location,
        right: Location,
    },
    SplitFile {
        source: Location,
        dest_dir: Location,
        chunk_size: u64,
    },
    JoinFiles {
        parts: Vec<Location>,
        dest: Location,
    },
    /// Countdown test job for testing job management features
    CountDown {
        duration_secs: u32,
        start_value: u32,
    },
    /// Collect jump-navigation candidates by walking the filesystem
    CollectJumpCandidates {
        root: String,
        include_files: bool,
        max_results: usize,
        max_depth: usize,
    },
}

/// Action to perform with custom function output
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PipeToAction {
    JumpToPath,
    ExecuteFile,
    ExecuteFileWithEditor,
}

/// Active job with execution state
#[derive(Debug, Clone)]
pub struct Job {
    pub spec: JobSpec,
    pub state: ExecutionState,
    pub progress: f64,
    pub started_at: Option<SystemTime>,
    pub cancel_requested: bool,
}

/// Job execution state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionState {
    Pending,
    Running,
    Cancelling,
    Completed,
}

/// Result of a completed job
#[derive(Debug, Clone)]
pub struct JobResult {
    pub id: JobId,
    pub kind: JobKind,
    pub completed_at: SystemTime,
    pub result: OpResult,
}

/// Operation result
#[derive(Debug, Clone)]
pub enum OpResult {
    Success(SuccessData),
    Failed(String),
    Cancelled,
}

/// Success data for different operation types
#[derive(Debug, Clone)]
pub enum SuccessData {
    DirectoryRead(Vec<crate::model::FileEntry>),
    SizeCalculated(u64),
    CustomFunctionOutput(String),
    SearchResults(Vec<crate::model::FileEntry>),
    FileContents(Vec<u8>),
    ComparisonResult(FileDiff),
    JumpCandidates(Vec<String>),
    None,
}

/// File comparison result
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub left_path: String,
    pub right_path: String,
    pub differences: Vec<DiffChunk>,
}

/// A chunk of differences in file comparison
#[derive(Debug, Clone)]
pub struct DiffChunk {
    pub left_start: usize,
    pub left_lines: Vec<String>,
    pub right_start: usize,
    pub right_lines: Vec<String>,
    pub chunk_type: DiffType,
}

/// Type of difference
#[derive(Debug, Clone, PartialEq)]
pub enum DiffType {
    Equal,
    Modified,
    Added,
    Deleted,
}

// ============================================================================
// Background Job Types (for UI display)
// ============================================================================

/// Unique identifier for a background job with display-friendly short ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackgroundJobId {
    pub uuid: JobId,
    pub short_id: u32,
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
#[derive(Debug, Clone)]
pub struct BackgroundJob {
    pub id: BackgroundJobId,
    pub name: String,
    pub description: String,
    pub status: JobStatus,
    pub progress_percent: f64,
    pub progress_message: String,
    pub current_operation_detail: String,
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
    
    /// Get status character for display
    pub fn status_char(&self) -> char {
        match self.status {
            JobStatus::Pending => 'P',
            JobStatus::Running => 'R',
            JobStatus::Completed => 'C',
            JobStatus::Failed => 'F',
            JobStatus::Cancelled => 'X',
        }
    }
}

/// Progress update for jobs
#[derive(Debug, Clone)]
pub struct JobProgress {
    pub percent: f64,
    pub message: String,
    pub current_operation_detail: String,
}

/// Manages background job queue and execution
#[derive(Debug)]
pub struct JobManager {
    /// FIFO queue of pending jobs
    pub queue: VecDeque<JobSpec>,
    /// Currently executing jobs
    pub active: HashMap<JobId, Job>,
    /// Recently completed jobs
    pub completed: VecDeque<JobResult>,
    /// Maximum parallel jobs
    pub max_parallel: usize,
    /// Performance metrics
    total_enqueued: u64,
    total_completed: u64,
    total_cancelled: u64,
    total_failed: u64,
}

impl JobManager {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(32), // Pre-allocate for common case
            active: HashMap::with_capacity(max_parallel),
            completed: VecDeque::with_capacity(100),
            max_parallel,
            total_enqueued: 0,
            total_completed: 0,
            total_cancelled: 0,
            total_failed: 0,
        }
    }
    
    /// Enqueue a new job and return its ID
    pub fn enqueue(&mut self, spec: JobSpec) -> JobId {
        let id = spec.id;
        self.queue.push_back(spec);
        self.total_enqueued += 1;
        id
    }
    
    /// Enqueue multiple jobs in batch (more efficient than individual enqueues)
    pub fn enqueue_batch(&mut self, specs: Vec<JobSpec>) -> Vec<JobId> {
        let mut ids = Vec::with_capacity(specs.len());
        for spec in specs {
            ids.push(spec.id);
            self.queue.push_back(spec);
            self.total_enqueued += 1;
        }
        ids
    }
    
    /// Check if a new job can be started
    pub fn can_start_job(&self) -> bool {
        self.active.len() < self.max_parallel && !self.queue.is_empty()
    }
    
    /// Pop the next job from the FIFO queue
    pub fn pop_next_job(&mut self) -> Option<JobSpec> {
        self.queue.pop_front()
    }
    
    /// Pop multiple jobs from the queue (batch operation)
    pub fn pop_next_jobs(&mut self, count: usize) -> Vec<JobSpec> {
        let available = (self.max_parallel - self.active.len()).min(self.queue.len()).min(count);
        let mut jobs = Vec::with_capacity(available);
        for _ in 0..available {
            if let Some(spec) = self.queue.pop_front() {
                jobs.push(spec);
            }
        }
        jobs
    }
    
    /// Mark a job as started
    pub fn start_job(&mut self, spec: JobSpec) {
        // Deduplication: Avoid duplicate ReadDirectory jobs for the same pane.
        if let JobKind::ReadDirectory { .. } = &spec.kind {
            if self.active.values().any(|j| j.spec.kind == spec.kind && j.spec.requesting_pane == spec.requesting_pane) {
                tracing::info!("[JobManager] Job {:?} (kind={:?}, pane={:?}) already active, skipping.", spec.id, spec.kind, spec.requesting_pane);
                return;
            }
        }

        let job = Job {
            spec: spec.clone(),
            state: ExecutionState::Pending,
            progress: 0.0,
            started_at: None,
            cancel_requested: false,
        };
        self.active.insert(spec.id, job);
    }
    
    /// Mark multiple jobs as started (batch operation)
    pub fn start_jobs(&mut self, specs: Vec<JobSpec>) {
        for spec in specs {
            let job = Job {
                spec: spec.clone(),
                state: ExecutionState::Pending,
                progress: 0.0,
                started_at: None,
                cancel_requested: false,
            };
            self.active.insert(spec.id, job);
        }
    }
    
    /// Update job progress
    pub fn update_progress(&mut self, job_id: JobId, progress: f64) {
        if let Some(job) = self.active.get_mut(&job_id) {
            job.progress = progress;
        }
    }
    
    /// Mark a job as completed
    pub fn complete_job(&mut self, job_id: JobId, result: OpResult) {
        if let Some(job) = self.active.remove(&job_id) {
            // Update statistics
            match &result {
                OpResult::Success(_) => self.total_completed += 1,
                OpResult::Failed(_) => self.total_failed += 1,
                OpResult::Cancelled => self.total_cancelled += 1,
            }
            
            let job_result = JobResult {
                id: job_id,
                kind: job.spec.kind.clone(),
                completed_at: SystemTime::now(),
                result,
            };
            self.completed.push_back(job_result);
            
            // Keep only last 100 completed jobs
            if self.completed.len() > 100 {
                self.completed.pop_front();
            }
        }
    }
    
    /// Request cancellation of a job
    pub fn request_cancel(&mut self, job_id: JobId) -> bool {
        // Cancel active job
        if let Some(job) = self.active.get_mut(&job_id) {
            job.spec.cancel_token.cancel();
            job.state = ExecutionState::Cancelling;
            job.cancel_requested = true;
            return true;
        }
        
        // Remove from queue
        if let Some(pos) = self.queue.iter().position(|spec| spec.id == job_id) {
            let spec = self.queue.remove(pos).unwrap();
            spec.cancel_token.cancel();
            
            self.total_cancelled += 1;
            
            let job_result = JobResult {
                id: job_id,
                kind: spec.kind,
                completed_at: SystemTime::now(),
                result: OpResult::Cancelled,
            };
            self.completed.push_back(job_result);
            return true;
        }
        
        false
    }
    
    /// Acknowledge job cancellation
    pub fn acknowledge_cancel(&mut self, job_id: JobId) {
        if let Some(job) = self.active.remove(&job_id) {
            self.total_cancelled += 1;
            
            let job_result = JobResult {
                id: job_id,
                kind: job.spec.kind,
                completed_at: SystemTime::now(),
                result: OpResult::Cancelled,
            };
            self.completed.push_back(job_result);
        }
    }
    
    /// Get job manager statistics
    pub fn stats(&self) -> JobManagerStats {
        JobManagerStats {
            queued: self.queue.len(),
            active: self.active.len(),
            completed_recent: self.completed.len(),
            max_parallel: self.max_parallel,
            total_enqueued: self.total_enqueued,
            total_completed: self.total_completed,
            total_cancelled: self.total_cancelled,
            total_failed: self.total_failed,
        }
    }
    
    /// Clear completed jobs older than the specified duration
    pub fn cleanup_old_completed(&mut self, max_age: Duration) {
        let now = SystemTime::now();
        self.completed.retain(|result| {
            if let Ok(elapsed) = now.duration_since(result.completed_at) {
                elapsed < max_age
            } else {
                true // Keep if we can't determine age
            }
        });
    }
}

/// Job manager statistics
#[derive(Debug, Clone)]
pub struct JobManagerStats {
    pub queued: usize,
    pub active: usize,
    pub completed_recent: usize,
    pub max_parallel: usize,
    pub total_enqueued: u64,
    pub total_completed: u64,
    pub total_cancelled: u64,
    pub total_failed: u64,
}

pub mod job_executor;
pub mod background_job_manager;

#[cfg(test)]
mod job_properties;

pub use job_executor::JobExecutor;
pub use job_executor::detect_conflicts;
pub use background_job_manager::{BackgroundJobManager, BackgroundJobEvent, BackgroundJobStats};
