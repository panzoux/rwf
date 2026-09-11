//! Snapshots for `DialogContent::ReadFailure` (Phase 7.22 §3.4).
//!
//! `kind` is set explicitly rather than classified from a message: classification
//! reads OS error codes, which differ between Windows and Unix, and a snapshot must
//! render the same on both CI runners.

use super::{snapshot_dialog, test_state};
use rwf_lib::job::{FailureKind, JobOrigin};
use rwf_lib::model::dialog::{Dialog, ReadFailureDialog, READ_FAILURE_RETRY};
use rwf_lib::model::{ActivePane, Location};
use std::path::PathBuf;

fn dialog(path: &str, origin: JobOrigin, kind: FailureKind, cause: &str) -> ReadFailureDialog {
    let mut d = ReadFailureDialog::new(
        5,
        ActivePane::Right,
        "Tab 5, right pane".to_string(),
        origin,
        Location::Local(PathBuf::from(path)),
        cause,
    );
    d.kind = kind;
    d
}

/// The report's own case: a dead SMB share restored at startup, the OS text in
/// Japanese. Dismiss is the default and carries the `[*…*]` marker.
#[test]
fn read_failure_unreachable_share_at_startup() {
    let state = test_state();
    let d = dialog(
        r"\\192.168.11.24\testfol",
        JobOrigin::SessionRestore,
        FailureKind::NetworkUnavailable,
        "ネットワーク名が見つかりません。 (os error 67)",
    );
    snapshot_dialog(
        "read_failure_unreachable_share_at_startup",
        &Dialog::read_failure(d),
        &state,
    );
}

/// A keypress needs no "when"; focus moved to Retry.
#[test]
fn read_failure_retry_focused() {
    let state = test_state();
    let mut d = dialog(
        "/mnt/backup/photos",
        JobOrigin::UserAction,
        FailureKind::NotFound,
        "No such file or directory (os error 2)",
    );
    d.focused_button = READ_FAILURE_RETRY;
    snapshot_dialog(
        "read_failure_retry_focused",
        &Dialog::read_failure(d),
        &state,
    );
}

/// The path keeps both ends when it cannot fit: the share and the leaf are the
/// informative parts.
#[test]
fn read_failure_long_path_is_truncated_in_the_middle() {
    let state = test_state();
    let d = dialog(
        r"\\fileserver.corp.example.com\departments\engineering\projects\2026\rwf\diagnostics\bundles\archive",
        JobOrigin::Refresh,
        FailureKind::PermissionDenied,
        "Access is denied. (os error 5)",
    );
    snapshot_dialog(
        "read_failure_long_path_is_truncated_in_the_middle",
        &Dialog::read_failure(d),
        &state,
    );
}
