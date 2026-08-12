//! Rendering and input handling for `DialogContent::MultiLineInput`
//! (Phase 7.17) — the diagnostic report prompt today, potentially other
//! free-form text prompts later.
//!
//! Kept separate from `basic.rs`'s `DialogContent::Input` handling: extending
//! the single-line `Input` dialog with a multi-line mode would touch the
//! render/input paths shared by every one of its other callers (Register
//! Folder, Create Directory, Create File, Custom Function Input). A dedicated
//! variant confines Phase 7.17 entirely to this file, `multiline_input.rs`'s
//! model counterpart in `rwf-lib`, and the `multiline_text_input` widget.

use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::model::dialog::MultiLineInputDialog;

use crate::ui::multiline_text_input::{MultiLineInputAction, MultiLineTextInput};

use super::common::{DIALOG_DIM, DIALOG_TEXT};
use super::DialogAction;

/// Left/right text margin inside the dialog's (already border-stripped)
/// content area — matches the `x + 2` / `width - 4` convention the single-line
/// `Input` dialog uses in `basic.rs`.
const TEXT_MARGIN: u16 = 2;

/// `render_dialog` doesn't carry an explicit size arm for `MultiLineInput`
/// (see `dialog/mod.rs`'s `min_content_height`/`dialog_height`/`dialog_width`
/// matches), so it falls through to those matches' `_` defaults: content
/// height 8, dialog height `max(70% of screen, content+2)` capped at
/// `screen-2`, and `default_dialog_width` for width. `handle_input` below
/// has no `Frame` to measure, so these two functions replicate that same
/// arithmetic from the terminal size alone — the same technique
/// `DeleteConfirm`'s scroll-clamp logic in `basic.rs` uses. If `mod.rs` ever
/// grows an explicit arm for `MultiLineInput`, update these to match.
fn mirrored_dialog_height(screen_height: u16) -> u16 {
    const MIRRORED_MIN_CONTENT_HEIGHT: u16 = 8;
    let min_dialog_height = MIRRORED_MIN_CONTENT_HEIGHT + 2;
    let percent_height = (screen_height * 70) / 100;
    percent_height
        .max(min_dialog_height)
        .min(screen_height.saturating_sub(2))
}

/// Text-area row count `handle_input` should assume, mirroring what `render`
/// actually lays out: content height minus borders minus prompt(1) minus
/// hint(1).
pub(super) fn text_area_height(screen_height: u16) -> u16 {
    let content_height = mirrored_dialog_height(screen_height).saturating_sub(2);
    content_height.saturating_sub(2).max(1)
}

/// Text-area column count `handle_input` should assume, mirroring
/// `default_dialog_width` minus borders minus the `TEXT_MARGIN` applied on
/// both sides.
pub(super) fn text_area_width(screen_width: u16) -> u16 {
    let dialog_width = super::default_dialog_width(screen_width);
    let content_width = dialog_width.saturating_sub(2);
    content_width.saturating_sub(TEXT_MARGIN * 2).max(1)
}

/// Render the multi-line input dialog: prompt, text box, hint. No button
/// row — Ctrl+Enter/Enter/Esc are the controls, same convention as the
/// single-line `Input` dialog.
pub(super) fn render_multiline_input_dialog(
    frame: &mut Frame,
    area: Rect,
    dialog: &MultiLineInputDialog,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Min(1),    // text box
            Constraint::Length(1), // hint
        ])
        .split(area);

    let item_width = area.width.saturating_sub(TEXT_MARGIN * 2);

    frame.render_widget(
        Paragraph::new(dialog.prompt.as_str()).style(DIALOG_TEXT),
        Rect::new(area.x + TEXT_MARGIN, chunks[0].y, item_width, 1),
    );

    {
        let mut mi = MultiLineTextInput::new(Some(dialog.text()));
        mi.set_cursor(dialog.cursor_line, dialog.cursor_col);
        mi.set_scroll(dialog.scroll_row);
        mi.set_width(item_width);
        mi.set_height(chunks[1].height);
        mi.render(
            frame,
            Rect::new(
                area.x + TEXT_MARGIN,
                chunks[1].y,
                item_width,
                chunks[1].height,
            ),
            true,
        );
    }

    frame.render_widget(
        // 41 columns, so the whole hint survives at the 80-wide standard size —
        // the interior is 46 with 2-column margins, leaving 42. The earlier
        // wording overflowed and lost "Esc to cancel" entirely at that width,
        // which is the one instruction a stuck user most needs.
        //
        // Ctrl+S is the advertised confirm key because it is the one that works
        // on every platform; Ctrl+Enter also confirms but is only distinguishable
        // from Enter on Windows (see `multiline_text_input.rs`), so leading with
        // it would strand Unix users.
        Paragraph::new("Ctrl+S confirm, Enter newline, Esc cancel").style(DIALOG_DIM),
        Rect::new(area.x + TEXT_MARGIN, chunks[2].y, item_width, 1),
    );
}

/// Handle key input for the multi-line input dialog. Delegates everything to
/// `MultiLineTextInput`, which owns Enter-vs-Ctrl+Enter-vs-Esc; unlike
/// `basic::handle_input` (single-line `Input`), Enter must NOT be
/// special-cased to `DialogAction::Confirm` here — see the corresponding
/// exclusion in `dialog::mod::handle_dialog_input`'s top-of-function Enter
/// interception.
pub(super) fn handle_input(dialog: &mut MultiLineInputDialog, key: KeyEvent) -> DialogAction {
    let (screen_w, screen_h) = crossterm::terminal::size().unwrap_or((80, 24));

    let mut mi = MultiLineTextInput::new(Some(dialog.text()));
    mi.set_cursor(dialog.cursor_line, dialog.cursor_col);
    mi.set_scroll(dialog.scroll_row);
    mi.set_width(text_area_width(screen_w));
    mi.set_height(text_area_height(screen_h));

    let action = mi.handle_input(&key);

    dialog.lines = mi.lines().to_vec();
    dialog.cursor_line = mi.cursor_line();
    dialog.cursor_col = mi.cursor_col();
    dialog.scroll_row = mi.scroll();

    match action {
        MultiLineInputAction::Confirm => DialogAction::Confirm,
        MultiLineInputAction::Cancel => DialogAction::Cancel,
        MultiLineInputAction::TextChanged | MultiLineInputAction::CursorMoved => DialogAction::None,
        MultiLineInputAction::None => DialogAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn plain_enter_inserts_newline_not_confirm() {
        let mut d = MultiLineInputDialog::new("Prompt", "hello");
        let action = handle_input(&mut d, key(KeyCode::Enter));
        assert_eq!(action, DialogAction::None);
        assert_eq!(d.text(), "hello\n");
    }

    #[test]
    fn ctrl_enter_confirms() {
        let mut d = MultiLineInputDialog::new("Prompt", "hello");
        let action = handle_input(&mut d, KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        assert_eq!(action, DialogAction::Confirm);
        assert_eq!(d.text(), "hello");
    }

    #[test]
    fn esc_cancels() {
        let mut d = MultiLineInputDialog::new("Prompt", "hello");
        let action = handle_input(&mut d, key(KeyCode::Esc));
        assert_eq!(action, DialogAction::Cancel);
    }

    #[test]
    fn typed_char_updates_dialog_state() {
        let mut d = MultiLineInputDialog::new("Prompt", "");
        handle_input(&mut d, key(KeyCode::Char('a')));
        handle_input(&mut d, key(KeyCode::Char('b')));
        assert_eq!(d.text(), "ab");
        assert_eq!(d.cursor_col, 2);
    }

    #[test]
    fn text_area_width_and_height_are_positive_at_standard_sizes() {
        for (w, h) in [(80u16, 24u16), (120, 40)] {
            assert!(text_area_width(w) > 0);
            assert!(text_area_height(h) > 0);
        }
    }
}
