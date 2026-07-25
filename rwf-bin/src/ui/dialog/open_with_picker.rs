//! Open With picker rendering and input handling (Phase 7.3).
//!
//! Modeled on `custom_function.rs`'s `render_custom_function_menu` /
//! `handle_menu_input`, but simpler: `OpenWithPickerDialog` has no separators
//! to skip, so navigation is plain index bounds.

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rwf_lib::config::ExtensionAssociation;
use rwf_lib::model::dialog::OpenWithPickerDialog;

use super::DialogAction;
use crate::ui::smart_truncate;

/// Handle key input for the Open With picker: Up/Down/Home/End navigation.
pub(super) fn handle_input(dialog: &mut OpenWithPickerDialog, key: KeyEvent) -> DialogAction {
    let OpenWithPickerDialog {
        candidates,
        selected_index,
        ..
    } = dialog;
    match key.code {
        KeyCode::Esc => return DialogAction::Cancel,
        KeyCode::Enter => return DialogAction::Confirm,
        KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
            if *selected_index > 0 {
                *selected_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
            if *selected_index + 1 < candidates.len() {
                *selected_index += 1;
            }
        }
        KeyCode::Home => {
            *selected_index = 0;
        }
        KeyCode::End => {
            *selected_index = candidates.len().saturating_sub(1);
        }
        _ => {}
    }
    DialogAction::None
}

/// Label shown for a candidate row: its description if present, else its command.
pub(super) fn candidate_label(assoc: &ExtensionAssociation) -> &str {
    match &assoc.description {
        Some(desc) if !desc.is_empty() => desc,
        _ => &assoc.command,
    }
}

pub(super) fn render_open_with_picker(
    frame: &mut Frame,
    area: Rect,
    candidates: &[ExtensionAssociation],
    selected_index: usize,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let item_width = area.width.saturating_sub(4) as usize;

    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("[Enter] Open  [Esc] Cancel").style(hint_style),
        Rect::new(area.x + 1, hint_y, area.width.saturating_sub(2), 1),
    );

    let list_height = area.height.saturating_sub(1) as usize;
    let scroll_start = if selected_index >= list_height {
        selected_index + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let ci = scroll_start + row;
        if ci >= candidates.len() {
            break;
        }
        let label = smart_truncate(candidate_label(&candidates[ci]), item_width, "…");
        let style = if ci == selected_index {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}
