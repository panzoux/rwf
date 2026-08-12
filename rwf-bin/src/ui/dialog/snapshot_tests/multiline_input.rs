//! Snapshots for `DialogContent::MultiLineInput` (Phase 7.17).

use super::{snapshot_dialog, test_state};
use crate::ui::multiline_text_input::MultiLineTextInput;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rwf_lib::model::dialog::{Dialog, DialogContent, MultiLineInputDialog};

#[test]
fn multiline_input_empty_default() {
    let state = test_state();
    let dialog = Dialog::multiline_input(
        "Diagnostic Report",
        "What happened? (problem / expected behaviour)",
        "",
    );
    snapshot_dialog("multiline_input_empty_default", &dialog, &state);
}

#[test]
fn multiline_input_with_multiple_lines() {
    let state = test_state();
    let dialog = Dialog::multiline_input(
        "Diagnostic Report",
        "What happened? (problem / expected behaviour)",
        "The pane froze after Ctrl+C.\nExpected: cancel the job.\nGot: nothing happened.",
    );
    snapshot_dialog("multiline_input_with_multiple_lines", &dialog, &state);
}

/// Mixed ASCII/CJK content across lines — the widget must never split a
/// double-width character, and the cursor (placed at the very end, on the
/// CJK line) must land in the right cell.
#[test]
fn multiline_input_cjk_mixed_content() {
    let state = test_state();
    let dialog = Dialog::multiline_input(
        "Diagnostic Report",
        "What happened? (problem / expected behaviour)",
        "hello world\n日本語のバグ報告テスト\nascii again",
    );
    snapshot_dialog("multiline_input_cjk_mixed_content", &dialog, &state);
}

/// Long enough text to force vertical scrolling. Built by driving the real
/// `MultiLineTextInput::handle_input` (Enter + a character, repeated) rather
/// than hand-setting `scroll_row`, so the scroll position matches what
/// actually happens when a user types this much — that's what keeps
/// `scroll_row` and the cursor in sync in the real app.
///
/// Deliberately does NOT go through `dialog::multiline_input::handle_input`
/// (the dialog-level wrapper), which infers its height from
/// `crossterm::terminal::size()`: that queries the *host* running this test,
/// not the fixed 80x24/120x40 `TestBackend` size the snapshot renders at, so
/// building the fixture that way would make the snapshot's content depend on
/// whatever terminal happened to run the test — non-reproducible across
/// machines/CI. Driving the widget directly with an explicit height fixes
/// the assumption to the exact same formula
/// (`dialog::multiline_input::text_area_height`) evaluated at a literal
/// screen height instead.
#[test]
fn multiline_input_scrolled_past_visible_height() {
    let state = test_state();

    let mut mi = MultiLineTextInput::new(Some("line 0".to_string()));
    mi.set_height(crate::ui::dialog::multiline_input::text_area_height(24));
    for i in 1..=20 {
        mi.handle_input(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        for c in format!("line {i}").chars() {
            mi.handle_input(&KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
    }

    let mut content =
        MultiLineInputDialog::new("What happened? (problem / expected behaviour)", "");
    content.lines = mi.lines().to_vec();
    content.cursor_line = mi.cursor_line();
    content.cursor_col = mi.cursor_col();
    content.scroll_row = mi.scroll();

    let dialog = Dialog {
        title: "Diagnostic Report".to_string(),
        content: DialogContent::MultiLineInput(content),
    };
    snapshot_dialog(
        "multiline_input_scrolled_past_visible_height",
        &dialog,
        &state,
    );
}
