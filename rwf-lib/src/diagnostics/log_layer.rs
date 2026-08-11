//! `tracing` bridge into the diagnostic bundle (Phase 7.15 stage 4).
//!
//! A [`Layer`] that mirrors log events into `logs.jsonl` while a session is
//! recording. Because it plugs into the subscriber rather than the call sites,
//! **not one `tracing::info!` in the codebase changes**.
//!
//! # Roles stay separate
//!
//! `session.log` remains the human-facing rolling log. `logs.jsonl` is the
//! machine-readable slice belonging to one session, sharing the `seq` counter
//! with `events.jsonl` so the two files merge into a single ordered timeline.
//!
//! # Self-feeding hazard
//!
//! The writer thread reports its own failures with `tracing::warn!`. Without a
//! guard, a failed log write would emit a warning, which this layer would
//! forward to the writer, which would fail again — a feedback loop out of one
//! bad disk. Events originating on the writer thread are therefore dropped
//! here, identified by thread name.

use serde::{Deserialize, Serialize};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// One line of `logs.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    /// Shares the counter with `events.jsonl`, so both files interleave into
    /// one ordered timeline.
    pub seq: u64,
    /// RFC 3339 local timestamp.
    pub ts: String,
    /// `ERROR`, `WARN`, `INFO`, `DEBUG` or `TRACE`.
    pub level: String,
    /// Module path the event came from.
    pub target: String,
    /// Source file, when the subscriber recorded one.
    pub file: Option<String>,
    /// Source line, when the subscriber recorded one.
    pub line: Option<u32>,
    /// The formatted `message` field, if present.
    pub message: Option<String>,
    /// All other structured fields, rendered with their `Debug` form.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// Mirrors `tracing` events into the running diagnostic session.
///
/// Install unconditionally at startup — see [`crate::logging::init_logging`].
/// The layer gates itself on session state, so it costs one atomic load per
/// event when idle.
pub struct DiagnosticLogLayer;

impl<S: Subscriber> Layer<S> for DiagnosticLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if !super::is_active() {
            return;
        }

        // See the module docs: never mirror the writer's own diagnostics.
        if current_thread_is_writer() {
            return;
        }

        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let metadata = event.metadata();
        let record = LogRecord {
            seq: super::next_seq(),
            ts: super::now_timestamp(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            file: metadata.file().map(str::to_string),
            line: metadata.line(),
            message: visitor.message,
            fields: visitor.fields,
        };

        super::send_log(record);
    }
}

/// Whether the calling thread is the diagnostic writer.
///
/// Extracted so the feedback-loop guard is testable: the whole mechanism rests
/// on `Thread::name()` actually returning the name the writer was spawned with,
/// which is worth asserting rather than assuming.
fn current_thread_is_writer() -> bool {
    std::thread::current().name() == Some(super::WRITER_THREAD_NAME)
}

/// Collects `tracing` fields into JSON, pulling `message` out separately.
#[derive(Default)]
struct JsonVisitor {
    message: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl JsonVisitor {
    fn insert(&mut self, field: &Field, value: serde_json::Value) {
        if field.name() == "message" {
            // `message` is the formatted body of `info!("...")`; keep it as a
            // plain string rather than burying it among the structured fields.
            self.message = value
                .as_str()
                .map(str::to_string)
                .or_else(|| Some(value.to_string()));
        } else {
            self.fields.insert(field.name().to_string(), value);
        }
    }
}

impl Visit for JsonVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.insert(field, serde_json::Value::String(format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, serde_json::Value::String(value.to_string()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, serde_json::Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, serde_json::Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, serde_json::Value::Bool(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visitor_separates_message_from_other_fields() {
        let mut visitor = JsonVisitor::default();
        visitor.fields.insert("count".into(), serde_json::json!(3));

        assert!(visitor.message.is_none());
        assert_eq!(visitor.fields["count"], 3);
    }

    /// The feedback-loop guard rests entirely on thread names being readable.
    /// A writer failure emits `tracing::warn!`; if this returned `false` there,
    /// the warning would be mirrored back to the failing writer, forever.
    #[test]
    fn writer_thread_is_recognised_by_name() {
        assert!(
            !current_thread_is_writer(),
            "the test thread must not be mistaken for the writer"
        );

        let handle = std::thread::Builder::new()
            .name(super::super::WRITER_THREAD_NAME.to_string())
            .spawn(current_thread_is_writer)
            .expect("spawn");

        assert!(
            handle.join().expect("join"),
            "a thread spawned with the writer's name must be recognised"
        );
    }

    #[test]
    fn log_record_round_trips() {
        let record = LogRecord {
            seq: 12,
            ts: "2026-08-11T00:00:00+09:00".to_string(),
            level: "INFO".to_string(),
            target: "rwf_lib::state".to_string(),
            file: Some("state/mod.rs".to_string()),
            line: Some(1044),
            message: Some("hello".to_string()),
            fields: serde_json::Map::new(),
        };

        let json = serde_json::to_string(&record).expect("serialize");
        let back: LogRecord = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.seq, 12);
        assert_eq!(back.level, "INFO");
        assert_eq!(back.message.as_deref(), Some("hello"));
        assert_eq!(back.line, Some(1044));
    }
}
