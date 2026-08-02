//! Job executor for processing JobSpecs
//!
//! This module implements the JobExecutor that dispatches jobs to the
//! appropriate backend methods and sends JobEvent updates.

use crate::backend::{ArchiveHandler, FilesystemBackend};
use crate::job::{JobId, JobKind, JobSpec, OpResult, PipeToAction, SuccessData};
use crate::model::viewer::{FileBytes, LineIndex, SeekableFile, TextEncoding, ViewerBuffer};
use crate::model::Location;
use crate::worker_pool::JobEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

/// Executor for processing job specifications
pub struct JobExecutor<B: FilesystemBackend, A: ArchiveHandler> {
    backend: Arc<B>,
    archive_handler: Arc<A>,
    event_sender: mpsc::UnboundedSender<JobEvent>,
}

impl<B: FilesystemBackend, A: ArchiveHandler> JobExecutor<B, A> {
    /// Create a new JobExecutor
    ///
    /// # Arguments
    ///
    /// * `backend` - The filesystem backend to use for operations
    /// * `archive_handler` - The archive handler for archive operations
    /// * `event_sender` - Channel for sending job events to the UI thread
    pub fn new(
        backend: Arc<B>,
        archive_handler: Arc<A>,
        event_sender: mpsc::UnboundedSender<JobEvent>,
    ) -> Self {
        Self {
            backend,
            archive_handler,
            event_sender,
        }
    }

    /// Execute a job specification
    ///
    /// Dispatches to the appropriate backend method based on JobKind
    /// and sends JobEvent updates via the event channel.
    pub async fn execute(&self, spec: JobSpec) {
        let job_id = spec.id;

        // Always send Started event first
        if let Err(e) = self.event_sender.send(JobEvent::Started(job_id)) {
            tracing::error!("Failed to send job started event for {:?}: {}", job_id, e);
        }

        let result = match &spec.kind {
            JobKind::ReadDirectory { location } => {
                self.execute_read_directory(location, &spec.cancel_token)
                    .await
            }
            JobKind::Copy { sources, dest } => self.execute_copy(sources, dest, &spec).await,
            JobKind::Move { sources, dest } => self.execute_move(sources, dest, &spec).await,
            JobKind::Delete { targets } => self.execute_delete(targets, &spec).await,
            JobKind::MoveToTrash {
                targets,
                force_fallback,
            } => {
                self.execute_move_to_trash(targets, *force_fallback, &spec)
                    .await
            }
            JobKind::RestoreFromTrash { records } => {
                self.execute_restore_from_trash(records, &spec).await
            }
            JobKind::EmptyTrash {
                scope,
                older_than_days,
                fallback_roots,
            } => {
                self.execute_empty_trash(
                    *scope,
                    *older_than_days,
                    fallback_roots,
                    &spec.cancel_token,
                )
                .await
            }
            JobKind::ScanTrash { fallback_roots } => {
                self.execute_scan_trash(fallback_roots, &spec.cancel_token)
                    .await
            }
            JobKind::Mkdir { location } => self.execute_mkdir(location, &spec.cancel_token).await,
            JobKind::CreateFile { location } => {
                self.execute_create_file(location, &spec.cancel_token).await
            }
            JobKind::ChangeAttributes { targets, attrs } => {
                self.execute_change_attributes(targets, attrs, &spec.cancel_token)
                    .await
            }
            JobKind::ChangeTimestamps { targets, times } => {
                self.execute_change_timestamps(targets, times, &spec.cancel_token)
                    .await
            }
            JobKind::CreateLink {
                target,
                link_path,
                kind,
            } => {
                self.execute_create_link(target, link_path, *kind, &spec.cancel_token)
                    .await
            }
            JobKind::Rename { from, to } => self.execute_rename(from, to, &spec.cancel_token).await,
            JobKind::CalculateSize { location } => {
                self.execute_calculate_size(location, &spec).await
            }
            JobKind::ExtractArchive { archive, dest } => {
                self.execute_extract_archive(archive, dest, &spec).await
            }
            JobKind::CreateArchive {
                sources,
                dest,
                original_size: _,
            } => self.execute_create_archive(sources, dest, &spec).await,
            JobKind::ExecuteCustomFunction {
                command,
                working_dir,
                pipe_to_action,
                shell,
            } => {
                self.execute_custom_function(
                    command,
                    working_dir,
                    pipe_to_action,
                    &spec,
                    shell.as_deref(),
                )
                .await
            }
            JobKind::SpawnProcess {
                program,
                args,
                wait,
            } => {
                self.execute_spawn_process(program, args, *wait, &spec)
                    .await
            }
            JobKind::Search {
                location,
                pattern,
                recursive,
            } => {
                self.execute_search(location, pattern, *recursive, &spec)
                    .await
            }
            JobKind::LoadFileForViewer {
                location,
                index_lines,
                large_file_threshold,
            } => {
                self.execute_load_file_for_viewer(
                    job_id,
                    location,
                    *index_lines,
                    *large_file_threshold,
                    &spec.cancel_token,
                )
                .await
            }
            JobKind::ViewerSearch {
                location,
                migemo_pattern,
                query,
                is_hex_mode,
                encoding,
                case_sensitive,
                large_file_threshold,
            } => {
                self.execute_viewer_search(
                    job_id,
                    location,
                    migemo_pattern.as_deref(),
                    query,
                    *is_hex_mode,
                    *encoding,
                    *case_sensitive,
                    *large_file_threshold,
                    &spec.cancel_token,
                )
                .await
            }
            JobKind::PatternRename {
                targets,
                find,
                replace,
                use_regex,
                case_sensitive,
            } => {
                self.execute_pattern_rename(
                    targets,
                    find,
                    replace,
                    *use_regex,
                    *case_sensitive,
                    &spec,
                )
                .await
            }
            JobKind::CompareFiles { left, right } => {
                self.execute_compare_files(left, right, &spec).await
            }
            JobKind::SplitFile {
                source,
                dest_dir,
                chunk_size,
            } => {
                self.execute_split_file(source, dest_dir, *chunk_size, &spec)
                    .await
            }
            JobKind::JoinFiles { parts, dest } => self.execute_join_files(parts, dest, &spec).await,
            JobKind::CountDown {
                duration_secs,
                start_value,
            } => {
                self.execute_countdown(*duration_secs, *start_value, &spec)
                    .await
            }
            JobKind::CollectJumpCandidates {
                root,
                include_files,
                max_results,
                max_depth,
            } => {
                self.execute_collect_jump_candidates(
                    root,
                    *include_files,
                    *max_results,
                    *max_depth,
                    &spec.cancel_token,
                )
                .await
            }
            JobKind::SuspendAndRun { .. } => {
                // Intercepted in app layer before pool submission — should never arrive here.
                crate::job::OpResult::Failed(
                    "SuspendAndRun reached worker pool unexpectedly".to_string(),
                )
            }
            JobKind::DetectFileType { path, purpose: _ } => {
                self.execute_detect_file_type(path, &spec.cancel_token)
                    .await
            }
            JobKind::DetectFileTypesBatch { paths } => {
                self.execute_detect_file_types_batch(paths, &spec).await
            }
        };

        // Send completion event based on result
        let event = match result {
            OpResult::Success(data) => JobEvent::Completed(job_id, data),
            OpResult::Failed(error) => JobEvent::Failed(job_id, error),
            OpResult::Cancelled => JobEvent::Cancelled(job_id),
        };

        if let Err(e) = self.event_sender.send(event) {
            tracing::trace!(
                "Failed to send job completion event (receiver likely closed): {}",
                e
            );
        }
    }

    /// Execute a read directory operation
    async fn execute_read_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        // Check if this is an archive location
        match location {
            Location::Archive { .. } => {
                // Use archive handler for archive locations
                match self
                    .archive_handler
                    .list_entries(location, cancel_token)
                    .await
                {
                    Ok(entries) => OpResult::Success(SuccessData::DirectoryRead(entries)),
                    Err(e) => OpResult::Failed(e.to_string()),
                }
            }
            _ => {
                // Use regular backend for other locations
                match self.backend.read_directory(location, cancel_token).await {
                    Ok(entries) => OpResult::Success(SuccessData::DirectoryRead(entries)),
                    Err(e) => OpResult::Failed(e.to_string()),
                }
            }
        }
    }

    /// Execute a copy operation
    async fn execute_copy(
        &self,
        sources: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        let total_files = sources.len();
        let decisions = spec.conflict_decisions.as_ref();

        for (i, source) in sources.iter().enumerate() {
            // Check cancellation before each file
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Check if there's a conflict decision for this file
            let decision = decisions.and_then(|d| d.iter().find(|d| d.source == *source));

            // Handle decision
            if let Some(dec) = decision {
                match &dec.action {
                    crate::job::ConflictAction::Skip => {
                        debug!("Skipping file due to conflict decision: {:?}", source);
                        continue;
                    }
                    crate::job::ConflictAction::OverwriteIfNewer => {
                        // Check if source is newer
                        let dest_location = if let Some(filename) = self.get_filename(source) {
                            self.join_location(dest, &filename)
                        } else {
                            dest.clone()
                        };

                        if let (Ok(src_meta), Ok(dst_meta)) = (
                            self.backend.get_entry(source).await,
                            self.backend.get_entry(&dest_location).await,
                        ) {
                            if src_meta.modified <= dst_meta.modified {
                                debug!("Skipping file (not newer): {:?}", source);
                                continue;
                            }
                        }
                    }
                    crate::job::ConflictAction::Rename { new_name } => {
                        // Copy with new name
                        if let Some(dest_path) = dest.path() {
                            let new_dest = Location::Local(dest_path.join(new_name));
                            if let Err(e) = self
                                .backend
                                .copy_file(source, &new_dest, &spec.cancel_token)
                                .await
                            {
                                return OpResult::Failed(format!(
                                    "Failed to copy {} as {}: {}",
                                    self.location_display(source),
                                    new_name,
                                    e
                                ));
                            }
                            debug!(
                                "Renamed and copied file: {} -> {}",
                                self.location_display(source),
                                new_name
                            );

                            // Progress update
                            let progress = if total_files > 0 {
                                (i + 1) as f64 / total_files as f64
                            } else {
                                1.0
                            };
                            if let Err(e) = self
                                .event_sender
                                .send(JobEvent::Progress(spec.id, progress))
                            {
                                tracing::error!(
                                    "Failed to send job progress event for {:?}: {}",
                                    spec.id,
                                    e
                                );
                            }
                            continue;
                        }
                    }
                    crate::job::ConflictAction::Force => {
                        // Proceed with normal copy (overwrite)
                    }
                }
            }

            // Calculate progress
            let progress = if total_files > 0 {
                (i as f64) / (total_files as f64)
            } else {
                0.0
            };

            // Send progress update
            if let Err(e) = self
                .event_sender
                .send(JobEvent::Progress(spec.id, progress))
            {
                tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
            }

            // Determine destination path
            let dest_location = if let Some(filename) = self.get_filename(source) {
                self.join_location(dest, &filename)
            } else {
                dest.clone()
            };

            // Copy the file
            if let Err(e) = self
                .backend
                .copy_file(source, &dest_location, &spec.cancel_token)
                .await
            {
                return OpResult::Failed(format!(
                    "Failed to copy {}: {}",
                    self.location_display(source),
                    e
                ));
            }
        }

        // Send final progress
        if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, 1.0)) {
            tracing::error!(
                "Failed to send job progress event (1.0) for {:?}: {}",
                spec.id,
                e
            );
        }

        OpResult::Success(SuccessData::None)
    }

    /// Execute a move operation
    async fn execute_move(
        &self,
        sources: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        let total_files = sources.len();
        let decisions = spec.conflict_decisions.as_ref();

        for (i, source) in sources.iter().enumerate() {
            // Check cancellation before each file
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Check if there's a conflict decision for this file
            let decision = decisions.and_then(|d| d.iter().find(|d| d.source == *source));

            // Handle decision
            if let Some(dec) = decision {
                match &dec.action {
                    crate::job::ConflictAction::Skip => {
                        debug!("Skipping file due to conflict decision: {:?}", source);
                        continue;
                    }
                    crate::job::ConflictAction::OverwriteIfNewer => {
                        // Check if source is newer
                        let dest_location = if let Some(filename) = self.get_filename(source) {
                            self.join_location(dest, &filename)
                        } else {
                            dest.clone()
                        };

                        if let (Ok(src_meta), Ok(dst_meta)) = (
                            self.backend.get_entry(source).await,
                            self.backend.get_entry(&dest_location).await,
                        ) {
                            if src_meta.modified <= dst_meta.modified {
                                debug!("Skipping file (not newer): {:?}", source);
                                continue;
                            }
                        }
                    }
                    crate::job::ConflictAction::Rename { new_name } => {
                        // Move with new name
                        if let Some(dest_path) = dest.path() {
                            let new_dest = Location::Local(dest_path.join(new_name));
                            if let Err(e) = self
                                .backend
                                .move_file(source, &new_dest, &spec.cancel_token)
                                .await
                            {
                                return OpResult::Failed(format!(
                                    "Failed to move {} as {}: {}",
                                    self.location_display(source),
                                    new_name,
                                    e
                                ));
                            }
                            debug!(
                                "Renamed and moved file: {} -> {}",
                                self.location_display(source),
                                new_name
                            );

                            // Progress update
                            let progress = if total_files > 0 {
                                (i + 1) as f64 / total_files as f64
                            } else {
                                1.0
                            };
                            if let Err(e) = self
                                .event_sender
                                .send(JobEvent::Progress(spec.id, progress))
                            {
                                tracing::error!(
                                    "Failed to send job progress event for {:?}: {}",
                                    spec.id,
                                    e
                                );
                            }
                            continue;
                        }
                    }
                    crate::job::ConflictAction::Force => {
                        // Proceed with normal move (overwrite)
                    }
                }
            }

            // Calculate progress
            let progress = if total_files > 0 {
                (i as f64) / (total_files as f64)
            } else {
                0.0
            };

            // Send progress update
            if let Err(e) = self
                .event_sender
                .send(JobEvent::Progress(spec.id, progress))
            {
                tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
            }

            // Determine destination path
            let dest_location = if let Some(filename) = self.get_filename(source) {
                self.join_location(dest, &filename)
            } else {
                dest.clone()
            };

            // Move the file
            if let Err(e) = self
                .backend
                .move_file(source, &dest_location, &spec.cancel_token)
                .await
            {
                return OpResult::Failed(format!(
                    "Failed to move {}: {}",
                    self.location_display(source),
                    e
                ));
            }
        }

        // Send final progress
        if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, 1.0)) {
            tracing::error!(
                "Failed to send job progress event (1.0) for {:?}: {}",
                spec.id,
                e
            );
        }

        OpResult::Success(SuccessData::None)
    }

    /// Execute a delete operation
    async fn execute_delete(&self, targets: &[Location], spec: &JobSpec) -> OpResult {
        let total_files = targets.len();

        for (i, target) in targets.iter().enumerate() {
            // Check cancellation before each file
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Calculate progress
            let progress = if total_files > 0 {
                (i as f64) / (total_files as f64)
            } else {
                0.0
            };

            // Send progress update
            if let Err(e) = self
                .event_sender
                .send(JobEvent::Progress(spec.id, progress))
            {
                tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
            }

            // Delete the file
            if let Err(e) = self.backend.delete_file(target, &spec.cancel_token).await {
                return OpResult::Failed(format!(
                    "Failed to delete {}: {}",
                    self.location_display(target),
                    e
                ));
            }
        }

        // Send final progress
        if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, 1.0)) {
            tracing::error!(
                "Failed to send job progress event (1.0) for {:?}: {}",
                spec.id,
                e
            );
        }

        OpResult::Success(SuccessData::None)
    }

    /// Execute a move-to-trash operation. Loops per-target (not fail-fast)
    /// so every target is represented in the result, matching the
    /// convention established by `execute_change_attributes` (see
    /// `plan/7.6.transactional_rollback.md` §8).
    async fn execute_move_to_trash(
        &self,
        targets: &[Location],
        force_fallback: bool,
        spec: &JobSpec,
    ) -> OpResult {
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        let mut outcomes = Vec::with_capacity(targets.len());
        for target in targets {
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            let outcome = match self
                .backend
                .move_to_trash(target, force_fallback, &spec.cancel_token)
                .await
            {
                Ok(record) => crate::model::TrashOutcome {
                    target: target.clone(),
                    record: Some(record),
                    result: Ok(()),
                },
                Err(e) => crate::model::TrashOutcome {
                    target: target.clone(),
                    record: None,
                    result: Err(e.to_string()),
                },
            };
            outcomes.push(outcome);
        }

        OpResult::Success(SuccessData::TrashMoved(outcomes))
    }

    /// Execute a restore-from-trash operation. Per-target loop, same
    /// rationale as `execute_move_to_trash`.
    async fn execute_restore_from_trash(
        &self,
        records: &[crate::model::TrashRecord],
        spec: &JobSpec,
    ) -> OpResult {
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        let mut outcomes = Vec::with_capacity(records.len());
        for record in records {
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }
            let result = match self
                .backend
                .restore_from_trash(record, &spec.cancel_token)
                .await
            {
                Ok(()) => Ok(()),
                Err(e) => Err(e.to_string()),
            };
            outcomes.push(crate::model::RestoreOutcome {
                original: record.original.clone(),
                result,
            });
        }

        OpResult::Success(SuccessData::TrashRestored(outcomes))
    }

    /// Execute an empty-trash operation (permanent purge).
    async fn execute_empty_trash(
        &self,
        scope: crate::model::EmptyTrashScope,
        older_than_days: Option<u32>,
        fallback_roots: &[std::path::PathBuf],
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        match self
            .backend
            .empty_trash(scope, older_than_days, fallback_roots)
            .await
        {
            Ok(purged) => OpResult::Success(SuccessData::TrashEmptied { purged }),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Execute a non-destructive trash count/size scan.
    async fn execute_scan_trash(
        &self,
        fallback_roots: &[std::path::PathBuf],
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        match self.backend.scan_trash(fallback_roots, cancel_token).await {
            Ok((count, total_size)) => {
                OpResult::Success(SuccessData::TrashScanned { count, total_size })
            }
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Execute a mkdir operation
    async fn execute_mkdir(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match self.backend.create_directory(location, cancel_token).await {
            Ok(_) => OpResult::Success(SuccessData::None),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Execute a create-file operation
    async fn execute_create_file(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match self.backend.create_file(location, cancel_token).await {
            Ok(_) => OpResult::Success(SuccessData::None),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Execute a create-link operation (symlink/hardlink/junction).
    async fn execute_create_link(
        &self,
        target: &Location,
        link_path: &Location,
        kind: crate::model::LinkCreateKind,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match self
            .backend
            .create_link(target, link_path, kind, cancel_token)
            .await
        {
            Ok(_) => OpResult::Success(SuccessData::None),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Execute an attribute-change operation across one or more targets.
    ///
    /// Loops per-file rather than failing fast so that a Vec<FileOpOutcome>
    /// covering every target is always returned (see plan/7.6.transactional_rollback.md §8).
    async fn execute_change_attributes(
        &self,
        targets: &[Location],
        attrs: &crate::model::AttributeChange,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        let mut outcomes = Vec::with_capacity(targets.len());
        for target in targets {
            let outcome = match self
                .backend
                .set_attributes(target, attrs, cancel_token)
                .await
            {
                Ok(old) => crate::model::FileOpOutcome {
                    target: target.clone(),
                    old: Some(old),
                    new: attrs.clone(),
                    result: Ok(()),
                },
                Err(e) => crate::model::FileOpOutcome {
                    target: target.clone(),
                    old: None,
                    new: attrs.clone(),
                    result: Err(e.to_string()),
                },
            };
            outcomes.push(outcome);
        }

        OpResult::Success(SuccessData::AttributesChanged(outcomes))
    }

    /// Execute a timestamp-change operation across one or more targets.
    async fn execute_change_timestamps(
        &self,
        targets: &[Location],
        times: &crate::model::TimestampChange,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        let mut outcomes = Vec::with_capacity(targets.len());
        for target in targets {
            let outcome = match self
                .backend
                .set_timestamps(target, times, cancel_token)
                .await
            {
                Ok(old) => crate::model::FileOpOutcome {
                    target: target.clone(),
                    old: Some(old),
                    new: times.clone(),
                    result: Ok(()),
                },
                Err(e) => crate::model::FileOpOutcome {
                    target: target.clone(),
                    old: None,
                    new: times.clone(),
                    result: Err(e.to_string()),
                },
            };
            outcomes.push(outcome);
        }

        OpResult::Success(SuccessData::TimestampsChanged(outcomes))
    }

    /// Execute a rename operation
    async fn execute_rename(
        &self,
        from: &Location,
        to: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match self.backend.rename_file(from, to, cancel_token).await {
            Ok(_) => OpResult::Success(SuccessData::None),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Execute a calculate size operation
    async fn execute_calculate_size(&self, location: &Location, spec: &JobSpec) -> OpResult {
        // Use the progress callback version to send updates
        let event_sender = self.event_sender.clone();
        let job_id = spec.id;

        // Track progress updates
        let last_update = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));

        let result = self
            .backend
            .calculate_directory_size_with_progress(
                location,
                &spec.cancel_token,
                Box::new(move |items_processed, _current_size| {
                    // Send progress updates every 100ms to avoid flooding
                    let mut last = last_update
                        .lock()
                        .expect("last_update mutex should not be poisoned");
                    if last.elapsed() > std::time::Duration::from_millis(100) {
                        // We don't know the total, so we can't calculate a percentage
                        // Send a progress value that indicates activity (oscillating between 0.3 and 0.7)
                        let progress = 0.5 + 0.2 * ((items_processed % 10) as f64 / 10.0 - 0.5);
                        if let Err(e) = event_sender.send(JobEvent::Progress(job_id, progress)) {
                            tracing::error!(
                                "Failed to send job progress event for {:?}: {}",
                                job_id,
                                e
                            );
                        }
                        *last = std::time::Instant::now();
                    }
                }),
            )
            .await;

        match result {
            Ok(size) => OpResult::Success(SuccessData::SizeCalculated(size)),
            Err(e) => {
                if spec.cancel_token.is_cancelled() {
                    OpResult::Cancelled
                } else {
                    OpResult::Failed(e.to_string())
                }
            }
        }
    }

    /// Execute an extract archive operation
    async fn execute_extract_archive(
        &self,
        archive: &Location,
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match self
            .archive_handler
            .extract_all(archive, dest, &spec.cancel_token)
            .await
        {
            Ok(()) => OpResult::Success(SuccessData::None),
            Err(e) => {
                if spec.cancel_token.is_cancelled() {
                    OpResult::Cancelled
                } else {
                    OpResult::Failed(e.to_string())
                }
            }
        }
    }

    /// Execute a create archive operation
    async fn execute_create_archive(
        &self,
        sources: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match self
            .archive_handler
            .create_archive(sources, dest, &spec.cancel_token)
            .await
        {
            Ok(()) => OpResult::Success(SuccessData::None),
            Err(e) => {
                if spec.cancel_token.is_cancelled() {
                    OpResult::Cancelled
                } else {
                    OpResult::Failed(e.to_string())
                }
            }
        }
    }

    /// Execute a custom function
    async fn execute_custom_function(
        &self,
        command: &str,
        working_dir: &Location,
        pipe_to_action: &Option<PipeToAction>,
        spec: &JobSpec,
        shell: Option<&str>,
    ) -> OpResult {
        // Extract local path from working directory
        let working_path = match working_dir {
            Location::Local(path) => path.clone(),
            _ => return OpResult::Failed("Custom functions only support local paths".to_string()),
        };

        // Check cancellation before starting
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        // Determine shell to use.
        // On Windows, cmd.exe gets /D before /C to disable AutoRun registry entries
        // (Clink and similar tools hook there; without /D they try to inject into the
        // non-interactive session and may return a non-zero exit code).
        #[derive(PartialEq)]
        enum ShellKind {
            Cmd,
            Other,
        }
        let (shell_cmd, shell_arg, shell_kind) = if let Some(shell_name) = shell {
            match shell_name {
                "bash" => ("bash", "-c", ShellKind::Other),
                "zsh" => ("zsh", "-c", ShellKind::Other),
                "powershell" | "powershell.exe" => ("powershell", "-Command", ShellKind::Other),
                "cmd" | "cmd.exe" => ("cmd", "/C", ShellKind::Cmd),
                _ => {
                    #[cfg(target_os = "windows")]
                    {
                        ("cmd", "/C", ShellKind::Cmd)
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        ("sh", "-c", ShellKind::Other)
                    }
                }
            }
        } else {
            #[cfg(target_os = "windows")]
            {
                ("cmd", "/C", ShellKind::Cmd)
            }
            #[cfg(not(target_os = "windows"))]
            {
                ("sh", "-c", ShellKind::Other)
            }
        };

        // Execute the command.
        // For cmd.exe on Windows we use raw_arg instead of arg so that Rust does NOT
        // apply its own \"-escaping.  cmd.exe has its own quoting rules and treats
        // backslash-quote as two literal characters, corrupting paths that contain
        // double-quoted strings.  raw_arg passes the token exactly as given.
        let mut cmd = tokio::process::Command::new(shell_cmd);
        #[cfg(target_os = "windows")]
        {
            if shell_kind == ShellKind::Cmd {
                cmd.raw_arg("/D").raw_arg("/C").raw_arg(command);
            } else {
                cmd.arg(shell_arg).arg(command);
            }
        }
        #[cfg(not(target_os = "windows"))]
        cmd.arg(shell_arg).arg(command);
        cmd.current_dir(working_path);
        let output = cmd.output().await;

        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

                    // Handle PipeToAction if specified
                    if let Some(_action) = pipe_to_action {
                        // PipeToAction handling will be done by the caller
                        // We just return the output
                        OpResult::Success(SuccessData::CustomFunctionOutput(stdout))
                    } else {
                        OpResult::Success(SuccessData::CustomFunctionOutput(stdout))
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    OpResult::Failed(stderr)
                }
            }
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }

    /// Spawn a program directly without a shell, avoiding cmd.exe quote-mangling on Windows.
    ///
    /// Stdio is never inherited from RWF's own console: callers of this job (`editor_job`,
    /// `system_open_job`) launch via `cmd /c <program> ...` on Windows, and cmd.exe honors
    /// the user's `HKCU\Software\Microsoft\Command Processor\AutoRun` hook (e.g. Clink) for
    /// *every* new cmd.exe instance, including this transient one. If that hook fails or
    /// prints anything, it writes straight to whatever console it inherited — which, without
    /// this redirection, is RWF's own alternate-screen TUI, corrupting the display. Piping
    /// all three streams to null makes the child fully detached from our console regardless
    /// of what it (or a shell hook wrapping it) tries to print.
    async fn execute_spawn_process(
        &self,
        program: &str,
        args: &[String],
        wait: bool,
        spec: &JobSpec,
    ) -> crate::job::OpResult {
        if spec.cancel_token.is_cancelled() {
            return crate::job::OpResult::Cancelled;
        }
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if wait {
            // Block this job (not the UI thread) until the child exits, so callers
            // can react to "editor closed" via the normal job-completion event.
            match cmd.status().await {
                Ok(_) => crate::job::OpResult::Success(crate::job::SuccessData::None),
                Err(e) => {
                    crate::job::OpResult::Failed(format!("cannot start '{}': {}", program, e))
                }
            }
        } else {
            match cmd.spawn() {
                Ok(_) => crate::job::OpResult::Success(crate::job::SuccessData::None),
                Err(e) => {
                    crate::job::OpResult::Failed(format!("cannot start '{}': {}", program, e))
                }
            }
        }
    }

    /// Execute a search operation
    async fn execute_search(
        &self,
        _location: &Location,
        _pattern: &str,
        _recursive: bool,
        _spec: &JobSpec,
    ) -> OpResult {
        // Search operations are not yet implemented
        // This is a placeholder for future implementation
        OpResult::Failed("Search not yet implemented".to_string())
    }

    /// Read the entire file into memory on the blocking pool and send ViewerReady.
    /// Using InMemory for all files means the UI thread (Tokio async worker) never
    /// touches OS file handles or mmap pages — eliminating page-fault stalls that
    /// occur when another process is concurrently writing to the file.
    async fn execute_load_file_for_viewer(
        &self,
        job_id: JobId,
        location: &Location,
        index_lines: bool,
        large_file_threshold: usize,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        let path = match location {
            Location::Local(p) => p.clone(),
            Location::Archive { .. } => {
                return OpResult::Failed("Archive file viewing not yet implemented".to_string())
            }
            _ => return OpResult::Failed("Unsupported location type for file viewing".to_string()),
        };

        // All file I/O runs on the blocking thread pool so the Tokio async thread
        // (which drives the UI event loop) is never stalled.
        //
        // Files ≤ inmem_threshold are read entirely into RAM. This avoids page-fault
        // delays from concurrent writes. The complete line index is also built inline
        // so ViewerReady arrives with a fully indexed, stable snapshot.
        //
        // Files > inmem_threshold use SeekableFile (File + Seek + Read, no mmap).
        let inmem_threshold = large_file_threshold;

        let path_for_open = path.clone();
        let index_lines_flag = index_lines;

        // Returns (FileBytes, encoding, Option<complete LineIndex>).
        // The Option is Some for the InMemory path (index built inline).
        let open_result: std::io::Result<(FileBytes, TextEncoding, Option<LineIndex>)> =
            tokio::task::spawn_blocking(move || {
                let meta = std::fs::metadata(&path_for_open)?;
                let file_size = meta.len() as usize;

                if file_size <= inmem_threshold {
                    let bytes = std::fs::read(&path_for_open)?;
                    let sample_len = bytes.len().min(16384);
                    let encoding = TextEncoding::detect(&bytes[..sample_len]);

                    let complete_index = if index_lines_flag {
                        let total = bytes.len();
                        let mut offsets: Vec<u64> = vec![0];
                        for (i, &b) in bytes.iter().enumerate() {
                            if b == b'\n' && i + 1 < total {
                                offsets.push((i + 1) as u64);
                            }
                        }
                        Some(LineIndex {
                            offsets,
                            is_complete: true,
                        })
                    } else {
                        None
                    };

                    Ok((FileBytes::InMemory(bytes), encoding, complete_index))
                } else {
                    // Large file: Seekable (File + Seek + Read, no mmap).
                    // On Windows, File::open uses FILE_SHARE_READ|FILE_SHARE_WRITE by
                    // default — concurrent writes (e.g. active log files) are safe.
                    let file = std::fs::File::open(&path_for_open)?;
                    let size = meta.len();
                    let seekable = SeekableFile::new(file, size);
                    let sample = seekable.read_bytes(0, 16384.min(size as usize))?;
                    let encoding = TextEncoding::detect(&sample);
                    Ok((FileBytes::Seekable(seekable), encoding, None))
                }
            })
            .await
            .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())));

        let (file_bytes, encoding, complete_index) = match open_result {
            Ok(triple) => triple,
            Err(e) => return OpResult::Failed(format!("Failed to open file: {}", e)),
        };

        // Small text-mode files arrive with a complete index, and hex mode needs none.
        // In both cases we can send ViewerReady immediately and return.
        if !index_lines || complete_index.is_some() {
            let idx = complete_index.unwrap_or_else(|| {
                let mut i = LineIndex::new();
                i.is_complete = true;
                i
            });
            let buffer = ViewerBuffer::new(file_bytes, idx);
            let _ = self
                .event_sender
                .send(JobEvent::ViewerReady(job_id, buffer, encoding));
            return OpResult::Success(SuccessData::None);
        }

        // Large text-mode file: send the buffer first so the visible viewport renders
        // before the full index is ready, then build the index on a separate blocking
        // thread using a dedicated file handle (never contends with SeekableFile render handle).
        let buffer = ViewerBuffer::new(file_bytes, LineIndex::new());
        let _ = self
            .event_sender
            .send(JobEvent::ViewerReady(job_id, buffer.clone(), encoding));

        let total = buffer.total_bytes();
        if total == 0 {
            buffer
                .line_index
                .lock()
                .expect("line_index mutex should not be poisoned")
                .is_complete = true;
            return OpResult::Success(SuccessData::None);
        }

        let buffer_for_scan = buffer.clone();
        let cancel = cancel_token.clone();
        let event_tx = self.event_sender.clone();
        let path_for_index = path.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            const CHUNK: usize = 4 * 1024 * 1024;

            // Open a dedicated handle for sequential scanning — never contends
            // with the SeekableFile render handle (or InMemory's in-memory vec).
            let mut index_file = match std::fs::File::open(&path_for_index) {
                Ok(f) => f,
                Err(_) => {
                    buffer_for_scan
                        .line_index
                        .lock()
                        .expect("line_index mutex should not be poisoned")
                        .is_complete = true;
                    return;
                }
            };

            let mut read_buf = vec![0u8; CHUNK];
            let mut abs_offset = 0u64;

            loop {
                if cancel.is_cancelled() {
                    return;
                }

                let n = match index_file.read(&mut read_buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };

                let mut local: Vec<u64> = Vec::new();
                for (i, &byte) in read_buf.iter().enumerate().take(n) {
                    let abs = abs_offset + i as u64;
                    if byte == b'\n' && abs + 1 < total as u64 {
                        local.push(abs + 1);
                    }
                }
                if !local.is_empty() {
                    buffer_for_scan
                        .line_index
                        .lock()
                        .expect("line_index mutex should not be poisoned")
                        .offsets
                        .extend_from_slice(&local);
                }
                abs_offset += n as u64;
                let _ = event_tx.send(JobEvent::Progress(job_id, abs_offset as f64 / total as f64));
            }

            buffer_for_scan
                .line_index
                .lock()
                .expect("line_index mutex should not be poisoned")
                .is_complete = true;
        })
        .await
        .ok();

        OpResult::Success(SuccessData::None)
    }

    /// Detect a single file's content type from its leading bytes (magic-byte
    /// detection, Phase 7.3). Reads at most the first 300 bytes on the blocking
    /// thread pool via `SeekableFile`, mirroring `execute_load_file_for_viewer`'s
    /// large-file read path so the async worker thread never touches file I/O.
    async fn execute_detect_file_type(
        &self,
        path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        match detect_file_type_blocking(path.to_path_buf()).await {
            Ok((kind, header_bytes)) => {
                OpResult::Success(SuccessData::FileTypeDetected { kind, header_bytes })
            }
            Err(e) => OpResult::Failed(format!(
                "Failed to detect file type for {}: {}",
                path.display(),
                e
            )),
        }
    }

    /// Detect content types for multiple files in one job (used to group marked
    /// files for the "Open With..." picker). A per-file read error is mapped to
    /// `DetectedKind::Unknown` for that entry rather than failing the whole batch —
    /// one unreadable file (permissions, mid-flight deletion) shouldn't block
    /// detection for the rest of the marked set. Checks `spec.cancel_token` once
    /// per item, same pattern as `execute_delete`/`execute_copy`/`execute_move` —
    /// marked-file sets are exactly the potentially-large case this job exists for.
    async fn execute_detect_file_types_batch(
        &self,
        paths: &[std::path::PathBuf],
        spec: &JobSpec,
    ) -> OpResult {
        let mut results = Vec::with_capacity(paths.len());
        for path in paths {
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }
            let kind = detect_file_type_blocking(path.clone())
                .await
                .map(|(kind, _header_bytes)| kind)
                .unwrap_or(crate::magic::DetectedKind::Unknown);
            results.push((path.clone(), kind));
        }
        OpResult::Success(SuccessData::FileTypesDetected(results))
    }

    /// Execute a background viewer search (hex or text mode).
    /// Sends ViewerSearchComplete when done; the standard Completed event follows.
    #[allow(clippy::too_many_arguments)]
    async fn execute_viewer_search(
        &self,
        job_id: JobId,
        location: &Location,
        migemo_pattern: Option<&str>,
        query: &str,
        is_hex_mode: bool,
        encoding: TextEncoding,
        case_sensitive: bool,
        large_file_threshold: usize,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        let path = match location {
            Location::Local(p) => p.clone(),
            _ => {
                return OpResult::Failed("Viewer search only supported for local files".to_string())
            }
        };
        let query = query.to_string();
        let migemo = migemo_pattern.map(|s| s.to_string());
        let event_tx = self.event_sender.clone();
        let cancel = cancel_token.clone();

        let matches: Vec<(usize, usize, usize)> = tokio::task::spawn_blocking(move || {
            use crate::model::viewer::hex_query_has_pattern;
            use std::io::{Read, Seek, SeekFrom};

            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => return vec![],
            };
            let file_size = meta.len() as usize;

            if is_hex_mode {
                // ── Hex search ────────────────────────────────────────────────────────
                if !hex_query_has_pattern(&query) {
                    return vec![];
                }

                let trimmed = query.trim();
                // Determine the byte needle from the query
                let needle_opt: Option<Vec<u8>> = if let Some(rest) = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                {
                    if rest.len() >= 2
                        && rest.len() % 2 == 0
                        && rest.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        (0..rest.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&rest[i..i + 2], 16).ok())
                            .collect()
                    } else {
                        None
                    }
                } else if trimmed.chars().all(|c| c.is_ascii_hexdigit())
                    && trimmed.len().is_multiple_of(2)
                {
                    (0..trimmed.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&trimmed[i..i + 2], 16).ok())
                        .collect()
                } else if trimmed.contains(' ')
                    && trimmed.chars().all(|c| c.is_ascii_hexdigit() || c == ' ')
                {
                    let no_space: String = trimmed.chars().filter(|&c| c != ' ').collect();
                    if no_space.len() >= 2 && no_space.len().is_multiple_of(2) {
                        (0..no_space.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&no_space[i..i + 2], 16).ok())
                            .collect()
                    } else {
                        None
                    }
                } else {
                    Some(trimmed.as_bytes().to_vec())
                };

                let needle = match needle_opt {
                    Some(n) if !n.is_empty() => n,
                    _ => return vec![],
                };
                let ci = !case_sensitive && needle.iter().any(|&b| b.is_ascii_alphabetic());
                let lower_needle: Vec<u8> = if ci {
                    needle.iter().map(|b| b.to_ascii_lowercase()).collect()
                } else {
                    vec![]
                };

                const CHUNK: usize = 4 * 1024 * 1024;
                let overlap = needle.len().saturating_sub(1);
                let mut result = Vec::new();
                let mut file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => return vec![],
                };
                let mut chunk_start = 0usize;
                while chunk_start < file_size {
                    if cancel.is_cancelled() {
                        return result;
                    }
                    let read_start = chunk_start.saturating_sub(overlap);
                    let read_end = (chunk_start + CHUNK).min(file_size);
                    if file.seek(SeekFrom::Start(read_start as u64)).is_err() {
                        break;
                    }
                    let mut buf = vec![0u8; read_end - read_start];
                    let mut pos = 0;
                    while pos < buf.len() {
                        match file.read(&mut buf[pos..]) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => pos += n,
                        }
                    }
                    buf.truncate(pos);
                    let n = needle.len();
                    if buf.len() >= n {
                        for i in 0..=buf.len() - n {
                            let matched = if ci {
                                lower_needle
                                    .iter()
                                    .zip(&buf[i..i + n])
                                    .all(|(&a, &b)| a == b.to_ascii_lowercase())
                            } else {
                                &buf[i..i + n] == needle.as_slice()
                            };
                            if matched {
                                let abs_s = read_start + i;
                                let abs_e = abs_s + n;
                                if abs_e > chunk_start {
                                    result.push((abs_s / 16, abs_s, abs_e));
                                }
                            }
                        }
                    }
                    chunk_start = read_end;
                }
                result
            } else {
                // ── Text search ───────────────────────────────────────────────────────
                let pattern = if let Some(mp) = migemo.as_deref() {
                    mp.to_string()
                } else if case_sensitive {
                    regex::escape(&query)
                } else {
                    format!("(?i){}", regex::escape(&query))
                };
                let re = match regex::Regex::new(&pattern) {
                    Ok(r) => r,
                    Err(_) => return vec![],
                };

                let mut matches: Vec<(usize, usize, usize)> = Vec::new();

                if file_size <= large_file_threshold {
                    // Small file: read all at once
                    let bytes = match std::fs::read(&path) {
                        Ok(b) => b,
                        Err(_) => return vec![],
                    };
                    let mut line_idx = 0usize;
                    let mut line_start = 0usize;
                    while line_start <= bytes.len() {
                        if cancel.is_cancelled() {
                            return matches;
                        }
                        let line_end = bytes[line_start..]
                            .iter()
                            .position(|&b| b == b'\n')
                            .map(|p| line_start + p + 1)
                            .unwrap_or(bytes.len());
                        let raw = &bytes[line_start..line_end.min(bytes.len())];
                        let raw = if raw.last() == Some(&b'\n') {
                            &raw[..raw.len() - 1]
                        } else {
                            raw
                        };
                        let raw = if raw.last() == Some(&b'\r') {
                            &raw[..raw.len() - 1]
                        } else {
                            raw
                        };
                        let decoded = encoding.decode(raw);
                        for m in re.find_iter(&decoded) {
                            matches.push((line_idx, m.start(), m.end()));
                        }
                        if line_end >= bytes.len() {
                            break;
                        }
                        line_start = line_end;
                        line_idx += 1;
                    }
                } else {
                    // Large file: two-pass (build line index, then search)
                    let mut file = match std::fs::File::open(&path) {
                        Ok(f) => f,
                        Err(_) => return vec![],
                    };
                    const CHUNK: usize = 4 * 1024 * 1024;
                    let mut line_offsets: Vec<u64> = vec![0];
                    let mut buf = vec![0u8; CHUNK];
                    let mut abs = 0u64;
                    loop {
                        if cancel.is_cancelled() {
                            return matches;
                        }
                        let n = match file.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => n,
                        };
                        for (i, &byte) in buf.iter().enumerate().take(n) {
                            if byte == b'\n' {
                                line_offsets.push(abs + i as u64 + 1);
                            }
                        }
                        abs += n as u64;
                    }
                    let mut file2 = match std::fs::File::open(&path) {
                        Ok(f) => f,
                        Err(_) => return matches,
                    };
                    for (line_idx, &start) in line_offsets.iter().enumerate() {
                        if cancel.is_cancelled() {
                            return matches;
                        }
                        let end = line_offsets
                            .get(line_idx + 1)
                            .copied()
                            .unwrap_or(file_size as u64);
                        let len = (end - start) as usize;
                        if len == 0 {
                            continue;
                        }
                        if file2.seek(SeekFrom::Start(start)).is_err() {
                            break;
                        }
                        let mut raw = vec![0u8; len.min(65536)];
                        let mut pos = 0;
                        while pos < raw.len() {
                            match file2.read(&mut raw[pos..]) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => pos += n,
                            }
                        }
                        raw.truncate(pos);
                        while matches!(raw.last(), Some(&b'\n') | Some(&b'\r')) {
                            raw.pop();
                        }
                        let decoded = encoding.decode(&raw);
                        for m in re.find_iter(&decoded) {
                            matches.push((line_idx, m.start(), m.end()));
                        }
                    }
                }
                matches
            }
        })
        .await
        .unwrap_or_default();

        let _ = event_tx.send(JobEvent::ViewerSearchComplete(job_id, matches));
        OpResult::Success(SuccessData::None)
    }

    /// Execute a pattern rename operation
    async fn execute_pattern_rename(
        &self,
        targets: &[Location],
        find: &str,
        replace: &str,
        use_regex: bool,
        case_sensitive: bool,
        spec: &JobSpec,
    ) -> OpResult {
        let total = targets.len();

        for (index, location) in targets.iter().enumerate() {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Get the current filename
            let current_name = match self.get_filename(location) {
                Some(name) => name,
                None => {
                    return OpResult::Failed(format!(
                        "Could not extract filename from location: {}",
                        self.location_display(location)
                    ));
                }
            };

            // Apply the pattern to get the new name
            let new_name = crate::pattern_rename::apply_rename_pattern(
                &current_name,
                find,
                replace,
                use_regex,
                case_sensitive,
            );

            // Skip if the name hasn't changed
            if new_name == current_name {
                continue;
            }

            // Get the parent location and create the new location
            let new_location = match location.parent() {
                Some(parent) => parent.join(&new_name),
                None => {
                    return OpResult::Failed(format!(
                        "Could not determine parent directory for: {}",
                        self.location_display(location)
                    ));
                }
            };

            // Perform the rename
            match self
                .backend
                .rename_file(location, &new_location, &spec.cancel_token)
                .await
            {
                Ok(_) => {
                    // Report progress
                    let progress = (index + 1) as f64 / total as f64;
                    if let Err(e) = self
                        .event_sender
                        .send(JobEvent::Progress(spec.id, progress))
                    {
                        tracing::error!(
                            "Failed to send job progress event for {:?}: {}",
                            spec.id,
                            e
                        );
                    }
                }
                Err(e) => {
                    return OpResult::Failed(format!(
                        "Failed to rename {} to {}: {}",
                        current_name, new_name, e
                    ));
                }
            }
        }

        OpResult::Success(SuccessData::None)
    }

    async fn execute_compare_files(
        &self,
        left: &Location,
        right: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        // Check for cancellation
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        // Read both files
        let left_contents = match self.read_file_as_string(left).await {
            Ok(contents) => contents,
            Err(e) => return OpResult::Failed(format!("Failed to read left file: {}", e)),
        };

        let right_contents = match self.read_file_as_string(right).await {
            Ok(contents) => contents,
            Err(e) => return OpResult::Failed(format!("Failed to read right file: {}", e)),
        };

        // Check for cancellation after reading files
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        // Split into lines
        let left_lines: Vec<String> = left_contents.lines().map(|s| s.to_string()).collect();
        let right_lines: Vec<String> = right_contents.lines().map(|s| s.to_string()).collect();

        // Perform simple line-by-line comparison
        let differences = self.compute_diff(&left_lines, &right_lines);

        let diff = crate::job::FileDiff {
            left_path: self.location_display(left),
            right_path: self.location_display(right),
            differences,
        };

        OpResult::Success(SuccessData::ComparisonResult(diff))
    }

    async fn execute_split_file(
        &self,
        source: &Location,
        dest_dir: &Location,
        chunk_size: u64,
        spec: &JobSpec,
    ) -> OpResult {
        // Check for cancellation
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        // Only support local files for now
        let (source_path, dest_path) = match (source, dest_dir) {
            (Location::Local(src), Location::Local(dst)) => (src, dst),
            _ => return OpResult::Failed("Split only supports local files".to_string()),
        };

        // Open source file
        let mut file = match tokio::fs::File::open(source_path).await {
            Ok(f) => f,
            Err(e) => return OpResult::Failed(format!("Failed to open source file: {}", e)),
        };

        // Get file size
        let metadata = match file.metadata().await {
            Ok(m) => m,
            Err(e) => return OpResult::Failed(format!("Failed to get file metadata: {}", e)),
        };
        let total_size = metadata.len();

        // Calculate number of chunks (for potential future use)
        let _num_chunks = total_size.div_ceil(chunk_size);

        // Get base filename
        let base_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");

        // Split the file
        use tokio::io::AsyncReadExt;
        let mut buffer = vec![0u8; chunk_size as usize];
        let mut chunk_index = 0;
        let mut bytes_read_total = 0u64;

        loop {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Read chunk
            let bytes_read = match file.read(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(e) => return OpResult::Failed(format!("Failed to read from source: {}", e)),
            };

            // Write chunk to file
            let chunk_name = format!("{}.part{:03}", base_name, chunk_index);
            let chunk_path = dest_path.join(&chunk_name);

            match tokio::fs::write(&chunk_path, &buffer[..bytes_read]).await {
                Ok(_) => {}
                Err(e) => {
                    return OpResult::Failed(format!(
                        "Failed to write chunk {}: {}",
                        chunk_index, e
                    ))
                }
            }

            chunk_index += 1;
            bytes_read_total += bytes_read as u64;

            // Report progress
            let progress = bytes_read_total as f64 / total_size as f64;
            if let Err(e) = self
                .event_sender
                .send(JobEvent::Progress(spec.id, progress))
            {
                tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
            }
        }

        OpResult::Success(SuccessData::None)
    }

    async fn execute_join_files(
        &self,
        parts: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        // Check for cancellation
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        // Only support local files for now
        let dest_path = match dest {
            Location::Local(path) => path,
            _ => return OpResult::Failed("Join only supports local files".to_string()),
        };

        // Create destination file
        let mut dest_file = match tokio::fs::File::create(dest_path).await {
            Ok(f) => f,
            Err(e) => return OpResult::Failed(format!("Failed to create destination file: {}", e)),
        };

        // Join all parts
        use tokio::io::AsyncWriteExt;
        let total_parts = parts.len();

        for (index, part) in parts.iter().enumerate() {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            let part_path = match part {
                Location::Local(path) => path,
                _ => return OpResult::Failed("All parts must be local files".to_string()),
            };

            // Read part
            let contents = match tokio::fs::read(part_path).await {
                Ok(data) => data,
                Err(e) => return OpResult::Failed(format!("Failed to read part {}: {}", index, e)),
            };

            // Write to destination
            match dest_file.write_all(&contents).await {
                Ok(_) => {}
                Err(e) => {
                    return OpResult::Failed(format!("Failed to write part {}: {}", index, e))
                }
            }

            // Report progress
            let progress = (index + 1) as f64 / total_parts as f64;
            if let Err(e) = self
                .event_sender
                .send(JobEvent::Progress(spec.id, progress))
            {
                tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
            }
        }

        OpResult::Success(SuccessData::None)
    }

    // Helper methods for file comparison

    async fn read_file_as_string(&self, location: &Location) -> Result<String, String> {
        match location {
            Location::Local(path) => tokio::fs::read_to_string(path)
                .await
                .map_err(|e| e.to_string()),
            _ => Err("Only local files supported for comparison".to_string()),
        }
    }

    fn compute_diff(
        &self,
        left_lines: &[String],
        right_lines: &[String],
    ) -> Vec<crate::job::DiffChunk> {
        use crate::job::{DiffChunk, DiffType};

        let mut chunks = Vec::new();
        let mut left_idx = 0;
        let mut right_idx = 0;

        while left_idx < left_lines.len() || right_idx < right_lines.len() {
            // Find matching lines
            let mut equal_lines = Vec::new();
            while left_idx < left_lines.len()
                && right_idx < right_lines.len()
                && left_lines[left_idx] == right_lines[right_idx]
            {
                equal_lines.push(left_lines[left_idx].clone());
                left_idx += 1;
                right_idx += 1;
            }

            if !equal_lines.is_empty() {
                chunks.push(DiffChunk {
                    left_start: left_idx - equal_lines.len(),
                    left_lines: equal_lines.clone(),
                    right_start: right_idx - equal_lines.len(),
                    right_lines: equal_lines,
                    chunk_type: DiffType::Equal,
                });
            }

            // Find differences
            if left_idx < left_lines.len() && right_idx < right_lines.len() {
                // Both have lines - this is a modification
                let left_diff = vec![left_lines[left_idx].clone()];
                let right_diff = vec![right_lines[right_idx].clone()];
                left_idx += 1;
                right_idx += 1;

                chunks.push(DiffChunk {
                    left_start: left_idx - 1,
                    left_lines: left_diff,
                    right_start: right_idx - 1,
                    right_lines: right_diff,
                    chunk_type: DiffType::Modified,
                });
            } else if left_idx < left_lines.len() {
                // Only left has lines - deletion
                let deleted = vec![left_lines[left_idx].clone()];
                left_idx += 1;

                chunks.push(DiffChunk {
                    left_start: left_idx - 1,
                    left_lines: deleted,
                    right_start: right_idx,
                    right_lines: Vec::new(),
                    chunk_type: DiffType::Deleted,
                });
            } else if right_idx < right_lines.len() {
                // Only right has lines - addition
                let added = vec![right_lines[right_idx].clone()];
                right_idx += 1;

                chunks.push(DiffChunk {
                    left_start: left_idx,
                    left_lines: Vec::new(),
                    right_start: right_idx - 1,
                    right_lines: added,
                    chunk_type: DiffType::Added,
                });
            }
        }

        chunks
    }

    // Helper methods

    /// Get the filename from a location
    fn get_filename(&self, location: &Location) -> Option<String> {
        match location {
            Location::Local(path) => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string()),
            Location::Ssh { path, .. } => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string()),
            Location::Cloud { path, .. } => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string()),
            Location::Archive { inner_path, .. } => inner_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string()),
        }
    }

    /// Join a filename to a location
    fn join_location(&self, location: &Location, filename: &str) -> Location {
        match location {
            Location::Local(path) => Location::Local(path.join(filename)),
            Location::Ssh { host, port, path } => Location::Ssh {
                host: host.clone(),
                port: *port,
                path: path.join(filename),
            },
            Location::Cloud {
                provider,
                bucket,
                path,
            } => Location::Cloud {
                provider: provider.clone(),
                bucket: bucket.clone(),
                path: path.join(filename),
            },
            Location::Archive {
                archive_path,
                inner_path,
            } => Location::Archive {
                archive_path: archive_path.clone(),
                inner_path: inner_path.join(filename),
            },
        }
    }

    /// Get a display string for a location
    fn location_display(&self, location: &Location) -> String {
        match location {
            Location::Local(path) => path.display().to_string(),
            Location::Ssh { host, port, path } => {
                format!("ssh://{}:{}{}", host, port, path.display())
            }
            Location::Cloud {
                provider,
                bucket,
                path,
            } => {
                format!("{}://{}/{}", provider, bucket, path.display())
            }
            Location::Archive {
                archive_path,
                inner_path,
            } => {
                format!(
                    "{}#{}",
                    self.location_display(archive_path),
                    inner_path.display()
                )
            }
        }
    }

    /// Execute a countdown test job
    ///
    /// Counts down from start_value to 0, sleeping 1 second between each count.
    /// Sends progress updates every second.
    /// Supports cancellation via cancel_token.
    /// NOTE: JobEvent::Started is sent by execute() wrapper, not here
    async fn execute_countdown(
        &self,
        _duration_secs: u32,
        start_value: u32,
        spec: &JobSpec,
    ) -> OpResult {
        let job_id = spec.id;
        let total = start_value;

        tracing::debug!(
            "CountDownJob: Starting job_id={:?} start_value={}",
            job_id,
            start_value
        );

        // Countdown loop
        for remaining in (0..=start_value).rev() {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                tracing::debug!(
                    "CountDownJob: Cancelled at remaining={} job_id={:?}",
                    remaining,
                    job_id
                );
                return OpResult::Cancelled;
            }

            // Calculate progress (0.0 to 1.0)
            let progress = if total > 0 {
                (total - remaining) as f64 / total as f64
            } else {
                1.0
            };

            // Send progress update with message
            let progress_msg = format!("Countdown: {}/{} seconds", remaining, total);
            let detail_msg = format!("Countdown test job - {} of {} seconds", remaining, total);

            let _ = self.event_sender.send(JobEvent::ProgressWithDetail(
                job_id,
                progress,
                progress_msg,
                detail_msg,
            ));

            // Sleep for 1 second (unless cancelled)
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    // Continue to next iteration
                }
                _ = async {
                    // Check cancellation during sleep
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if spec.cancel_token.is_cancelled() {
                            break;
                        }
                    }
                } => {
                    return OpResult::Cancelled;
                }
            }
        }

        OpResult::Success(SuccessData::None)
    }

    /// Walk the filesystem collecting jump-navigation candidates.
    /// Yields periodically so other tasks can run; respects cancellation.
    async fn execute_collect_jump_candidates(
        &self,
        root: &str,
        include_files: bool,
        max_results: usize,
        max_depth: usize,
        cancel: &CancellationToken,
    ) -> OpResult {
        const IGNORE: &[&str] = &[
            ".git",
            ".svn",
            ".hg",
            "node_modules",
            "target",
            "__pycache__",
            ".cache",
            ".tox",
            "venv",
            ".venv",
        ];

        let mut candidates: Vec<String> = Vec::new();
        let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(std::path::PathBuf::from(root), 0)];

        while let Some((dir, depth)) = stack.pop() {
            if cancel.is_cancelled() {
                return OpResult::Cancelled;
            }
            if candidates.len() >= max_results {
                break;
            }
            if let Ok(read_dir) = std::fs::read_dir(&dir) {
                for entry in read_dir.flatten() {
                    if cancel.is_cancelled() {
                        return OpResult::Cancelled;
                    }
                    let path = entry.path();
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if IGNORE.iter().any(|ign| name_str == *ign) {
                        continue;
                    }
                    let is_dir = path.is_dir();
                    if is_dir || include_files {
                        let p = path.to_string_lossy().into_owned();
                        candidates.push(p);
                        if candidates.len() >= max_results {
                            break;
                        }
                    }
                    if is_dir && depth < max_depth {
                        stack.push((path, depth + 1));
                    }
                }
            }
            // Yield after each directory to keep the event loop responsive
            tokio::task::yield_now().await;
        }

        OpResult::Success(SuccessData::JumpCandidates(candidates))
    }
}

/// Read the leading bytes of a file and run magic-byte detection on them.
/// Runs entirely on the blocking thread pool (open + `SeekableFile::read_bytes`)
/// so the async worker thread never blocks on file I/O — same pattern as the
/// large-file read path in `execute_load_file_for_viewer`.
async fn detect_file_type_blocking(
    path: std::path::PathBuf,
) -> std::io::Result<(crate::magic::DetectedKind, Vec<u8>)> {
    const SNIFF_LEN: usize = 300;

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let size = file.metadata()?.len();
        let seekable = SeekableFile::new(file, size);
        let sample = seekable.read_bytes(0, SNIFF_LEN.min(size as usize))?;
        let kind = crate::magic::detect_kind(&sample);
        Ok((kind, sample))
    })
    .await
    .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())))
}

// ============================================================================
// Conflict Detection Helper Function
// ============================================================================

/// Detect file conflicts between source files and destination
///
/// Returns a list of file conflicts (directories are logged and skipped)
pub async fn detect_conflicts(
    sources: &[Location],
    dest: &Location,
    backend: &dyn FilesystemBackend,
    job_id: crate::job::JobId,
    tab_id: usize,
) -> Result<Vec<crate::model::dialog::ConflictPair>, String> {
    use chrono::Local;

    debug!(
        "detect_conflicts: Checking {} sources against dest {:?}",
        sources.len(),
        dest
    );
    let mut conflicts = Vec::new();

    for source in sources {
        // Calculate destination path
        let dest_location = calculate_destination_path(source, dest);
        debug!(
            "detect_conflicts: Source {:?} -> Dest {:?}",
            source, dest_location
        );

        // Check if destination exists
        match backend.get_entry(&dest_location).await {
            Ok(dest_entry) => {
                debug!("detect_conflicts: Destination exists: {:?}", dest_location);
                // Source entry for comparison
                let source_entry = match backend.get_entry(source).await {
                    Ok(entry) => entry,
                    Err(e) => {
                        debug!(
                            "detect_conflicts: Failed to read source {:?}: {}",
                            source, e
                        );
                        continue; // Skip if can't read source
                    }
                };

                let conflict = crate::model::dialog::ConflictPair {
                    source: source_entry.clone(),
                    dest: dest_entry,
                    source_path: source.clone(),
                    dest_path: dest_location.clone(),
                    is_directory: source_entry.is_dir,
                };

                if conflict.is_directory {
                    // Log directory conflict to task pane and skip
                    let timestamp = Local::now().format("[%H:%M:%S]");
                    let _log_msg = format!(
                        "{} [Job {}] [Tab {}] Copy: \"{}\" conflicts. Skipping",
                        timestamp,
                        &job_id.0.to_string()[..8],
                        tab_id + 1,
                        dest_location.display_path()
                    );
                    // Note: This log needs to be passed back via StateUpdateResult
                    // For now, we just skip directories
                } else {
                    debug!(
                        "detect_conflicts: Adding file conflict for {:?}",
                        dest_location
                    );
                    conflicts.push(conflict);
                }
            }
            Err(e) => {
                debug!(
                    "detect_conflicts: Destination does not exist: {:?} ({})",
                    dest_location, e
                );
                // No conflict - destination doesn't exist
            }
        }
    }

    debug!("detect_conflicts: Found {} conflicts", conflicts.len());
    Ok(conflicts)
}

/// Helper function to calculate destination path for a source file
fn calculate_destination_path(source: &Location, dest: &Location) -> Location {
    match (source, dest) {
        (Location::Local(src_path), Location::Local(dest_path)) => {
            if dest_path.is_dir() {
                // Destination is a directory, append source filename
                if let Some(filename) = src_path.file_name() {
                    Location::Local(dest_path.join(filename))
                } else {
                    dest.clone()
                }
            } else {
                // Destination is a file, use as-is
                dest.clone()
            }
        }
        _ => dest.clone(), // For non-local locations, use dest as-is
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LocalFilesystemBackend;
    use crate::backend::MockArchiveHandler;
    use crate::job::{JobKind, JobSpec};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_execute_read_directory() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(temp_dir.path().to_path_buf()),
        });

        executor.execute(spec).await;

        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        // Then a completed event
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(_, SuccessData::DirectoryRead(_)))
        ));
    }

    #[tokio::test]
    async fn test_execute_mkdir() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let new_dir = temp_dir.path().join("test_dir");
        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(new_dir.clone()),
        });

        executor.execute(spec).await;

        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        // Then a completed event
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(_, SuccessData::None))
        ));

        // Directory should exist
        assert!(new_dir.exists());
    }

    #[tokio::test]
    async fn test_execute_create_file() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let new_file = temp_dir.path().join("test_file.txt");
        let spec = JobSpec::new(JobKind::CreateFile {
            location: Location::Local(new_file.clone()),
        });

        executor.execute(spec).await;

        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        // Then a completed event
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(_, SuccessData::None))
        ));

        // File should exist
        assert!(new_file.exists());
        assert!(new_file.is_file());
    }

    #[tokio::test]
    async fn test_execute_change_attributes() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let file1 = temp_dir.path().join("a.txt");
        let file2 = temp_dir.path().join("b.txt");
        tokio::fs::write(&file1, b"x").await.unwrap();
        tokio::fs::write(&file2, b"y").await.unwrap();

        let spec = JobSpec::new(JobKind::ChangeAttributes {
            targets: vec![
                Location::Local(file1.clone()),
                Location::Local(file2.clone()),
            ],
            attrs: crate::model::AttributeChange {
                #[cfg(windows)]
                readonly: None,
                #[cfg(windows)]
                hidden: Some(true),
                #[cfg(windows)]
                system: None,
                #[cfg(windows)]
                archive: None,
                #[cfg(unix)]
                mode: None,
            },
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        let event = event_rx.recv().await;
        match event {
            Some(JobEvent::Completed(_, SuccessData::AttributesChanged(outcomes))) => {
                assert_eq!(outcomes.len(), 2);
                assert!(outcomes.iter().all(|o| o.result.is_ok()));
            }
            other => panic!("Expected AttributesChanged success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_execute_change_timestamps() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let file1 = temp_dir.path().join("a.txt");
        tokio::fs::write(&file1, b"x").await.unwrap();

        let new_modified =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        let spec = JobSpec::new(JobKind::ChangeTimestamps {
            targets: vec![Location::Local(file1.clone())],
            times: crate::model::TimestampChange {
                modified: Some(new_modified),
                accessed: None,
                #[cfg(windows)]
                created: None,
            },
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        let event = event_rx.recv().await;
        match event {
            Some(JobEvent::Completed(_, SuccessData::TimestampsChanged(outcomes))) => {
                assert_eq!(outcomes.len(), 1);
                assert!(outcomes[0].result.is_ok());
            }
            other => panic!("Expected TimestampsChanged success, got {other:?}"),
        }

        let modified_after = tokio::fs::metadata(&file1)
            .await
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(modified_after, new_modified);
    }

    #[tokio::test]
    async fn test_execute_create_link() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let target = temp_dir.path().join("target.txt");
        let link = temp_dir.path().join("link.txt");
        tokio::fs::write(&target, b"content").await.unwrap();

        let spec = JobSpec::new(JobKind::CreateLink {
            target: Location::Local(target),
            link_path: Location::Local(link.clone()),
            kind: crate::model::LinkCreateKind::Hardlink,
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(_, SuccessData::None))
        ));

        assert!(link.exists());
    }

    #[tokio::test]
    async fn test_execute_spawn_process_succeeds_with_suppressed_stdio() {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        // Mirrors what `system_open_job`/`editor_job` actually spawn on Windows: a
        // transient `cmd.exe` that may print startup noise (e.g. a user's Clink AutoRun
        // hook failing to inject) if its stdio is inherited from us — `echo` stands in
        // for that noise here. The job must still complete successfully with stdio
        // redirected away from our own console.
        #[cfg(target_os = "windows")]
        let (program, args) = (
            "cmd".to_string(),
            vec![
                "/c".to_string(),
                "echo".to_string(),
                "should not reach our screen".to_string(),
            ],
        );
        #[cfg(not(target_os = "windows"))]
        let (program, args) = (
            "echo".to_string(),
            vec!["should not reach our screen".to_string()],
        );

        let spec = JobSpec::new(JobKind::SpawnProcess {
            program,
            args,
            wait: true,
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(_, SuccessData::None))
        ));
    }

    #[tokio::test]
    async fn test_execute_copy() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        // Create a source file
        let source_file = temp_dir.path().join("source.txt");
        tokio::fs::write(&source_file, b"test content")
            .await
            .unwrap();

        let dest_dir = temp_dir.path().join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(source_file.clone())],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Should receive started, progress, and completed events
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                JobEvent::Started(_) => received_started = true,
                JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started);
        assert!(received_completed);

        // Destination file should exist
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
    }

    #[tokio::test]
    async fn test_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(temp_dir.path().to_path_buf()),
        });

        // Cancel the job before execution
        spec.cancel_token.cancel();

        executor.execute(spec).await;

        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        // Then a cancelled event
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Cancelled(_))));
    }

    // ── DetectFileType / DetectFileTypesBatch (Phase 7.3) ──────────────────────

    #[tokio::test]
    async fn test_execute_detect_file_type_png_signature() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let png_path = temp_dir.path().join("picture.dat");
        let mut bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&[0u8; 32]); // trailing filler, irrelevant to detection
        tokio::fs::write(&png_path, &bytes).await.unwrap();

        let spec = JobSpec::new(JobKind::DetectFileType {
            path: png_path,
            purpose: crate::job::DetectFileTypePurpose::FileInfoDisplay,
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        let Some(JobEvent::Completed(_, SuccessData::FileTypeDetected { kind, header_bytes })) =
            event
        else {
            panic!("expected FileTypeDetected, got {:?}", event);
        };
        assert_eq!(kind, crate::magic::DetectedKind::Png);
        assert_eq!(
            &header_bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[tokio::test]
    async fn test_execute_detect_file_type_plain_text_is_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let text_path = temp_dir.path().join("notes.txt");
        tokio::fs::write(&text_path, b"just some plain text, nothing magic here\n")
            .await
            .unwrap();

        let spec = JobSpec::new(JobKind::DetectFileType {
            path: text_path.clone(),
            purpose: crate::job::DetectFileTypePurpose::FallbackOpen {
                location: crate::model::Location::Local(text_path),
            },
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(
                _,
                SuccessData::FileTypeDetected {
                    kind: crate::magic::DetectedKind::Unknown,
                    ..
                }
            ))
        ));
    }

    #[tokio::test]
    async fn test_execute_detect_file_type_reports_failure_for_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let missing_path = temp_dir.path().join("does_not_exist.bin");

        let spec = JobSpec::new(JobKind::DetectFileType {
            path: missing_path,
            purpose: crate::job::DetectFileTypePurpose::FileInfoDisplay,
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Failed(_, _))));
    }

    #[tokio::test]
    async fn test_execute_detect_file_types_batch_mixed_signatures() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let png_path = temp_dir.path().join("a.dat");
        tokio::fs::write(&png_path, [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .await
            .unwrap();

        let pdf_path = temp_dir.path().join("b.dat");
        tokio::fs::write(&pdf_path, b"%PDF-1.7\n%...")
            .await
            .unwrap();

        let text_path = temp_dir.path().join("c.dat");
        tokio::fs::write(&text_path, b"hello world").await.unwrap();

        // Included to prove a bad entry doesn't fail the whole batch.
        let missing_path = temp_dir.path().join("missing.dat");

        let spec = JobSpec::new(JobKind::DetectFileTypesBatch {
            paths: vec![
                png_path.clone(),
                pdf_path.clone(),
                text_path.clone(),
                missing_path.clone(),
            ],
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        let Some(JobEvent::Completed(_, SuccessData::FileTypesDetected(results))) = event else {
            panic!("expected FileTypesDetected, got {:?}", event);
        };

        assert_eq!(results.len(), 4);
        assert_eq!(
            results.iter().find(|(p, _)| *p == png_path).unwrap().1,
            crate::magic::DetectedKind::Png
        );
        assert_eq!(
            results.iter().find(|(p, _)| *p == pdf_path).unwrap().1,
            crate::magic::DetectedKind::Pdf
        );
        assert_eq!(
            results.iter().find(|(p, _)| *p == text_path).unwrap().1,
            crate::magic::DetectedKind::Unknown
        );
        assert_eq!(
            results.iter().find(|(p, _)| *p == missing_path).unwrap().1,
            crate::magic::DetectedKind::Unknown
        );
    }

    #[tokio::test]
    async fn test_execute_detect_file_type_empty_file_is_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let empty_path = temp_dir.path().join("empty.dat");
        tokio::fs::write(&empty_path, b"").await.unwrap();

        let spec = JobSpec::new(JobKind::DetectFileType {
            path: empty_path,
            purpose: crate::job::DetectFileTypePurpose::FileInfoDisplay,
        });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(
                _,
                SuccessData::FileTypeDetected {
                    kind: crate::magic::DetectedKind::Unknown,
                    ..
                }
            ))
        ));
    }

    #[tokio::test]
    async fn test_execute_detect_file_types_batch_empty_paths_returns_empty_results() {
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::DetectFileTypesBatch { paths: vec![] });

        executor.execute(spec).await;

        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(JobEvent::Completed(_, SuccessData::FileTypesDetected(results))) if results.is_empty()
        ));
    }

    #[tokio::test]
    async fn test_execute_move_to_trash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("trash_me.txt");
        std::fs::write(&file_path, b"x").unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::MoveToTrash {
            targets: vec![Location::Local(file_path.clone())],
            force_fallback: false,
        });
        executor.execute(spec).await;

        assert!(matches!(event_rx.recv().await, Some(JobEvent::Started(_))));
        let event = event_rx.recv().await;
        match event {
            Some(JobEvent::Completed(_, SuccessData::TrashMoved(outcomes))) => {
                assert_eq!(outcomes.len(), 1);
                assert!(outcomes[0].result.is_ok());
                assert!(outcomes[0].record.is_some());
            }
            other => panic!("expected TrashMoved success, got {other:?}"),
        }
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_execute_move_to_trash_partial_failure_does_not_fail_fast() {
        let temp_dir = TempDir::new().unwrap();
        let ok_path = temp_dir.path().join("exists.txt");
        std::fs::write(&ok_path, b"x").unwrap();
        let missing_path = temp_dir.path().join("does_not_exist.txt");
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::MoveToTrash {
            targets: vec![
                Location::Local(ok_path.clone()),
                Location::Local(missing_path.clone()),
            ],
            force_fallback: false,
        });
        executor.execute(spec).await;

        let _ = event_rx.recv().await; // Started
        let event = event_rx.recv().await;
        match event {
            Some(JobEvent::Completed(_, SuccessData::TrashMoved(outcomes))) => {
                assert_eq!(
                    outcomes.len(),
                    2,
                    "both targets must be represented, not fail-fast"
                );
                assert!(outcomes[0].result.is_ok());
                assert!(outcomes[1].result.is_err());
            }
            other => panic!("expected TrashMoved (partial success), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_execute_restore_from_trash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("restore_via_executor.txt");
        std::fs::write(&file_path, b"x").unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let cancel_token = CancellationToken::new();
        let record = backend
            .move_to_trash(&Location::Local(file_path.clone()), false, &cancel_token)
            .await
            .unwrap();

        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::RestoreFromTrash {
            records: vec![record],
        });
        executor.execute(spec).await;

        let _ = event_rx.recv().await; // Started
        let event = event_rx.recv().await;
        match event {
            Some(JobEvent::Completed(_, SuccessData::TrashRestored(outcomes))) => {
                assert_eq!(outcomes.len(), 1);
                assert!(outcomes[0].result.is_ok());
            }
            other => panic!("expected TrashRestored success, got {other:?}"),
        }
        assert!(file_path.exists());
    }

    /// Best-effort cleanup of a real `.rwf-trash` directory this test wrote
    /// to at the true filesystem volume root (not a `TempDir`). Runs on
    /// `Drop` so it fires even if an assertion in the test body panics.
    /// Mirrors `RealTrashDirCleanup` in `backend/local.rs`.
    struct RealTrashDirCleanup(std::path::PathBuf);
    impl Drop for RealTrashDirCleanup {
        fn drop(&mut self) {
            if let Ok(entries) = std::fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_execute_empty_trash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("fallback_purge_via_executor.txt");
        std::fs::write(&file_path, b"x").unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let cancel_token = CancellationToken::new();
        // force_fallback: true, so this uses .rwf-trash at the real volume
        // root, NOT the real OS Recycle Bin. Scope is Fallback-only (NOT
        // All/OsManaged) so this test never purges the real OS trash.
        backend
            .move_to_trash(&Location::Local(file_path.clone()), true, &cancel_token)
            .await
            .expect("forced fallback move should succeed");
        let volume_root = temp_dir.path().ancestors().last().unwrap().to_path_buf();
        let trash_dir = volume_root.join(".rwf-trash");
        let _cleanup = RealTrashDirCleanup(trash_dir.clone());

        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::EmptyTrash {
            scope: crate::model::EmptyTrashScope::Fallback,
            older_than_days: None,
            fallback_roots: vec![volume_root],
        });
        executor.execute(spec).await;

        let _ = event_rx.recv().await; // Started
        let event = event_rx.recv().await;
        match event {
            Some(JobEvent::Completed(_, SuccessData::TrashEmptied { purged })) => {
                assert!(purged >= 1);
            }
            other => panic!("expected TrashEmptied, got {other:?}"),
        }
    }
}
