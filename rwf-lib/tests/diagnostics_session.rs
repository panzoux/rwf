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
    // (snapshot coverage lives in its own test below)
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

/// Snapshot pairs land on disk with matching stems and a `Snapshot` event.
///
/// Runs as its own `#[test]` but shares the process-global collector with the
/// test above, so it must start from and return to the idle state. The suite
/// runs with `--test-threads=1`.
#[test]
fn snapshots_are_written_as_txt_json_pairs() {
    use rwf_lib::diagnostics::{DiagnosticStateSnapshot, SnapshotTrigger};
    use rwf_lib::{AppConfig, AppState};

    let temp = tempfile::tempdir().expect("tempdir");
    let paths =
        diagnostics::start_session(temp.path().to_path_buf(), "test").expect("session starts");

    let state = AppState::new(AppConfig::default());

    for (trigger, screen) in [
        (SnapshotTrigger::Start, "first screen\nsecond row"),
        (SnapshotTrigger::Manual, "日本語のパス\nmixed ascii"),
        (SnapshotTrigger::Final, "last screen"),
    ] {
        let captured =
            DiagnosticStateSnapshot::capture(&state, diagnostics::peek_seq(), trigger.label());
        diagnostics::observe_snapshot(trigger, screen.to_string(), &captured);
    }

    diagnostics::stop_session(None).expect("session ends");

    let snap_dir = paths.snapshots();
    let mut names: Vec<String> = std::fs::read_dir(&snap_dir)
        .expect("snapshots dir readable")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "000-start.json",
            "000-start.txt",
            "001-manual.json",
            "001-manual.txt",
            "002-final.json",
            "002-final.txt",
        ],
        "unexpected snapshot files"
    );

    // The screen text is stored verbatim, CJK included.
    let manual = std::fs::read_to_string(snap_dir.join("001-manual.txt")).expect("txt readable");
    assert_eq!(manual, "日本語のパス\nmixed ascii");

    // The state half parses and carries the trigger.
    let state_json: Value = serde_json::from_str(
        &std::fs::read_to_string(snap_dir.join("001-manual.json")).expect("json readable"),
    )
    .expect("state snapshot is valid JSON");
    assert_eq!(state_json["trigger"], "manual");
    assert!(state_json["ui"]["mode"].is_string());
    assert!(state_json["tabs"]["items"].is_array());

    // Snapshot events are on the same timeline as everything else.
    let events = read_events(&paths.dir);
    let snapshot_events: Vec<&Value> = events.iter().filter(|e| e["type"] == "Snapshot").collect();
    assert_eq!(snapshot_events.len(), 3, "one event per snapshot");
    assert_eq!(snapshot_events[1]["data"]["trigger"], "manual");
    assert_eq!(snapshot_events[1]["data"]["rows"], 2);

    assert!(!diagnostics::is_active());
}

/// A pane's entry lists must never reach the snapshot: they are the reason the
/// projection is hand-written rather than derived (plan §5.4).
#[test]
fn state_snapshot_stays_bounded_as_entries_grow() {
    use rwf_lib::diagnostics::DiagnosticStateSnapshot;
    use rwf_lib::model::{FileEntry, Location};
    use rwf_lib::{AppConfig, AppState};

    // `test_utils` is `#[cfg(test)]`-gated and so unreachable from an
    // integration test, which compiles as a separate crate against the public
    // API. Build entries directly rather than exposing the fixture.
    fn entry(name: String) -> FileEntry {
        FileEntry {
            location: Location::Local(std::path::PathBuf::from(&name)),
            name,
            size: 1234,
            is_dir: false,
            is_hidden: false,
            modified: std::time::SystemTime::UNIX_EPOCH,
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    let mut small = AppState::new(AppConfig::default());
    let baseline = serde_json::to_string(&DiagnosticStateSnapshot::capture(&small, 0, "test"))
        .expect("serialize")
        .len();

    let entries: Vec<_> = (0..5_000)
        .map(|i| entry(format!("entry_{i:05}.txt")))
        .collect();
    {
        let tab = small.current_tab_mut();
        tab.left_pane.raw_entries = entries.clone();
        tab.left_pane.entries = entries;
    }

    let loaded = serde_json::to_string(&DiagnosticStateSnapshot::capture(&small, 0, "test"))
        .expect("serialize")
        .len();

    // Only the counts and the cursor name change; 5,000 entries must not show up.
    assert!(
        loaded < baseline + 200,
        "snapshot grew with entry count: {baseline} -> {loaded} bytes"
    );
}

/// `tracing` events are mirrored into `logs.jsonl` with seq numbers drawn from
/// the same counter as `events.jsonl`, so the two merge into one timeline.
///
/// Uses a thread-local subscriber (`with_default`) rather than the global one:
/// `init_logging` can only run once per process, and the other tests in this
/// binary must not inherit a subscriber.
#[test]
fn tracing_events_are_mirrored_into_logs_jsonl() {
    use tracing_subscriber::layer::SubscriberExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let subscriber = tracing_subscriber::registry().with(rwf_lib::diagnostics::DiagnosticLogLayer);

    let paths = tracing::subscriber::with_default(subscriber, || {
        diagnostics::start_session(temp.path().to_path_buf(), "test").expect("starts");

        tracing::info!("plain message");
        tracing::warn!(count = 7, name = "widget", "structured message");

        diagnostics::observe(|| DiagnosticEvent::Note {
            message: "interleaved".to_string(),
        });

        diagnostics::stop_session(None).expect("ends")
    });

    let raw = std::fs::read_to_string(paths.logs()).expect("logs.jsonl exists");
    let logs: Vec<Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON"))
        .collect();

    let plain = logs
        .iter()
        .find(|l| l["message"] == "plain message")
        .expect("info event mirrored");
    assert_eq!(plain["level"], "INFO");
    assert!(plain["target"].is_string());

    let structured = logs
        .iter()
        .find(|l| l["message"] == "structured message")
        .expect("warn event mirrored");
    assert_eq!(structured["level"], "WARN");
    assert_eq!(structured["fields"]["count"], 7);
    assert_eq!(structured["fields"]["name"], "widget");

    // The shared counter is what lets a consumer merge both files by `seq`.
    // Collect every seq from both streams and require no duplicates.
    let events = read_events(&paths.dir);
    let mut all: Vec<u64> = events
        .iter()
        .chain(logs.iter())
        .map(|r| r["seq"].as_u64().expect("seq present"))
        .collect();
    let count = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(
        all.len(),
        count,
        "seq must be unique across events.jsonl and logs.jsonl"
    );

    assert!(!diagnostics::is_active());
}
