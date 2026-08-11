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

pub mod log_layer;
mod record;
mod state_snapshot;
mod writer;

pub use log_layer::{DiagnosticLogLayer, LogRecord};
pub use record::{truncate_detail, variant_name, DiagnosticEvent, DiagnosticRecord, DETAIL_MAX};
pub use state_snapshot::{
    ActiveJobSnapshot, BackgroundJobSnapshot, DiagnosticStateSnapshot, JobsSnapshot,
    LayoutSnapshot, LeapSnapshot, PaneSnapshot, SearchSnapshot, TabSnapshot, TabsSnapshot,
    UiSnapshot, ViewerSnapshot,
};
pub use writer::SessionPaths;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use writer::{WriterMessage, WRITER_THREAD_NAME};

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

/// Take the next sequence number.
///
/// Shared by [`observe`] and the log layer so `events.jsonl` and `logs.jsonl`
/// merge into one totally ordered timeline.
pub(crate) fn next_seq() -> u64 {
    HANDLE
        .get()
        .map_or(0, |h| h.seq.fetch_add(1, Ordering::Relaxed))
}

/// Write `config_effective.json` into the running session.
///
/// `json` is built by the caller because the collector has no access to
/// `AppState` — the observer contract in this module's docs. No-op when idle.
///
/// Serializing the resolved config is deliberately preferred over copying
/// `config.json`: every field carries `#[serde(default)]`, so the file says
/// what the user *wrote* while the struct says what the program *ran with*.
/// Only the latter reproduces a bug, or survives a default changing between
/// versions.
pub fn record_effective_config(json: String) {
    let Some(handle) = HANDLE.get() else {
        return;
    };
    if !handle.active.load(Ordering::Relaxed) {
        return;
    }
    let _ = handle.tx.send(WriterMessage::Config(json));
}

/// Forward a mirrored `tracing` event to the writer.
///
/// Called only from [`log_layer::DiagnosticLogLayer`], which has already
/// checked that a session is active and that the event did not originate on the
/// writer thread.
pub(crate) fn send_log(record: LogRecord) {
    if let Some(handle) = HANDLE.get() {
        let _ = handle.tx.send(WriterMessage::Log(Box::new(record)));
    }
}

/// Why a snapshot was taken. Becomes part of the filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTrigger {
    /// Automatic, at session start.
    Start,
    /// User pressed the snapshot key.
    Manual,
    /// Automatic, at session end.
    Final,
}

impl SnapshotTrigger {
    /// Filename-safe label.
    pub fn label(self) -> &'static str {
        match self {
            SnapshotTrigger::Start => "start",
            SnapshotTrigger::Manual => "manual",
            SnapshotTrigger::Final => "final",
        }
    }
}

/// Upper bound on snapshots per session.
///
/// Bounds a runaway session (a key stuck on auto-repeat, a script hammering
/// the binding) rather than any expected usage.
pub const MAX_SNAPSHOTS_PER_SESSION: usize = 200;

/// Snapshots taken in the running session, used to enforce
/// [`MAX_SNAPSHOTS_PER_SESSION`].
static SNAPSHOT_COUNT: AtomicU64 = AtomicU64::new(0);

/// Record a screen/state capture pair.
///
/// `screen` is the rendered text (see `rwf-bin`'s `screen_text::buffer_to_text`)
/// and `state` the semantic projection. Both are produced on the main loop,
/// which owns render state; this function only forwards them.
///
/// Emits a [`DiagnosticEvent::Snapshot`] into `events.jsonl` so the capture is
/// positioned on the same timeline as everything else.
pub fn observe_snapshot(trigger: SnapshotTrigger, screen: String, state: &DiagnosticStateSnapshot) {
    let Some(handle) = HANDLE.get() else {
        return;
    };
    if !handle.active.load(Ordering::Relaxed) {
        return;
    }

    let taken = SNAPSHOT_COUNT.fetch_add(1, Ordering::Relaxed);
    if taken >= MAX_SNAPSHOTS_PER_SESSION as u64 {
        // Log once, on the transition past the cap, then stay quiet.
        if taken == MAX_SNAPSHOTS_PER_SESSION as u64 {
            tracing::warn!(
                "diagnostics: snapshot cap ({MAX_SNAPSHOTS_PER_SESSION}) reached, \
                 further snapshots in this session are dropped"
            );
        }
        return;
    }

    let state_json = match serde_json::to_string_pretty(state) {
        Ok(json) => Some(json),
        Err(e) => {
            tracing::warn!("diagnostics: state snapshot not serializable: {e}");
            None
        }
    };

    let label = trigger.label();
    observe(|| DiagnosticEvent::Snapshot {
        trigger: label.to_string(),
        rows: screen.lines().count(),
    });

    let _ = handle
        .tx
        .send(WriterMessage::Snapshot(Box::new(writer::SnapshotPayload {
            seq: handle.seq.load(Ordering::Relaxed).saturating_sub(1),
            trigger: label,
            screen,
            state_json,
        })));
}

/// Sequence number the next observed event will receive.
///
/// Lets the caller stamp a [`DiagnosticStateSnapshot`] with the same `seq` the
/// accompanying `Snapshot` event will carry.
pub fn peek_seq() -> u64 {
    HANDLE.get().map_or(0, |h| h.seq.load(Ordering::Relaxed))
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
    SNAPSHOT_COUNT.store(0, Ordering::Relaxed);
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

#[cfg(test)]
mod keybinding_tests {
    use crate::input::{Action, KeyBindings};

    /// The diagnostic keys must be bound in **all three** keymaps.
    ///
    /// Viewer and Leap swallow nearly every key, so a NormalMode-only binding
    /// would make viewer rendering bugs and Leap filter stalls impossible to
    /// capture — a large slice of the problems this feature targets.
    #[test]
    fn diagnostic_keys_are_bound_in_every_mode() {
        let kb = KeyBindings::embedded_defaults();

        for (mode, map) in [
            ("NormalMode", &kb.normal_mode),
            ("ViewerMode", &kb.viewer_mode),
            ("LeapMode", &kb.leap_mode),
        ] {
            assert_eq!(
                map.get("F12"),
                Some(&Action::ToggleDiagnosticSession),
                "F12 must toggle a diagnostic session in {mode}"
            );
            assert_eq!(
                map.get("F11"),
                Some(&Action::DiagnosticSnapshot),
                "F11 must take a diagnostic snapshot in {mode}"
            );
        }
    }

    /// F11/F12 were chosen because nothing else used them. If a later feature
    /// claims one, the duplicate checker would flag it — this pins the intent.
    #[test]
    fn diagnostic_keys_are_not_shared_with_another_action() {
        let kb = KeyBindings::embedded_defaults();

        for (mode, map) in [
            ("NormalMode", &kb.normal_mode),
            ("ViewerMode", &kb.viewer_mode),
            ("LeapMode", &kb.leap_mode),
        ] {
            let diag_keys: Vec<&String> = map
                .iter()
                .filter(|(_, a)| {
                    matches!(
                        a,
                        Action::ToggleDiagnosticSession | Action::DiagnosticSnapshot
                    )
                })
                .map(|(k, _)| k)
                .collect();
            assert_eq!(
                diag_keys.len(),
                2,
                "{mode} should bind exactly the two diagnostic actions, got {diag_keys:?}"
            );
        }
    }
}

#[cfg(test)]
mod config_tests {
    use crate::config::{AppConfig, DiagnosticsConfig};

    #[test]
    fn diagnostics_config_defaults_to_enabled_with_a_prompt() {
        let cfg = DiagnosticsConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.prompt_for_report);
        assert!(
            cfg.output_directory.is_empty(),
            "empty means default_diagnostics_dir()"
        );
    }

    /// Config JSON is PascalCase for TWF compatibility (see CLAUDE.md).
    #[test]
    fn diagnostics_config_serialises_as_pascal_case() {
        let json = serde_json::to_value(DiagnosticsConfig::default()).expect("serialize");
        assert!(json.get("Enabled").is_some(), "got: {json}");
        assert!(json.get("OutputDirectory").is_some(), "got: {json}");
        assert!(json.get("PromptForReport").is_some(), "got: {json}");
    }

    /// Every field needs `#[serde(default)]` so older config files still load.
    #[test]
    fn diagnostics_section_tolerates_missing_and_partial_config() {
        let absent: AppConfig = serde_json::from_str("{}").expect("empty config loads");
        assert!(absent.diagnostics.enabled);

        let partial: AppConfig =
            serde_json::from_str(r#"{"Diagnostics":{"Enabled":false}}"#).expect("partial loads");
        assert!(!partial.diagnostics.enabled);
        assert!(
            partial.diagnostics.prompt_for_report,
            "unspecified fields must fall back to defaults"
        );
    }

    /// `key_bindings` carries `#[serde(skip)]`, so a bundle that serialises only
    /// `AppConfig` would silently omit the keymap — which is what makes a
    /// no-action `Key` event interpretable. Pinning the fact the capture code
    /// depends on.
    #[test]
    fn app_config_json_does_not_carry_keybindings() {
        let json = serde_json::to_value(AppConfig::default()).expect("serialize");
        assert!(
            json.get("KeyBindings").is_none() && json.get("key_bindings").is_none(),
            "AppConfig unexpectedly serialises keybindings; \
             App::record_effective_config adds them separately and would now duplicate them"
        );
    }
}
