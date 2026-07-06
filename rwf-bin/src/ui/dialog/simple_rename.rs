//! Simple rename dialog rendering and input handling.
//!
//! Rendering split from dialog/mod.rs in M3 (move-only; snapshot-protected).
//! Input handling moved from dialog/mod.rs in M4 S5.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crossterm::event::{KeyEvent, KeyModifiers};
use rwf_lib::model::dialog::{DialogUiState, SimpleRenameDialog};

use super::DialogAction;

/// Handle key input for the Simple Rename dialog — identical Tab/Enter/Esc/TextInput
/// logic as FileMask.
pub(super) fn handle_input(dialog: &mut SimpleRenameDialog, key: KeyEvent) -> DialogAction {
    let SimpleRenameDialog {
        input,
        ui:
            DialogUiState {
                cursor_pos,
                scroll_pos,
                focused_field,
            },
    } = dialog;
    use crate::ui::text_input::{TextInput, TextInputAction};
    use crossterm::event::KeyCode;
    if key.code == KeyCode::Tab {
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            *focused_field = if *focused_field == 0 {
                2
            } else {
                *focused_field - 1
            };
        } else {
            *focused_field = (*focused_field + 1) % 3;
        }
        return DialogAction::None;
    }
    if key.code == KeyCode::Esc {
        return DialogAction::Cancel;
    }
    if key.code == KeyCode::Enter {
        return match *focused_field {
            2 => DialogAction::Cancel,
            _ => DialogAction::Confirm,
        };
    }
    if *focused_field == 0 {
        let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.clone());
        ti.set_cursor(*cursor_pos);
        ti.set_scroll(*scroll_pos);
        let action = ti.handle_input(&key);
        *input = ti.text().to_string();
        *cursor_pos = ti.cursor();
        *scroll_pos = ti.scroll();
        match action {
            TextInputAction::Confirm => return DialogAction::Confirm,
            TextInputAction::Cancel => return DialogAction::Cancel,
            _ => return DialogAction::None,
        }
    }
    DialogAction::None
}

pub(super) fn render_simple_rename_dialog(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    cursor_pos: usize,
    scroll_pos: usize,
    focused_field: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;
    let item_width = area.width.saturating_sub(4);

    frame.render_widget(
        Paragraph::new("New name:").style(base_style),
        Rect::new(area.x + 2, chunks[0].y, item_width, 1),
    );

    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(
            frame,
            Rect::new(area.x + 2, chunks[1].y, item_width, 1),
            focused_field == 0,
        );
    }

    frame.render_widget(
        Paragraph::new("(Enter to confirm, Esc to cancel)").style(hint_style),
        Rect::new(area.x + 2, chunks[2].y, item_width, 1),
    );

    let focused_item = crate::ui::dialog::common::DIALOG_SELECTED;
    let ok_style = if focused_field == 1 {
        focused_item
    } else {
        base_style
    };
    let cancel_style = if focused_field == 2 {
        focused_item
    } else {
        base_style
    };
    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line)
            .alignment(Alignment::Center)
            .style(base_style),
        chunks[4],
    );
}
