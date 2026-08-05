//! Trash browser dialog rendering and input handling (Phase 7.7 Task 16).

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crossterm::event::{KeyCode, KeyEvent};
use rwf_lib::model::dialog::TrashBrowserDialog;
use rwf_lib::model::format_size;

use crate::ui::smart_truncate;

use super::DialogAction;

/// Handle key input: Up/Down/Home/End navigate, Enter confirms restore of the
/// selected item, Esc cancels (handled generically by the caller).
pub(super) fn handle_input(dialog: &mut TrashBrowserDialog, key: KeyEvent) -> DialogAction {
    let TrashBrowserDialog {
        records,
        selected_index,
    } = dialog;
    match key.code {
        KeyCode::Esc => return DialogAction::Cancel,
        KeyCode::Enter if !records.is_empty() => return DialogAction::Confirm,
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected_index > 0 {
                *selected_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected_index + 1 < records.len() {
                *selected_index += 1;
            }
        }
        KeyCode::Home => *selected_index = 0,
        KeyCode::End => *selected_index = records.len().saturating_sub(1),
        _ => {}
    }
    DialogAction::None
}

fn format_deleted_at(unix_ts: i64) -> String {
    use chrono::{DateTime, Local, Utc};
    DateTime::<Utc>::from_timestamp(unix_ts, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "?".to_string())
}

fn record_original_path(record: &rwf_lib::model::TrashRecord) -> String {
    record.original.display_path()
}

pub(super) fn render_trash_browser_dialog(
    frame: &mut Frame,
    area: Rect,
    records: &[rwf_lib::model::TrashRecord],
    selected_index: usize,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let item_width = area.width.saturating_sub(4) as usize;
    let clamped_sel = selected_index.min(records.len().saturating_sub(1));

    // Row text truncates the path (deleted-time + size prefix leaves limited room), so the
    // selected item's full restore destination is spelled out here instead — this is where it
    // will actually land, and it's the one thing a truncated row can hide.
    let dest_y = area.y + area.height.saturating_sub(2);
    if let Some(selected) = records.get(clamped_sel) {
        let dest = smart_truncate(
            &record_original_path(selected),
            item_width.saturating_sub("Restore to: ".len()),
            "…",
        );
        frame.render_widget(
            Paragraph::new(format!("Restore to: {dest}")).style(hint_style),
            Rect::new(area.x + 2, dest_y, item_width as u16, 1),
        );
    }

    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("Enter: restore  Esc: close  ↑↓: select").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    let list_height = area.height.saturating_sub(2) as usize;
    let scroll_start = if clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let idx = scroll_start + row;
        if idx >= records.len() {
            break;
        }
        let record = &records[idx];
        let deleted = record
            .trash_location
            .deleted_at()
            .map(format_deleted_at)
            .unwrap_or_else(|| "?".to_string());
        let size = format_size(record.size);
        let prefix = format!("{deleted}  {size:>10}  ");
        let path_width = item_width.saturating_sub(prefix.len() + 1);
        let path = smart_truncate(&record_original_path(record), path_width, "…");
        let style = if idx == clamped_sel {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(format!(" {prefix}{path}")).style(style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}
