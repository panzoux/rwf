//! Snapshots for `DialogContent::SplitJoinDialog`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent, SplitJoinDialogContent, SplitJoinMode};

#[test]
fn split_join_split_mode() {
    let state = test_state();
    let dialog = Dialog::split_join_dialog();
    snapshot_dialog("split_join_split", &dialog, &state);
}

#[test]
fn split_join_join_mode() {
    let state = test_state();
    let dialog = Dialog {
        title: "Split/Join Files".to_string(),
        content: DialogContent::SplitJoinDialog(SplitJoinDialogContent {
            mode: SplitJoinMode::Join,
            chunk_size_mb: 100,
        }),
    };
    snapshot_dialog("split_join_join", &dialog, &state);
}

#[test]
fn split_join_custom_chunk_size() {
    let state = test_state();
    let dialog = Dialog {
        title: "Split/Join Files".to_string(),
        content: DialogContent::SplitJoinDialog(SplitJoinDialogContent {
            mode: SplitJoinMode::Split,
            chunk_size_mb: 50,
        }),
    };
    snapshot_dialog("split_join_custom_size", &dialog, &state);
}
