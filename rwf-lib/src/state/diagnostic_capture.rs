//! Builds a `DiagnosticStateSnapshot` from `AppState` (moved up from `diagnostics`
//! so the diagnostics core need not depend on `state`/`dialog`/`job`).

use crate::diagnostics::state_snapshot::DrivePollingSnapshot;
use crate::diagnostics::*;
use crate::model::PaneModel;
use crate::AppState;

impl DiagnosticStateSnapshot {
    /// Project `state` into a bounded, serializable snapshot.
    ///
    /// Takes a read-only borrow and mutates nothing — this runs from the main
    /// loop's render path, where state purity is mandatory.
    pub fn capture(state: &AppState, seq: u64, trigger: &str) -> Self {
        Self {
            seq,
            ts: crate::diagnostics::now_timestamp(),
            trigger: trigger.to_string(),
            ui: UiSnapshot {
                mode: format!("{:?}", state.ui.mode),
                active_pane: format!("{:?}", state.ui.active_pane),
                show_hidden: state.ui.show_hidden,
                range_marking_start: state.ui.range_marking_start,
                layout: LayoutSnapshot {
                    pane_width: state.ui.layout.pane_width,
                    pane_height: state.ui.layout.pane_height,
                    show_tab_bar: state.ui.layout.show_tab_bar,
                    show_task_panel: state.ui.layout.show_task_panel,
                    task_panel_height: state.ui.layout.task_panel_height,
                    viewer_layout: format!("{:?}", state.ui.layout.viewer_layout),
                    viewer_anchor_pane: format!("{:?}", state.ui.layout.viewer_anchor_pane),
                },
            },
            tabs: TabsSnapshot {
                active_index: state.tabs.active_index,
                items: state
                    .tabs
                    .tabs
                    .iter()
                    .map(|tab| TabSnapshot {
                        id: tab.id,
                        left: PaneSnapshot::capture(&tab.left_pane),
                        right: PaneSnapshot::capture(&tab.right_pane),
                        left_pane_width: tab.left_pane_width,
                    })
                    .collect(),
            },
            jobs: JobsSnapshot {
                active: state
                    .jobs
                    .active
                    .values()
                    .map(|job| ActiveJobSnapshot {
                        job_id: format!("{:?}", job.spec.id),
                        kind: crate::diagnostics::variant_name(&format!("{:?}", job.spec.kind))
                            .to_string(),
                        state: format!("{:?}", job.state),
                        progress: job.progress,
                    })
                    .collect(),
                background: state
                    .background_jobs
                    .get_active_jobs()
                    .map(|job| BackgroundJobSnapshot {
                        name: job.name.clone(),
                        status: format!("{:?}", job.status),
                        progress_percent: job.progress_percent,
                        tab_id: job.tab_id,
                    })
                    .collect(),
            },
            search: SearchSnapshot {
                query: state.search.query.clone(),
                result_count: state.search.results.len(),
                current_index: state.search.current_index,
                case_sensitive: state.search.case_sensitive,
                use_regex: state.search.use_regex,
                use_migemo: state.search.use_migemo,
            },
            leap: state.leap.as_ref().map(|leap| LeapSnapshot {
                buffer: leap.buffer.clone(),
                root_dir: leap.root_dir.display().to_string(),
                root_cursor: leap.root_cursor,
                depth: leap.dir_stack.len(),
            }),
            viewer: state.viewer.as_ref().map(|viewer| ViewerSnapshot {
                path: viewer.location.display_path(),
                mode: format!("{:?}", viewer.mode),
                encoding: format!("{:?}", viewer.encoding),
                line_offset: viewer.line_offset,
                column_offset: viewer.column_offset,
                search_query: viewer.search_query.clone(),
                match_count: viewer.search_matches.len(),
                is_loading: viewer.is_loading,
                is_searching: viewer.is_searching,
            }),
            polling: {
                use crate::model::polling::StopReason;
                let mut drives: Vec<DrivePollingSnapshot> = state
                    .polling
                    .drives
                    .iter()
                    .map(|(drive, polled)| DrivePollingSnapshot {
                        drive: drive.clone(),
                        interval_ms: polled.interval.as_millis() as u64,
                        state: match (polled.stopped, polled.failing) {
                            (Some(StopReason::Manual), _) => "stopped-manual",
                            (Some(StopReason::Auto), _) => "stopped-auto",
                            (None, true) => "failing",
                            (None, false) => "active",
                        }
                        .to_string(),
                        last_poll_ms_ago: polled
                            .last_poll
                            .map(|at| at.elapsed().as_millis() as u64),
                        polls: polled.timing.map_or(0, |t| t.count),
                        last_poll_duration_ms: polled.timing.map(|t| t.last.as_millis() as u64),
                        average_poll_duration_ms: polled
                            .timing
                            .map(|t| t.average.as_millis() as u64),
                        slowest_poll_duration_ms: polled
                            .timing
                            .map(|t| t.slowest.as_millis() as u64),
                    })
                    .collect();
                drives.sort_by(|a, b| a.drive.cmp(&b.drive));
                drives
            },
            dialogs: state
                .dialogs
                .stack
                .iter()
                .map(|dialog| dialog.title.clone())
                .collect(),
        }
    }
}

impl PaneSnapshot {
    fn capture(pane: &PaneModel) -> Self {
        Self {
            path: pane.current_location.display_path(),
            cursor: pane.cursor,
            scroll_offset: pane.scroll_offset,
            cursor_entry: pane.entries.get(pane.cursor).map(|e| e.name.clone()),
            entry_count: pane.entries.len(),
            raw_entry_count: pane.raw_entries.len(),
            marked_count: pane.marking.count(),
            sort_mode: format!("{:?}", pane.sort_mode),
            sort_order: format!("{:?}", pane.sort_order),
            display_mode: format!("{:?}", pane.display_mode),
            file_mask: pane.file_mask.clone(),
            is_loading: pane.is_loading,
            active_job_id: pane.active_job_id.map(|id| format!("{id:?}")),
            pending_cursor_name: pane.pending_cursor_name.clone(),
        }
    }
}
