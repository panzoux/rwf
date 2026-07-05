//! Navigation history dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use crate::ui::smart_truncate;

pub(super) fn render_history_dialog(
    frame: &mut Frame,
    area: Rect,
    entries: &[rwf_lib::model::Location],
    selected_index: usize,
    current_pos: usize,
) {
    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let current_marker_style = Style::default()
        .fg(Color::Yellow)
        .bg(Color::Gray)
        .add_modifier(Modifier::BOLD);

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
