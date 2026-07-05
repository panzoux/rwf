//! Background Job Manager for UI display
//!
//! This module provides a higher-level abstraction over the internal JobManager,
//! tracking user-visible metadata (name, description, tab info) and maintaining
//! JobStatus for UI display.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, Semaphore};

use crate::job::{BackgroundJob, BackgroundJobId, JobId, JobProgress, JobSpec, JobStatus};
use std::fmt;

/// Events from BackgroundJobManager to UI
#[derive(Debug, Clone)]
pub enum BackgroundJobEvent {
    Started(BackgroundJob),
    Updated(BackgroundJob),
    Completed(BackgroundJob),
    Failed(BackgroundJob, String),
    Cancelled(BackgroundJob),
}

/// Job expiry entry for priority queue (min-heap ordered by expiry_time)
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JobExpiry {
    pub expiry_time: Instant,
    pub job_id: JobId,
}

// Implement Ord for min-heap (earliest expiry first)
impl Ord for JobExpiry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering so earliest expiry is at top
        other.expiry_time.cmp(&self.expiry_time)
    }
}

impl PartialOrd for JobExpiry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Manages background jobs for UI display
pub struct BackgroundJobManager {
    /// Map from internal JobId to BackgroundJob
    jobs: HashMap<JobId, BackgroundJob>,
    /// Semaphore for concurrency limiting
    semaphore: Arc<Semaphore>,
    /// Sequential ID counter for display
    next_short_id: u32,
    /// Event channel for job updates
    event_tx: mpsc::UnboundedSender<BackgroundJobEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<BackgroundJobEvent>>,
    /// Maximum parallel jobs
    max_parallel: usize,
    /// Priority queue for job cleanup (min-heap ordered by expiry_time)
    cleanup_queue: BinaryHeap<JobExpiry>,
    /// How long to keep completed/cancelled jobs before cleanup
    cleanup_delay: Duration,
}

impl fmt::Debug for BackgroundJobManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackgroundJobManager")
            .field("jobs", &self.jobs.len())
            .field("next_short_id", &self.next_short_id)
            .field("max_parallel", &self.max_parallel)
            .finish()
    }
}

impl BackgroundJobManager {
    /// Create a new BackgroundJobManager
    pub fn new(max_parallel: usize, cleanup_delay: Duration) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let semaphore = Arc::new(Semaphore::new(max_parallel));

        Self {
            jobs: HashMap::new(),
            semaphore,
            next_short_id: 1,
            event_tx,
            event_rx: Some(event_rx),
            max_parallel,
            cleanup_queue: BinaryHeap::new(),
            cleanup_delay,
        }
    }

    /// Create a new BackgroundJobManager with default cleanup delay (5 seconds)
    pub fn with_default_cleanup(max_parallel: usize) -> Self {
        Self::new(max_parallel, Duration::from_secs(5))
    }

    /// Get the event receiver (can only be called once)
    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<BackgroundJobEvent>> {
        self.event_rx.take()
    }

    /// Get the event sender for cloning
    pub fn get_event_sender(&self) -> mpsc::UnboundedSender<BackgroundJobEvent> {
        self.event_tx.clone()
    }

    /// Get the semaphore for job execution
    pub fn get_semaphore(&self) -> Arc<Semaphore> {
        self.semaphore.clone()
    }

    /// Start a new background job
    pub fn start_job(
        &mut self,
        name: String,
        description: String,
        tab_id: usize,
        tab_name: String,
        job_spec: JobSpec,
    ) -> BackgroundJobId {
        let short_id = self.next_short_id;
        self.next_short_id += 1;

        let background_job_id = BackgroundJobId {
            uuid: job_spec.id,
            short_id,
        };

        let background_job = BackgroundJob {
            id: background_job_id,
            name: name.clone(),
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

        // Send Started event
        let _ = self
            .event_tx
            .send(BackgroundJobEvent::Started(background_job.clone()));

        // Store job
        self.jobs.insert(job_spec.id, background_job);

        background_job_id
    }

    /// Update job status to Running
    pub fn mark_job_running(&mut self, job_id: JobId) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.status = JobStatus::Running;
            let _ = self.event_tx.send(BackgroundJobEvent::Updated(job.clone()));
        }
    }

    /// Update job progress
    pub fn update_progress(&mut self, job_id: JobId, progress: JobProgress) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.progress_percent = progress.percent;
            job.progress_message = progress.message;
            job.current_operation_detail = progress.current_operation_detail;
            let _ = self.event_tx.send(BackgroundJobEvent::Updated(job.clone()));
        }
    }

    /// Mark job as completed
    pub fn mark_job_completed(&mut self, job_id: JobId) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.status = JobStatus::Completed;
            job.progress_percent = 100.0;
            job.end_time = Some(SystemTime::now());

            // Schedule cleanup
            let expiry_time = Instant::now() + self.cleanup_delay;
            self.cleanup_queue.push(JobExpiry {
                expiry_time,
                job_id,
            });

            let _ = self
                .event_tx
                .send(BackgroundJobEvent::Completed(job.clone()));
        }
    }

    /// Mark job as failed
    pub fn mark_job_failed(&mut self, job_id: JobId, error: String) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.status = JobStatus::Failed;
            job.progress_message = error.clone();
            job.end_time = Some(SystemTime::now());

            // Schedule cleanup
            let expiry_time = Instant::now() + self.cleanup_delay;
            self.cleanup_queue.push(JobExpiry {
                expiry_time,
                job_id,
            });

            let _ = self
                .event_tx
                .send(BackgroundJobEvent::Failed(job.clone(), error));
        }
    }

    /// Mark job as cancelled
    pub fn mark_job_cancelled(&mut self, job_id: JobId) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.status = JobStatus::Cancelled;
            job.end_time = Some(SystemTime::now());

            // Schedule cleanup
            let expiry_time = Instant::now() + self.cleanup_delay;
            self.cleanup_queue.push(JobExpiry {
                expiry_time,
                job_id,
            });

            let _ = self
                .event_tx
                .send(BackgroundJobEvent::Cancelled(job.clone()));
        }
    }

    /// Cancel a job by ID
    pub fn cancel_job(&mut self, job_id: JobId) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            if job.is_active() {
                job.cancel_token.cancel();
                job.status = JobStatus::Cancelled;
                job.end_time = Some(SystemTime::now());

                // Schedule cleanup (same as mark_job_cancelled)
                let expiry_time = Instant::now() + self.cleanup_delay;
                self.cleanup_queue.push(JobExpiry {
                    expiry_time,
                    job_id,
                });

                let _ = self
                    .event_tx
                    .send(BackgroundJobEvent::Cancelled(job.clone()));
            }
        }
    }

    /// Cleanup jobs that have expired (called every 1 second)
    /// Returns the number of jobs cleaned up
    pub fn cleanup_expired_jobs(&mut self) -> usize {
        let now = Instant::now();
        let mut cleaned = 0;

        // Remove all expired jobs from heap
        while let Some(job_expiry) = self.cleanup_queue.peek() {
            if job_expiry.expiry_time > now {
                break; // No more expired jobs
            }

            // Clone job_id before popping to avoid borrow issues
            let job_id = job_expiry.job_id;

            // Remove from heap
            self.cleanup_queue.pop();

            // Remove from jobs map if still exists and is not active
            if let Some(job) = self.jobs.get(&job_id) {
                if !job.is_active() {
                    self.jobs.remove(&job_id);
                    cleaned += 1;
                }
            }
        }

        cleaned
    }

    /// Get all active jobs
    pub fn get_active_jobs(&self) -> impl Iterator<Item = &BackgroundJob> {
        self.jobs.values().filter(|j| j.is_active())
    }

    /// Get all jobs (active + completed)
    pub fn get_all_jobs(&self) -> impl Iterator<Item = &BackgroundJob> {
        self.jobs.values()
    }

    /// Get a specific job by ID
    pub fn get_job(&self, job_id: JobId) -> Option<&BackgroundJob> {
        self.jobs.get(&job_id)
    }

    /// Get a mutable reference to a specific job by ID
    pub fn get_job_mut(&mut self, job_id: JobId) -> Option<&mut BackgroundJob> {
        self.jobs.get_mut(&job_id)
    }

    /// Check if a tab has active jobs
    pub fn is_tab_busy(&self, tab_id: usize) -> bool {
        self.jobs
            .values()
            .any(|j| j.tab_id == tab_id && j.is_active())
    }

    /// Get count of active jobs for a tab
    pub fn get_active_job_count(&self, tab_id: usize) -> usize {
        self.jobs
            .values()
            .filter(|j| j.tab_id == tab_id && j.is_active())
            .count()
    }

    /// Get job by short ID
    pub fn get_job_by_short_id(&self, short_id: u32) -> Option<&BackgroundJob> {
        self.jobs.values().find(|j| j.id.short_id == short_id)
    }

    /// Get job by short ID (mutable)
    pub fn get_job_by_short_id_mut(&mut self, short_id: u32) -> Option<&mut BackgroundJob> {
        self.jobs.values_mut().find(|j| j.id.short_id == short_id)
    }

    /// Remove completed/failed/cancelled jobs older than specified duration
    pub fn cleanup_old_jobs(&mut self, max_age: std::time::Duration) {
        let now = SystemTime::now();
        self.jobs.retain(|_, job| {
            if !job.is_active() {
                if let Some(end_time) = job.end_time {
                    if let Ok(elapsed) = now.duration_since(end_time) {
                        return elapsed < max_age;
                    }
                }
            }
            true
        });
    }

    /// Get statistics
    pub fn stats(&self) -> BackgroundJobStats {
        let active = self.jobs.values().filter(|j| j.is_active()).count();
        let completed = self
            .jobs
            .values()
            .filter(|j| j.status == JobStatus::Completed)
            .count();
        let failed = self
            .jobs
            .values()
            .filter(|j| j.status == JobStatus::Failed)
            .count();
        let cancelled = self
            .jobs
            .values()
            .filter(|j| j.status == JobStatus::Cancelled)
            .count();
        let pending = self
            .jobs
            .values()
            .filter(|j| j.status == JobStatus::Pending)
            .count();

        BackgroundJobStats {
            total: self.jobs.len(),
            active,
            completed,
            failed,
            cancelled,
            pending,
            max_parallel: self.max_parallel,
        }
    }
}

/// Statistics for background jobs
#[derive(Debug, Clone)]
pub struct BackgroundJobStats {
    pub total: usize,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub pending: usize,
    pub max_parallel: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobKind, Location};
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_background_job_is_active() {
        let job = BackgroundJob {
            id: BackgroundJobId {
                uuid: JobId::new(),
                short_id: 1,
            },
            name: "Test".to_string(),
            description: String::new(),
            status: JobStatus::Running,
            progress_percent: 0.0,
            progress_message: String::new(),
            current_operation_detail: String::new(),
            start_time: SystemTime::now(),
            end_time: None,
            cancel_token: CancellationToken::new(),
            tab_id: 0,
            tab_name: String::new(),
        };

        assert!(job.is_active());

        let completed_job = BackgroundJob {
            status: JobStatus::Completed,
            ..job
        };
        assert!(!completed_job.is_active());
    }

    #[test]
    fn test_status_char() {
        let mut job = BackgroundJob {
            id: BackgroundJobId {
                uuid: JobId::new(),
                short_id: 1,
            },
            name: "Test".to_string(),
            description: String::new(),
            status: JobStatus::Pending,
            progress_percent: 0.0,
            progress_message: String::new(),
            current_operation_detail: String::new(),
            start_time: SystemTime::now(),
            end_time: None,
            cancel_token: CancellationToken::new(),
            tab_id: 0,
            tab_name: String::new(),
        };

        assert_eq!(job.status_char(), 'P');

        job.status = JobStatus::Running;
        assert_eq!(job.status_char(), 'R');

        job.status = JobStatus::Completed;
        assert_eq!(job.status_char(), 'C');

        job.status = JobStatus::Failed;
        assert_eq!(job.status_char(), 'F');

        job.status = JobStatus::Cancelled;
        assert_eq!(job.status_char(), 'X');
    }

    #[test]
    fn test_background_job_manager_start_job() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        let job_spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });

        let job_id = manager.start_job(
            "Test Job".to_string(),
            "Test Description".to_string(),
            0,
            "Tab 1".to_string(),
            job_spec.clone(),
        );

        assert_eq!(job_id.short_id, 1);

        let job = manager.get_job(job_spec.id).unwrap();
        assert_eq!(job.name, "Test Job");
        assert_eq!(job.status, JobStatus::Pending);
    }

    #[test]
    fn test_background_job_manager_get_active_jobs() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        // Create two active jobs
        let spec1 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });
        let spec2 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp2")),
        });

        manager.start_job(
            "Job 1".to_string(),
            String::new(),
            0,
            String::new(),
            spec1.clone(),
        );
        manager.start_job(
            "Job 2".to_string(),
            String::new(),
            0,
            String::new(),
            spec2.clone(),
        );

        assert_eq!(manager.get_active_job_count(0), 2);
        assert!(manager.is_tab_busy(0));
        assert!(!manager.is_tab_busy(1));
    }

    #[test]
    fn test_background_job_manager_cancel_job() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });

        manager.start_job(
            "Job".to_string(),
            String::new(),
            0,
            String::new(),
            spec.clone(),
        );
        manager.mark_job_running(spec.id);

        assert!(manager.is_tab_busy(0));

        manager.cancel_job(spec.id);

        let job = manager.get_job(spec.id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[test]
    fn test_background_job_manager_update_progress() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });

        manager.start_job(
            "Job".to_string(),
            String::new(),
            0,
            String::new(),
            spec.clone(),
        );
        manager.mark_job_running(spec.id);

        let progress = JobProgress {
            percent: 50.0,
            message: "Processing...".to_string(),
            current_operation_detail: "file.txt".to_string(),
        };
        manager.update_progress(spec.id, progress);

        let job = manager.get_job(spec.id).unwrap();
        assert_eq!(job.progress_percent, 50.0);
        assert_eq!(job.progress_message, "Processing...");
        assert_eq!(job.current_operation_detail, "file.txt");
    }

    #[test]
    fn test_background_job_manager_mark_completed() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });

        manager.start_job(
            "Job".to_string(),
            String::new(),
            0,
            String::new(),
            spec.clone(),
        );
        manager.mark_job_completed(spec.id);

        let job = manager.get_job(spec.id).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.progress_percent, 100.0);
        assert!(job.end_time.is_some());
    }

    #[test]
    fn test_background_job_manager_mark_failed() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });

        manager.start_job(
            "Job".to_string(),
            String::new(),
            0,
            String::new(),
            spec.clone(),
        );
        manager.mark_job_failed(spec.id, "Error occurred".to_string());

        let job = manager.get_job(spec.id).unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.progress_message, "Error occurred");
    }

    #[test]
    fn test_background_job_manager_get_job_by_short_id() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        let spec1 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });
        let spec2 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp2")),
        });

        manager.start_job(
            "Job 1".to_string(),
            String::new(),
            0,
            String::new(),
            spec1.clone(),
        );
        manager.start_job(
            "Job 2".to_string(),
            String::new(),
            0,
            String::new(),
            spec2.clone(),
        );

        let job1 = manager.get_job_by_short_id(1);
        let job2 = manager.get_job_by_short_id(2);

        assert!(job1.is_some());
        assert!(job2.is_some());
        assert_eq!(job1.unwrap().name, "Job 1");
        assert_eq!(job2.unwrap().name, "Job 2");

        // Non-existent short_id
        let job3 = manager.get_job_by_short_id(99);
        assert!(job3.is_none());
    }

    #[test]
    fn test_background_job_manager_stats() {
        let mut manager = BackgroundJobManager::with_default_cleanup(4);

        // Create jobs with different statuses
        let spec1 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp")),
        });
        let spec2 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp2")),
        });
        let spec3 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/tmp3")),
        });

        manager.start_job(
            "Job 1".to_string(),
            String::new(),
            0,
            String::new(),
            spec1.clone(),
        );
        manager.start_job(
            "Job 2".to_string(),
            String::new(),
            0,
            String::new(),
            spec2.clone(),
        );
        manager.start_job(
            "Job 3".to_string(),
            String::new(),
            0,
            String::new(),
            spec3.clone(),
        );

        manager.mark_job_completed(spec1.id);
        manager.mark_job_failed(spec2.id, "error".to_string());

        let stats = manager.stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.active, 1); // Job 3 is still pending
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.pending, 1);
    }
}
