//! Property-based tests for JobManager
//!
//! **Validates: Requirements 15.11**

use crate::job::{JobManager, JobSpec, JobKind};
use crate::model::Location;
use proptest::prelude::*;
use std::path::PathBuf;

/// Strategy for generating Location values
fn arb_location() -> impl Strategy<Value = Location> {
    prop_oneof![
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/tmp/{}", s)))),
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/home/{}", s)))),
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/var/{}", s)))),
    ]
}

/// Strategy for generating JobKind values
fn arb_job_kind() -> impl Strategy<Value = JobKind> {
    prop_oneof![
        arb_location().prop_map(|loc| JobKind::ReadDirectory { location: loc }),
        arb_location().prop_map(|loc| JobKind::Mkdir { location: loc }),
        arb_location().prop_map(|loc| JobKind::CalculateSize { location: loc }),
        (arb_location(), arb_location()).prop_map(|(from, to)| JobKind::Rename { from, to }),
    ]
}

/// Strategy for generating JobSpec values
fn arb_job_spec() -> impl Strategy<Value = JobSpec> {
    arb_job_kind().prop_map(|kind| JobSpec::new(kind))
}

proptest! {
    /// **Property 20: FIFO Job Ordering**
    ///
    /// **Validates: Requirements 15.11**
    ///
    /// This property verifies that the JobManager maintains strict FIFO (first-in-first-out)
    /// ordering for job execution. Jobs must be dequeued in the exact order they were enqueued.
    ///
    /// **Property Statement:**
    /// For any sequence of jobs enqueued to the JobManager, when jobs are popped from the queue,
    /// they must be returned in the same order they were enqueued.
    ///
    /// **Test Strategy:**
    /// 1. Create a JobManager
    /// 2. Enqueue a sequence of jobs and record their IDs in order
    /// 3. Pop jobs from the queue
    /// 4. Verify that the popped job IDs match the enqueued order
    #[test]
    fn prop_fifo_job_ordering(job_specs in prop::collection::vec(arb_job_spec(), 1..20)) {
        let mut manager = JobManager::new(4);
        
        // Enqueue all jobs and collect their IDs in order
        let enqueued_ids: Vec<_> = job_specs.into_iter()
            .map(|spec| manager.enqueue(spec))
            .collect();
        
        // Pop all jobs and collect their IDs
        let mut popped_ids = Vec::new();
        while let Some(spec) = manager.pop_next_job() {
            popped_ids.push(spec.id);
        }
        
        // Verify FIFO ordering
        prop_assert_eq!(popped_ids, enqueued_ids);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_fifo_ordering_simple() {
        let mut manager = JobManager::new(4);
        
        // Enqueue three jobs
        let id1 = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test1")),
        }));
        let id2 = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test2")),
        }));
        let id3 = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test3")),
        }));
        
        // Pop jobs and verify order
        let job1 = manager.pop_next_job().unwrap();
        assert_eq!(job1.id, id1);
        
        let job2 = manager.pop_next_job().unwrap();
        assert_eq!(job2.id, id2);
        
        let job3 = manager.pop_next_job().unwrap();
        assert_eq!(job3.id, id3);
        
        // Queue should be empty
        assert!(manager.pop_next_job().is_none());
    }

    #[test]
    fn test_can_start_job() {
        let mut manager = JobManager::new(2); // max_parallel = 2
        
        // Enqueue a job
        manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test")),
        }));
        
        // Should be able to start a job
        assert!(manager.can_start_job());
        
        // Start two jobs
        let spec1 = manager.pop_next_job().unwrap();
        manager.start_job(spec1);
        
        manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test2")),
        }));
        let spec2 = manager.pop_next_job().unwrap();
        manager.start_job(spec2);
        
        // Now at max_parallel, should not be able to start more
        manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test3")),
        }));
        assert!(!manager.can_start_job());
    }

    #[test]
    fn test_job_cancellation_from_queue() {
        let mut manager = JobManager::new(4);
        
        // Enqueue jobs
        let id1 = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test1")),
        }));
        let id2 = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test2")),
        }));
        let id3 = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/test3")),
        }));
        
        // Cancel the middle job
        assert!(manager.request_cancel(id2));
        
        // Pop remaining jobs - should only get id1 and id3
        let job1 = manager.pop_next_job().unwrap();
        assert_eq!(job1.id, id1);
        
        let job3 = manager.pop_next_job().unwrap();
        assert_eq!(job3.id, id3);
        
        // Queue should be empty
        assert!(manager.pop_next_job().is_none());
        
        // Cancelled job should be in completed
        assert_eq!(manager.completed.len(), 1);
    }

    #[test]
    fn test_completed_job_limit() {
        let mut manager = JobManager::new(4);
        
        // Complete 150 jobs (more than the 100 limit)
        for i in 0..150 {
            let id = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from(format!("/test{}", i))),
            }));
            let spec = manager.pop_next_job().unwrap();
            manager.start_job(spec);
            manager.complete_job(id, crate::job::OpResult::Success(crate::job::SuccessData::None));
        }
        
        // Should only keep last 100
        assert_eq!(manager.completed.len(), 100);
    }
}
