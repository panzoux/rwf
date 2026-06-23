//! Integration tests for directory caching
//!
//! This module tests the complete caching workflow including:
//! - Cache hits when navigating to cached directories
//! - Cache misses when navigating to uncached directories
//! - Cache invalidation after file operations

#[cfg(test)]
mod tests {
    use crate::state::{AppState, update_state, Transition};
    use crate::config::AppConfig;
    use crate::model::{Location, FileEntry, ActivePane};
    use crate::job::{JobKind, OpResult, SuccessData};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_entry(name: &str, size: u64, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{}", name))),
            size,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    /// Test cache hit: navigating to a cached directory should not create a job
    #[test]
    fn test_cache_hit() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let location = Location::Local(PathBuf::from("/test/dir1"));
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
        ];
        
        // Populate cache
        state.cache.insert(location.clone(), entries.clone());
        
        // Navigate to cached location
        let result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: location.clone(),
        });
        
        // Should not create a job (cache hit)
        assert_eq!(result.jobs_to_start.len(), 0);
        
        // Pane should have the cached entries
        let pane = state.active_pane();
        assert_eq!(pane.entries.len(), 2);
        assert_eq!(pane.current_location, location);
    }

    /// Test cache miss: navigating to an uncached directory should create a job
    #[test]
    fn test_cache_miss() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let location = Location::Local(PathBuf::from("/test/dir1"));
        
        // Navigate to uncached location
        let result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: location.clone(),
        });
        
        // Should create a ReadDirectory job (cache miss)
        assert_eq!(result.jobs_to_start.len(), 1);
        
        match &result.jobs_to_start[0].kind {
            JobKind::ReadDirectory { location: job_loc } => {
                assert_eq!(job_loc, &location);
            }
            _ => panic!("Expected ReadDirectory job"),
        }
    }

    /// Test cache invalidation after copy operation
    #[test]
    fn test_cache_invalidation_after_copy() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let dest_location = Location::Local(PathBuf::from("/test/dest"));
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
        ];
        
        // Populate cache for destination
        state.cache.insert(dest_location.clone(), entries.clone());
        
        // Verify cache has entries
        assert!(state.cache.get(&dest_location).is_some());
        
        // Simulate a copy job
        let job_spec = crate::job::JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/test/source/file.txt"))],
            dest: dest_location.clone(),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Complete the copy job
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::None),
        });
        
        // Cache should be invalidated for destination
        assert!(state.cache.get(&dest_location).is_none());
    }

    /// Test cache invalidation after move operation
    #[test]
    fn test_cache_invalidation_after_move() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let dest_location = Location::Local(PathBuf::from("/test/dest"));
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
        ];
        
        // Populate cache for destination
        state.cache.insert(dest_location.clone(), entries.clone());
        
        // Verify cache has entries
        assert!(state.cache.get(&dest_location).is_some());
        
        // Simulate a move job
        let job_spec = crate::job::JobSpec::new(JobKind::Move {
            sources: vec![Location::Local(PathBuf::from("/test/source/file.txt"))],
            dest: dest_location.clone(),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Complete the move job
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::None),
        });
        
        // Cache should be invalidated for destination
        assert!(state.cache.get(&dest_location).is_none());
    }

    /// Test cache invalidation after delete operation
    #[test]
    fn test_cache_invalidation_after_delete() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let parent_location = Location::Local(PathBuf::from("/test"));
        let file_location = Location::Local(PathBuf::from("/test/file.txt"));
        let entries = vec![
            create_test_entry("file.txt", 100, false),
        ];
        
        // Populate cache for parent directory
        state.cache.insert(parent_location.clone(), entries.clone());
        
        // Verify cache has entries
        assert!(state.cache.get(&parent_location).is_some());
        
        // Simulate a delete job
        let job_spec = crate::job::JobSpec::new(JobKind::Delete {
            targets: vec![file_location],
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Complete the delete job
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::None),
        });
        
        // Cache should be invalidated for parent directory
        assert!(state.cache.get(&parent_location).is_none());
    }

    /// Test cache invalidation after mkdir operation
    #[test]
    fn test_cache_invalidation_after_mkdir() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let parent_location = Location::Local(PathBuf::from("/test"));
        let new_dir_location = Location::Local(PathBuf::from("/test/newdir"));
        let entries = vec![
            create_test_entry("file.txt", 100, false),
        ];
        
        // Populate cache for parent directory
        state.cache.insert(parent_location.clone(), entries.clone());
        
        // Verify cache has entries
        assert!(state.cache.get(&parent_location).is_some());
        
        // Simulate a mkdir job
        let job_spec = crate::job::JobSpec::new(JobKind::Mkdir {
            location: new_dir_location,
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Complete the mkdir job
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::None),
        });
        
        // Cache should be invalidated for parent directory
        assert!(state.cache.get(&parent_location).is_none());
    }

    /// Test cache invalidation after rename operation
    #[test]
    fn test_cache_invalidation_after_rename() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let parent_location = Location::Local(PathBuf::from("/test"));
        let from_location = Location::Local(PathBuf::from("/test/oldname.txt"));
        let to_location = Location::Local(PathBuf::from("/test/newname.txt"));
        let entries = vec![
            create_test_entry("oldname.txt", 100, false),
        ];
        
        // Populate cache for parent directory
        state.cache.insert(parent_location.clone(), entries.clone());
        
        // Verify cache has entries
        assert!(state.cache.get(&parent_location).is_some());
        
        // Simulate a rename job
        let job_spec = crate::job::JobSpec::new(JobKind::Rename {
            from: from_location,
            to: to_location,
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Complete the rename job
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::None),
        });
        
        // Cache should be invalidated for parent directory
        assert!(state.cache.get(&parent_location).is_none());
    }

    /// Test that ReadDirectory job completion populates cache
    #[test]
    fn test_read_directory_populates_cache() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let location = Location::Local(PathBuf::from("/test/dir1"));
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
        ];
        
        // Verify cache is empty
        assert!(state.cache.get(&location).is_none());
        
        // Simulate a ReadDirectory job
        let job_spec = crate::job::JobSpec::new(JobKind::ReadDirectory {
            location: location.clone(),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Complete the job with directory entries
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::DirectoryRead(entries.clone())),
        });
        
        // Cache should now have the entries
        let cached = state.cache.get(&location);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 2);
    }

    /// Test cache hit with multiple navigations
    #[test]
    fn test_cache_hit_with_multiple_navigations() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let location1 = Location::Local(PathBuf::from("/test/dir1"));
        let location2 = Location::Local(PathBuf::from("/test/dir2"));
        let entries1 = vec![create_test_entry("file1.txt", 100, false)];
        let entries2 = vec![create_test_entry("file2.txt", 200, false)];
        
        // Populate cache for both locations
        state.cache.insert(location1.clone(), entries1.clone());
        state.cache.insert(location2.clone(), entries2.clone());
        
        // Navigate to location1 (cache hit)
        let result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: location1.clone(),
        });
        assert_eq!(result.jobs_to_start.len(), 0);
        assert_eq!(state.active_pane().entries.len(), 1);
        
        // Navigate to location2 (cache hit)
        let result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: location2.clone(),
        });
        assert_eq!(result.jobs_to_start.len(), 0);
        assert_eq!(state.active_pane().entries.len(), 1);
        
        // Navigate back to location1 (cache hit)
        let result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: location1.clone(),
        });
        assert_eq!(result.jobs_to_start.len(), 0);
        
        // Verify we're at location1 with correct entries
        let pane = state.active_pane();
        assert_eq!(pane.current_location, location1);
        assert_eq!(pane.entries.len(), 1);
        assert_eq!(pane.entries[0].name, "file1.txt");
    }
}
