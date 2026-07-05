//! Registered folder selector rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use crate::ui::smart_truncate;

pub(super) fn render_registered_folder_selector(
    frame: &mut Frame,
    area: Rect,
    folders: &[rwf_lib::model::dialog::RegisteredFolder],
    selected_index: usize,
    filter: &str,
) {
    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);

    let item_width = area.width.saturating_sub(4) as usize;

    // Compute filtered list
    let filtered: Vec<&rwf_lib::model::dialog::RegisteredFolder> = if filter.is_empty() {
        folders.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        folders
            .iter()
            .filter(|f| {
                f.name.to_lowercase().contains(&lower) || f.path.to_lowercase().contains(&lower)
            })
            .collect()
    };

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    // Hint line (second-to-last row) and search line (last row)
    let hint_y = area.y + area.height.saturating_sub(2);
    let search_y = area.y + area.height.saturating_sub(1);

    frame.render_widget(
        Paragraph::new("[Enter] Jump to folder [Delete] Remove selected [Esc] Cancel")
            .style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("/{}", filter)).style(base_style),
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
        let folder = filtered[fi];
        let label = if folder.name.is_empty() {
            smart_truncate(&folder.path, item_width.saturating_sub(2), "…")
        } else {
            smart_truncate(
                &format!("{} — {}", folder.name, folder.path),
                item_width.saturating_sub(2),
                "…",
            )
        };
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
