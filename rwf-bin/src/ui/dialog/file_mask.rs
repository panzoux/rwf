//! File mask dialog rendering and input handling.
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
use rwf_lib::model::dialog::{DialogUiState, FileMaskDialog};

use super::DialogAction;

/// Handle key input for the File Mask dialog: Tab cycles textbox → OK → Cancel,
/// Esc cancels, Enter confirms (or cancels if the Cancel button is focused), and
/// text editing delegates to the shared `TextInput` widget when the textbox is focused.
pub(super) fn handle_input(dialog: &mut FileMaskDialog, key: KeyEvent) -> DialogAction {
    let FileMaskDialog {
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
    // Tab cycles: 0 (textbox) → 1 (OK) → 2 (Cancel) → 0
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
            2 => DialogAction::Cancel,  // Cancel button
            _ => DialogAction::Confirm, // textbox or OK button
        };
    }
    // Delegate text editing to TextInput widget only when textbox is focused
    if *focused_field == 0 {
        let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.clone()); // rule #1: for Vi U (revert) command
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

/// Render dialog content based on type
/// Render the Sort dialog (sort key + order + OK/Cancel)
///
/// Layout (11 lines total, per DIALOG_DESIGN_SPEC.md):
///   5 = label "Sort by:" + 4 items
///   1 = spacer
///   3 = label "Order:" + 2 items
///   1 = spacer
///   1 = buttons [*OK*] [Cancel]
pub(super) fn render_file_mask_dialog(
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
            Constraint::Length(1), // blank
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint1: Multiple patterns
            Constraint::Length(1), // hint2: Exclusion
            Constraint::Length(1), // hint3: Regexp
            Constraint::Length(1), // blank/spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;
    let item_width = area.width.saturating_sub(4);

    // Prompt
    frame.render_widget(
        Paragraph::new("Enter file mask (* = any chars, ? = single char):").style(base_style),
        Rect::new(area.x + 2, chunks[1].y, item_width, 1),
    );

    // Textbox
    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(
            frame,
            Rect::new(area.x + 2, chunks[2].y, item_width, 1),
            focused_field == 0,
        );
    }

    // Hint lines
    frame.render_widget(
        Paragraph::new("Multiple patterns: *.txt *.doc").style(hint_style),
        Rect::new(area.x + 2, chunks[3].y, item_width, 1),
    );
    frame.render_widget(
        Paragraph::new("Exclusion: :*.txt :temp*").style(hint_style),
        Rect::new(area.x + 2, chunks[4].y, item_width, 1),
    );
    frame.render_widget(
        Paragraph::new("Regexp: /.*\\.json$/ /TEST/i /Test/").style(hint_style),
        Rect::new(area.x + 2, chunks[5].y, item_width, 1),
    );

    // Buttons [*OK*] [Cancel]
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
        chunks[7],
    );
}
