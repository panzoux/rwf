//! Record types written to `events.jsonl`.
//!
//! One [`DiagnosticRecord`] per line. Every record carries a `seq` and a `ts`;
//! `seq` is the authoritative ordering (timestamps collide at millisecond
//! resolution and cannot order events emitted from different threads).

use serde::{Deserialize, Serialize};

/// Maximum length of a `Transition` debug payload before truncation.
///
/// Some transitions carry whole entry lists in their `Debug` output; without a
/// bound a single record could be megabytes.
pub const DETAIL_MAX: usize = 512;

/// One line of `events.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRecord {
    /// Globally monotonic, assigned at observation time.
    pub seq: u64,
    /// RFC 3339 local timestamp.
    pub ts: String,
    /// The observed event, flattened into `{"type": ..., "data": {...}}`.
    #[serde(flatten)]
    pub event: DiagnosticEvent,
}

/// An observed event.
///
/// Kept deliberately coarse — see `plan/7.15.diagnostic_report.md` §4.2. The
/// aim is reconstructing causality, not tracing every function call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DiagnosticEvent {
    /// First record of every session.
    SessionStart {
        /// `CARGO_PKG_VERSION` of the running binary.
        rwf_version: String,
        /// How the session was started (`"env"`, `"key"`, `"api"`).
        trigger: String,
    },
    /// Last record of every session.
    SessionEnd,
    /// A keypress that survived repeat-debounce.
    Key {
        /// Formatted key string as RWF's own binding lookup sees it.
        key: String,
        /// `UIMode` at the time of the press.
        mode: String,
        /// Title of the topmost dialog, if any — dialogs consume keys first.
        dialog: Option<String>,
    },
    /// A state transition. Job events also arrive here, mapped by
    /// `event_receiver::map_job_event_to_transition`.
    Transition {
        /// Variant name only, for cheap filtering.
        name: String,
        /// Truncated `Debug` rendering of the whole transition.
        detail: String,
    },
    /// A job entering `JobManager`.
    JobSubmit {
        /// `JobId` rendered as a string.
        job_id: String,
        /// `JobKind` variant name.
        kind: String,
    },
    /// The main loop is about to sleep.
    ///
    /// The highest-value event in this feature: a delay between a job finishing
    /// and the UI reflecting it shows up here as an oversized `next_wakeup_ms`.
    /// See `plan/7.15.diagnostic_report.md` §1.5.
    Wake {
        /// Computed adaptive-poll timeout.
        next_wakeup_ms: u64,
        /// Whether any pane is mid-`ReadDirectory`.
        any_pane_loading: bool,
        /// Count of jobs `JobManager` considers active.
        active_jobs: usize,
    },
    /// A frame was drawn.
    Render {
        /// Terminal width in cells.
        width: u16,
        /// Terminal height in cells.
        height: u16,
        /// `UIMode` at draw time.
        mode: String,
    },
    /// Free-form marker, for tests and for recording diagnostics-internal problems.
    Note {
        /// Human-readable message.
        message: String,
    },
}

/// Extract the variant name from a `Debug` rendering.
///
/// `"CursorMove { pane: Left }"` → `"CursorMove"`, `"Quit"` → `"Quit"`,
/// `"Foo(1)"` → `"Foo"`. Avoids a match arm per variant on an enum with well
/// over a hundred of them, which would rot on every addition elsewhere.
pub fn variant_name(debug: &str) -> &str {
    debug
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
}

/// Truncate to [`DETAIL_MAX`], appending an ellipsis marker when cut.
///
/// Respects char boundaries — transition payloads routinely contain CJK paths.
pub fn truncate_detail(s: &str) -> String {
    if s.len() <= DETAIL_MAX {
        return s.to_string();
    }
    let mut end = DETAIL_MAX;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated]", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_name_handles_struct_tuple_and_unit_variants() {
        assert_eq!(variant_name("CursorMove { pane: Left }"), "CursorMove");
        assert_eq!(variant_name("Quit"), "Quit");
        assert_eq!(variant_name("CompleteJob(JobId(1))"), "CompleteJob");
        assert_eq!(
            variant_name("ViewerScrollDown { lines: 3 }"),
            "ViewerScrollDown"
        );
        assert_eq!(variant_name(""), "");
    }

    #[test]
    fn truncate_detail_leaves_short_input_untouched() {
        assert_eq!(truncate_detail("short"), "short");
    }

    #[test]
    fn truncate_detail_bounds_long_input() {
        let long = "a".repeat(DETAIL_MAX * 2);
        let out = truncate_detail(&long);
        assert!(out.starts_with(&"a".repeat(DETAIL_MAX)));
        assert!(out.ends_with("…[truncated]"));
    }

    /// Transition payloads carry user paths, which are routinely CJK in this
    /// project. Truncating mid-codepoint would panic on the slice.
    #[test]
    fn truncate_detail_does_not_split_multibyte_chars() {
        let cjk = "日本語のパス".repeat(200);
        assert!(cjk.len() > DETAIL_MAX);
        let out = truncate_detail(&cjk);
        assert!(out.ends_with("…[truncated]"));
        // Round-trip through str proves no invalid boundary was produced.
        assert!(out.chars().count() > 0);
    }

    #[test]
    fn record_serializes_with_flattened_type_and_data() {
        let record = DiagnosticRecord {
            seq: 42,
            ts: "2026-08-11T23:41:52.113+09:00".to_string(),
            event: DiagnosticEvent::Transition {
                name: "CursorMove".to_string(),
                detail: "CursorMove { pane: Left }".to_string(),
            },
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse");

        assert_eq!(value["seq"], 42);
        assert_eq!(value["type"], "Transition");
        assert_eq!(value["data"]["name"], "CursorMove");
        assert_eq!(value["ts"], "2026-08-11T23:41:52.113+09:00");
    }

    #[test]
    fn record_round_trips() {
        let record = DiagnosticRecord {
            seq: 7,
            ts: "2026-08-11T00:00:00+09:00".to_string(),
            event: DiagnosticEvent::Wake {
                next_wakeup_ms: 1000,
                any_pane_loading: true,
                active_jobs: 2,
            },
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let back: DiagnosticRecord = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.seq, 7);
        match back.event {
            DiagnosticEvent::Wake {
                next_wakeup_ms,
                any_pane_loading,
                active_jobs,
            } => {
                assert_eq!(next_wakeup_ms, 1000);
                assert!(any_pane_loading);
                assert_eq!(active_jobs, 2);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }
}
