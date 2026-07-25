//! Snapshots for `DialogContent::FileInfo`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::{FileEntry, LinkKind, Location};
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn file_info_regular_file() {
    let state = test_state();
    let entry = FileEntry {
        name: "test.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/test.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let dialog = Dialog::file_info(&entry);
    snapshot_dialog("file_info_regular_file", &dialog, &state);
}

#[test]
fn file_info_directory_with_size() {
    let state = test_state();
    let entry = FileEntry {
        name: "my_folder".to_string(),
        location: Location::Local(PathBuf::from("/test/my_folder")),
        size: 0,
        is_dir: true,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: Some(1048576), // 1 MB calculated
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let dialog = Dialog::file_info(&entry);
    snapshot_dialog("file_info_directory_with_size", &dialog, &state);
}

#[test]
fn file_info_symlink() {
    let state = test_state();
    let entry = FileEntry {
        name: "link".to_string(),
        location: Location::Local(PathBuf::from("/test/link")),
        size: 0,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: true,
        link_target: Some(PathBuf::from("/test/target.txt")),
        link_kind: Some(LinkKind::Symlink),
    };
    let dialog = Dialog::file_info(&entry);
    snapshot_dialog("file_info_symlink", &dialog, &state);
}

#[test]
fn file_info_detected_type() {
    let state = test_state();
    let entry = FileEntry {
        name: "photo.png".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/photo.png")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("PNG image".to_string());
    }
    snapshot_dialog("file_info_detected_type", &dialog, &state);
}

#[test]
fn file_info_detecting() {
    let state = test_state();
    let entry = FileEntry {
        name: "photo.png".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/photo.png")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detecting = true;
    }
    snapshot_dialog("file_info_detecting", &dialog, &state);
}
