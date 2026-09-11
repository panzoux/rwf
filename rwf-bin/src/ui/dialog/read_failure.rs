//! Pane read-failure dialog (Phase 7.22 §3.4): rendering and input.
//!
//! Four slots, one per question: the title says *what* failed, the first line
//! *where* and *when*, then the path on a line of its own (the most useful thing in
//! the dialog, and the most likely to be long), the OS's own words and a hint for
//! *why*. `[Retry]` / `[Dismiss]`, with Dismiss the default.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    widgets::{Paragraph, Wrap},
    Frame,
};
use rwf_lib::model::dialog::{
    DialogContent, ReadFailureDialog, READ_FAILURE_DISMISS, READ_FAILURE_RETRY,
};
use unicode_width::UnicodeWidthStr;

use super::common::{DIALOG_DIM, DIALOG_TEXT};
use super::frame::render_dialog_buttons;
use super::DialogAction;
use crate::ui::smart_truncate;

/// Columns between the border and the text on each side (1 border + 2 indent).
const SIDE: u16 = 3;

/// Wide enough for the path on one line where the terminal allows.
pub(super) fn dialog_width(d: &ReadFailureDialog, screen_width: u16) -> u16 {
    let widest = [
        d.location.display_path().width(),
        d.context_line().width(),
        d.cause.width(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0) as u16;
    (widest + 2 * SIDE)
        .clamp(56, 96)
        .min(screen_width.saturating_sub(2))
}

/// Rows a string needs when wrapped to `text_width` columns.
fn rows(s: &str, text_width: u16) -> u16 {
    s.width().max(1).div_ceil(usize::from(text_width.max(1))) as u16
}

/// Interior rows, matching the layout in [`render_read_failure_dialog`].
pub(super) fn content_height(d: &ReadFailureDialog, screen_width: u16) -> u16 {
    let text_width = dialog_width(d, screen_width).saturating_sub(2 * SIDE);
    let hint_rows = d.kind.hint().map_or(0, |h| 1 + rows(h, text_width));
    1 + rows(&d.context_line(), text_width) + 1 + 1 + rows(&d.cause, text_width) + hint_rows + 3
}

pub(super) fn render_read_failure_dialog(
    frame: &mut Frame,
    content: &DialogContent,
    area: Rect,
    d: &ReadFailureDialog,
) {
    let text_width = area.width.saturating_sub(4);
    let context = d.context_line();
    let hint = d.kind.hint();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                                       // top margin
            Constraint::Length(rows(&context, text_width)),              // where — when
            Constraint::Length(1),                                       // blank
            Constraint::Length(1),                                       // path
            Constraint::Length(rows(&d.cause, text_width)),              // the OS's words
            Constraint::Length(u16::from(hint.is_some())),               // blank
            Constraint::Length(hint.map_or(0, |h| rows(h, text_width))), // hint
            Constraint::Min(0),
            Constraint::Length(3), // buttons
        ])
        .split(area);
    let inset = |r: Rect| Rect::new(r.x + 2, r.y, r.width.saturating_sub(4), r.height);

    frame.render_widget(
        Paragraph::new(context)
            .style(DIALOG_TEXT)
            .wrap(Wrap { trim: true }),
        inset(chunks[1]),
    );
    frame.render_widget(
        Paragraph::new(smart_truncate(
            &d.location.display_path(),
            usize::from(text_width),
            "…",
        ))
        .style(DIALOG_TEXT.add_modifier(Modifier::BOLD)),
        inset(chunks[3]),
    );
    frame.render_widget(
        Paragraph::new(d.cause.as_str())
            .style(DIALOG_TEXT)
            .wrap(Wrap { trim: false }),
        inset(chunks[4]),
    );
    if let Some(hint) = hint {
        frame.render_widget(
            Paragraph::new(hint)
                .style(DIALOG_DIM)
                .wrap(Wrap { trim: true }),
            inset(chunks[6]),
        );
    }
    render_dialog_buttons(frame, chunks[8], content, d.focused_button);
}

/// `Enter` is intercepted upstream as Confirm and acts on the focused button; `Esc`
/// dismisses. `r` retries directly.
pub(super) fn handle_input(d: &mut ReadFailureDialog, key: KeyEvent) -> DialogAction {
    let plain = key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT;
    match key.code {
        KeyCode::Esc => DialogAction::Cancel,
        KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
            toggle(d);
            DialogAction::None
        }
        KeyCode::Char('h') | KeyCode::Char('l') if plain => {
            toggle(d);
            DialogAction::None
        }
        KeyCode::Char('r') if plain => {
            d.focused_button = READ_FAILURE_RETRY;
            DialogAction::Confirm
        }
        _ => DialogAction::None,
    }
}

fn toggle(d: &mut ReadFailureDialog) {
    d.focused_button = if d.focused_button == READ_FAILURE_RETRY {
        READ_FAILURE_DISMISS
    } else {
        READ_FAILURE_RETRY
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use rwf_lib::job::JobOrigin;
    use rwf_lib::model::{ActivePane, Location};

    fn dialog() -> ReadFailureDialog {
        ReadFailureDialog::new(
            3,
            ActivePane::Left,
            "Tab 1, left pane".to_string(),
            JobOrigin::UserAction,
            Location::Local(std::path::PathBuf::from("/mnt/x")),
            "gone",
        )
    }

    #[test]
    fn arrows_move_focus_between_the_two_buttons() {
        let mut d = dialog();
        assert_eq!(d.focused_button, READ_FAILURE_DISMISS);
        handle_input(&mut d, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(d.focused_button, READ_FAILURE_RETRY);
        handle_input(&mut d, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(d.focused_button, READ_FAILURE_DISMISS);
    }

    #[test]
    fn r_retries_and_esc_dismisses() {
        let mut d = dialog();
        assert_eq!(
            handle_input(
                &mut d,
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)
            ),
            DialogAction::Confirm
        );
        assert_eq!(d.focused_button, READ_FAILURE_RETRY);
        assert_eq!(
            handle_input(&mut d, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            DialogAction::Cancel
        );
    }
}
