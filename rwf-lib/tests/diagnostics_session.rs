//! End-to-end coverage for a diagnostic session (Phase 7.15, stages 1-2).
//!
//! The collector handle is process-global and installed once via `OnceLock`, so
//! session start/stop cannot be exercised from a `#[cfg(test)]` module that also
//! asserts the *inactive* path — the two would interfere. This separate test
//! binary owns the active-session side.
//!
//! For the same reason everything here runs inside one `#[test]`: parallel tests
//! in a single binary share the global handle. The suite runs with
//! `--test-threads=1`, but sequencing the assertions explicitly keeps this
//! correct regardless.

use std::path::Path;

use rwf_lib::diagnostics::{self, DiagnosticEvent};
use serde_json::Value;

fn read_events(dir: &Path) -> Vec<Value> {
    let raw = std::fs::read_to_string(dir.join("events.jsonl")).expect("events.jsonl exists");
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each line is valid JSON"))
        .collect()
}

#[test]
fn session_records_events_and_writes_a_complete_bundle() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();

    // --- inactive before start -------------------------------------------
    assert!(!diagnostics::is_active());
    assert!(diagnostics::current_session().is_none());

    // --- start ------------------------------------------------------------
    let paths = diagnostics::start_session(root.clone(), "test").expect("session starts");
    assert!(diagnostics::is_active());
    assert_eq!(
        diagnostics::current_session().map(|p| p.dir.clone()),
        Some(paths.dir.clone())
    );

    // Starting twice must not clobber the running session.
    assert!(
        diagnostics::start_session(root.clone(), "test").is_none(),
        "a second start while active should be rejected"
    );

    // --- record -----------------------------------------------------------
    for i in 0..5 {
        diagnostics::observe(|| DiagnosticEvent::Note {
            message: format!("note {i}"),
        });
    }
    diagnostics::observe(|| DiagnosticEvent::Wake {
        next_wakeup_ms: 1000,
        any_pane_loading: false,
        active_jobs: 0,
    });

    // --- stop -------------------------------------------------------------
    let finished = diagnostics::stop_session(Some("it hung for a second\n".to_string()))
        .expect("session ends");
    assert_eq!(finished.dir, paths.dir);
    assert!(!diagnostics::is_active());
    assert!(diagnostics::current_session().is_none());

    // --- bundle layout ----------------------------------------------------
    // stop_session is acknowledged by the writer, so the bundle is complete by
    // the time it returns; no sleeping or polling is needed here.
    assert!(paths.dir.is_dir(), "session directory exists");
    assert!(paths.events().is_file(), "events.jsonl written");
    assert!(paths.metadata().is_file(), "metadata.json written");
    assert!(paths.report().is_file(), "report.txt written");
    assert!(paths.snapshots().is_dir(), "snapshots/ created");

    // --- event stream -----------------------------------------------------
    let events = read_events(&paths.dir);

    assert_eq!(
        events.first().and_then(|e| e["type"].as_str()),
        Some("SessionStart")
    );
    assert_eq!(
        events.last().and_then(|e| e["type"].as_str()),
        Some("SessionEnd")
    );
    // SessionStart + 5 notes + 1 wake + SessionEnd
    assert_eq!(events.len(), 8, "unexpected event count: {events:#?}");

    // seq is the ordering contract: monotonic and gap-free.
    let seqs: Vec<u64> = events
        .iter()
        .map(|e| e["seq"].as_u64().expect("seq is a number"))
        .collect();
    let expected: Vec<u64> = (0..seqs.len() as u64).collect();
    assert_eq!(seqs, expected, "seq must be gap-free and monotonic");

    // Payloads survive the round trip.
    assert_eq!(events[1]["type"], "Note");
    assert_eq!(events[1]["data"]["message"], "note 0");
    assert_eq!(events[6]["type"], "Wake");
    assert_eq!(events[6]["data"]["next_wakeup_ms"], 1000);

    // --- report and metadata ---------------------------------------------
    let report = std::fs::read_to_string(paths.report()).expect("report readable");
    assert_eq!(report, "it hung for a second\n");

    let metadata: Value = serde_json::from_str(
        &std::fs::read_to_string(paths.metadata()).expect("metadata readable"),
    )
    .expect("metadata is valid JSON");
    assert_eq!(metadata["session_id"], paths.session_id.as_str());
    assert_eq!(metadata["os"], std::env::consts::OS);
    assert!(metadata["rwf_version"].is_string());
    assert!(metadata["started_at"].is_string());
    assert!(metadata["ended_at"].is_string());

    // --- inactive again ---------------------------------------------------
    // Observations after stop must not reach the closed session.
    diagnostics::observe(|| DiagnosticEvent::Note {
        message: "after stop".to_string(),
    });
    assert_eq!(
        read_events(&paths.dir).len(),
        8,
        "events recorded after stop must not be appended"
    );

    // --- a cancelled report still keeps the bundle ------------------------
    let started = diagnostics::start_session(root, "test").expect("second session starts");
    assert_ne!(
        started.dir, paths.dir,
        "a session started in the same second must not reuse the first directory"
    );
    diagnostics::observe(|| DiagnosticEvent::Note {
        message: "second".to_string(),
    });
    let second = diagnostics::stop_session(None).expect("second session ends");
    assert_eq!(second.dir, started.dir);

    let placeholder = std::fs::read_to_string(second.report()).expect("report written");
    assert!(
        placeholder.contains("no description"),
        "cancelling the prompt must still produce a bundle, got: {placeholder:?}"
    );

    // The first bundle must survive the second session untouched.
    assert_eq!(
        read_events(&paths.dir).len(),
        8,
        "a later session must not overwrite an earlier bundle"
    );
}
