//! Snapshots for `DialogContent::ComparisonView`.

use super::{snapshot_dialog, test_state};
use rwf_lib::job::{DiffChunk, DiffType, FileDiff};
use rwf_lib::model::dialog::Dialog;

fn sample_diff() -> FileDiff {
    FileDiff {
        left_path: "/test/left.txt".to_string(),
        right_path: "/test/right.txt".to_string(),
        differences: vec![DiffChunk {
            left_start: 1,
            left_lines: vec!["alpha".to_string(), "beta".to_string()],
            right_start: 1,
            right_lines: vec!["alpha".to_string(), "BETA".to_string()],
            chunk_type: DiffType::Modified,
        }],
    }
}

#[test]
fn comparison_view_identical() {
    let state = test_state();
    let dialog = Dialog::comparison_view(FileDiff {
        left_path: "/test/a.txt".to_string(),
        right_path: "/test/b.txt".to_string(),
        differences: vec![],
    });
    snapshot_dialog("comparison_view_identical", &dialog, &state);
}

#[test]
fn comparison_view_modified() {
    let state = test_state();
    let dialog = Dialog::comparison_view(sample_diff());
    snapshot_dialog("comparison_view_modified", &dialog, &state);
}
