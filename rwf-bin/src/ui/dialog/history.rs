//! Navigation history dialog rendering and input handling.
//!
//! Rendering split from dialog/mod.rs in M3 (move-only; snapshot-protected).
//! Input handling moved from dialog/mod.rs in M4 S5.

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crate::ui::smart_truncate;

use crossterm::event::KeyEvent;
use rwf_lib::model::dialog::{Dialog, DialogContent, HistoryDialogContent};

use super::DialogAction;

/// Handle key input: Up/Down/j/k navigate, Tab/Left/Right/h/l switch pane,
/// Enter jumps, Esc cancels. Takes the whole `Dialog` (not just its content)
/// because pane switching also updates the dialog's title.
pub(super) fn handle_input(dialog: &mut Dialog, key: KeyEvent) -> DialogAction {
    use crossterm::event::KeyCode;
    use rwf_lib::model::ui::ActivePane;

    // ── Pane switch (Tab, Left arrow, Right arrow, h, l) ──────────────
    let switch_to: Option<ActivePane> = match key.code {
        KeyCode::Tab => {
            let cur =
                if let DialogContent::HistoryDialog(HistoryDialogContent { active_pane, .. }) =
                    &dialog.content
                {
                    *active_pane
                } else {
                    unreachable!()
                };
            Some(match cur {
                ActivePane::Left => ActivePane::Right,
                ActivePane::Right => ActivePane::Left,
            })
        }
        KeyCode::Left | KeyCode::Char('h') => Some(ActivePane::Left),
        KeyCode::Right | KeyCode::Char('l') => Some(ActivePane::Right),
        _ => None,
    };

    if let Some(new_pane) = switch_to {
        // update content
        if let DialogContent::HistoryDialog(HistoryDialogContent { active_pane, .. }) =
            &mut dialog.content
        {
            *active_pane = new_pane;
        }
        // update title separately (no borrow conflict — different fields)
        let pane_label = match new_pane {
            ActivePane::Left => "Left",
            ActivePane::Right => "Right",
        };
        if let Some(bar) = dialog.title.rfind('|') {
            let prefix = dialog.title[..bar].to_string();
            dialog.title = format!("{}| {}]", prefix, pane_label);
        }
        return DialogAction::None;
    }

    // ── Cursor navigation ──────────────────────────────────────────────
    if let DialogContent::HistoryDialog(HistoryDialogContent {
        left_entries,
        right_entries,
        left_selected,
        right_selected,
        active_pane,
        ..
    }) = &mut dialog.content
    {
        let (sel, total) = match active_pane {
            ActivePane::Left => (left_selected, left_entries.len()),
            ActivePane::Right => (right_selected, right_entries.len()),
        };
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') => {
                if *sel + 1 < total {
                    *sel += 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *sel > 0 {
                    *sel -= 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                *sel = total.saturating_sub(1);
            }
            KeyCode::End | KeyCode::Char('G') => {
                *sel = 0;
            }
            _ => {}
        }
    }
    DialogAction::None
}

pub(super) fn render_history_dialog(
    frame: &mut Frame,
    area: Rect,
    entries: &[rwf_lib::model::Location],
    selected_index: usize,
    current_pos: usize,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let current_marker_style =
        crate::ui::dialog::common::DIALOG_ACCENT_YELLOW.add_modifier(Modifier::BOLD);

    let item_width = area.width.saturating_sub(4) as usize;

    // Hint line at the bottom
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("Enter: jump  Esc: cancel  ↑↓: navigate").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    // Entries (oldest at bottom, newest at top — reversed display)
    // visible_area: all rows except the last hint line
    let list_height = area.height.saturating_sub(1) as usize;
    let total = entries.len();

    // Compute scroll window so selected_index stays visible (reversed display)
    // Display index = total - 1 - entry_index (newest at row 0)
    let display_selected = total.saturating_sub(1).saturating_sub(selected_index);
    let scroll_start = if display_selected >= list_height {
        display_selected + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let display_idx = scroll_start + row;
        if display_idx >= total {
            break;
        }
        // Convert display index back to stack index (reversed)
        let entry_idx = total - 1 - display_idx;
        let entry = &entries[entry_idx];
        let path_str = smart_truncate(&entry.display_path(), item_width.saturating_sub(3), "…");

        let (prefix, row_style) = if entry_idx == selected_index {
            (">", selected_style)
        } else if entry_idx == current_pos {
            ("*", current_marker_style)
        } else {
            (" ", base_style)
        };

        let line = format!("{} {}", prefix, path_str);
        frame.render_widget(
            Paragraph::new(line).style(row_style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}
