//! Custom function selector and menu rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crate::ui::smart_truncate;

pub(super) fn render_custom_function_selector(
    frame: &mut Frame,
    area: Rect,
    functions: &[rwf_lib::model::dialog::CustomFunction],
    selected_index: usize,
    filter: &str,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let item_width = area.width.saturating_sub(4) as usize;

    let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
        functions.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        functions
            .iter()
            .filter(|f| {
                f.name.to_lowercase().contains(&lower)
                    || f.description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&lower)
            })
            .collect()
    };

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    let hint_y = area.y + area.height.saturating_sub(2);
    let search_y = area.y + area.height.saturating_sub(1);

    frame.render_widget(
        Paragraph::new("[Enter] Execute  [Esc] Cancel").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("/{}", filter)).style(base_style),
        Rect::new(area.x + 2, search_y, item_width as u16, 1),
    );

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
        let func = filtered[fi];
        let name_w = item_width.saturating_sub(2);
        let label = if let Some(desc) = &func.description {
            let desc_w = name_w.saturating_sub(func.name.len() + 3);
            if desc_w > 4 {
                format!(
                    "{:<name_w$}",
                    format!("{}  {}", func.name, smart_truncate(desc, desc_w, "…")),
                    name_w = name_w
                )
            } else {
                smart_truncate(&func.name, name_w, "…")
            }
        } else {
            smart_truncate(&func.name, name_w, "…")
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

pub(super) fn render_custom_function_menu(
    frame: &mut Frame,
    area: Rect,
    items: &[rwf_lib::model::dialog::MenuItem],
    selected_index: usize,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let sep_style = crate::ui::dialog::common::DIALOG_DIM;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    // item_width = inner width - 4 (2 left-indent + 2 right-margin for items)
    let item_width = area.width.saturating_sub(4) as usize;

    // Hint at offset+1 with full inner width-2 — avoids the 1-char right clip from offset+2
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("[Enter] Execute  [Esc] Close").style(hint_style),
        Rect::new(area.x + 1, hint_y, area.width.saturating_sub(2), 1),
    );

    let list_height = area.height.saturating_sub(1) as usize;
    let scroll_start = if selected_index >= list_height {
        selected_index + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let ii = scroll_start + row;
        if ii >= items.len() {
            break;
        }
        let item = &items[ii];
        if item.is_separator() {
            // Separator spans item_width
            let sep = "─".repeat(item_width.saturating_sub(1));
            frame.render_widget(
                Paragraph::new(sep).style(sep_style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        } else {
            // No truncation: dialog is sized to fit the longest label
            let style = if ii == selected_index {
                selected_style
            } else {
                base_style
            };
            frame.render_widget(
                Paragraph::new(format!(" {}", item.name)).style(style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        }
    }
}
