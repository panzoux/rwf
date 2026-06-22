//! LEAP bar — rendered in place of the pane summary line while in UIMode::Leap.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use rwf_lib::config::NoMatchFeedback;
use rwf_lib::model::{FileEntry, LeapState};

const LABEL:        &str = "LEAP";
const SCROLL_IND:   &str = "◂";
const NO_MATCH_STR: &str = " (no match)";
// Cursor block: one styled space shown at the typing position
const CURSOR_BLOCK: &str = " ";

const COL_LABEL:     Color = Color::Rgb(243, 139, 168); // catppuccin red
const COL_TRAIL:     Color = Color::Rgb(88,  91,  112); // dim gray
const COL_SEP:       Color = Color::Rgb(108, 112, 134); // mid gray
const COL_LOCAL:     Color = Color::Rgb(249, 226, 175); // yellow
const COL_CURSOR_FG: Color = Color::Rgb(30,  30,  46);  // dark bg (cursor text)
const COL_NO_MATCH:  Color = Color::Rgb(108, 112, 134); // mid gray

/// Render the LEAP bar into `area`.
pub fn render_leap_bar(
    frame: &mut Frame,
    area: Rect,
    leap: &LeapState,
    visible_entries: &[FileEntry],
    no_match_feedback: &NoMatchFeedback,
) {
    let width = area.width as usize;
    if width < 8 { return; }

    // "LEAP " = 5 chars
    let label_width = LABEL.len() + 1;

    // Right anchor: "(no match)" when Inline feedback and empty result
    let right_str: Option<String> = match no_match_feedback {
        NoMatchFeedback::Inline if visible_entries.is_empty()
            && !leap.local_filter().is_empty() => Some(NO_MATCH_STR.to_string()),
        _ => None,
    };
    let right_width = right_str.as_ref().map(|s| s.chars().count()).unwrap_or(0);

    // Scrollable zone: space left for trail+local+cursor
    let scroll_zone = width.saturating_sub(label_width + right_width + 1); // +1 for scroll indicator slot

    // Build visible buffer content: trail + local_filter + cursor block
    let trail   = leap.trail();
    let local   = leap.local_filter();
    let full_buf: String = format!("{}{}{}", trail, local, CURSOR_BLOCK);
    let full_len = full_buf.chars().count();

    let (scrolled, visible_start) = if full_len > scroll_zone {
        (true, full_len - scroll_zone)
    } else {
        (false, 0)
    };

    // Build spans
    let label_style   = Style::default().fg(COL_LABEL).add_modifier(Modifier::BOLD);
    let trail_style   = Style::default().fg(COL_TRAIL);
    let sep_style     = Style::default().fg(COL_SEP);
    let local_style   = Style::default().fg(COL_LOCAL);
    let cursor_style  = Style::default().fg(COL_CURSOR_FG).bg(COL_LOCAL);
    let no_match_style= Style::default().fg(COL_NO_MATCH);

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(format!("{} ", LABEL), label_style));

    if scrolled {
        spans.push(Span::styled(SCROLL_IND, trail_style));
    } else {
        spans.push(Span::raw(" ")); // keep alignment
    }

    // Render the visible portion of the buffer with per-character coloring.
    // We walk the full_buf and skip the first `visible_start` chars, then emit
    // styled spans based on position relative to trail boundary.
    let trail_len = trail.chars().count();
    let local_len = local.chars().count();
    // total = trail_len + local_len + 1 (cursor)

    let mut _char_idx = 0usize; // position in full_buf (used by future loop tracking)
    let mut trail_span = String::new();
    let mut sep_span   = String::new();
    let mut local_span = String::new();

    for (ci, c) in trail.char_indices().map(|(_, c)| c).enumerate() {
        if ci >= visible_start {
            if c == '/' {
                if !trail_span.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut trail_span), trail_style));
                }
                if !sep_span.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut sep_span), sep_style));
                }
                sep_span.push(c);
            } else {
                if !sep_span.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut sep_span), sep_style));
                }
                trail_span.push(c);
            }
        }
        _char_idx += 1;
    }
    if !trail_span.is_empty() { spans.push(Span::styled(trail_span, trail_style)); }
    if !sep_span.is_empty()   { spans.push(Span::styled(sep_span, sep_style)); }

    let local_start = trail_len;
    for (ci, c) in local.chars().enumerate() {
        let global_idx = local_start + ci;
        if global_idx >= visible_start {
            local_span.push(c);
        }
        _char_idx += 1;
    }
    if !local_span.is_empty() { spans.push(Span::styled(local_span, local_style)); }

    // Cursor block
    let cursor_start = trail_len + local_len;
    if cursor_start >= visible_start {
        spans.push(Span::styled(CURSOR_BLOCK, cursor_style));
    }

    // Right-anchor text (no match)
    if let Some(ref nm) = right_str {
        spans.push(Span::styled(nm.clone(), no_match_style));
    }

    let line = Line::from(spans);
    let bg_color = Color::Rgb(24, 24, 37);
    let para = Paragraph::new(line)
        .style(Style::default().bg(bg_color));
    frame.render_widget(para, area);
}
