//! Event receiver for UI thread
//!
//! This module implements the event receiver that processes JobEvents from the
//! worker pool and converts them to Transitions for the state management system.
//!
//! **Validates: Requirements 21.1, 21.5, 21.8, 26.7**

use crate::state::{Transition, AppState, StateUpdateResult, update_state};
use crate::worker_pool::JobEvent;
use crate::job::OpResult;
use tracing::debug;

/// Process a JobEvent and convert it to a Transition
///
/// This function maps JobEvents received from the worker pool to appropriate
/// Transition enum values that can be fed to the update_state function.
///
/// # Arguments
///
/// * `event` - The JobEvent received from the worker pool
///
/// # Returns
///
/// A Transition that represents the state change corresponding to the event
pub fn map_job_event_to_transition(event: JobEvent) -> Transition {
    match event {
        JobEvent::Started(job_id) => {
            debug!("JobEvent::Started received for job_id={:?}", job_id);
            // Job has started - mark as running and update UI
            Transition::JobStarted { job_id }
        }

        JobEvent::Progress(job_id, progress) => {
            Transition::UpdateJobProgress { job_id, progress }
        }

        JobEvent::ProgressWithDetail(job_id, progress, progress_message, operation_detail) => {
            Transition::UpdateJobProgressWithDetail {
                job_id,
                progress,
                progress_message,
                operation_detail,
            }
        }

        JobEvent::Completed(job_id, success_data) => {
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(success_data)
            }
        }

        JobEvent::Failed(job_id, error) => {
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed(error)
            }
        }

        JobEvent::Cancelled(job_id) => {
            Transition::AcknowledgeCancel { job_id }
        }
    }
}

/// Process JobEvents from the worker pool in a non-blocking manner
///
/// This function attempts to receive events from the worker pool without blocking
/// and processes them by converting to transitions and updating state.
///
/// # Arguments
///
/// * `pool` - Mutable reference to the WorkerPool to receive events from
/// * `state` - Mutable reference to the AppState to update
///
/// # Returns
///
/// A vector of StateUpdateResults from processing all available events
pub fn process_pending_events<B: crate::backend::FilesystemBackend + 'static, A: crate::backend::ArchiveHandler + 'static>(
    pool: &mut crate::worker_pool::WorkerPool<B, A>,
    state: &mut AppState,
) -> Vec<StateUpdateResult> {
    let mut results = Vec::new();

    // Process all available events without blocking
    while let Some(event) = pool.try_recv_event() {
        debug!("process_pending_events: Received event {:?}", 
            match &event {
                JobEvent::Started(_) => "Started",
                JobEvent::Progress(_, _) => "Progress",
                JobEvent::ProgressWithDetail(_, _, _, _) => "ProgressWithDetail",
                JobEvent::Completed(_, _) => "Completed",
                JobEvent::Failed(_, _) => "Failed",
                JobEvent::Cancelled(_) => "Cancelled",
            }
        );
        let transition = map_job_event_to_transition(event);
        let result = update_state(state, transition);
        results.push(result);
    }

    results
}

/// Async version that waits for the next event
///
/// This function waits for the next JobEvent from the worker pool and processes it.
/// Use this in async contexts where blocking is acceptable.
///
/// # Arguments
///
/// * `pool` - Mutable reference to the WorkerPool to receive events from
/// * `state` - Mutable reference to the AppState to update
///
/// # Returns
///
/// An Option containing the StateUpdateResult if an event was received, None if the channel closed
pub async fn process_next_event<B: crate::backend::FilesystemBackend + 'static, A: crate::backend::ArchiveHandler + 'static>(
    pool: &mut crate::worker_pool::WorkerPool<B, A>,
    state: &mut AppState,
) -> Option<StateUpdateResult> {
    let event = pool.recv_event().await?;
    let transition = map_job_event_to_transition(event);
    let result = update_state(state, transition);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobId, SuccessData};
    use crate::state::AppConfig;

    #[test]
    fn test_map_started_event() {
        let job_id = JobId::new();
        let event = JobEvent::Started(job_id);
        
        let transition = map_job_event_to_transition(event);
        
        match transition {
            Transition::JobStarted { job_id: id } => {
                assert_eq!(id, job_id);
            }
            _ => panic!("Expected JobStarted transition"),
        }
    }

    #[test]
    fn test_map_progress_event() {
        let job_id = JobId::new();
        let event = JobEvent::Progress(job_id, 0.5);
        
        let transition = map_job_event_to_transition(event);
        
        match transition {
            Transition::UpdateJobProgress { job_id: id, progress } => {
                assert_eq!(id, job_id);
                assert_eq!(progress, 0.5);
            }
            _ => panic!("Expected UpdateJobProgress transition"),
        }
    }

    #[test]
    fn test_map_completed_event() {
        let job_id = JobId::new();
        let event = JobEvent::Completed(job_id, SuccessData::None);
        
        let transition = map_job_event_to_transition(event);
        
        match transition {
            Transition::CompleteJob { job_id: id, result } => {
                assert_eq!(id, job_id);
                assert!(matches!(result, OpResult::Success(_)));
            }
            _ => panic!("Expected CompleteJob transition"),
        }
    }

    #[test]
    fn test_map_failed_event() {
        let job_id = JobId::new();
        let error = "Test error".to_string();
        let event = JobEvent::Failed(job_id, error.clone());
        
        let transition = map_job_event_to_transition(event);
        
        match transition {
            Transition::CompleteJob { job_id: id, result } => {
                assert_eq!(id, job_id);
                match result {
                    OpResult::Failed(err) => assert_eq!(err, error),
                    _ => panic!("Expected Failed result"),
                }
            }
            _ => panic!("Expected CompleteJob transition"),
        }
    }

    #[test]
    fn test_map_cancelled_event() {
        let job_id = JobId::new();
        let event = JobEvent::Cancelled(job_id);
        
        let transition = map_job_event_to_transition(event);
        
        match transition {
            Transition::AcknowledgeCancel { job_id: id } => {
                assert_eq!(id, job_id);
            }
            _ => panic!("Expected AcknowledgeCancel transition"),
        }
    }

    #[tokio::test]
    async fn test_process_pending_events_empty() {
        use crate::backend::{LocalFilesystemBackend, MockArchiveHandler};
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let mut pool = crate::worker_pool::WorkerPool::new(2, backend, archive_handler);
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let results = process_pending_events(&mut pool, &mut state);
        
        // No events, should return empty vector
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_process_pending_events_with_events() {
        use crate::job::{JobSpec, JobKind};
        use crate::model::Location;
        use crate::backend::{LocalFilesystemBackend, MockArchiveHandler};
        use std::path::PathBuf;
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let mut pool = crate::worker_pool::WorkerPool::new(2, backend, archive_handler);
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Submit a job to generate events
        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test")),
        });
        let _job_id = spec.id;
        pool.submit_job(spec);
        
        // Wait a bit for events to be generated
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Process events
        let results = process_pending_events(&mut pool, &mut state);
        
        // Should have received at least started and completed/failed events
        assert!(results.len() >= 2);
    }

    #[tokio::test]
    async fn test_process_next_event() {
        use crate::job::{JobSpec, JobKind};
        use crate::model::Location;
        use crate::backend::{LocalFilesystemBackend, MockArchiveHandler};
        use std::path::PathBuf;
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let mut pool = crate::worker_pool::WorkerPool::new(2, backend, archive_handler);
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Submit a job
        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test")),
        });
        pool.submit_job(spec);
        
        // Process next event with timeout (should be Started)
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            process_next_event(&mut pool, &mut state)
        ).await;
        assert!(result.is_ok(), "First event should arrive within timeout");
        assert!(result.unwrap().is_some());
        
        // Process next event with timeout (should be Completed or Failed)
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            process_next_event(&mut pool, &mut state)
        ).await;
        assert!(result.is_ok(), "Second event should arrive within timeout");
        assert!(result.unwrap().is_some());
    }
}
