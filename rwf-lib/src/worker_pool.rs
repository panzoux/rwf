//! Worker Pool for asynchronous job execution
//!
//! This module implements the rwf Worker Pool that executes jobs
//! asynchronously on a fixed number of worker threads.

use crate::job::{JobSpec, JobId, JobExecutor};
use crate::backend::{FilesystemBackend, ArchiveHandler};
use tokio::sync::mpsc;
use std::sync::Arc;

/// Events sent from workers to the UI thread
#[derive(Debug, Clone)]
pub enum JobEvent {
    /// Job has started execution
    Started(JobId),
    /// Job progress update (job_id, progress 0.0-1.0)
    Progress(JobId, f64),
    /// Job completed successfully
    Completed(JobId, crate::job::SuccessData),
    /// Job failed with error
    Failed(JobId, String),
    /// Job was cancelled
    Cancelled(JobId),
}

/// Worker pool managing background job execution
pub struct WorkerPool {
    workers: Vec<tokio::task::JoinHandle<()>>,
    job_sender: mpsc::UnboundedSender<JobSpec>,
    event_receiver: mpsc::UnboundedReceiver<JobEvent>,
}

impl WorkerPool {
    /// Create a new worker pool with the specified number of workers
    ///
    /// # Arguments
    ///
    /// * `worker_count` - Number of worker threads to spawn (default: 4)
    /// * `backend` - The filesystem backend to use for operations
    /// * `archive_handler` - The archive handler for archive operations
    ///
    /// # Returns
    ///
    /// A new WorkerPool instance ready to accept jobs
    pub fn new<B: FilesystemBackend + 'static, A: ArchiveHandler + 'static>(
        worker_count: usize,
        backend: Arc<B>,
        archive_handler: Arc<A>,
    ) -> Self {
        let (job_sender, job_receiver) = mpsc::unbounded_channel::<JobSpec>();
        let (event_sender, event_receiver) = mpsc::unbounded_channel::<JobEvent>();
        
        // Create a shared receiver that all workers can pull from
        let job_receiver = Arc::new(tokio::sync::Mutex::new(job_receiver));
        
        let mut workers = Vec::new();
        
        for worker_id in 0..worker_count {
            let job_rx = Arc::clone(&job_receiver);
            let event_tx = event_sender.clone();
            let backend = Arc::clone(&backend);
            let archive_handler = Arc::clone(&archive_handler);
            
            let handle = tokio::spawn(async move {
                tracing::debug!("Worker {} started", worker_id);
                
                // Create a JobExecutor for this worker
                let executor = JobExecutor::new(backend, archive_handler, event_tx);
                
                loop {
                    // Lock the receiver and try to get a job
                    let spec = {
                        let mut rx = job_rx.lock().await;
                        rx.recv().await
                    };
                    
                    match spec {
                        Some(spec) => {
                            tracing::debug!("Worker {} executing job {:?}", worker_id, spec.id);
                            
                            // Execute the job using the executor
                            executor.execute(spec).await;
                        }
                        None => {
                            tracing::debug!("Worker {} shutting down", worker_id);
                            break;
                        }
                    }
                }
            });
            
            workers.push(handle);
        }
        
        Self {
            workers,
            job_sender,
            event_receiver,
        }
    }
    
    /// Submit a job to the worker pool
    ///
    /// Jobs are executed in FIFO order by available workers.
    ///
    /// # Arguments
    ///
    /// * `spec` - The job specification to execute
    pub fn submit_job(&self, spec: JobSpec) {
        if let Err(e) = self.job_sender.send(spec) {
            tracing::error!("Failed to submit job: {}", e);
        }
    }
    
    /// Try to receive a job event without blocking
    ///
    /// Returns None if no events are available.
    pub fn try_recv_event(&mut self) -> Option<JobEvent> {
        self.event_receiver.try_recv().ok()
    }
    
    /// Receive a job event, waiting if necessary
    ///
    /// Returns None if the event channel is closed.
    pub async fn recv_event(&mut self) -> Option<JobEvent> {
        self.event_receiver.recv().await
    }
    
    /// Shutdown the worker pool
    ///
    /// Drops the job sender, causing all workers to exit after completing
    /// their current jobs.
    pub async fn shutdown(self) {
        // Drop the sender to signal workers to exit
        drop(self.job_sender);
        
        // Wait for all workers to finish
        for worker in self.workers {
            if let Err(e) = worker.await {
                tracing::error!("Worker panicked during shutdown: {}", e);
            }
        }
        
        tracing::info!("Worker pool shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobSpec, JobKind};
    use crate::model::Location;
    use crate::backend::LocalFilesystemBackend;
    use std::path::PathBuf;
    use crate::backend::MockArchiveHandler;

    #[tokio::test]
    async fn test_worker_pool_creation() {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let pool = WorkerPool::new(4, backend, archive_handler);
        
        // Pool should be created successfully
        assert_eq!(pool.workers.len(), 4);
    }

    #[tokio::test]
    async fn test_job_submission() {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let mut pool = WorkerPool::new(2, backend, archive_handler);
        
        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test")),
        });
        
        let job_id = spec.id;
        pool.submit_job(spec);
        
        // Should receive started event
        let event = pool.recv_event().await;
        assert!(matches!(event, Some(JobEvent::Started(id)) if id == job_id));
        
        // Should receive completed or failed event (directory may not exist)
        let event = pool.recv_event().await;
        assert!(matches!(event, Some(JobEvent::Completed(id, _) | JobEvent::Failed(id, _)) if id == job_id));
    }

    #[tokio::test]
    async fn test_multiple_jobs_fifo() {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let mut pool = WorkerPool::new(1, backend, archive_handler); // Single worker to ensure FIFO
        
        let spec1 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test1")),
        });
        let id1 = spec1.id;
        
        let spec2 = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test2")),
        });
        let id2 = spec2.id;
        
        pool.submit_job(spec1);
        pool.submit_job(spec2);
        
        // First job should start first
        let event = pool.recv_event().await;
        assert!(matches!(event, Some(JobEvent::Started(id)) if id == id1));
        
        // First job should complete or fail
        let event = pool.recv_event().await;
        assert!(matches!(event, Some(JobEvent::Completed(id, _) | JobEvent::Failed(id, _)) if id == id1));
        
        // Second job should start
        let event = pool.recv_event().await;
        assert!(matches!(event, Some(JobEvent::Started(id)) if id == id2));
        
        // Second job should complete or fail
        let event = pool.recv_event().await;
        assert!(matches!(event, Some(JobEvent::Completed(id, _) | JobEvent::Failed(id, _)) if id == id2));
    }

    #[tokio::test]
    async fn test_shutdown() {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let pool = WorkerPool::new(2, backend, archive_handler);
        
        // Shutdown should complete without hanging
        pool.shutdown().await;
    }
}
