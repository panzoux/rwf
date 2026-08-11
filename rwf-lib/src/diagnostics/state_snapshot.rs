//! Structured application-state capture (Phase 7.15 §5.4).
//!
//! `events.jsonl` is a stream of *deltas*. Without an absolute initial
//! condition a `Transition` stream cannot be resolved to actual state — you can
//! see that the cursor moved down, but not from where, in which directory, or
//! with what filter applied. The paired `.txt` screen dump shows what was
//! *rendered*, which is not the same thing: a stale render is one of the very
//! bugs being hunted. This is the third leg.
//!
//! # Why a hand-written projection
//!
//! `AppState` is deliberately **not** serialized wholesale:
//!
//! - [`PaneModel`](crate::model::PaneModel) holds both `raw_entries` and
//!   `entries` — two complete directory listings *per pane*. Ten tabs over a
//!   5,000-entry directory would be ~200,000 `FileEntry` serializations per
//!   snapshot.
//! - `SearchModel` holds a migemo `CompactDictionary` and a
//!   `RefCell<HashMap<String, Regex>>` cache: large, and `Regex` has no
//!   meaningful serialized form.
//! - `AppState` is not `Serialize` today, and making it so would couple
//!   diagnostics to state internals — violating the observer contract in
//!   [`super`] — and churn on every field added elsewhere.
//!
//! So this is a documented, stable contract that changes only when someone
//! deliberately changes it. Entry lists are omitted: what was visible is
//! already in the paired `.txt`, and the rest is recoverable from the path.
//! The two files are complementary, not redundant.

use serde::{Deserialize, Serialize};

use crate::model::PaneModel;
use crate::AppState;

/// A semantic snapshot of the application at one instant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticStateSnapshot {
    /// Matches the `seq` of the corresponding `Snapshot` event and the paired
    /// `.txt` filename stem.
    pub seq: u64,
    /// RFC 3339 local timestamp.
    pub ts: String,
    /// `start`, `manual` or `final`.
    pub trigger: String,
    /// UI mode, active pane, layout.
    pub ui: UiSnapshot,
    /// Every open tab, with both panes.
    pub tabs: TabsSnapshot,
    /// Jobs in flight.
    pub jobs: JobsSnapshot,
    /// Incremental-search state.
    pub search: SearchSnapshot,
    /// Leap navigation state, when active.
    pub leap: Option<LeapSnapshot>,
    /// Viewer position and metadata, when open. Never file contents.
    pub viewer: Option<ViewerSnapshot>,
    /// Dialog stack, outermost first — **variant titles only**.
    ///
    /// Payloads are excluded on purpose: they hold in-progress user text
    /// (rename targets, search queries, custom-function `$I` input). The title
    /// answers "which dialog was up" without dumping half-typed content.
    pub dialogs: Vec<String>,
}

/// UI mode and layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSnapshot {
    /// `UIMode` variant name.
    pub mode: String,
    /// `Left` or `Right`.
    pub active_pane: String,
    /// Whether hidden files are shown.
    pub show_hidden: bool,
    /// Cursor index range marking started from, if active.
    pub range_marking_start: Option<usize>,
    /// Geometry and panel visibility.
    pub layout: LayoutSnapshot,
}

/// Pane geometry and panel visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct LayoutSnapshot {
    pub pane_width: usize,
    pub pane_height: usize,
    pub show_tab_bar: bool,
    pub show_task_panel: bool,
    pub task_panel_height: usize,
    pub viewer_layout: String,
}

/// All tabs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabsSnapshot {
    /// Index of the active tab.
    pub active_index: usize,
    /// One entry per open tab.
    pub items: Vec<TabSnapshot>,
}

/// One tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSnapshot {
    /// Stable tab id (not the display index).
    pub id: usize,
    /// Left pane.
    pub left: PaneSnapshot,
    /// Right pane.
    pub right: PaneSnapshot,
}

/// One pane, without its entry lists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSnapshot {
    /// Current directory.
    pub path: String,
    /// Cursor index into `entries`.
    pub cursor: usize,
    /// First visible row.
    pub scroll_offset: usize,
    /// Name at the cursor — the single most useful field for reproducing.
    pub cursor_entry: Option<String>,
    /// Entries after filtering.
    ///
    /// An unexplained gap against `raw_entry_count` is the signature of a file
    /// mask or search filter the user forgot was active.
    pub entry_count: usize,
    /// Entries before filtering.
    pub raw_entry_count: usize,
    /// Marked entries in this pane (marking is per-pane).
    pub marked_count: usize,
    /// `SortMode` variant name.
    pub sort_mode: String,
    /// `SortOrder` variant name.
    pub sort_order: String,
    /// `DisplayMode` variant name.
    pub display_mode: String,
    /// Active file mask, if any.
    pub file_mask: Option<String>,
    /// Whether a `ReadDirectory` is in flight.
    ///
    /// Together with `active_job_id` this directly encodes the permanent
    /// `is_loading` failure mode described in the project's `ReadDirectory`
    /// job contract: `is_loading` true with no `active_job_id` means the pane
    /// is stuck forever.
    pub is_loading: bool,
    /// Job that owns the in-flight read, if any.
    pub active_job_id: Option<String>,
    /// Cursor name to restore once the pending read completes.
    pub pending_cursor_name: Option<String>,
}

/// Jobs in flight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobsSnapshot {
    /// `JobManager` active jobs.
    pub active: Vec<ActiveJobSnapshot>,
    /// `BackgroundJobManager` active jobs (the ones the UI shows).
    pub background: Vec<BackgroundJobSnapshot>,
}

/// One `JobManager` job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ActiveJobSnapshot {
    pub job_id: String,
    pub kind: String,
    pub state: String,
    pub progress: f64,
}

/// One `BackgroundJobManager` job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct BackgroundJobSnapshot {
    pub name: String,
    pub status: String,
    pub progress_percent: f64,
    pub tab_id: usize,
}

/// Incremental-search state, without the result list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct SearchSnapshot {
    pub query: String,
    pub result_count: usize,
    pub current_index: Option<usize>,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub use_migemo: bool,
}

/// Leap navigation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct LeapSnapshot {
    pub buffer: String,
    pub root_dir: String,
    pub root_cursor: usize,
    pub depth: usize,
}

/// Viewer position and metadata. Never file contents or the line index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ViewerSnapshot {
    pub path: String,
    pub mode: String,
    pub encoding: String,
    pub line_offset: usize,
    pub column_offset: usize,
    pub search_query: Option<String>,
    pub match_count: usize,
    pub is_loading: bool,
    pub is_searching: bool,
}

impl DiagnosticStateSnapshot {
    /// Project `state` into a bounded, serializable snapshot.
    ///
    /// Takes a read-only borrow and mutates nothing — this runs from the main
    /// loop's render path, where state purity is mandatory.
    pub fn capture(state: &AppState, seq: u64, trigger: &str) -> Self {
        Self {
            seq,
            ts: super::now_timestamp(),
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
                        kind: super::variant_name(&format!("{:?}", job.spec.kind)).to_string(),
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
