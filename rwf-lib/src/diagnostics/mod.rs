//! Diagnostic session recording (Phase 7.15).
//!
//! Captures a reproducible bug case as one self-contained folder: structured
//! events, screen snapshots, environment metadata and a user description, so
//! the chain `input → state change → job → render → problem` can be
//! reconstructed after the fact.
//!
//! # Design contract
//!
//! This module is an **observer**. It holds no reference to `AppState`, so it
//! cannot become part of the control path even by accident — a property
//! enforced by the process-global handle rather than by convention.
//!
//! Observation points are deliberately few (see
//! `plan/7.15.diagnostic_report.md` §1.2):
//!
//! - [`observe`] in `state::update_state` — every `Transition`, and therefore
//!   every `JobEvent`, since `event_receiver::process_pending_events` maps job
//!   events into transitions before applying them.
//! - [`observe`] in `job::JobManager::start_job` — every job submission.
//! - `App::handle_key_event`, `App::render` and the main loop in `rwf-bin`.
//!
//! # Cost when inactive
//!
//! [`observe`] takes a closure. With no session running the cost is one
//! `OnceLock` read plus one relaxed atomic load; the closure is never called,
//! so payload formatting is not paid for. This is what makes the feature
//! viable in release builds.

mod record;
mod writer;

pub use record::{truncate_detail, variant_name, DiagnosticEvent, DiagnosticRecord, DETAIL_MAX};
pub use writer::SessionPaths;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use writer::WriterMessage;

static HANDLE: OnceLock<DiagnosticHandle> = OnceLock::new();

/// How long [`stop_session`] waits for the writer to finish the bundle.
const WRITER_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Process-global observation endpoint.
///
/// Global rather than a field on `AppState` because the observation points
/// span a free function in `rwf-lib`, an app struct in `rwf-bin`, and (from
/// stage 4) a `tracing` layer with no access to either. Threading a reference
/// through all of them would mean signature churn across the whole codebase.
struct DiagnosticHandle {
    active: AtomicBool,
    seq: AtomicU64,
    tx: UnboundedSender<WriterMessage>,
    /// Locked on session start/stop and by the UI badge once per frame — never
    /// per observed event, so it stays off the path that must not contend.
    current: Mutex<Option<ActiveSession>>,
}

/// Bookkeeping for the running session, used by the UI indicator.
#[derive(Debug, Clone)]
struct ActiveSession {
    paths: SessionPaths,
    started: std::time::Instant,
}

fn init_handle() -> DiagnosticHandle {
    let (tx, rx) = unbounded_channel();
    writer::spawn(rx);
    DiagnosticHandle {
        active: AtomicBool::new(false),
        seq: AtomicU64::new(0),
        tx,
        current: Mutex::new(None),
    }
}

/// RFC 3339 local timestamp.
pub(crate) fn now_timestamp() -> String {
    chrono::Local::now().to_rfc3339()
}

/// Whether a diagnostic session is currently recording.
pub fn is_active() -> bool {
    HANDLE
        .get()
        .is_some_and(|h| h.active.load(Ordering::Relaxed))
}

/// Directory of the running session, if any.
pub fn current_session() -> Option<SessionPaths> {
    let handle = HANDLE.get()?;
    let guard = handle.current.lock().ok()?;
    guard.as_ref().map(|s| s.paths.clone())
}

/// How long the running session has been recording.
///
/// Drives the `● DIAG mm:ss` indicator. Returns `None` when idle.
pub fn session_elapsed() -> Option<std::time::Duration> {
    let handle = HANDLE.get()?;
    let guard = handle.current.lock().ok()?;
    guard.as_ref().map(|s| s.started.elapsed())
}

/// Record an event, if a session is running.
///
/// `build` is only invoked when recording is active, so callers may format
/// freely inside it without paying for it in the common case:
///
/// ```ignore
/// diagnostics::observe(|| DiagnosticEvent::Transition {
///     name: variant_name(&debug).to_string(),
///     detail: truncate_detail(&debug),
/// });
/// ```
pub fn observe(build: impl FnOnce() -> DiagnosticEvent) {
    let Some(handle) = HANDLE.get() else {
        return;
    };
    if !handle.active.load(Ordering::Relaxed) {
        return;
    }

    let record = DiagnosticRecord {
        seq: handle.seq.fetch_add(1, Ordering::Relaxed),
        ts: now_timestamp(),
        event: build(),
    };

    // A closed channel means the writer thread is gone; there is nothing
    // useful to do about it and it must not disturb the caller.
    let _ = handle.tx.send(WriterMessage::Record(Box::new(record)));
}

/// Build a session id that does not collide with an existing directory.
///
/// The id is second-resolution, so two sessions started within the same second
/// would otherwise share a directory and the second would truncate the first
/// bundle's `events.jsonl`. Rare in interactive use, routine in tests and when
/// toggling the session key twice in quick succession.
///
/// This performs a few `stat` calls — acceptable because session start is a
/// one-shot user action, not the hot path the "no I/O" rule protects.
fn unique_session_id(root: &Path) -> String {
    let base = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    if !root.join(&base).exists() {
        return base;
    }
    for n in 2..100 {
        let candidate = format!("{base}-{n}");
        if !root.join(&candidate).exists() {
            return candidate;
        }
    }
    // Pathological case; let the writer log whatever failure results.
    base
}

/// Start recording into a new session directory under `root`.
///
/// Returns the session paths, or `None` if a session is already running or the
/// writer could not be reached. The directory itself is created on the writer
/// thread, so this call performs no filesystem I/O.
pub fn start_session(root: PathBuf, trigger: &str) -> Option<SessionPaths> {
    let handle = HANDLE.get_or_init(init_handle);

    if handle.active.load(Ordering::Relaxed) {
        return None;
    }

    let paths = SessionPaths::new(root.clone(), unique_session_id(&root));

    if handle
        .tx
        .send(WriterMessage::StartSession(Box::new(paths.clone())))
        .is_err()
    {
        tracing::warn!("diagnostics: writer unavailable, session not started");
        return None;
    }

    if let Ok(mut current) = handle.current.lock() {
        *current = Some(ActiveSession {
            paths: paths.clone(),
            started: std::time::Instant::now(),
        });
    }
    handle.active.store(true, Ordering::Relaxed);

    let trigger = trigger.to_string();
    observe(|| DiagnosticEvent::SessionStart {
        rwf_version: env!("CARGO_PKG_VERSION").to_string(),
        trigger,
    });

    Some(paths)
}

/// Stop recording and finalise the bundle.
///
/// `report` is the user's description; `None` records a placeholder rather than
/// discarding the session — the user already did the work of reproducing the
/// problem, and losing that to a cancelled prompt would be hostile.
///
/// Returns the finished session's paths, or `None` if nothing was running.
pub fn stop_session(report: Option<String>) -> Option<SessionPaths> {
    let handle = HANDLE.get()?;
    if !handle.active.load(Ordering::Relaxed) {
        return None;
    }

    observe(|| DiagnosticEvent::SessionEnd);
    handle.active.store(false, Ordering::Relaxed);

    // Wait for the bundle to reach disk. The writer thread is detached, so
    // returning early would let process exit race the final flush and lose
    // `metadata.json` / `report.txt`. Bounded so a wedged writer degrades to a
    // short pause rather than a hang — and only ever reached on an explicit,
    // user-initiated session stop, never on the UI loop's hot path.
    let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
    if handle
        .tx
        .send(WriterMessage::EndSession {
            report,
            ack: ack_tx,
        })
        .is_ok()
        && ack_rx.recv_timeout(WRITER_DRAIN_TIMEOUT).is_err()
    {
        tracing::warn!("diagnostics: writer did not finish within the drain timeout");
    }

    handle
        .current
        .lock()
        .ok()
        .and_then(|mut c| c.take())
        .map(|s| s.paths)
}

/// Default location for diagnostic bundles.
///
/// Mirrors [`crate::logging::default_log_dir`]: a project-local `diagnostics/`
/// when running from a tree that already has `logs/`, otherwise under the
/// platform data directory.
pub fn default_diagnostics_dir() -> PathBuf {
    let local_logs = PathBuf::from("logs");
    if local_logs.is_dir() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join("diagnostics");
        }
    }

    if let Some(data_dir) = dirs::data_dir() {
        data_dir.join("rwf").join("diagnostics")
    } else {
        PathBuf::from(".").join("diagnostics")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// The disabled path must not evaluate the payload closure. This is the
    /// property that lets the feature ship in release builds.
    #[test]
    fn observe_does_not_evaluate_closure_when_no_session_runs() {
        // Note: relies on no session having been started in this test binary.
        // `start_session` is exercised in tests/diagnostics_session.rs, which
        // runs as a separate binary.
        static CALLS: AtomicUsize = AtomicUsize::new(0);

        for _ in 0..1000 {
            observe(|| {
                CALLS.fetch_add(1, Ordering::Relaxed);
                DiagnosticEvent::Note {
                    message: "should never be built".to_string(),
                }
            });
        }

        assert_eq!(CALLS.load(Ordering::Relaxed), 0);
        assert!(!is_active());
    }

    #[test]
    fn stop_session_without_start_is_a_noop() {
        assert!(stop_session(None).is_none());
    }

    #[test]
    fn current_session_is_none_when_idle() {
        assert!(current_session().is_none());
    }

    #[test]
    fn default_diagnostics_dir_ends_with_diagnostics() {
        assert!(default_diagnostics_dir().ends_with("diagnostics"));
    }
}
