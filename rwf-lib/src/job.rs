//! Job management for background file operations
//!
//! This module implements the JobManager and related types for managing
//! asynchronous file operations via the rwf Worker Pool.

use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;
use tokio_util::sync::CancellationToken;
use crate::model::Location;

/// Unique identifier for a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

/// Job specification submitted to the worker pool
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub id: JobId,
    pub kind: JobKind,
    pub created_at: SystemTime,
    pub cancel_token: CancellationToken,
}

impl JobSpec {
    pub fn new(kind: JobKind) -> Self {
        Self {
            id: JobId(0), // Will be assigned by JobManager
            kind,
            created_at: SystemTime::now(),
            cancel_token: CancellationToken::new(),
        }
    }
}

/// Types of background jobs
#[derive(Debug, Clone)]
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
    },
    ExecuteCustomFunction {
        command: String,
        working_dir: Location,
        pipe_to_action: Option<PipeToAction>,
        shell: Option<String>,
    },
    Search {
        location: Location,
        pattern: String,
        recursive: bool,
    },
    LoadFileForViewer {
        location: Location,
    },
    PatternRename {
        targets: Vec<Location>,
        pattern: String,
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
}

/// Job execution state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionState {
    Running,
    Cancelling,
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
    /// Next job ID
    next_id: u64,
}

impl JobManager {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            active: HashMap::new(),
            completed: VecDeque::new(),
            max_parallel,
            next_id: 0,
        }
    }
    
    /// Enqueue a new job and return its ID
    pub fn enqueue(&mut self, mut spec: JobSpec) -> JobId {
        spec.id = JobId(self.next_id);
        self.next_id += 1;
        let id = spec.id;
        self.queue.push_back(spec);
        id
    }
    
    /// Check if a new job can be started
    pub fn can_start_job(&self) -> bool {
        self.active.len() < self.max_parallel && !self.queue.is_empty()
    }
    
    /// Pop the next job from the FIFO queue
    pub fn pop_next_job(&mut self) -> Option<JobSpec> {
        self.queue.pop_front()
    }
    
    /// Mark a job as started
    pub fn start_job(&mut self, spec: JobSpec) {
        let job = Job {
            spec: spec.clone(),
            state: ExecutionState::Running,
            progress: 0.0,
            started_at: Some(SystemTime::now()),
        };
        self.active.insert(spec.id, job);
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
            return true;
        }
        
        // Remove from queue
        if let Some(pos) = self.queue.iter().position(|spec| spec.id == job_id) {
            let spec = self.queue.remove(pos).unwrap();
            spec.cancel_token.cancel();
            
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
            let job_result = JobResult {
                id: job_id,
                kind: job.spec.kind,
                completed_at: SystemTime::now(),
                result: OpResult::Cancelled,
            };
            self.completed.push_back(job_result);
        }
    }
}

pub mod job_executor;

#[cfg(test)]
mod job_properties;

pub use job_executor::JobExecutor;
