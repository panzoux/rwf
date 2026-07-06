//! Drive selection dialog rendering and input handling.
//!
//! Rendering split from dialog/mod.rs in M3 (move-only; snapshot-protected).
//! Input handling moved from dialog/mod.rs in M4 S5.

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crossterm::event::{KeyEvent, KeyModifiers};
use rwf_lib::model::dialog::DriveSelectionDialog;

use crate::ui::smart_truncate;

use super::DialogAction;

/// Handle key input: incremental search + arrow navigation.
pub(super) fn handle_input(dialog: &mut DriveSelectionDialog, key: KeyEvent) -> DialogAction {
    let DriveSelectionDialog {
        drives,
        selected_index,
        filter,
    } = dialog;
    use crossterm::event::KeyCode;
    let filtered_count = if filter.is_empty() {
        drives.len()
    } else {
        let lower = filter.to_lowercase();
        drives
            .iter()
            .filter(|d| {
                d.display_label().to_lowercase().contains(&lower)
                    || d.path.to_lowercase().contains(&lower)
            })
            .count()
    };
    match key.code {
        KeyCode::Esc => return DialogAction::Cancel,
        KeyCode::Enter => return DialogAction::Confirm,
        KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
            if *selected_index > 0 {
                *selected_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
            if *selected_index + 1 < filtered_count {
                *selected_index += 1;
            }
        }
        KeyCode::Home => {
            *selected_index = 0;
        }
        KeyCode::End => {
            *selected_index = filtered_count.saturating_sub(1);
        }
        KeyCode::Backspace => {
            if !filter.is_empty() {
                let mut chars = filter.chars();
                chars.next_back();
                *filter = chars.as_str().to_string();
                *selected_index = 0;
            }
        }
        // Ctrl+K: clear search (also handle raw \x0b from Windows Console API)
        // Do NOT reset selected_index — cursor stays on current item.
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            filter.clear();
        }
        KeyCode::Char('\x0b') => {
            filter.clear();
        }
        // Printable chars: add to search filter (reset to top for new search)
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
                && !key.modifiers.contains(KeyModifiers::SUPER) =>
        {
            filter.push(c);
            *selected_index = 0;
        }
        _ => {}
    }
    DialogAction::None
}

pub(super) fn render_drive_selection_dialog(
    frame: &mut Frame,
    area: Rect,
    drives: &[rwf_lib::model::dialog::DriveInfo],
    selected_index: usize,
    filter: &str,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;
    let search_style = crate::ui::dialog::common::DIALOG_TEXT;

    let item_width = area.width.saturating_sub(4) as usize;

    // Compute filtered list
    let filtered: Vec<&rwf_lib::model::dialog::DriveInfo> = if filter.is_empty() {
        drives.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        drives
            .iter()
            .filter(|d| {
                d.display_label().to_lowercase().contains(&lower)
                    || d.path.to_lowercase().contains(&lower)
            })
            .collect()
    };

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    // Hint line (second-to-last row) and search line (last row)
    let hint_y = area.y + area.height.saturating_sub(2);
    let search_y = area.y + area.height.saturating_sub(1);

    frame.render_widget(
        Paragraph::new("Enter: go  Esc: cancel  ↑↓: select  Bksp: del char  ^K: clear")
            .style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("/{}", filter)).style(search_style),
        Rect::new(area.x + 2, search_y, item_width as u16, 1),
    );

    // List area (all rows except hint + search)
    let list_height = area.height.saturating_sub(2) as usize;
    let scroll_start = if clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let fi = scroll_start + row;
        if fi >= filtered.len() {
            break;
        }
        let drive = filtered[fi];
        let label = smart_truncate(&drive.display_label(), item_width.saturating_sub(2), "…");
        let style = if fi == clamped_sel {
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
