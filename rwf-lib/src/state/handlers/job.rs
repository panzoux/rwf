use crate::job::JobSpec;
use crate::state::{update_state, AppState, PaneRefresh, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_job_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        use std::time::SystemTime;

        match transition {
            Transition::EnqueueJob { spec } => {
                self.jobs.enqueue(spec.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::StartNextJob => {
                if self.jobs.can_start_job() {
                    if let Some(spec) = self.jobs.pop_next_job() {
                        self.jobs.start_job(spec.clone());
                        Some(StateUpdateResult::with_job(spec))
                    } else {
                        Some(StateUpdateResult::none())
                    }
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::JobStarted { job_id } => {
                if let Some(job) = self.jobs.active.get_mut(job_id) {
                    job.state = crate::job::ExecutionState::Running;
                    job.started_at = Some(SystemTime::now());
                }

                let log_entry = self.background_jobs.get_job(*job_id).map(|bg_job| {
                    let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                    format!(
                        "{} [Job {}] [Tab {}] {}: Started",
                        timestamp,
                        bg_job.id.short_id,
                        bg_job.tab_id + 1,
                        bg_job.name
                    )
                });

                if let Some(log) = log_entry {
                    self.background_jobs.mark_job_running(*job_id);
                    Some(StateUpdateResult {
                        jobs_to_start: Vec::new(),
                        jobs_to_cancel: Vec::new(),
                        completed_jobs: Vec::new(),
                        failed_jobs: Vec::new(),
                        cancelled_jobs: Vec::new(),
                        started_jobs: vec![*job_id],
                        task_panel_logs: vec![log],
                        panes_to_refresh: Vec::new(),
                        ui_changed: true,
                        reload_keybindings: false,
                    })
                } else {
                    Some(StateUpdateResult::with_ui_change())
                }
            }
            Transition::UpdateJobProgress { job_id, progress } => {
                self.jobs.update_progress(*job_id, *progress);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdateJobProgressWithDetail {
                job_id,
                progress,
                progress_message,
                operation_detail,
            } => {
                self.jobs.update_progress(*job_id, *progress);
                let needs_running = self
                    .background_jobs
                    .get_job(*job_id)
                    .map(|j| j.status == crate::job::JobStatus::Pending)
                    .unwrap_or(false);

                let job_progress = crate::job::JobProgress {
                    percent: *progress,
                    message: progress_message.clone(),
                    current_operation_detail: operation_detail.clone(),
                };
                self.background_jobs.update_progress(*job_id, job_progress);

                if needs_running {
                    self.background_jobs.mark_job_running(*job_id);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CompleteJob { job_id, result } => {
                tracing::info!(
                    "[CompleteJob] Received completion event for job={:?}",
                    job_id
                );
                let job_spec = self.jobs.active.get(job_id).map(|job| job.spec.clone());
                tracing::debug!(
                    "[CompleteJob] Processing job_id={:?}, has_spec={}, result_type={}",
                    job_id,
                    job_spec.is_some(),
                    match result {
                        crate::job::OpResult::Success(_) => "Success",
                        crate::job::OpResult::Failed(_) => "Failed",
                        crate::job::OpResult::Cancelled => "Cancelled",
                    }
                );

                let log_entry = self.background_jobs.get_job(*job_id).map(|bg_job| {
                    let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                    match result {
                        crate::job::OpResult::Success(_) => format!(
                            "{} [Job {}] [Tab {}] {}: [OK]",
                            timestamp,
                            bg_job.id.short_id,
                            bg_job.tab_id + 1,
                            bg_job.name
                        ),
                        crate::job::OpResult::Failed(e) => {
                            let detail = e.trim();
                            if detail.is_empty() {
                                format!(
                                    "{} [Job {}] [Tab {}] {}: [FAIL]",
                                    timestamp,
                                    bg_job.id.short_id,
                                    bg_job.tab_id + 1,
                                    bg_job.name
                                )
                            } else {
                                format!(
                                    "{} [Job {}] [Tab {}] {}: [FAIL] — {}",
                                    timestamp,
                                    bg_job.id.short_id,
                                    bg_job.tab_id + 1,
                                    bg_job.name,
                                    detail
                                )
                            }
                        }
                        crate::job::OpResult::Cancelled => format!(
                            "{} [Job {}] [Tab {}] {}: [WARN] Cancelled",
                            timestamp,
                            bg_job.id.short_id,
                            bg_job.tab_id + 1,
                            bg_job.name
                        ),
                    }
                });

                if let crate::job::OpResult::Failed(ref error_message) = result {
                    if let Some(ref spec) = job_spec {
                        // ExecuteCustomFunction failures go to the task panel log only (no
                        // modal). DetectFileType/FileInfoDisplay failures also skip the modal:
                        // the still-open File Information dialog already shows its own
                        // "detection failed" line (Phase 7.3 §7) — a second, stacked Error
                        // dialog for the same failure would be redundant.
                        // CheckAssociationMismatch failures also skip the modal: the completion
                        // arm below fails open and runs the association command anyway, so a
                        // stacked Error dialog would be a spurious false alarm on top of the
                        // (usually successful) association execution.
                        // FallbackOpen failures also skip the modal: the completion arm below
                        // falls back to the text viewer as its own safety net, so a stacked
                        // Error dialog would be a redundant double-report.
                        // ResolveAssociation failures (Phase 7.3b) also skip the modal: the
                        // completion arm below fails open to extension-only resolution (its
                        // own safety net), so a stacked Error dialog would be redundant.
                        // ContextMenuLabel failures (Phase 7.3b, Task 9) also skip the modal:
                        // the completion arm below sets the still-open ContextMenu dialog's
                        // "detection failed" label as its own inline feedback, so a stacked
                        // Error dialog over a context menu would be a redundant double-report.
                        let skip_dialog = matches!(
                            &spec.kind,
                            crate::job::JobKind::ExecuteCustomFunction { .. }
                        ) || matches!(
                            &spec.kind,
                            crate::job::JobKind::DetectFileType {
                                purpose: crate::job::DetectFileTypePurpose::FileInfoDisplay,
                                ..
                            }
                        ) || matches!(
                            &spec.kind,
                            crate::job::JobKind::DetectFileType {
                                purpose:
                                    crate::job::DetectFileTypePurpose::CheckAssociationMismatch { .. },
                                ..
                            }
                        ) || matches!(
                            &spec.kind,
                            crate::job::JobKind::DetectFileType {
                                purpose: crate::job::DetectFileTypePurpose::FallbackOpen { .. },
                                ..
                            }
                        ) || matches!(
                            &spec.kind,
                            crate::job::JobKind::DetectFileType {
                                purpose: crate::job::DetectFileTypePurpose::ResolveAssociation { .. },
                                ..
                            }
                        ) || matches!(
                            &spec.kind,
                            crate::job::JobKind::DetectFileType {
                                purpose: crate::job::DetectFileTypePurpose::ContextMenuLabel,
                                ..
                            }
                        );
                        let op_name = match &spec.kind {
                            crate::job::JobKind::ReadDirectory { .. } => "Read directory",
                            crate::job::JobKind::Copy { .. } => "Copy",
                            crate::job::JobKind::Move { .. } => "Move",
                            crate::job::JobKind::Delete { .. } => "Delete",
                            crate::job::JobKind::MoveToTrash { .. } => "Move to trash",
                            crate::job::JobKind::RestoreFromTrash { .. } => "Restore from trash",
                            crate::job::JobKind::EmptyTrash { .. } => "Empty trash",
                            crate::job::JobKind::ScanTrash { .. } => "Scan trash",
                            crate::job::JobKind::ListTrash { .. } => "List trash",
                            crate::job::JobKind::Mkdir { .. } => "Create directory",
                            crate::job::JobKind::CreateFile { .. } => "Create file",
                            crate::job::JobKind::ChangeAttributes { .. } => "Change attributes",
                            crate::job::JobKind::ChangeTimestamps { .. } => "Change timestamps",
                            crate::job::JobKind::CreateLink { .. } => "Create link",
                            crate::job::JobKind::Rename { .. } => "Rename",
                            crate::job::JobKind::CalculateSize { .. } => "Calculate size",
                            crate::job::JobKind::ExtractArchive { .. } => "Extract archive",
                            crate::job::JobKind::CreateArchive { .. } => "Create archive",
                            crate::job::JobKind::ExecuteCustomFunction { .. } => {
                                "Execute custom function"
                            }
                            crate::job::JobKind::Search { .. } => "Search",
                            crate::job::JobKind::LoadFileForViewer { .. } => "Load file for viewer",
                            crate::job::JobKind::ViewerSearch { .. } => "Viewer search",
                            crate::job::JobKind::PatternRename { .. } => "Pattern rename",
                            crate::job::JobKind::CompareFiles { .. } => "File comparison",
                            crate::job::JobKind::SplitFile { .. } => "File split",
                            crate::job::JobKind::JoinFiles { .. } => "File join",
                            crate::job::JobKind::CountDown { .. } => "Countdown",
                            crate::job::JobKind::CollectJumpCandidates { .. } => {
                                "Collect jump candidates"
                            }
                            crate::job::JobKind::SpawnProcess { .. } => "Spawn process",
                            crate::job::JobKind::SuspendAndRun { .. } => "Terminal editor",
                            // Phase 7.3 foundation (Task 1): nothing constructs these yet;
                            // completion routing lands in a later task.
                            crate::job::JobKind::DetectFileType { .. } => "Detect file type",
                            crate::job::JobKind::DetectFileTypesBatch { .. } => "Detect file types",
                        };
                        if !skip_dialog {
                            let error_dialog =
                                crate::model::Dialog::from_job_failure(op_name, error_message);
                            self.dialogs.push(error_dialog);
                        }
                    }
                }

                self.jobs.complete_job(*job_id, result.clone());

                if let Some(ref spec) = job_spec {
                    match &spec.kind {
                        crate::job::JobKind::ReadDirectory { location } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::DirectoryRead(entries),
                            ) = result
                            {
                                self.cache.insert(location.clone(), entries.clone());
                            }
                        }
                        crate::job::JobKind::Copy { dest, .. }
                        | crate::job::JobKind::Move { dest, .. }
                        | crate::job::JobKind::ExtractArchive { dest, .. } => {
                            self.cache.invalidate(dest);
                        }
                        // MoveToTrash removes entries from their directory exactly as
                        // Delete does — only the destination differs — so it must
                        // invalidate the same caches. It previously fell through to the
                        // catch-all and invalidated nothing.
                        crate::job::JobKind::Delete { targets }
                        | crate::job::JobKind::MoveToTrash { targets, .. } => {
                            for target in targets {
                                if let Some(parent) = target.parent() {
                                    self.cache.invalidate(&parent);
                                }
                                self.cache.invalidate_prefix(target);
                                self.navigation_cache.invalidate_prefix(target);
                            }
                        }
                        crate::job::JobKind::Rename { from, to } => {
                            if let Some(parent) = from.parent() {
                                self.cache.invalidate(&parent);
                            }
                            self.cache.invalidate_prefix(from);
                            self.cache.invalidate_prefix(to);
                            self.navigation_cache.invalidate_prefix(from);
                            self.navigation_cache.invalidate_prefix(to);
                        }
                        crate::job::JobKind::PatternRename { targets, .. } => {
                            for target in targets {
                                if let Some(parent) = target.parent() {
                                    self.cache.invalidate(&parent);
                                }
                            }
                        }
                        crate::job::JobKind::Mkdir { location }
                        | crate::job::JobKind::CreateFile { location } => {
                            if let Some(parent) = location.parent() {
                                self.cache.invalidate(&parent);
                            }
                        }
                        crate::job::JobKind::CreateLink { link_path, .. } => {
                            if let Some(parent) = link_path.parent() {
                                self.cache.invalidate(&parent);
                            }
                        }
                        _ => {}
                    }
                }

                let mut result_obj = StateUpdateResult::with_ui_change();
                match result {
                    crate::job::OpResult::Success(_) => result_obj.completed_jobs.push(*job_id),
                    crate::job::OpResult::Failed(_) => result_obj.failed_jobs.push(*job_id),
                    crate::job::OpResult::Cancelled => result_obj.cancelled_jobs.push(*job_id),
                }

                if let Some(spec) = job_spec {
                    tracing::info!(
                        "[CompleteJob] Job spec kind={:?}, requesting_pane={:?}",
                        spec.kind,
                        spec.requesting_pane
                    );
                    match &spec.kind {
                        crate::job::JobKind::ReadDirectory { location } => {
                            tracing::info!("[CompleteJob::ReadDirectory] location={}, requesting_pane={:?}, success={}", location.display_path(), spec.requesting_pane, matches!(result, crate::job::OpResult::Success(_)));
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::DirectoryRead(entries),
                            ) = result
                            {
                                if let Some((requesting_tab_id, pane_side)) = spec.requesting_pane {
                                    tracing::info!("[CompleteJob::ReadDirectory] Looking up tab_id={}, current_tabs.len()={}", requesting_tab_id, self.tabs.tabs.len());
                                    for (idx, t) in self.tabs.tabs.iter().enumerate() {
                                        tracing::debug!(
                                            "[CompleteJob::ReadDirectory] Tab[{}].id={}",
                                            idx,
                                            t.id
                                        );
                                    }

                                    if let Some(tab) = self
                                        .tabs
                                        .tabs
                                        .iter_mut()
                                        .find(|t| t.id == requesting_tab_id)
                                    {
                                        let pane = match pane_side {
                                            crate::model::ActivePane::Left => &mut tab.left_pane,
                                            crate::model::ActivePane::Right => &mut tab.right_pane,
                                        };

                                        // Verify job ownership
                                        if pane.active_job_id == Some(*job_id) {
                                            let pane_name = match pane_side {
                                                crate::model::ActivePane::Left => "Left",
                                                crate::model::ActivePane::Right => "Right",
                                            };
                                            tracing::info!("[CompleteJob::ReadDirectory] Found tab! Updating {} pane with {} entries", pane_name, entries.len());
                                            if pane.raw_entries != *entries {
                                                pane.raw_entries = entries.clone();
                                                pane.entries = entries.clone();
                                                pane.is_loading = false;
                                                pane.apply_sort();
                                                pane.apply_current_filter();
                                                pane.update_scroll(
                                                    self.ui.layout.pane_height,
                                                    self.config.ui.scroll_offset,
                                                );
                                                if let Some(name) = pane.pending_cursor_name.take()
                                                {
                                                    if let Some(pos) = pane
                                                        .entries
                                                        .iter()
                                                        .position(|e| e.name == name)
                                                    {
                                                        pane.cursor = pos;
                                                        pane.update_scroll(
                                                            self.ui.layout.pane_height,
                                                            self.config.ui.scroll_offset,
                                                        );
                                                    }
                                                }
                                                result_obj.ui_changed = true;
                                            } else {
                                                pane.is_loading = false;
                                                pane.pending_cursor_name = None;
                                            }
                                            pane.active_job_id = None; // Job complete
                                        } else {
                                            tracing::warn!("[CompleteJob::ReadDirectory] Stale job result (id={:?}, expected={:?}). Discarding.", job_id, pane.active_job_id);
                                        }
                                    } else {
                                        tracing::warn!("[CompleteJob::ReadDirectory] Tab not found (likely closed)! tab_id={}, job_id={:?}. Cancelling job.", requesting_tab_id, job_id);
                                        self.background_jobs.cancel_job(*job_id);
                                    }
                                } else {
                                    // Fallback to old behavior
                                    tracing::warn!("[CompleteJob::ReadDirectory] Using fallback path - requesting_pane is None! location={}", location.display_path());
                                    result_obj.ui_changed = false;
                                    for tab in self.tabs.tabs.iter_mut() {
                                        if tab.left_pane.current_location == *location {
                                            tracing::debug!("[CompleteJob::ReadDirectory::Fallback] Updating left pane via fallback");
                                            tab.left_pane.raw_entries = entries.clone();
                                            tab.left_pane.entries = entries.clone();
                                            tab.left_pane.is_loading = false;
                                            tab.left_pane.apply_sort();
                                            tab.left_pane.apply_current_filter();
                                            tab.left_pane.update_scroll(
                                                self.ui.layout.pane_height,
                                                self.config.ui.scroll_offset,
                                            );
                                            result_obj.ui_changed = true;
                                        }
                                        if tab.right_pane.current_location == *location {
                                            tracing::debug!("[CompleteJob::ReadDirectory::Fallback] Updating right pane via fallback");
                                            tab.right_pane.raw_entries = entries.clone();
                                            tab.right_pane.entries = entries.clone();
                                            tab.right_pane.is_loading = false;
                                            tab.right_pane.apply_sort();
                                            tab.right_pane.apply_current_filter();
                                            tab.right_pane.update_scroll(
                                                self.ui.layout.pane_height,
                                                self.config.ui.scroll_offset,
                                            );
                                            result_obj.ui_changed = true;
                                        }
                                    }
                                }
                            } else {
                                // Reset loading state on failure/cancellation
                                if let Some((requesting_tab_id, pane_side)) = spec.requesting_pane {
                                    if let Some(tab) = self
                                        .tabs
                                        .tabs
                                        .iter_mut()
                                        .find(|t| t.id == requesting_tab_id)
                                    {
                                        let pane = match pane_side {
                                            crate::model::ActivePane::Left => &mut tab.left_pane,
                                            crate::model::ActivePane::Right => &mut tab.right_pane,
                                        };
                                        pane.is_loading = false;
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::LoadFileForViewer { .. } => {
                            // Buffer was already delivered via ViewerReady event.
                            // On final Completed just mark loading as done.
                            if let crate::job::OpResult::Success(_) = result {
                                if let Some(ref mut viewer) = self.viewer {
                                    viewer.is_loading = false;
                                }
                                result_obj.ui_changed = true;
                            }
                        }
                        crate::job::JobKind::ViewerSearch { .. } => {
                            // Results delivered via ViewerSearchComplete event; nothing to do.
                        }
                        crate::job::JobKind::CompareFiles { .. } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::ComparisonResult(diff),
                            ) = result
                            {
                                let comp_res = update_state(
                                    self,
                                    Transition::ShowComparisonView { diff: diff.clone() },
                                );
                                result_obj.ui_changed = comp_res.ui_changed;
                            }
                        }
                        crate::job::JobKind::Copy { dest, .. }
                        | crate::job::JobKind::ExtractArchive { dest, .. }
                        | crate::job::JobKind::CreateArchive { dest, .. }
                        | crate::job::JobKind::SplitFile { dest_dir: dest, .. } => {
                            for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                if tab.left_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh {
                                        tab_id: tab_idx,
                                        pane: crate::model::ActivePane::Left,
                                    });
                                }
                                if tab.right_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh {
                                        tab_id: tab_idx,
                                        pane: crate::model::ActivePane::Right,
                                    });
                                }
                            }
                        }
                        // Restore is the inverse of MoveToTrash, but it *adds* entries
                        // back rather than removing them, so the in-memory path used by
                        // Delete/MoveToTrash does not apply — the restored files are not
                        // in any pane's list to begin with. Refresh whichever panes are
                        // showing a destination directory instead.
                        //
                        // Locations come from the outcomes, not the job spec: only
                        // targets that actually succeeded should trigger a re-read.
                        crate::job::JobKind::RestoreFromTrash { .. } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::TrashRestored(outcomes),
                            ) = result
                            {
                                let restored_dirs: Vec<_> = outcomes
                                    .iter()
                                    .filter(|o| o.result.is_ok())
                                    .filter_map(|o| o.original.parent())
                                    .collect();

                                for dir in &restored_dirs {
                                    self.cache.invalidate(dir);
                                    self.navigation_cache.invalidate_prefix(dir);
                                }

                                for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                    if restored_dirs.contains(&tab.left_pane.current_location) {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Left,
                                        });
                                    }
                                    if restored_dirs.contains(&tab.right_pane.current_location) {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Right,
                                        });
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::JoinFiles { dest, .. } => {
                            if let Some(parent) = dest.parent() {
                                for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                    if tab.left_pane.current_location == parent {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Left,
                                        });
                                    }
                                    if tab.right_pane.current_location == parent {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Right,
                                        });
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::Move { sources, dest } => {
                            for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                if tab.left_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh {
                                        tab_id: tab_idx,
                                        pane: crate::model::ActivePane::Left,
                                    });
                                }
                                if tab.right_pane.current_location == *dest {
                                    result_obj.panes_to_refresh.push(PaneRefresh {
                                        tab_id: tab_idx,
                                        pane: crate::model::ActivePane::Right,
                                    });
                                }
                            }
                            for source in sources {
                                if let Some(parent) = source.parent() {
                                    for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                        if tab.left_pane.current_location == parent {
                                            result_obj.panes_to_refresh.push(PaneRefresh {
                                                tab_id: tab_idx,
                                                pane: crate::model::ActivePane::Left,
                                            });
                                        }
                                        if tab.right_pane.current_location == parent {
                                            result_obj.panes_to_refresh.push(PaneRefresh {
                                                tab_id: tab_idx,
                                                pane: crate::model::ActivePane::Right,
                                            });
                                        }
                                    }
                                }
                            }
                            self.unmark_all_panes();
                        }
                        crate::job::JobKind::Rename { from, to } => {
                            // In-memory update: no ReadDirectory needed for a single rename.
                            // Guard on the record's own `succeeded` flag, not the outer
                            // OpResult variant — execute_rename (Phase 7.6) always returns
                            // OpResult::Success now, even when the rename itself failed (the
                            // failure is recorded per-record instead of failing the whole
                            // job), so checking the raw OpResult variant here would patch
                            // the pane even on a failed rename.
                            let renamed_ok = if let crate::job::OpResult::Success(
                                crate::job::SuccessData::OperationRecords(records),
                            ) = result
                            {
                                records.first().is_some_and(|r| r.succeeded)
                            } else {
                                false
                            };
                            if renamed_ok {
                                let new_name = to
                                    .path()
                                    .and_then(|p| p.file_name())
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                if !new_name.is_empty() {
                                    let pane_height = self.ui.layout.pane_height;
                                    let scroll_offset = self.config.ui.scroll_offset;
                                    for tab in self.tabs.tabs.iter_mut() {
                                        for pane in [&mut tab.left_pane, &mut tab.right_pane] {
                                            if let Some(e) = pane
                                                .raw_entries
                                                .iter_mut()
                                                .find(|e| &e.location == from)
                                            {
                                                e.name = new_name.clone();
                                                e.location = to.clone();
                                            }
                                            pane.apply_sort();
                                            pane.apply_current_filter();
                                            pane.update_scroll(pane_height, scroll_offset);
                                        }
                                    }
                                    result_obj.ui_changed = true;
                                }
                            }
                        }
                        // MoveToTrash shares this arm with Delete: from the pane's point
                        // of view both simply remove the targets from their directory.
                        // Without it a successful trash move left the entries listed and
                        // still marked until something else forced a refresh — the Layer 1
                        // refresh contract in plan/ROADMAP.md was not being met.
                        crate::job::JobKind::Delete { targets }
                        | crate::job::JobKind::MoveToTrash { targets, .. } => {
                            if let crate::job::OpResult::Success(_) = result {
                                // In-memory removal: remove deleted entries from all panes without
                                // triggering a full ReadDirectory (same approach as Rename).
                                let pane_height = self.ui.layout.pane_height;
                                let scroll_offset = self.config.ui.scroll_offset;
                                let mut any_changed = false;
                                for tab in self.tabs.tabs.iter_mut() {
                                    for pane in [&mut tab.left_pane, &mut tab.right_pane] {
                                        let before = pane.raw_entries.len();
                                        pane.raw_entries.retain(|e| !targets.contains(&e.location));
                                        if pane.raw_entries.len() != before {
                                            pane.apply_current_filter();
                                            pane.apply_sort();
                                            if pane.entries.is_empty() {
                                                pane.cursor = 0;
                                            } else {
                                                pane.cursor =
                                                    pane.cursor.min(pane.entries.len() - 1);
                                            }
                                            pane.update_scroll(pane_height, scroll_offset);
                                            any_changed = true;
                                        }
                                    }
                                }
                                if any_changed {
                                    result_obj.ui_changed = true;
                                }
                            } else {
                                result_obj.panes_to_refresh.push(PaneRefresh {
                                    tab_id: self.tabs.active_index,
                                    pane: self.ui.active_pane,
                                });
                            }
                            self.unmark_all_panes();
                        }
                        crate::job::JobKind::PatternRename { .. }
                        | crate::job::JobKind::Mkdir { .. }
                        | crate::job::JobKind::CreateFile { .. } => {
                            result_obj.panes_to_refresh.push(PaneRefresh {
                                tab_id: self.tabs.active_index,
                                pane: self.ui.active_pane,
                            });
                        }
                        // Unlike Mkdir/CreateFile, the link lands in the
                        // *opposite* pane's directory (design: dest_dir is
                        // always `state.opposite_pane()`), so refreshing
                        // "the active pane" is wrong here — find whichever
                        // pane is actually showing that directory, same
                        // pattern as Copy/Move/JoinFiles above.
                        crate::job::JobKind::CreateLink { link_path, .. } => {
                            if let Some(parent) = link_path.parent() {
                                for (tab_idx, tab) in self.tabs.tabs.iter().enumerate() {
                                    if tab.left_pane.current_location == parent {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Left,
                                        });
                                    }
                                    if tab.right_pane.current_location == parent {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: tab_idx,
                                            pane: crate::model::ActivePane::Right,
                                        });
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::CalculateSize { location } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::SizeCalculated(size),
                            ) = result
                            {
                                for tab in self.tabs.tabs.iter_mut() {
                                    if let Some(entry) = tab
                                        .left_pane
                                        .entries
                                        .iter_mut()
                                        .find(|e| e.location == *location)
                                    {
                                        entry.calculated_size = Some(*size);
                                    }
                                    if let Some(entry) = tab
                                        .right_pane
                                        .entries
                                        .iter_mut()
                                        .find(|e| e.location == *location)
                                    {
                                        entry.calculated_size = Some(*size);
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::ExecuteCustomFunction {
                            command,
                            pipe_to_action,
                            ..
                        } => {
                            match result {
                                crate::job::OpResult::Failed(ref e) => {
                                    tracing::info!("[CompleteJob] ExecuteCustomFunction FAILED: cmd={:?} stderr={:?}", command, e.trim());
                                }
                                crate::job::OpResult::Success(
                                    crate::job::SuccessData::CustomFunctionOutput(ref out),
                                ) => {
                                    tracing::info!("[CompleteJob] ExecuteCustomFunction OK: pipe_to_action={:?} stdout={:?}", pipe_to_action, out.trim());
                                }
                                _ => {}
                            }
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::CustomFunctionOutput(ref stdout),
                            ) = result
                            {
                                // Handle PipeToAction first — the output drives navigation/execution
                                if let Some(ref action) = pipe_to_action {
                                    // Strip whitespace then surrounding quotes that cmd.exe echo may leave
                                    let output = stdout.trim().trim_matches('"');
                                    tracing::info!(
                                        "[CompleteJob] PipeToAction={:?} output={:?}",
                                        action,
                                        output
                                    );
                                    match crate::pipe_to_action::process_pipe_to_action(action, output) {
                                        Ok(crate::pipe_to_action::PipeToActionResult::JumpToPath(location)) => {
                                            // Navigate the active pane to the target location
                                            let pane = self.ui.active_pane;
                                            let tab = self.current_tab_mut();
                                            let tab_id = tab.id;
                                            let pane_model = match pane {
                                                crate::model::ActivePane::Left  => &mut tab.left_pane,
                                                crate::model::ActivePane::Right => &mut tab.right_pane,
                                            };
                                            pane_model.current_location = location.clone();
                                            pane_model.entries.clear();
                                            pane_model.is_loading = true;
                                            pane_model.cursor = 0;
                                            pane_model.scroll_offset = 0;
                                            let job_spec = crate::job::JobSpec::new(
                                                crate::job::JobKind::ReadDirectory { location }
                                            ).with_requesting_pane(tab_id, pane);
                                            result_obj.jobs_to_start.push(job_spec);
                                            result_obj.ui_changed = true;
                                        }
                                        Ok(crate::pipe_to_action::PipeToActionResult::ExecuteFile(path)) => {
                                            let job_spec = crate::job::JobSpec::new(crate::job::JobKind::ExecuteCustomFunction {
                                                command: path.to_string_lossy().to_string(),
                                                working_dir: self.active_pane().current_location.clone(),
                                                pipe_to_action: None,
                                                shell: None,
                                            });
                                            result_obj.jobs_to_start.push(job_spec);
                                        }
                                        Ok(crate::pipe_to_action::PipeToActionResult::ExecuteFileWithEditor(path)) => {
                                            let kind = Self::editor_job(&self.config, path.to_string_lossy().to_string(), false);
                                            result_obj.jobs_to_start.push(crate::job::JobSpec::new(kind));
                                        }
                                        Err(e) => {
                                            tracing::warn!("[CompleteJob] PipeToAction failed: {}", e);
                                            result_obj.task_panel_logs.push(format!("  PipeToAction error: {}", e));
                                            result_obj.ui_changed = true;
                                        }
                                    }
                                } else {
                                    // No pipe_to_action: check for editor-closed reload prompt,
                                    // otherwise refresh the active pane.
                                    let config_manager = crate::config::ConfigManager::new();
                                    let config_path =
                                        config_manager.config_path().to_string_lossy().to_string();
                                    if command.contains(&config_path) {
                                        let dialog = crate::model::Dialog::action_confirm(
                                            "Configuration Editor Closed",
                                            "Reload configuration?",
                                            None,
                                            crate::model::ConfirmableAction::ReloadConfig,
                                        );
                                        self.dialogs.push(dialog);
                                    } else {
                                        result_obj.panes_to_refresh.push(PaneRefresh {
                                            tab_id: self.tabs.active_index,
                                            pane: self.ui.active_pane,
                                        });
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::SpawnProcess {
                            args, wait: true, ..
                        } => {
                            // GUI editor launched with wait_for_exit=true (config editor): the job
                            // only completes once the editor process exits, so a Success here means
                            // "editor closed" — show the reload prompt if it was the config file.
                            if let crate::job::OpResult::Success(_) = result {
                                let config_manager = crate::config::ConfigManager::new();
                                let config_path =
                                    config_manager.config_path().to_string_lossy().to_string();
                                if args.iter().any(|a| a == &config_path) {
                                    let dialog = crate::model::Dialog::action_confirm(
                                        "Configuration Editor Closed",
                                        "Reload configuration?",
                                        None,
                                        crate::model::ConfirmableAction::ReloadConfig,
                                    );
                                    self.dialogs.push(dialog);
                                }
                            }
                        }
                        crate::job::JobKind::CollectJumpCandidates { include_files, .. } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::JumpCandidates(new_candidates),
                            ) = result
                            {
                                let include_files = *include_files;
                                let job_id_val = *job_id;
                                for dialog in self.dialogs.stack.iter_mut().rev() {
                                    let matched = match &dialog.content {
                                        crate::model::dialog::DialogContent::JumpToFile(
                                            crate::model::dialog::JumpToFileDialog {
                                                loading_job_id,
                                                ..
                                            },
                                        ) => *loading_job_id == Some(job_id_val),
                                        crate::model::dialog::DialogContent::JumpToPath(
                                            crate::model::dialog::JumpToPathDialog {
                                                loading_job_id,
                                                ..
                                            },
                                        ) => !include_files && *loading_job_id == Some(job_id_val),
                                        _ => false,
                                    };
                                    if matched {
                                        match &mut dialog.content {
                                            crate::model::dialog::DialogContent::JumpToFile(
                                                crate::model::dialog::JumpToFileDialog {
                                                    candidates,
                                                    suggestions,
                                                    loading_job_id,
                                                    query,
                                                    ..
                                                },
                                            ) => {
                                                let mut seen: std::collections::HashSet<String> =
                                                    candidates.iter().cloned().collect();
                                                for c in new_candidates {
                                                    if seen.insert(c.clone()) {
                                                        candidates.push(c.clone());
                                                    }
                                                }
                                                *loading_job_id = None;
                                                *suggestions = crate::model::dialog::filter_jump_to_file_suggestions(candidates, query);
                                                result_obj.ui_changed = true;
                                            }
                                            crate::model::dialog::DialogContent::JumpToPath(
                                                crate::model::dialog::JumpToPathDialog {
                                                    candidates,
                                                    suggestions,
                                                    loading_job_id,
                                                    query,
                                                    ..
                                                },
                                            ) => {
                                                let mut seen: std::collections::HashSet<String> =
                                                    candidates.iter().cloned().collect();
                                                for c in new_candidates {
                                                    if seen.insert(c.clone()) {
                                                        candidates.push(c.clone());
                                                    }
                                                }
                                                *loading_job_id = None;
                                                *suggestions = crate::model::dialog::filter_jump_to_path_suggestions(candidates, query);
                                                result_obj.ui_changed = true;
                                            }
                                            _ => {}
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::DetectFileType { path, purpose } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::FileTypeDetected { kind, header_bytes },
                            ) = result
                            {
                                match purpose {
                                    crate::job::DetectFileTypePurpose::CheckAssociationMismatch {
                                        command,
                                        working_dir,
                                        shell,
                                    } => {
                                        // Unlike CollectJumpCandidates above, this arm doesn't match
                                        // completion against a `loading_job_id` still open on some
                                        // dialog — there's nothing to correlate against. The purpose
                                        // captured command/working_dir/shell at job-creation time, so
                                        // this job carries everything needed to complete the action on
                                        // its own; even a "stale" confirm just re-runs the originally
                                        // captured command, which is harmless. No staleness guard needed.
                                        let ext = path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("");
                                        if self.config.magic_byte_detection_enabled
                                            && crate::magic::is_mismatch(ext, *kind)
                                        {
                                            let filename = path
                                                .file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| path.display().to_string());
                                            let ext_note = if ext.is_empty() {
                                                "no extension".to_string()
                                            } else {
                                                format!(".{}", ext)
                                            };
                                            result_obj.task_panel_logs.push(format!(
                                                "[Warning] Type mismatch: {} looks like {} (extension {})",
                                                filename,
                                                kind.label(),
                                                ext_note
                                            ));
                                            let dialog = crate::model::Dialog::type_mismatch_warning(
                                                path.clone(),
                                                *kind,
                                                command.clone(),
                                                working_dir.clone(),
                                                shell.clone(),
                                            );
                                            self.dialogs.push(dialog);
                                            result_obj.ui_changed = true;
                                        } else {
                                            result_obj.jobs_to_start.push(
                                                crate::job::JobSpec::execute_association(
                                                    command.clone(),
                                                    working_dir.clone(),
                                                    shell.clone(),
                                                ),
                                            );
                                        }
                                    }
                                    crate::job::DetectFileTypePurpose::FallbackOpen { location } => {
                                        // Neither an ExtensionAssociation nor a FileTypeMapping
                                        // matched this file's extension (Phase 7.3 §6). Route by
                                        // the detected content: a known non-text kind opens via
                                        // the OS default association; Unknown falls through to
                                        // the internal text viewer exactly as before this task.
                                        open_by_detected_kind(self, *kind, location, &mut result_obj);
                                    }
                                    crate::job::DetectFileTypePurpose::ResolveAssociation {
                                        location,
                                    } => {
                                        // Detect-then-resolve (Phase 7.3b): we now have the
                                        // detected kind, so resolve candidates FileType-first with
                                        // extension fallback via the shared `candidates_for` (same
                                        // logic the DetectFileTypesBatch arm below uses).
                                        let ext_lower = path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .map(|e| e.to_lowercase())
                                            .unwrap_or_default();
                                        let candidates =
                                            crate::input::candidates_for(self, *kind, &ext_lower);
                                        match candidates.len() {
                                            0 => {
                                                // No association matched even with content-type
                                                // awareness — same fallback FallbackOpen would have
                                                // used, reusing the kind we already detected instead
                                                // of starting a second detect job.
                                                open_by_detected_kind(
                                                    self, *kind, location, &mut result_obj,
                                                );
                                            }
                                            1 => {
                                                let assoc = &candidates[0];
                                                match crate::input::expand_association_command(
                                                    self, assoc,
                                                ) {
                                                    Ok((command, working_dir, shell)) => {
                                                        // Mirrors CheckAssociationMismatch's mismatch
                                                        // gate above — we already have `kind` in hand,
                                                        // so no second detect job is started.
                                                        if self.config.magic_byte_detection_enabled
                                                            && crate::magic::is_mismatch(
                                                                &ext_lower, *kind,
                                                            )
                                                        {
                                                            let filename = path
                                                                .file_name()
                                                                .map(|n| {
                                                                    n.to_string_lossy().to_string()
                                                                })
                                                                .unwrap_or_else(|| {
                                                                    path.display().to_string()
                                                                });
                                                            let ext_note = if ext_lower.is_empty() {
                                                                "no extension".to_string()
                                                            } else {
                                                                format!(".{}", ext_lower)
                                                            };
                                                            result_obj.task_panel_logs.push(format!(
                                                                "[Warning] Type mismatch: {} looks like {} (extension {})",
                                                                filename,
                                                                kind.label(),
                                                                ext_note
                                                            ));
                                                            let dialog =
                                                                crate::model::Dialog::type_mismatch_warning(
                                                                    path.clone(),
                                                                    *kind,
                                                                    command,
                                                                    working_dir,
                                                                    shell,
                                                                );
                                                            self.dialogs.push(dialog);
                                                            result_obj.ui_changed = true;
                                                        } else {
                                                            result_obj.jobs_to_start.push(
                                                                crate::job::JobSpec::execute_association(
                                                                    command, working_dir, shell,
                                                                ),
                                                            );
                                                        }
                                                    }
                                                    Err(_) => {
                                                        tracing::debug!("ResolveAssociation: association command expansion failed, falling back to viewer");
                                                        let sub_res = update_state(
                                                            self,
                                                            Transition::OpenTextViewer {
                                                                location: location.clone(),
                                                            },
                                                        );
                                                        result_obj.absorb(sub_res);
                                                    }
                                                }
                                            }
                                            _ => {
                                                let dialog = crate::model::Dialog::open_with_picker(
                                                    vec![path.clone()],
                                                    candidates,
                                                    Some(*kind),
                                                );
                                                self.dialogs.push(dialog);
                                                result_obj.ui_changed = true;
                                            }
                                        }
                                    }
                                    crate::job::DetectFileTypePurpose::FileInfoDisplay => {
                                        // On-demand detection requested from the still-open File
                                        // Information dialog (Phase 7.3 §7). Mirrors the
                                        // CollectJumpCandidates pattern above: scan the dialog
                                        // stack (not just the top) for the FileInfo dialog whose
                                        // detected_type_job_id matches this completed job, since
                                        // other dialogs may have been pushed on top meanwhile.
                                        let job_id_val = *job_id;
                                        for dialog in self.dialogs.stack.iter_mut().rev() {
                                            if let crate::model::dialog::DialogContent::FileInfo(
                                                d,
                                            ) = &mut dialog.content
                                            {
                                                if d.detected_type_job_id == Some(job_id_val) {
                                                    let ext = path
                                                        .extension()
                                                        .and_then(|e| e.to_str())
                                                        .unwrap_or("");
                                                    let label = kind.label().to_string();
                                                    let label = if crate::magic::is_mismatch(
                                                        ext, *kind,
                                                    ) {
                                                        if ext.is_empty() {
                                                            format!(
                                                                "{} (mismatch — file has no extension)",
                                                                label
                                                            )
                                                        } else {
                                                            format!(
                                                                "{} (mismatch — extension implies .{})",
                                                                label, ext
                                                            )
                                                        }
                                                    } else {
                                                        label
                                                    };
                                                    result_obj.task_panel_logs.push(format!(
                                                        "[System] Detected type: {} for {}",
                                                        label, d.file_name
                                                    ));
                                                    d.detected_type = Some(label);
                                                    d.detecting = false;
                                                    d.detected_type_job_id = None;
                                                    // Retain only the first 64 bytes for
                                                    // display — the dialog shows at most 4
                                                    // hex rows (16 bytes each), and truncating
                                                    // here (rather than at render time) keeps
                                                    // the dialog struct itself small; nothing
                                                    // downstream needs the full ≤300-byte
                                                    // sniff sample.
                                                    let truncated_header: Vec<u8> =
                                                        header_bytes.iter().take(64).copied().collect();
                                                    // Auto-detect on the same (truncated) bytes
                                                    // that get stored/displayed, so the initial
                                                    // encoding shown always matches what's on
                                                    // screen. This is just the starting point —
                                                    // the user can cycle away from it with `e`
                                                    // (Phase 7.3b, Task 12).
                                                    d.header_encoding = Some(
                                                        crate::model::viewer::TextEncoding::detect(
                                                            &truncated_header,
                                                        ),
                                                    );
                                                    d.header_bytes = Some(truncated_header);
                                                    result_obj.ui_changed = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    crate::job::DetectFileTypePurpose::ContextMenuLabel => {
                                        // Live detection for the "Open With..." row label
                                        // (Phase 7.3b, Task 9). Same stack-scan pattern as
                                        // FileInfoDisplay above: find the ContextMenu dialog
                                        // whose detected_type_job_id matches this completed job
                                        // (other dialogs may have been pushed on top meanwhile).
                                        let job_id_val = *job_id;
                                        for dialog in self.dialogs.stack.iter_mut().rev() {
                                            if let crate::model::dialog::DialogContent::ContextMenu(
                                                d,
                                            ) = &mut dialog.content
                                            {
                                                if d.detected_type_job_id == Some(job_id_val) {
                                                    d.detected_type_label =
                                                        Some(kind.label().to_string());
                                                    d.detected_type_job_id = None;
                                                    result_obj.ui_changed = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if let crate::job::DetectFileTypePurpose::CheckAssociationMismatch {
                                command,
                                working_dir,
                                shell,
                            } = purpose
                            {
                                // Fail-open: detection failed (e.g. a non-Local `display_path()`
                                // like "archive.zip#inner/notes.txt" isn't a real filesystem path
                                // that `std::fs::File::open` can read) or was cancelled. Don't
                                // block the user's explicit association — run the command anyway.
                                // This restores exact pre-7.3 behavior: ExecuteAssociationChecked
                                // used to go straight to ExecuteAssociation -> ExecuteCustomFunction,
                                // which silently no-ops for a non-Local working_dir on its own.
                                result_obj.jobs_to_start.push(
                                    crate::job::JobSpec::execute_association(
                                        command.clone(),
                                        working_dir.clone(),
                                        shell.clone(),
                                    ),
                                );
                            } else if let crate::job::DetectFileTypePurpose::FallbackOpen {
                                location,
                            } = purpose
                            {
                                // Safety net: the detect job failed or was cancelled (e.g. the
                                // file became unreadable between listing and detection, or a
                                // non-Local location slipped through despite the CheckFallbackFileType
                                // guard). Don't silently drop the open — fall back to the text
                                // viewer, matching pre-Task-5 behavior for this fallback path.
                                let sub_res = update_state(
                                    self,
                                    Transition::OpenTextViewer {
                                        location: location.clone(),
                                    },
                                );
                                result_obj.absorb(sub_res);
                            } else if let crate::job::DetectFileTypePurpose::ResolveAssociation {
                                location,
                            } = purpose
                            {
                                // Fail-open (Phase 7.3b): detection failed or was cancelled, so
                                // there's no detected kind to resolve FileType-matching candidates
                                // with. Fall back to extension-only resolution (pure-extension
                                // entries — same rules as the flag-off path) rather than dropping
                                // the open entirely.
                                let ext_lower = path
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .map(|e| e.to_lowercase())
                                    .unwrap_or_default();
                                let candidates = if ext_lower.is_empty() {
                                    Vec::new()
                                } else {
                                    crate::input::candidates_for_extension(self, &ext_lower)
                                };
                                match candidates.len() {
                                    0 => {
                                        // No kind available to try OpenWithSystem — pre-7.3
                                        // behavior for "no candidates" was the text viewer, so
                                        // that's the safety net here too.
                                        let sub_res = update_state(
                                            self,
                                            Transition::OpenTextViewer {
                                                location: location.clone(),
                                            },
                                        );
                                        result_obj.absorb(sub_res);
                                    }
                                    1 => {
                                        let assoc = &candidates[0];
                                        if let Ok((command, working_dir, shell)) =
                                            crate::input::expand_association_command(self, assoc)
                                        {
                                            result_obj.jobs_to_start.push(
                                                crate::job::JobSpec::execute_association(
                                                    command, working_dir, shell,
                                                ),
                                            );
                                        } else {
                                            let sub_res = update_state(
                                                self,
                                                Transition::OpenTextViewer {
                                                    location: location.clone(),
                                                },
                                            );
                                            result_obj.absorb(sub_res);
                                        }
                                    }
                                    _ => {
                                        let dialog = crate::model::Dialog::open_with_picker(
                                            vec![path.clone()],
                                            candidates,
                                            None,
                                        );
                                        self.dialogs.push(dialog);
                                        result_obj.ui_changed = true;
                                    }
                                }
                            } else if matches!(
                                purpose,
                                crate::job::DetectFileTypePurpose::FileInfoDisplay
                            ) {
                                // Detection failed or was cancelled — clear `detecting` so the
                                // dialog doesn't show "Detecting..." forever. Scan the stack the
                                // same way as the success path above.
                                let job_id_val = *job_id;
                                for dialog in self.dialogs.stack.iter_mut().rev() {
                                    if let crate::model::dialog::DialogContent::FileInfo(d) =
                                        &mut dialog.content
                                    {
                                        if d.detected_type_job_id == Some(job_id_val) {
                                            d.detecting = false;
                                            d.detected_type_job_id = None;
                                            d.detected_type = Some("detection failed".to_string());
                                            result_obj.ui_changed = true;
                                            break;
                                        }
                                    }
                                }
                            } else if matches!(
                                purpose,
                                crate::job::DetectFileTypePurpose::ContextMenuLabel
                            ) {
                                // Detection failed or was cancelled — clear the in-flight job id
                                // so the row doesn't show "(detecting...)" forever. Same
                                // stack-scan pattern as FileInfoDisplay's failure arm above.
                                let job_id_val = *job_id;
                                for dialog in self.dialogs.stack.iter_mut().rev() {
                                    if let crate::model::dialog::DialogContent::ContextMenu(d) =
                                        &mut dialog.content
                                    {
                                        if d.detected_type_job_id == Some(job_id_val) {
                                            d.detected_type_job_id = None;
                                            d.detected_type_label =
                                                Some("detection failed".to_string());
                                            result_obj.ui_changed = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        // Batch "Open With..." (Phase 7.3 §3, multi-select): 2+ marked files
                        // were detected together for grouping purposes only — each group's
                        // execution still goes through the same per-file ExecuteAssociationChecked
                        // gate as the single-file flow (see `checked_association_job`), so
                        // per-file magic-byte mismatch warnings still fire. No multi-path
                        // command execution (MacroExpander has no such support); this is a
                        // deliberate deviation from the plan's literal "pass all paths as
                        // arguments" wording.
                        crate::job::JobKind::DetectFileTypesBatch { .. } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::FileTypesDetected(pairs),
                            ) = result
                            {
                                let groups = group_by_kind_and_ext(pairs.clone());
                                for ((kind, ext), paths) in groups {
                                    // Phase 7.3b: FileType-first resolution using the kind this
                                    // group was detected as, extension fallback via the shared
                                    // `candidates_for` (same logic the single-file
                                    // ResolveAssociation completion arm above uses).
                                    let candidates = crate::input::candidates_for(self, kind, &ext);
                                    match candidates.len() {
                                        0 => {
                                            result_obj.task_panel_logs.push(format!(
                                                "[Skipped] No association for group .{} ({} files)",
                                                ext,
                                                paths.len()
                                            ));
                                        }
                                        1 => {
                                            let assoc = &candidates[0];
                                            if let Ok((command, working_dir, shell)) =
                                                crate::input::expand_association_command(
                                                    self, assoc,
                                                )
                                            {
                                                for path in paths {
                                                    result_obj.jobs_to_start.push(
                                                        self.checked_association_job(
                                                            path,
                                                            command.clone(),
                                                            working_dir.clone(),
                                                            shell.clone(),
                                                        ),
                                                    );
                                                }
                                            }
                                            // Expansion failure: Open With is association-only
                                            // (no viewer fallback, matching Action::OpenWith's
                                            // single-file behavior) — skip the group silently.
                                        }
                                        _ => {
                                            let dialog = crate::model::Dialog::open_with_picker(
                                                paths,
                                                candidates,
                                                Some(kind),
                                            );
                                            self.dialogs.push(dialog);
                                            result_obj.ui_changed = true;
                                        }
                                    }
                                }
                            }
                        }
                        crate::job::JobKind::ScanTrash { fallback_roots } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::TrashScanned { count, total_size },
                            ) = result
                            {
                                if *count == 0 {
                                    result_obj
                                        .task_panel_logs
                                        .push("[Info] Trash is already empty".to_string());
                                } else {
                                    let dialog = crate::model::Dialog::action_confirm(
                                        "Empty Trash",
                                        format!(
                                            "Permanently empty {} item{} ({}) from the trash? This cannot be undone.",
                                            count,
                                            if *count == 1 { "" } else { "s" },
                                            crate::model::format_size(*total_size)
                                        ),
                                        Some(crate::model::ConfirmStats {
                                            count: *count,
                                            total_size: *total_size,
                                        }),
                                        crate::model::ConfirmableAction::EmptyTrash {
                                            fallback_roots: fallback_roots.clone(),
                                        },
                                    );
                                    self.dialogs.push(dialog);
                                }
                                result_obj.ui_changed = true;
                            }
                        }
                        crate::job::JobKind::ListTrash { .. } => {
                            if let crate::job::OpResult::Success(
                                crate::job::SuccessData::TrashListed(records),
                            ) = result
                            {
                                if records.is_empty() {
                                    result_obj
                                        .task_panel_logs
                                        .push("[Info] Trash is already empty".to_string());
                                } else {
                                    self.dialogs
                                        .push(crate::model::Dialog::trash_browser(records.clone()));
                                }
                                result_obj.ui_changed = true;
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(log) = log_entry {
                    result_obj.task_panel_logs.push(log);
                }

                match result {
                    crate::job::OpResult::Success(_) => {
                        self.background_jobs.mark_job_completed(*job_id)
                    }
                    crate::job::OpResult::Failed(e) => {
                        self.background_jobs.mark_job_failed(*job_id, e.clone())
                    }
                    crate::job::OpResult::Cancelled => {
                        self.background_jobs.mark_job_cancelled(*job_id)
                    }
                }

                Some(result_obj)
            }
            Transition::CancelJob { job_id } => {
                if self.jobs.request_cancel(*job_id) {
                    Some(StateUpdateResult::with_cancel(*job_id))
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::AcknowledgeCancel { job_id } => {
                self.jobs.acknowledge_cancel(*job_id);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::NavigateToHistoryIndex { pane, index } => {
                let location = {
                    let tab = self.current_tab_mut();
                    tab.history.jump_to_index(*pane, *index)
                };
                if let Some(location) = location {
                    let cached_entries = self.cache.get(&location);
                    let tab_id = self.current_tab().id;
                    let tab = self.current_tab_mut();
                    let pane_model = match pane {
                        crate::model::ActivePane::Left => &mut tab.left_pane,
                        crate::model::ActivePane::Right => &mut tab.right_pane,
                    };
                    pane_model.current_location = location.clone();
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;
                    if let Some(entries) = cached_entries {
                        pane_model.raw_entries = entries.clone();
                        pane_model.entries = entries;
                        pane_model.is_loading = false;
                        pane_model.apply_sort();
                        pane_model.apply_current_filter();
                        Some(StateUpdateResult::with_ui_change())
                    } else {
                        pane_model.entries.clear();
                        pane_model.is_loading = true;
                        let job_spec =
                            JobSpec::new(crate::job::JobKind::ReadDirectory { location })
                                .with_requesting_pane(tab_id, *pane);
                        pane_model.active_job_id = Some(job_spec.id);
                        Some(StateUpdateResult::with_job(job_spec))
                    }
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            _ => None,
        }
    }
}

/// Route an open by the file's already-detected content type (Phase 7.3 Task 5 /
/// 7.3b): a known non-text kind opens via the OS default association; `Unknown`
/// falls through to the internal text viewer. Shared by `FallbackOpen`'s success
/// arm and `ResolveAssociation`'s "0 candidates after detection" case (Phase
/// 7.3b) so both routes to "no explicit association, decide by content type"
/// can't drift apart — and so `ResolveAssociation` doesn't need to start a
/// second detect job just to reuse this decision.
fn open_by_detected_kind(
    state: &mut AppState,
    kind: crate::magic::DetectedKind,
    location: &crate::model::Location,
    result_obj: &mut StateUpdateResult,
) {
    let follow_up = if kind == crate::magic::DetectedKind::Unknown {
        Transition::OpenTextViewer {
            location: location.clone(),
        }
    } else {
        result_obj.task_panel_logs.push(format!(
            "[System] Detected {}; opening {} via OS default",
            kind.label(),
            location.display_path()
        ));
        Transition::OpenWithSystem {
            path: location.display_path(),
        }
    };
    let sub_res = update_state(state, follow_up);
    result_obj.absorb(sub_res);
}

/// Group `(path, detected_kind)` pairs by the `(DetectedKind, extension)` pair
/// (Phase 7.3 §3, batch "Open With..."). Two-level grouping because
/// `ExtensionAssociation` candidates are keyed on extension, not content type —
/// primary key is the detected kind, secondary key is the extension, so each
/// resulting group has exactly one extension and therefore one candidate set.
///
/// Order is preserved: groups appear in first-occurrence order, and within a
/// group paths appear in input order. `DetectFileTypesBatch`'s executor
/// preserves the input path order (see `execute_detect_file_types_batch`), so
/// this makes grouping deterministic without requiring `Ord` on `DetectedKind`.
pub(crate) fn group_by_kind_and_ext(
    pairs: Vec<(std::path::PathBuf, crate::magic::DetectedKind)>,
) -> Vec<(
    (crate::magic::DetectedKind, String),
    Vec<std::path::PathBuf>,
)> {
    let mut groups: Vec<(
        (crate::magic::DetectedKind, String),
        Vec<std::path::PathBuf>,
    )> = Vec::new();
    for (path, kind) in pairs {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let key = (kind, ext);
        if let Some(existing) = groups.iter_mut().find(|(k, _)| *k == key) {
            existing.1.push(path);
        } else {
            groups.push((key, vec![path]));
        }
    }
    groups
}

#[cfg(test)]
mod group_by_kind_and_ext_tests {
    use super::group_by_kind_and_ext;
    use crate::magic::DetectedKind;
    use std::path::PathBuf;

    #[test]
    fn groups_by_kind_and_extension_pair() {
        let pairs = vec![
            (PathBuf::from("/a.png"), DetectedKind::Png),
            (PathBuf::from("/b.png"), DetectedKind::Png),
            (PathBuf::from("/c.pdf"), DetectedKind::Pdf),
        ];
        let groups = group_by_kind_and_ext(pairs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, (DetectedKind::Png, "png".to_string()));
        assert_eq!(
            groups[0].1,
            vec![PathBuf::from("/a.png"), PathBuf::from("/b.png")]
        );
        assert_eq!(groups[1].0, (DetectedKind::Pdf, "pdf".to_string()));
        assert_eq!(groups[1].1, vec![PathBuf::from("/c.pdf")]);
    }

    #[test]
    fn same_extension_different_kind_forms_separate_groups() {
        // A .dat file that's actually a PNG vs. a .dat file that's actually a PDF:
        // same extension, different detected content type -> must not merge, since
        // they'd resolve to the same ExtensionAssociation candidates by extension
        // alone but the grouping key is (kind, ext), not just ext.
        let pairs = vec![
            (PathBuf::from("/a.dat"), DetectedKind::Png),
            (PathBuf::from("/b.dat"), DetectedKind::Pdf),
        ];
        let groups = group_by_kind_and_ext(pairs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, (DetectedKind::Png, "dat".to_string()));
        assert_eq!(groups[1].0, (DetectedKind::Pdf, "dat".to_string()));
    }

    #[test]
    fn extension_is_lowercased_for_grouping() {
        let pairs = vec![
            (PathBuf::from("/a.PNG"), DetectedKind::Png),
            (PathBuf::from("/b.png"), DetectedKind::Png),
        ];
        let groups = group_by_kind_and_ext(pairs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn no_extension_groups_under_empty_string() {
        let pairs = vec![
            (PathBuf::from("/Makefile"), DetectedKind::Unknown),
            (PathBuf::from("/README"), DetectedKind::Unknown),
        ];
        let groups = group_by_kind_and_ext(pairs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, (DetectedKind::Unknown, String::new()));
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn empty_input_produces_no_groups() {
        assert!(group_by_kind_and_ext(Vec::new()).is_empty());
    }

    #[test]
    fn preserves_first_occurrence_order_across_interleaved_input() {
        let pairs = vec![
            (PathBuf::from("/a.png"), DetectedKind::Png),
            (PathBuf::from("/b.pdf"), DetectedKind::Pdf),
            (PathBuf::from("/c.png"), DetectedKind::Png),
        ];
        let groups = group_by_kind_and_ext(pairs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, (DetectedKind::Png, "png".to_string()));
        assert_eq!(
            groups[0].1,
            vec![PathBuf::from("/a.png"), PathBuf::from("/c.png")]
        );
        assert_eq!(groups[1].0, (DetectedKind::Pdf, "pdf".to_string()));
    }
}

#[cfg(test)]
mod trash_refresh_tests {
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::{Location, TrashOutcome};
    use crate::state::{update_state, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use std::path::PathBuf;

    fn loc(name: &str) -> Location {
        Location::Local(PathBuf::from(format!("/work/{name}")))
    }

    /// Regression: a successful `MoveToTrash` left the trashed entries listed and
    /// still marked, because the completion handler had no arm for that JobKind and
    /// fell through to the catch-all. Found by a diagnostic bundle on 2026-08-11 —
    /// `CompleteJob { kind: MoveToTrash, result: Success(TrashMoved(..)) }` was the
    /// last transition of the session, with no refresh after it.
    #[test]
    fn move_to_trash_completion_removes_entries_from_panes() {
        let mut state = test_state();
        let doomed = loc("doomed.txt");
        let kept = loc("kept.txt");

        {
            let pane = &mut state.current_tab_mut().left_pane;
            pane.current_location = Location::Local(PathBuf::from("/work"));
            pane.raw_entries = vec![
                FileEntryBuilder::new("doomed.txt")
                    .location(doomed.clone())
                    .build(),
                FileEntryBuilder::new("kept.txt")
                    .location(kept.clone())
                    .build(),
            ];
            pane.entries = pane.raw_entries.clone();
            pane.marking.mark(doomed.clone());
        }

        let spec = JobSpec::new(JobKind::MoveToTrash {
            targets: vec![doomed.clone()],
            force_fallback: false,
        });
        let job_id = spec.id;
        state.jobs.start_job(spec);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::TrashMoved(vec![TrashOutcome {
                    target: doomed.clone(),
                    record: None,
                    result: Ok(()),
                }])),
            },
        );

        let pane = &state.current_tab().left_pane;
        let names: Vec<&str> = pane.entries.iter().map(|e| e.name.as_str()).collect();

        assert_eq!(
            names,
            vec!["kept.txt"],
            "trashed entry must leave the pane listing"
        );
        assert_eq!(
            pane.marking.count(),
            0,
            "trashed entry must not stay marked"
        );
        assert!(result.ui_changed, "removing an entry must signal a redraw");
    }

    /// The same completion must invalidate the caches, or navigating away and back
    /// re-serves the stale listing.
    #[test]
    fn move_to_trash_completion_invalidates_caches() {
        let mut state = test_state();
        let dir = Location::Local(PathBuf::from("/work"));
        let doomed = loc("doomed.txt");

        state.cache.insert(
            dir.clone(),
            vec![FileEntryBuilder::new("doomed.txt")
                .location(doomed.clone())
                .build()],
        );
        assert!(state.cache.get(&dir).is_some(), "precondition: cached");

        let spec = JobSpec::new(JobKind::MoveToTrash {
            targets: vec![doomed.clone()],
            force_fallback: false,
        });
        let job_id = spec.id;
        state.jobs.start_job(spec);

        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::TrashMoved(vec![TrashOutcome {
                    target: doomed,
                    record: None,
                    result: Ok(()),
                }])),
            },
        );

        assert!(
            state.cache.get(&dir).is_none(),
            "parent directory cache must be invalidated after a trash move"
        );
    }
}

#[cfg(test)]
mod restore_refresh_tests {
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::{Location, RestoreOutcome, TrashRecord};
    use crate::state::{update_state, Transition};
    use crate::test_utils::test_state;
    use std::path::PathBuf;

    fn dir(p: &str) -> Location {
        Location::Local(PathBuf::from(p))
    }

    fn restore_job() -> JobSpec {
        // The records themselves are not read by the completion handler — the
        // restored locations come from the outcomes — so an empty list is fine.
        JobSpec::new(JobKind::RestoreFromTrash {
            records: Vec::<TrashRecord>::new(),
        })
    }

    /// Regression: restoring from trash put files back on disk but left every pane
    /// showing the old listing, because the completion handler had no arm for
    /// `RestoreFromTrash`. Same class as the `MoveToTrash` bug, opposite direction.
    #[test]
    fn restore_completion_refreshes_panes_showing_the_destination() {
        let mut state = test_state();
        {
            let tab = state.current_tab_mut();
            tab.left_pane.current_location = dir("/work");
            tab.right_pane.current_location = dir("/elsewhere");
        }

        let spec = restore_job();
        let job_id = spec.id;
        state.jobs.start_job(spec);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::TrashRestored(vec![RestoreOutcome {
                    original: dir("/work/back.txt"),
                    result: Ok(()),
                }])),
            },
        );

        assert_eq!(
            result.panes_to_refresh.len(),
            1,
            "only the pane showing the restore destination should refresh, got {:?}",
            result.panes_to_refresh
        );
        assert_eq!(
            result.panes_to_refresh[0].pane,
            crate::model::ActivePane::Left
        );
    }

    /// A restore that failed must not trigger a re-read: nothing changed on disk,
    /// and a spurious ReadDirectory would mask the failure with a normal-looking
    /// refresh.
    #[test]
    fn failed_restore_does_not_refresh() {
        let mut state = test_state();
        state.current_tab_mut().left_pane.current_location = dir("/work");

        let spec = restore_job();
        let job_id = spec.id;
        state.jobs.start_job(spec);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::TrashRestored(vec![RestoreOutcome {
                    original: dir("/work/back.txt"),
                    result: Err("permission denied".to_string()),
                }])),
            },
        );

        assert!(
            result.panes_to_refresh.is_empty(),
            "a failed restore must not refresh"
        );
    }
}
