//! Jump to Directory dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crate::ui::smart_truncate;

use super::chunk_path_preview;
use super::common::{DIALOG_DIM, DIALOG_INPUT, DIALOG_SELECTED, DIALOG_TEXT};

pub(super) fn render_jump_to_path_dialog(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    cursor_pos: usize,
    suggestions: &[String],
    selected_index: usize,
    is_loading: bool,
) {
    let base_style = DIALOG_TEXT;
    let selected_style = DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = DIALOG_DIM;
    let input_style = DIALOG_INPUT;
    let sep_style = DIALOG_DIM;
    let preview_style = DIALOG_INPUT;

    let clamped_sel = if suggestions.is_empty() {
        0
    } else {
        selected_index.min(suggestions.len() - 1)
    };
    let item_width = area.width.saturating_sub(4) as usize;

    // ── Row 0: input field + hit count ────────────────────────────────────
    let status = if is_loading {
        if suggestions.is_empty() {
            "searching…".to_string()
        } else {
            format!("{}+ hits", suggestions.len())
        }
    } else if suggestions.is_empty() {
        "No match".to_string()
    } else {
        format!("{} hits", suggestions.len())
    };
    let status_width: u16 = 10;
    let input_width = area.width.saturating_sub(status_width + 3).max(4);
    // Horizontal scroll: show the end of the query when cursor is near it
    let q_chars: Vec<char> = query.chars().collect();
    let visible_chars = input_width as usize;
    let scroll = cursor_pos.saturating_sub(visible_chars);
    let visible_query: String = q_chars.iter().skip(scroll).take(visible_chars).collect();
    let input_text = format!("{:<width$}", visible_query, width = visible_chars);
    frame.render_widget(
        Paragraph::new(input_text).style(input_style),
        Rect::new(area.x + 1, area.y, input_width, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("{:>width$}", status, width = status_width as usize))
            .style(base_style),
        Rect::new(area.x + 1 + input_width, area.y, status_width, 1),
    );

    // ── Row 1: separator ──────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new("─".repeat(item_width)).style(sep_style),
        Rect::new(area.x + 2, area.y + 1, item_width as u16, 1),
    );

    // ── Rows 2..height-6: suggestion list ────────────────────────────────
    // Footer = sep(1) + preview(4) + hint(1) = 6 rows
    let header_rows: u16 = 2;
    let footer_rows: u16 = 6;
    let list_height = area.height.saturating_sub(header_rows + footer_rows) as usize;
    let scroll_start = if list_height > 0 && clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };
    for row in 0..list_height {
        let si = scroll_start + row;
        if si >= suggestions.len() {
            break;
        }
        let path = &suggestions[si];
        let label = smart_truncate(path, item_width.saturating_sub(2), "…");
        let style = if si == clamped_sel {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(
                area.x + 2,
                area.y + header_rows + row as u16,
                item_width as u16,
                1,
            ),
        );
    }

    // ── Row height-6: separator before preview ────────────────────────────
    let sep2_y = area.y + area.height.saturating_sub(6);
    frame.render_widget(
        Paragraph::new("─".repeat(item_width)).style(sep_style),
        Rect::new(area.x + 2, sep2_y, item_width as u16, 1),
    );

    // ── Rows height-5..height-2: full-path preview (4 lines, char-chunked) ─
    let preview_y = area.y + area.height.saturating_sub(5);
    let preview_w = area.width.saturating_sub(2);
    let preview_lines = if !suggestions.is_empty() && clamped_sel < suggestions.len() {
        let raw = &suggestions[clamped_sel];
        let text = if raw.len() > 1024 {
            &raw[..1024]
        } else {
            raw.as_str()
        };
        chunk_path_preview(text, preview_w, 4)
    } else {
        vec![]
    };
    frame.render_widget(
        Paragraph::new(preview_lines).style(preview_style),
        Rect::new(area.x + 1, preview_y, preview_w, 4),
    );

    // ── Row height-1: hint ────────────────────────────────────────────────
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("↑↓:select  Enter:jump  Esc:cancel  Bksp:del  ^K:clear").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    // ── Cursor rendering on input row ─────────────────────────────────────
    let cursor_x_in_visible = cursor_pos.saturating_sub(scroll);
    let cursor_screen_x = (area.x + 1 + cursor_x_in_visible as u16).min(area.x + input_width);
    frame.set_cursor_position((cursor_screen_x, area.y));
}
