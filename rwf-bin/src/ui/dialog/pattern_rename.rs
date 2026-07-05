//! Pattern rename dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::smart_truncate;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_pattern_rename_dialog(
    frame: &mut Frame,
    area: Rect,
    find: &str,
    find_cursor_pos: usize,
    find_scroll_pos: usize,
    replace: &str,
    replace_cursor_pos: usize,
    replace_scroll_pos: usize,
    use_regex: bool,
    case_sensitive: bool,
    preview: &[(String, String)],
    focused_field: usize,
    preview_scroll: usize,
    preview_horizontal_scroll: usize,
    error_message: Option<&str>,
    preview_mode: u8,
    show_all: bool,
) {
    let base = crate::ui::dialog::common::DIALOG_TEXT;
    let hint = crate::ui::dialog::common::DIALOG_DIM;
    let active = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let w = area.width as usize;

    // Helper: render one labeled textbox row
    let render_textbox = |frame: &mut Frame,
                          y: u16,
                          label: &str,
                          text: &str,
                          cursor: usize,
                          scroll: usize,
                          focused: bool| {
        let label_len = label.len() as u16;
        let tw = area.width.saturating_sub(label_len + 2) as usize;
        frame.render_widget(
            Paragraph::new(label.to_string()).style(base),
            Rect::new(area.x, y, label_len, 1),
        );
        let visible: String = text.chars().skip(scroll).take(tw).collect();
        let input_style = if focused {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            crate::ui::dialog::common::DIALOG_SELECTED
        };
        frame.render_widget(
            Paragraph::new(visible).style(input_style),
            Rect::new(area.x + label_len, y, tw as u16, 1),
        );
        if focused {
            let cx = area.x + label_len + cursor.saturating_sub(scroll) as u16;
            frame.set_cursor_position((cx.min(area.x + area.width.saturating_sub(1)), y));
        }
    };

    // Row 0: Find field
    render_textbox(
        frame,
        area.y,
        "Find:    ",
        find,
        find_cursor_pos,
        find_scroll_pos,
        focused_field == 0,
    );

    // Row 1: Replace field
    render_textbox(
        frame,
        area.y + 1,
        "Replace: ",
        replace,
        replace_cursor_pos,
        replace_scroll_pos,
        focused_field == 1,
    );

    // Row 2: regex/case flags + expert syntax hint
    let regex_mark = if use_regex { "[●]" } else { "[○]" };
    let case_mark = if case_sensitive { "[●]" } else { "[○]" };
    let flags_line = format!(
        "{} Regex (Alt+R) {} Case (Alt+S) | s/find/repl/[gi] tr/from/to/",
        regex_mark, case_mark
    );
    frame.render_widget(
        Paragraph::new(smart_truncate(&flags_line, w.saturating_sub(1), "…")).style(hint),
        Rect::new(area.x, area.y + 2, area.width, 1),
    );

    // Row 3: preview-mode selector + filter selector
    {
        let modes = ["SIDE-BY-SIDE", "Preview", "Original"];
        let mut spans: Vec<Span> = Vec::new();
        for (i, &name) in modes.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", hint));
            }
            if preview_mode == i as u8 {
                spans.push(Span::styled(format!("[{}]", name), active));
            } else {
                spans.push(Span::styled(name.to_string(), hint));
            }
        }
        spans.push(Span::styled("  (Alt+P)", hint));
        spans.push(Span::styled("   Filter: ", hint));
        if !show_all {
            spans.push(Span::styled("[MATCHES]", active));
            spans.push(Span::styled("  ", hint));
            spans.push(Span::styled("All", hint));
        } else {
            spans.push(Span::styled("MATCHES", hint));
            spans.push(Span::styled("  ", hint));
            spans.push(Span::styled("[All]", active));
        }
        spans.push(Span::styled("  (Alt+A)", hint));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(hint),
            Rect::new(area.x, area.y + 3, area.width, 1),
        );
    }

    // Row 4: horizontal separator — highlighted when filelist (focused_field==2) has focus
    let (sep_style, sep_line) = if focused_field == 2 {
        let dashes: String = std::iter::repeat_n('─', w.saturating_sub(9)).collect();
        (
            crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD),
            format!("▶ LIST {}", dashes),
        )
    } else {
        (hint, std::iter::repeat_n('─', w).collect())
    };
    frame.render_widget(
        Paragraph::new(sep_line).style(sep_style),
        Rect::new(area.x, area.y + 4, area.width, 1),
    );

    // Rows 5..area.height-1: filtered preview list
    let preview_area_h = area.height.saturating_sub(6); // find+replace+flags+mode-row+separator+status
    let filtered: Vec<&(String, String)> = if show_all {
        preview.iter().collect()
    } else {
        preview.iter().filter(|(a, b)| a != b).collect()
    };
    let max_scroll = filtered.len().saturating_sub(preview_area_h as usize);
    let effective_scroll = preview_scroll.min(max_scroll);
    let col_w = w.saturating_sub(5) / 2; // 2=indicator+space, 3=" ║ "

    for (i, (original, renamed)) in filtered
        .iter()
        .skip(effective_scroll)
        .take(preview_area_h as usize)
        .enumerate()
    {
        let y = area.y + 5 + i as u16;
        if y >= area.y + area.height.saturating_sub(1) {
            break;
        }
        let changed = original != renamed;
        let indicator = if changed { "√" } else { "╴" };
        // Horizontal scroll applied to content only; indicator and ║ stay fixed
        let line = match preview_mode {
            0 => {
                // Side-by-side: scroll both columns independently, separator stays put
                let orig: String = original
                    .chars()
                    .skip(preview_horizontal_scroll)
                    .take(col_w)
                    .collect();
                let new_name: String = renamed
                    .chars()
                    .skip(preview_horizontal_scroll)
                    .take(col_w)
                    .collect();
                format!(
                    "{} {:<col_w$} ║ {}",
                    indicator,
                    orig,
                    new_name,
                    col_w = col_w
                )
            }
            1 => {
                let content_w = area.width.saturating_sub(2) as usize;
                let scrolled: String = renamed
                    .chars()
                    .skip(preview_horizontal_scroll)
                    .take(content_w)
                    .collect();
                format!("{} {}", indicator, scrolled)
            }
            _ => {
                let content_w = area.width.saturating_sub(2) as usize;
                let scrolled: String = original
                    .chars()
                    .skip(preview_horizontal_scroll)
                    .take(content_w)
                    .collect();
                format!("{} {}", indicator, scrolled)
            }
        };
        let row_style = if changed {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Gray).bg(Color::DarkGray)
        };
        frame.render_widget(
            Paragraph::new(line).style(row_style),
            Rect::new(area.x, y, area.width, 1),
        );
    }

    // Last row: error or status
    let status_y = area.y + area.height.saturating_sub(1);
    if let Some(err) = error_message {
        frame.render_widget(
            Paragraph::new(smart_truncate(err, w.saturating_sub(1), "…"))
                .style(Style::default().fg(Color::Red).bg(Color::DarkGray)),
            Rect::new(area.x, status_y, area.width, 1),
        );
    } else {
        let match_count = preview.iter().filter(|(a, b)| a != b).count();
        let status = format!(
            " Matches: {}  Alt+R/S: regex/case  Enter: OK  Esc: cancel",
            match_count
        );
        frame.render_widget(
            Paragraph::new(smart_truncate(&status, w.saturating_sub(1), "…")).style(hint),
            Rect::new(area.x, status_y, area.width, 1),
        );
    }
}
