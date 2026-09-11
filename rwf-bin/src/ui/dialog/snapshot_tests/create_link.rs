//! Snapshots for `DialogContent::CreateLink`.
//!
//! Uses synthetic nonexistent paths (project convention — see other
//! snapshot modules): `CreateLinkDialog::new()` can't stat them, so
//! Hardlink/Junction always render as unavailable here. That's an accepted
//! simplification for snapshot determinism (a real TempDir path would
//! differ per test run and break the snapshot); the availability logic
//! itself is covered by real-file unit tests in
//! `rwf-lib/src/model/dialog/create_link.rs`.
//!
//! Two layouts (Phase 7.18): Windows offers Symlink/Hardlink/Junction, Unix has no
//! Junction. The `two_kinds` snapshots set `CreateLinkDialog::kinds` explicitly, so
//! they render identically — and run — on both CI runners. The three-kind ones need
//! `LinkCreateKind::Junction`, which only exists on Windows.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent};
use rwf_lib::model::{LinkCreateKind, Location};
use std::path::PathBuf;

fn dialog() -> Dialog {
    Dialog::create_link(
        Location::Local(PathBuf::from("/test/report.docx")),
        PathBuf::from("/test/dest"),
    )
}

/// The Unix layout, forced on every platform.
fn two_kinds(mut dialog: Dialog) -> Dialog {
    if let DialogContent::CreateLink(d) = &mut dialog.content {
        d.kinds = vec![LinkCreateKind::Symlink, LinkCreateKind::Hardlink];
    }
    dialog
}

fn focus_name(dialog: &mut Dialog) {
    if let DialogContent::CreateLink(d) = &mut dialog.content {
        d.focused_field = 1;
        d.link_name = "renamed_link.docx".to_string();
        d.link_name_cursor_pos = d.link_name.chars().count();
    }
}

fn focus_ok(dialog: &mut Dialog) {
    if let DialogContent::CreateLink(d) = &mut dialog.content {
        d.focused_field = d.ok_index();
    }
}

#[cfg_attr(
    unix,
    ignore = "by design: the three-kind layout includes Junction, which exists only on \
              Windows; the create_link_two_kinds_* snapshots cover Unix"
)]
#[test]
fn create_link_default() {
    let state = test_state();
    snapshot_dialog("create_link_default", &dialog(), &state);
}

#[cfg_attr(
    unix,
    ignore = "by design: the three-kind layout includes Junction, which exists only on \
              Windows; the create_link_two_kinds_* snapshots cover Unix"
)]
#[test]
fn create_link_focused_on_name() {
    let state = test_state();
    let mut dialog = dialog();
    focus_name(&mut dialog);
    snapshot_dialog("create_link_focused_on_name", &dialog, &state);
}

#[cfg_attr(
    unix,
    ignore = "by design: the three-kind layout includes Junction, which exists only on \
              Windows; the create_link_two_kinds_* snapshots cover Unix"
)]
#[test]
fn create_link_focused_on_ok() {
    let state = test_state();
    let mut dialog = dialog();
    focus_ok(&mut dialog);
    snapshot_dialog("create_link_focused_on_ok", &dialog, &state);
}

#[test]
fn create_link_two_kinds_default() {
    let state = test_state();
    snapshot_dialog(
        "create_link_two_kinds_default",
        &two_kinds(dialog()),
        &state,
    );
}

#[test]
fn create_link_two_kinds_focused_on_name() {
    let state = test_state();
    let mut dialog = two_kinds(dialog());
    focus_name(&mut dialog);
    snapshot_dialog("create_link_two_kinds_focused_on_name", &dialog, &state);
}

#[test]
fn create_link_two_kinds_focused_on_ok() {
    let state = test_state();
    let mut dialog = two_kinds(dialog());
    focus_ok(&mut dialog);
    snapshot_dialog("create_link_two_kinds_focused_on_ok", &dialog, &state);
}
