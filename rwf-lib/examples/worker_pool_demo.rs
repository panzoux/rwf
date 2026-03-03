//! Worker Pool demonstration
//!
//! This example shows how to use the WorkerPool to execute jobs asynchronously.

use rwf_lib::{WorkerPool, JobSpec, JobKind, JobEvent};
use rwf_lib::backend::LocalFilesystemBackend;
use rwf_lib::archive::ZipArchiveHandler;
use rwf_lib::model::Location;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!("Creating worker pool with 4 workers...");
    let backend = Arc::new(LocalFilesystemBackend::new());
    let archive_handler = Arc::new(ZipArchiveHandler::new());
    let mut pool = WorkerPool::new(4, backend, archive_handler);
    
    // Submit some jobs
    println!("Submitting jobs...");
    
    let job1 = JobSpec::new(JobKind::ReadDirectory {
        location: Location::Local(PathBuf::from("/tmp")),
    });
    let id1 = job1.id;
    pool.submit_job(job1);
    
    let job2 = JobSpec::new(JobKind::Mkdir {
        location: Location::Local(PathBuf::from("/tmp/test_dir")),
    });
    let id2 = job2.id;
    pool.submit_job(job2);
    
    // Process events
    println!("Processing events...");
    let mut completed_count = 0;
    
    while completed_count < 2 {
        if let Some(event) = pool.recv_event().await {
            match event {
                JobEvent::Started(job_id) => {
                    println!("Job {:?} started", job_id);
                }
                JobEvent::Progress(job_id, progress) => {
                    println!("Job {:?} progress: {:.1}%", job_id, progress * 100.0);
                }
                JobEvent::Completed(job_id, _) => {
                    println!("Job {:?} completed successfully", job_id);
                    completed_count += 1;
                }
                JobEvent::Failed(job_id, error) => {
                    println!("Job {:?} failed: {}", job_id, error);
                    completed_count += 1;
                }
                JobEvent::Cancelled(job_id) => {
                    println!("Job {:?} was cancelled", job_id);
                    completed_count += 1;
                }
            }
        }
    }
    
    println!("All jobs completed. Shutting down...");
    pool.shutdown().await;
    println!("Worker pool shut down successfully");
}
