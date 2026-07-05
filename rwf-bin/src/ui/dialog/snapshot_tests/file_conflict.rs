//! Snapshots for `DialogContent::FileConflict`.

use super::{snapshot_dialog, test_state};
use rwf_lib::config::EditMode;
use rwf_lib::model::dialog::{ConflictPair, Dialog};
use rwf_lib::model::{FileEntry, Location};
use std::path::PathBuf;
use std::time::SystemTime;

fn entry(name: &str, dir: &str, size: u64) -> FileEntry {
    FileEntry {
        name: name.to_string(),
        location: Location::Local(PathBuf::from(format!("{dir}/{name}"))),
        size,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    }
}

fn conflict(name: &str) -> ConflictPair {
    let source = entry(name, "/test/src", 100);
    let dest = entry(name, "/test/dst", 200);
    ConflictPair {
        source_path: source.location.clone(),
        dest_path: dest.location.clone(),
        source,
        dest,
        is_directory: false,
    }
}

#[test]
fn file_conflict_single() {
    let state = test_state();
    let dialog = Dialog::file_conflict(vec![conflict("a.txt")], 0, EditMode::Emacs, "Copy");
    snapshot_dialog("file_conflict_single", &dialog, &state);
}

#[test]
fn file_conflict_multiple_second_selected() {
    let state = test_state();
    let dialog = Dialog::file_conflict(
        vec![conflict("a.txt"), conflict("b.txt"), conflict("c.txt")],
        1,
        EditMode::Emacs,
        "Move",
    );
    snapshot_dialog("file_conflict_multi", &dialog, &state);
}
