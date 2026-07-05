//! Snapshots for `DialogContent::JobManager`.
//!
//! Job rows render short id, status, name and percent (no wall-clock data),
//! so a single deterministic job is safe. Multiple jobs are avoided: the
//! manager stores jobs in a HashMap and row order would be nondeterministic.

use super::{snapshot_dialog, test_state};
use rwf_lib::job::{JobKind, JobSpec};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use rwf_lib::AppState;
use std::path::PathBuf;

fn state_with_one_job() -> AppState {
    let mut state = test_state();
    let spec = JobSpec::new(JobKind::Copy {
        sources: vec![Location::Local(PathBuf::from("/test/a.txt"))],
        dest: Location::Local(PathBuf::from("/test/out")),
    });
    state.background_jobs.start_job(
        "Copy a.txt".to_string(),
        "Copy /test/a.txt -> /test/out".to_string(),
        0,
        "Tab 1".to_string(),
        spec,
    );
    state
}

#[test]
fn job_manager_empty() {
    let state = test_state();
    let dialog = Dialog::job_manager();
    snapshot_dialog("job_manager_empty", &dialog, &state);
}

#[test]
fn job_manager_one_pending_job() {
    let state = state_with_one_job();
    let dialog = Dialog::job_manager();
    snapshot_dialog("job_manager_one_job", &dialog, &state);
}

#[test]
fn job_manager_close_button_focused() {
    let state = state_with_one_job();
    let mut dialog = Dialog::job_manager();
    if let rwf_lib::model::dialog::DialogContent::JobManager { focused_field, .. } =
        &mut dialog.content
    {
        *focused_field = 1; // Close button
    }
    snapshot_dialog("job_manager_close_focused", &dialog, &state);
}
