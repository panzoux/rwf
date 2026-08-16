//! Snapshots for `DialogContent::Confirmation`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{ConfirmableAction, Dialog};

#[test]
fn confirmation_short_message() {
    let state = test_state();
    let dialog = Dialog::action_confirm(
        "Confirm Action",
        "Are you sure?",
        None,
        ConfirmableAction::ReloadConfig,
    );
    snapshot_dialog("confirmation_short_message", &dialog, &state);
}

#[test]
fn confirmation_long_message() {
    let state = test_state();
    let dialog = Dialog::action_confirm(
        "Delete File",
        "This file will be permanently deleted and cannot be recovered. Are you absolutely sure?",
        None,
        ConfirmableAction::ReloadConfig,
    );
    snapshot_dialog("confirmation_long_message", &dialog, &state);
}

#[test]
fn confirmation_multiline_message() {
    let state = test_state();
    let dialog = Dialog::action_confirm(
        "Replace Files",
        "Some files already exist in the destination.\nDo you want to overwrite them?",
        None,
        ConfirmableAction::ReloadConfig,
    );
    snapshot_dialog("confirmation_multiline_message", &dialog, &state);
}

/// Simulates Phase 7.6's Undo/Redo blocked-rows summary: one header line
/// plus one `  - reason` line per blocked row, well past the
/// `CONFIRMATION_MESSAGE_MAX_LINES` cap (12) — proves the dialog truncates
/// with a visible "... N more" indicator instead of panicking or blowing
/// past the screen.
#[test]
fn confirmation_many_line_message_truncates() {
    let state = test_state();
    let mut message = String::from("18 of 20 rows can be undone, 2 blocked:");
    for i in 1..=18 {
        message.push_str(&format!("\n  - row {i}: destination already exists"));
    }
    let dialog = Dialog::action_confirm(
        "Undo",
        message,
        None,
        ConfirmableAction::ExecuteReversal {
            actions: vec![],
            operation_name: "Copy".to_string(),
            resulting_is_undo: true,
        },
    );
    snapshot_dialog("confirmation_many_line_message_truncates", &dialog, &state);
}

/// `ConfirmStats` (item count / total size) renders below the message when
/// present, e.g. `EmptyTrash`'s "N items, X MB" summary.
#[test]
fn confirmation_with_stats() {
    let state = test_state();
    let dialog = Dialog::action_confirm(
        "Empty Trash",
        "Permanently empty 3 items (1.5 MB) from the trash? This cannot be undone.",
        Some(rwf_lib::model::dialog::ConfirmStats {
            count: 3,
            total_size: 1_572_864,
        }),
        ConfirmableAction::EmptyTrash {
            fallback_roots: vec![],
        },
    );
    snapshot_dialog("confirmation_with_stats", &dialog, &state);
}
