//! Background writer thread and on-disk session layout.
//!
//! The writer owns every file handle in a diagnostic bundle. Nothing on the UI
//! loop or in a job worker ever touches the filesystem on behalf of
//! diagnostics — they only push messages onto an unbounded channel.
//!
//! **Every failure here is logged and swallowed.** A full disk, a read-only
//! directory or a serialization error must never propagate into RWF.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use tokio::sync::mpsc::UnboundedReceiver;

use super::record::DiagnosticRecord;

/// Filesystem layout of one diagnostic session.
#[derive(Debug, Clone)]
pub struct SessionPaths {
    /// `<root>/<session_id>`.
    pub dir: PathBuf,
    /// Timestamp-derived identifier, e.g. `20260811-234152`.
    pub session_id: String,
}

impl SessionPaths {
    /// Build paths for a new session rooted at `root`.
    pub fn new(root: PathBuf, session_id: String) -> Self {
        let dir = root.join(&session_id);
        Self { dir, session_id }
    }

    /// `events.jsonl` — one [`DiagnosticRecord`] per line.
    pub fn events(&self) -> PathBuf {
        self.dir.join("events.jsonl")
    }

    /// `metadata.json` — environment and session timing.
    pub fn metadata(&self) -> PathBuf {
        self.dir.join("metadata.json")
    }

    /// `report.txt` — the user's description of the problem.
    pub fn report(&self) -> PathBuf {
        self.dir.join("report.txt")
    }

    /// `snapshots/` — paired `.txt` / `.json` screen and state captures.
    pub fn snapshots(&self) -> PathBuf {
        self.dir.join("snapshots")
    }
}

/// Messages accepted by the writer thread.
pub(crate) enum WriterMessage {
    /// Open a new session directory and start accepting records.
    StartSession(Box<SessionPaths>),
    /// Append one record to `events.jsonl`.
    Record(Box<DiagnosticRecord>),
    /// Flush, write `metadata.json` and `report.txt`, close the session.
    EndSession {
        /// User-supplied description, or `None` if the prompt was cancelled.
        report: Option<String>,
        /// Signalled once the bundle is fully on disk.
        ///
        /// Without this the writer thread is detached and process exit races
        /// the final flush — a session ended just before quitting would lose
        /// `metadata.json` and `report.txt`.
        ack: std::sync::mpsc::SyncSender<()>,
    },
}

/// Spawn the writer thread.
///
/// A plain OS thread rather than a tokio task: the work is blocking file I/O,
/// and a dedicated thread cannot starve the async runtime no matter how slow
/// the disk is. `blocking_recv` is safe here because a freshly spawned thread
/// carries no runtime context.
///
/// If the thread cannot be spawned, diagnostics is silently unavailable and
/// the application is otherwise unaffected.
pub(crate) fn spawn(rx: UnboundedReceiver<WriterMessage>) {
    let spawned = std::thread::Builder::new()
        .name("rwf-diagnostics".to_string())
        .spawn(move || run(rx));

    if let Err(e) = spawned {
        tracing::warn!("diagnostics: writer thread could not be spawned: {e}");
    }
}

fn run(mut rx: UnboundedReceiver<WriterMessage>) {
    let mut session: Option<ActiveSession> = None;

    while let Some(msg) = rx.blocking_recv() {
        match msg {
            WriterMessage::StartSession(paths) => {
                // Drop any session left open by a crash-adjacent stop.
                if let Some(previous) = session.take() {
                    previous.finish(None);
                }
                session = ActiveSession::open(*paths);
            }
            WriterMessage::Record(record) => {
                if let Some(active) = session.as_mut() {
                    active.write_record(&record);
                }
            }
            WriterMessage::EndSession { report, ack } => {
                if let Some(active) = session.take() {
                    active.finish(report);
                }
                // Receiver may already have timed out; that is not our problem.
                let _ = ack.send(());
            }
        }
    }

    // Channel closed (process shutting down) — save whatever we have.
    if let Some(active) = session.take() {
        active.finish(None);
    }
}

/// An open session: the directory exists and `events.jsonl` is writable.
struct ActiveSession {
    paths: SessionPaths,
    events: BufWriter<File>,
    started_at: String,
    /// Set after the first write error so the log records it once, not per record.
    write_failed: bool,
}

impl ActiveSession {
    /// Create the session directory and open `events.jsonl`.
    ///
    /// The `mkdir` happens here, on the writer thread, rather than at session
    /// start on the UI loop — keeping all diagnostics I/O off the hot path.
    fn open(paths: SessionPaths) -> Option<Self> {
        if let Err(e) = fs::create_dir_all(paths.snapshots()) {
            tracing::warn!(
                "diagnostics: cannot create session dir {}: {e}",
                paths.dir.display()
            );
            return None;
        }

        let file = match File::create(paths.events()) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("diagnostics: cannot open events.jsonl: {e}");
                return None;
            }
        };

        tracing::info!("diagnostics: session started at {}", paths.dir.display());

        Some(Self {
            paths,
            events: BufWriter::new(file),
            started_at: super::now_timestamp(),
            write_failed: false,
        })
    }

    fn write_record(&mut self, record: &DiagnosticRecord) {
        if self.write_failed {
            return;
        }

        let line = match serde_json::to_string(record) {
            Ok(line) => line,
            Err(e) => {
                tracing::warn!("diagnostics: record {} not serializable: {e}", record.seq);
                return;
            }
        };

        // Flushed per record: a diagnostic bundle is most valuable precisely
        // when the process is about to misbehave, so durability beats
        // throughput at these event rates.
        let written = writeln!(self.events, "{line}").and_then(|()| self.events.flush());
        if let Err(e) = written {
            tracing::warn!("diagnostics: events.jsonl write failed: {e}");
            self.write_failed = true;
        }
    }

    fn finish(mut self, report: Option<String>) {
        if let Err(e) = self.events.flush() {
            tracing::warn!("diagnostics: final flush failed: {e}");
        }

        self.write_metadata();

        let body = report
            .unwrap_or_else(|| "(no description — the report prompt was cancelled)\n".to_string());
        if let Err(e) = fs::write(self.paths.report(), body) {
            tracing::warn!("diagnostics: report.txt write failed: {e}");
        }

        tracing::info!(
            "diagnostics: session written to {}",
            self.paths.dir.display()
        );
    }

    fn write_metadata(&self) {
        let metadata = serde_json::json!({
            "session_id": self.paths.session_id,
            "started_at": self.started_at,
            "ended_at": super::now_timestamp(),
            "rwf_version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "terminal": std::env::var("TERM").unwrap_or_else(|_| "unknown".to_string()),
            "term_program": std::env::var("TERM_PROGRAM").ok(),
            "wt_session": std::env::var("WT_SESSION").ok(),
        });

        match serde_json::to_string_pretty(&metadata) {
            Ok(json) => {
                if let Err(e) = fs::write(self.paths.metadata(), json) {
                    tracing::warn!("diagnostics: metadata.json write failed: {e}");
                }
            }
            Err(e) => tracing::warn!("diagnostics: metadata not serializable: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_paths_are_rooted_under_the_session_id() {
        let paths = SessionPaths::new(PathBuf::from("/tmp/diag"), "20260811-234152".to_string());

        assert!(paths.dir.ends_with("20260811-234152"));
        assert!(paths.events().ends_with("events.jsonl"));
        assert!(paths.metadata().ends_with("metadata.json"));
        assert!(paths.report().ends_with("report.txt"));
        assert!(paths.snapshots().ends_with("snapshots"));
    }
}
