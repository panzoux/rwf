//! Context menu dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crate::ui::smart_truncate;

pub(super) fn render_context_menu_dialog(
    frame: &mut Frame,
    area: Rect,
    options: &[rwf_lib::model::dialog::ContextMenuOption],
    selected_index: usize,
) {
    use rwf_lib::model::dialog::ContextMenuAction;

    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let sep_style = crate::ui::dialog::common::DIALOG_DIM;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let item_width = area.width.saturating_sub(4) as usize;

    // hint on last row
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("[Enter] Select  [Esc] Cancel").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    let list_height = area.height.saturating_sub(1) as usize;
    // compute scroll so selected item is visible
    let scroll_start = if selected_index >= list_height {
        selected_index + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let oi = scroll_start + row;
        if oi >= options.len() {
            break;
        }
        let opt = &options[oi];
        let is_sep = matches!(opt.action, ContextMenuAction::Separator);
        if is_sep {
            let sep_text = "─".repeat(item_width.saturating_sub(2));
            frame.render_widget(
                Paragraph::new(format!(" {}", sep_text)).style(sep_style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        } else {
            let label = smart_truncate(&opt.label, item_width.saturating_sub(2), "…");
            let style = if oi == selected_index {
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
}
