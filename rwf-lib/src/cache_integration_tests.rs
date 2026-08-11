//! Integration tests for directory caching
//!
//! This module tests the complete caching workflow including:
//! - Cache hits when navigating to cached directories
//! - Cache misses when navigating to uncached directories
//! - Cache invalidation after file operations

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, OpResult, SuccessData};
    use crate::model::{ActivePane, Location};
    use crate::state::{update_state, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use std::path::PathBuf;

    /// Test cache hit: navigating to a cached directory should not create a job
    #[test]
    fn test_cache_hit() {
        let mut state = test_state();

        let location = Location::Local(PathBuf::from("/test/dir1"));
        let entries = vec![
            FileEntryBuilder::new("file1.txt").size(100).build(),
            FileEntryBuilder::new("file2.txt").size(200).build(),
        ];

        // Populate cache
        state.cache.insert(location.clone(), entries.clone());

        // Navigate to cached location
        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: location.clone(),
            },
        );

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
        let mut state = test_state();

        let location = Location::Local(PathBuf::from("/test/dir1"));

        // Navigate to uncached location
        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: location.clone(),
            },
        );

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
        let mut state = test_state();

        let dest_location = Location::Local(PathBuf::from("/test/dest"));
        let entries = vec![FileEntryBuilder::new("file1.txt").size(100).build()];

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
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Cache should be invalidated for destination
        assert!(state.cache.get(&dest_location).is_none());
    }

    /// Test cache invalidation after move operation
    #[test]
    fn test_cache_invalidation_after_move() {
        let mut state = test_state();

        let dest_location = Location::Local(PathBuf::from("/test/dest"));
        let entries = vec![FileEntryBuilder::new("file1.txt").size(100).build()];

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
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Cache should be invalidated for destination
        assert!(state.cache.get(&dest_location).is_none());
    }

    /// Test cache invalidation after delete operation
    #[test]
    fn test_cache_invalidation_after_delete() {
        let mut state = test_state();

        let parent_location = Location::Local(PathBuf::from("/test"));
        let file_location = Location::Local(PathBuf::from("/test/file.txt"));
        let entries = vec![FileEntryBuilder::new("file.txt").size(100).build()];

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
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Cache should be invalidated for parent directory
        assert!(state.cache.get(&parent_location).is_none());
    }

    /// Test cache invalidation after mkdir operation
    #[test]
    fn test_cache_invalidation_after_mkdir() {
        let mut state = test_state();

        let parent_location = Location::Local(PathBuf::from("/test"));
        let new_dir_location = Location::Local(PathBuf::from("/test/newdir"));
        let entries = vec![FileEntryBuilder::new("file.txt").size(100).build()];

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
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Cache should be invalidated for parent directory
        assert!(state.cache.get(&parent_location).is_none());
    }

    /// Test cache invalidation after rename operation
    #[test]
    fn test_cache_invalidation_after_rename() {
        let mut state = test_state();

        let parent_location = Location::Local(PathBuf::from("/test"));
        let from_location = Location::Local(PathBuf::from("/test/oldname.txt"));
        let to_location = Location::Local(PathBuf::from("/test/newname.txt"));
        let entries = vec![FileEntryBuilder::new("oldname.txt").size(100).build()];

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
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Cache should be invalidated for parent directory
        assert!(state.cache.get(&parent_location).is_none());
    }

    /// Test that ReadDirectory job completion populates cache
    #[test]
    fn test_read_directory_populates_cache() {
        let mut state = test_state();

        let location = Location::Local(PathBuf::from("/test/dir1"));
        let entries = vec![
            FileEntryBuilder::new("file1.txt").size(100).build(),
            FileEntryBuilder::new("file2.txt").size(200).build(),
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
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::DirectoryRead(entries.clone())),
            },
        );

        // Cache should now have the entries
        let cached = state.cache.get(&location);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 2);
    }

    /// Test cache hit with multiple navigations
    #[test]
    fn test_cache_hit_with_multiple_navigations() {
        let mut state = test_state();

        let location1 = Location::Local(PathBuf::from("/test/dir1"));
        let location2 = Location::Local(PathBuf::from("/test/dir2"));
        let entries1 = vec![FileEntryBuilder::new("file1.txt").size(100).build()];
        let entries2 = vec![FileEntryBuilder::new("file2.txt").size(200).build()];

        // Populate cache for both locations
        state.cache.insert(location1.clone(), entries1.clone());
        state.cache.insert(location2.clone(), entries2.clone());

        // Navigate to location1 (cache hit)
        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: location1.clone(),
            },
        );
        assert_eq!(result.jobs_to_start.len(), 0);
        assert_eq!(state.active_pane().entries.len(), 1);

        // Navigate to location2 (cache hit)
        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: location2.clone(),
            },
        );
        assert_eq!(result.jobs_to_start.len(), 0);
        assert_eq!(state.active_pane().entries.len(), 1);

        // Navigate back to location1 (cache hit)
        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: location1.clone(),
            },
        );
        assert_eq!(result.jobs_to_start.len(), 0);

        // Verify we're at location1 with correct entries
        let pane = state.active_pane();
        assert_eq!(pane.current_location, location1);
        assert_eq!(pane.entries.len(), 1);
        assert_eq!(pane.entries[0].name, "file1.txt");
    }

    /// Regression test: ChangeLocation's cache-hit path must keep `raw_entries` in sync with
    /// `entries`, not just update `entries`. Otherwise a later in-place patch (e.g. Rename,
    /// which mutates `raw_entries` and then rebuilds `entries` from it via
    /// `apply_current_filter`) clobbers the correct listing with stale data left over from
    /// whatever directory was last actually read from disk.
    #[test]
    fn test_cache_hit_navigation_syncs_raw_entries() {
        let mut state = test_state();

        let root = Location::Local(PathBuf::from("/test"));
        let root_entries = vec![FileEntryBuilder::new("ftest2").dir(true).build()];
        state.cache.insert(root.clone(), root_entries.clone());

        // Pane currently shows some other, already-read directory's contents (as if the user
        // had navigated deeper and is now backing out via a cache hit).
        state.active_pane_mut().raw_entries = vec![FileEntryBuilder::new("f2").dir(true).build()];
        state.active_pane_mut().entries = state.active_pane().raw_entries.clone();

        let result = update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: root.clone(),
            },
        );
        assert_eq!(
            result.jobs_to_start.len(),
            0,
            "root should be served from cache"
        );

        let pane = state.active_pane();
        assert_eq!(pane.entries.len(), 1);
        assert_eq!(pane.entries[0].name, "ftest2");
        assert_eq!(
            pane.raw_entries.len(),
            1,
            "raw_entries must be refreshed on a cache hit, not left stale"
        );
        assert_eq!(pane.raw_entries[0].name, "ftest2");
    }

    /// Regression test for "rename doesn't update the list": renaming an entry must be visible
    /// even when the pane arrived at its current location via a cache-hit navigation (e.g.
    /// Backspace onto an already-visited directory) rather than a fresh ReadDirectory.
    #[test]
    fn test_rename_completion_reflected_after_cache_hit_navigation() {
        let mut state = test_state();

        let root = Location::Local(PathBuf::from("/test"));
        let ftest2 = Location::Local(PathBuf::from("/test/ftest2"));
        let gftest2 = Location::Local(PathBuf::from("/test/gftest2"));

        let root_entries = vec![FileEntryBuilder::new("ftest2")
            .dir(true)
            .location(ftest2.clone())
            .build()];
        state.cache.insert(root.clone(), root_entries.clone());

        // Simulate having just navigated out of ftest2's own (now stale) listing.
        state.active_pane_mut().raw_entries = vec![FileEntryBuilder::new("f2").dir(true).build()];
        state.active_pane_mut().entries = state.active_pane().raw_entries.clone();

        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: root.clone(),
            },
        );

        let job_spec = crate::job::JobSpec::new(JobKind::Rename {
            from: ftest2.clone(),
            to: gftest2.clone(),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        // Mirrors what execute_rename (Phase 7.6) actually returns on success:
        // a single OperationRecord with succeeded: true, not SuccessData::None.
        let record = crate::model::OperationRecord {
            source: Some(ftest2.clone()),
            destination: Some(gftest2.clone()),
            succeeded: true,
            failure_reason: None,
            undo: crate::model::UndoAvailability::Available(crate::model::ReversalAction::Rename {
                from: gftest2.clone(),
                to: ftest2.clone(),
            }),
        };
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::OperationRecords(vec![record])),
            },
        );

        let pane = state.active_pane();
        assert_eq!(
            pane.entries.len(),
            1,
            "rename must not replace the listing with stale raw_entries"
        );
        assert_eq!(
            pane.entries[0].name, "gftest2",
            "renamed entry should be reflected in the visible listing"
        );
    }

    /// Regression test for "ghost directory": deleting a directory must invalidate its OWN
    /// cache entry, not just the parent's. Otherwise recreating a directory of the same name
    /// and entering it serves the deleted directory's stale cached listing.
    #[test]
    fn test_delete_invalidates_cache_for_target_itself() {
        let mut state = test_state();

        let ftest2 = Location::Local(PathBuf::from("/test/ftest2"));
        let f2 = Location::Local(PathBuf::from("/test/ftest2/f2"));

        state.cache.insert(
            ftest2.clone(),
            vec![FileEntryBuilder::new("f2").dir(true).build()],
        );
        assert!(state.cache.get(&ftest2).is_some());
        let _ = f2; // documents what the stale entry represents

        let job_spec = crate::job::JobSpec::new(JobKind::Delete {
            targets: vec![ftest2.clone()],
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        assert!(
            state.cache.get(&ftest2).is_none(),
            "deleting a directory must invalidate its own cache entry, not just the parent's"
        );
    }

    /// Regression test for a suspected contributor to "leap mode arrow-nav lands on an
    /// unexpected entry": unlike the directory-listing `DirectoryCache`, `navigation_cache`
    /// (which remembers cursor/scroll per path, with no TTL) was never invalidated by
    /// delete/rename. A directory's remembered cursor position could survive its deletion
    /// indefinitely and get restored — against a since-recreated directory's unrelated
    /// contents — the next time that exact path was visited.
    #[test]
    fn test_delete_invalidates_stale_navigation_cursor() {
        let mut state = test_state();

        let ftest1 = Location::Local(PathBuf::from("/test/ftest1"));

        // Simulate having previously browsed ftest1 with the cursor left on its 6th entry.
        state.navigation_cache.save(ftest1.clone(), 5, 2);
        assert_eq!(state.navigation_cache.restore(&ftest1), Some((5, 2)));

        let job_spec = crate::job::JobSpec::new(JobKind::Delete {
            targets: vec![ftest1.clone()],
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        assert_eq!(
            state.navigation_cache.restore(&ftest1),
            None,
            "deleting a directory must invalidate its remembered cursor position too, \
             otherwise recreating a directory at the same path can restore a cursor that \
             has nothing to do with the new contents"
        );
    }

    /// Same gap for Rename: the `from` path's remembered cursor must not survive under the
    /// old key once the directory has been renamed away from it.
    #[test]
    fn test_rename_invalidates_stale_navigation_cursor_for_from_path() {
        let mut state = test_state();

        let from = Location::Local(PathBuf::from("/test/ftest2"));
        let to = Location::Local(PathBuf::from("/test/gftest2"));

        state.navigation_cache.save(from.clone(), 3, 0);

        let job_spec = crate::job::JobSpec::new(JobKind::Rename {
            from: from.clone(),
            to: to.clone(),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        assert_eq!(
            state.navigation_cache.restore(&from),
            None,
            "renaming a directory must invalidate its old path's remembered cursor"
        );
    }
}
