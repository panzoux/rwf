//! File viewer UI widget (text and hex mode)
//!
//! Windowed rendering: only the visible viewport lines are decoded per frame.
//! The line-offset index grows asynchronously; the status bar shows how many
//! lines have been indexed so far with a '+' suffix while still loading.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use rwf_lib::config::ColorScheme;
use rwf_lib::model::{Location, TextEncoding, UIMode, ViewerMode, ViewerState};
use unicode_width::UnicodeWidthChar;

use super::colors::parse_color;
use super::{pad_to_width, sanitize_for_display, smart_truncate};
use unicode_width::UnicodeWidthStr;

/// Render the file viewer.
/// `sbs_focused`: true when this is a SideBySide viewer that currently has
/// keyboard focus — shows `[Mode]` title brackets and focus border style.
#[allow(clippy::too_many_arguments)]
pub fn render_viewer(
    frame: &mut Frame,
    area: Rect,
    viewer: &ViewerState,
    colors: &ColorScheme,
    ui_mode: UIMode,
    search_input: &str,
    command_input: &str,
    sbs_focused: bool,
) {
    let fg = parse_color(&colors.text_viewer_foreground_color);
    let bg = parse_color(&colors.text_viewer_background_color);
    let status_fg = parse_color(&colors.text_viewer_status_foreground_color);
    let status_bg = parse_color(&colors.text_viewer_status_background_color);

    // ── Block title: mode label + position / encoding / column info ───────────
    // Brackets signal that this viewer panel has keyboard focus (SideBySide mode).
    let mode_label = match (viewer.mode, sbs_focused) {
        (ViewerMode::Text, false) => "Text Viewer",
        (ViewerMode::Hex, false) => "Hex Viewer",
        (ViewerMode::Text, true) => "[Text Viewer]",
        (ViewerMode::Hex, true) => "[Hex Viewer]",
    };

    // Search count + case label for the title (shown when results exist).
    let title_search = if let Some(ref aq) = viewer.address_query {
        // Address jump: show address + byte-match count if any.
        if !viewer.search_matches.is_empty() {
            let total = viewer.search_matches.len();
            let idx = viewer.search_match_index.map(|i| i + 1).unwrap_or(0);
            format!(" | @{} {}/{}", aq.trim(), idx, total)
        } else {
            format!(" | @{}", aq.trim())
        }
    } else if let Some(ref _q) = viewer.search_query {
        let total = viewer.search_matches.len();
        let case_label = if viewer.case_sensitive { "Aa" } else { "aA" };
        if total > 0 {
            let idx = viewer.search_match_index.map(|i| i + 1).unwrap_or(0);
            format!(" | {}/{} [{}]", idx, total, case_label)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // File size (always shown when buffer is available).
    let title_size = viewer
        .buffer
        .as_ref()
        .map(|b| format!(" | Size: {}", format_with_commas(b.total_bytes())))
        .unwrap_or_default();

    let title = match viewer.mode {
        ViewerMode::Text => {
            if let Some(ref buffer) = viewer.buffer {
                let (indexed, complete) = {
                    let idx = buffer
                        .line_index
                        .lock()
                        .expect("line_index mutex should not be poisoned");
                    (idx.offsets.len(), idx.is_complete)
                };
                let pos = if indexed == 0 {
                    0
                } else {
                    viewer.line_offset + 1
                };
                let suffix = if complete { "" } else { "+" };
                format!(
                    " {} | {}/{}{} | {} | Col:{}{}{} ",
                    mode_label,
                    pos,
                    indexed,
                    suffix,
                    viewer.encoding.name(),
                    viewer.column_offset + 1,
                    title_search,
                    title_size
                )
            } else {
                format!(" {} | Loading… ", mode_label)
            }
        }
        ViewerMode::Hex => {
            if viewer.buffer.is_none() {
                format!(" {} | Loading… ", mode_label)
            } else {
                let total_bytes = viewer.buffer.as_ref().map(|b| b.total_bytes()).unwrap_or(0);
                let byte_offset = viewer.line_offset * 16;
                format!(
                    " {} | 0x{:08X} / 0x{:08X} | {}{}{} ",
                    mode_label,
                    byte_offset,
                    total_bytes,
                    viewer.encoding.name(),
                    title_search,
                    title_size
                )
            }
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(fg).bg(bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    // ── Layout: content | filename-bar | hint-bar ─────────────────────────────
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let content_area = chunks[0];
    let filename_area = chunks[1];
    let hint_area = chunks[2];

    // ── Content ───────────────────────────────────────────────────────────────
    match viewer.mode {
        ViewerMode::Text => render_text_content(frame, content_area, viewer, fg, bg),
        ViewerMode::Hex => render_hex_content(frame, content_area, viewer, fg, bg),
    }

    // ── Filename bar: path (left) + search status (right) ────────────────────
    let filename = viewer.location.display_path();
    let right_str = search_bar_status(viewer);

    let bar_w = filename_area.width as usize;
    let right_len = right_str.width();
    // Reserve 1 leading space + space for right portion.
    let left_avail = bar_w.saturating_sub(right_len).saturating_sub(1);
    // smart_truncate: show beginning + end (keeps drive root AND filename/extension visible).
    let shortened = smart_truncate(&filename, left_avail, "…");
    let left_display = format!(" {}", pad_to_width(&shortened, left_avail));

    let search_fg = if viewer.search_query.is_some()
        && viewer.search_matches.is_empty()
        && viewer.address_query.is_none()
    {
        Color::Red
    } else {
        status_fg
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left_display, Style::default().fg(status_fg).bg(status_bg)),
            Span::styled(right_str, Style::default().fg(search_fg).bg(status_bg)),
        ])),
        filename_area,
    );

    // ── Hint bar: search input when active, key hints otherwise ──────────────
    match ui_mode {
        UIMode::ViewerSearch => {
            let prompt = if viewer.search_forward { '/' } else { '?' };
            let case_label = if viewer.case_sensitive {
                "[Aa]"
            } else {
                "[aA]"
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!(" {} {}{}", case_label, prompt, search_input),
                        Style::default().fg(Color::Yellow).bg(bg),
                    ),
                    Span::styled(
                        "█",
                        Style::default()
                            .fg(Color::Yellow)
                            .bg(bg)
                            .add_modifier(Modifier::SLOW_BLINK),
                    ),
                ])),
                hint_area,
            );
        }
        UIMode::ViewerCommand => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!(" :{}", command_input),
                        Style::default().fg(Color::Yellow).bg(bg),
                    ),
                    Span::styled(
                        "█",
                        Style::default()
                            .fg(Color::Yellow)
                            .bg(bg)
                            .add_modifier(Modifier::SLOW_BLINK),
                    ),
                    Span::styled(
                        "  g/<:First  G/>:Last  Esc:Cancel",
                        Style::default().fg(Color::DarkGray).bg(bg),
                    ),
                ])),
                hint_area,
            );
        }
        _ if viewer.search_query.is_some() => {
            frame.render_widget(
                Paragraph::new(
                    " n:Next  N:Prev  Ctrl+^:Case  /:Fwd  ?:Back  Ctrl+U:ClearHL  Esc:Close",
                )
                .style(Style::default().fg(Color::DarkGray).bg(bg)),
                hint_area,
            );
        }
        _ => {
            let hint = match viewer.mode {
                ViewerMode::Text =>
                    " b:Hex/Text  e:Encoding  ←/→:Scroll  Shift+←/→/↑/↓:×10  Ctrl+F/Space:PgDn  Ctrl+B:PgUp  /:Search  Ctrl+U:ClearHL  Esc:Close",
                ViewerMode::Hex =>
                    " b:Hex/Text  e:Encoding  Shift+↑/↓:×10  PgDn/PgUp  Ctrl+F/Space:PgDn  Ctrl+B:PgUp  /:Search(hex/addr/text)  Ctrl+U:ClearHL  Esc:Close",
            };
            frame.render_widget(
                Paragraph::new(hint).style(Style::default().fg(Color::DarkGray).bg(bg)),
                hint_area,
            );
        }
    }
}

/// Build the right-side string for the filename bar.
/// Only shows "not found" feedback; count and size live in the block title.
fn search_bar_status(viewer: &ViewerState) -> String {
    if viewer.is_searching {
        return " (searching...) ".to_string();
    }
    // Address jumps have no byte matches; don't flag them as "not found".
    if viewer.search_matches.is_empty() && viewer.address_query.is_none() {
        if let Some(ref q) = viewer.search_query {
            let case_label = if viewer.case_sensitive { "Aa" } else { "aA" };
            return format!(" {} [{}] not found ", q, case_label);
        }
    }
    String::new()
}

fn format_with_commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

// ── Text rendering ────────────────────────────────────────────────────────────

fn render_text_content(frame: &mut Frame, area: Rect, viewer: &ViewerState, fg: Color, bg: Color) {
    let viewport_height = area.height as usize;
    let viewport_width = area.width as usize;

    let buffer = match viewer.buffer.as_ref() {
        Some(b) => b,
        None => {
            frame.render_widget(
                Paragraph::new("(Loading…)").style(Style::default().fg(Color::DarkGray).bg(bg)),
                area,
            );
            return;
        }
    };

    // Snapshot the index once per frame so we hold the lock minimally.
    let (indexed_count, _complete) = {
        let idx = buffer
            .line_index
            .lock()
            .expect("line_index mutex should not be poisoned");
        (idx.offsets.len(), idx.is_complete)
    };

    if indexed_count == 0 {
        frame.render_widget(
            Paragraph::new("(Loading…)").style(Style::default().fg(Color::DarkGray).bg(bg)),
            area,
        );
        return;
    }

    let num_digits = if indexed_count >= 10000 {
        5
    } else if indexed_count >= 1000 {
        4
    } else {
        3
    };
    let prefix_width = num_digits + 3; // " NNN | "
    let content_width = viewport_width.saturating_sub(prefix_width);

    let mut rendered: Vec<Line> = Vec::with_capacity(viewport_height);

    for i in 0..viewport_height {
        let line_idx = viewer.line_offset + i;
        if line_idx >= indexed_count {
            rendered.push(Line::from(Span::styled(
                " ".repeat(viewport_width),
                Style::default().fg(fg).bg(bg),
            )));
            continue;
        }

        // Decode only this single line from the mmap — no full-file copy.
        let raw = match viewer.get_line_bytes(line_idx) {
            Some(b) => b,
            None => {
                rendered.push(Line::from(Span::raw("")));
                continue;
            }
        };
        // Decode then sanitize: control chars become Unicode control-picture
        // symbols so they can't corrupt the terminal or break width accounting.
        let decoded = sanitize_for_display(&viewer.encoding.decode(&raw));

        let num_str = format!("{:>width$}", line_idx + 1, width = num_digits);
        let prefix = format!(" {} | ", num_str);

        // Collect match ranges for this line: (byte_start, byte_end) in decoded string.
        let line_match_ranges: Vec<(usize, usize)> = viewer
            .search_matches
            .iter()
            .filter(|&&(l, _, _)| l == line_idx)
            .map(|&(_, s, e)| (s, e))
            .collect();

        let has_match = !line_match_ranges.is_empty();
        let num_style = if has_match {
            Style::default().fg(Color::Yellow).bg(bg)
        } else {
            Style::default().fg(Color::DarkGray).bg(bg)
        };

        let normal_style = Style::default().fg(fg).bg(bg);
        let match_style = Style::default().fg(Color::Black).bg(Color::Yellow);
        let current_style = Style::default()
            .fg(Color::Black)
            .bg(Color::LightYellow)
            .add_modifier(Modifier::BOLD);

        // Byte range of the active match on this line, if any.
        let current_match_range: Option<(usize, usize)> = viewer
            .search_match_index
            .and_then(|mi| viewer.search_matches.get(mi))
            .and_then(|&(ml, ms, me)| if ml == line_idx { Some((ms, me)) } else { None });

        let content_spans = highlight_spans(
            &decoded,
            &line_match_ranges,
            current_match_range,
            viewer.column_offset,
            content_width,
            normal_style,
            match_style,
            current_style,
        );

        let mut line_spans = vec![Span::styled(prefix, num_style)];
        line_spans.extend(content_spans);
        rendered.push(Line::from(line_spans));
    }

    frame.render_widget(
        Paragraph::new(rendered).style(Style::default().fg(fg).bg(bg)),
        area,
    );
}

// ── Hex rendering ─────────────────────────────────────────────────────────────

fn render_hex_content(frame: &mut Frame, area: Rect, viewer: &ViewerState, fg: Color, bg: Color) {
    if viewer.buffer.is_none() {
        frame.render_widget(
            Paragraph::new("(Loading…)").style(Style::default().fg(Color::DarkGray).bg(bg)),
            area,
        );
        return;
    }

    let addr_style = Style::default().fg(Color::DarkGray).bg(bg);
    let hex_style = Style::default().fg(fg).bg(bg);
    let ascii_style = Style::default().fg(Color::Cyan).bg(bg);
    let sep_style = Style::default().fg(Color::DarkGray).bg(bg);
    let hex_match_style = Style::default().fg(Color::Black).bg(Color::Yellow);
    let ascii_match_style = Style::default().fg(Color::Black).bg(Color::Yellow);
    let hex_current_style = Style::default()
        .fg(Color::Black)
        .bg(Color::LightYellow)
        .add_modifier(Modifier::BOLD);
    let ascii_current_style = Style::default()
        .fg(Color::Black)
        .bg(Color::LightYellow)
        .add_modifier(Modifier::BOLD);

    // ── Column header (fixed, does not scroll) ────────────────────────────────
    // Reserve the first row for the header; the rest is the scrollable data area.
    let (header_area, data_area) = if area.height > 1 {
        (
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height - 1,
            },
        )
    } else {
        (Rect { width: 0, ..area }, area) // no room for header
    };

    if header_area.width > 0 {
        let hdr_style = Style::default()
            .fg(Color::White)
            .bg(bg)
            .add_modifier(Modifier::DIM);
        // Hex column labels: "+0 " × 8, extra space, "+8 " × 8  → 49 chars total
        let hex_cols_1: String = (0u8..8).map(|i| format!("+{:X} ", i)).collect();
        let hex_cols_2: String = (8u8..16).map(|i| format!("+{:X} ", i)).collect();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Offset  ", hdr_style), // 10 chars — aligns with address
                Span::styled(hex_cols_1, hdr_style),   // 24 chars
                Span::styled(" ", hdr_style),          //  1 char  — mid-gap
                Span::styled(hex_cols_2, hdr_style),   // 24 chars
                Span::styled(" |", sep_style),         //  2 chars
                Span::styled("0123456789ABCDEF", ascii_style), // 16 chars
                Span::styled("|", sep_style),
            ])),
            header_area,
        );
    }

    let viewport_height = data_area.height as usize;

    // Current match byte range (file offsets).
    let current_match_bytes: Option<(usize, usize)> = viewer
        .search_match_index
        .and_then(|mi| viewer.search_matches.get(mi))
        .map(|&(_, s, e)| (s, e));

    let mut rendered: Vec<Line> = Vec::with_capacity(viewport_height);
    for i in 0..viewport_height {
        let line_idx = viewer.line_offset + i;
        if let Some((offset, bytes)) = viewer.get_hex_bytes_vec(line_idx) {
            // Collect match byte ranges that overlap this row.
            let row_end = offset + bytes.len();
            let row_matches: Vec<(usize, usize)> = viewer
                .search_matches
                .iter()
                .map(|&(_, s, e)| (s, e))
                .filter(|&(s, e)| s < row_end && e > offset)
                .collect();

            let addr_highlight = viewer.address_query.as_deref().and_then(|aq| {
                if line_idx == viewer.line_offset {
                    compute_addr_highlight(aq, offset)
                } else {
                    None
                }
            });
            rendered.push(hex_row_spans(
                offset,
                &bytes,
                &row_matches,
                current_match_bytes,
                addr_style,
                hex_style,
                hex_match_style,
                hex_current_style,
                ascii_style,
                ascii_match_style,
                ascii_current_style,
                sep_style,
                viewer.encoding,
                addr_highlight,
            ));
        } else {
            rendered.push(Line::from(Span::styled("", hex_style)));
        }
    }

    frame.render_widget(
        Paragraph::new(rendered).style(Style::default().fg(fg).bg(bg)),
        data_area,
    );
}

// ── Hex row rendering with per-byte highlighting ──────────────────────────────

/// Find the highlight range (start, end) within the 8-char address display string
/// for an address-jump search. Strips "0x"/"0X" prefix, uppercases, then finds the
/// resulting substring inside `format!("{:08X}", offset)`.
fn compute_addr_highlight(query: &str, offset: usize) -> Option<(usize, usize)> {
    let q = query.trim();
    let q = q
        .strip_prefix("0x")
        .or_else(|| q.strip_prefix("0X"))
        .unwrap_or(q);
    if q.is_empty() {
        return None;
    }
    let q_upper = q.to_ascii_uppercase();
    let addr_str = format!("{:08X}", offset);
    addr_str
        .find(q_upper.as_str())
        .map(|s| (s, s + q_upper.len()))
}

/// Build a full hex-viewer row Line with per-byte match highlighting.
/// `match_ranges` and `current_match` are file byte offsets (absolute).
#[allow(clippy::too_many_arguments)]
fn hex_row_spans(
    offset: usize,
    bytes: &[u8],
    match_ranges: &[(usize, usize)],
    current_match: Option<(usize, usize)>,
    addr_style: Style,
    hex_style: Style,
    hex_match_style: Style,
    hex_current_style: Style,
    ascii_style: Style,
    ascii_match_style: Style,
    ascii_current_style: Style,
    sep_style: Style,
    encoding: TextEncoding,
    // Highlight range within the 8-char address string, for address-jump searches.
    addr_highlight: Option<(usize, usize)>,
) -> Line<'static> {
    let byte_styles = |abs: usize| -> (Style, Style) {
        if current_match.is_some_and(|(s, e)| abs >= s && abs < e) {
            (hex_current_style, ascii_current_style)
        } else if match_ranges.iter().any(|&(s, e)| abs >= s && abs < e) {
            (hex_match_style, ascii_match_style)
        } else {
            (hex_style, ascii_style)
        }
    };

    // ── Hex section ───────────────────────────────────────────────────────────
    let mut hex_spans: Vec<Span<'static>> = Vec::new();
    let mut hex_buf = String::new();
    let mut hex_cur = hex_style;

    for (i, &byte) in bytes.iter().enumerate() {
        if i == 8 {
            // Mid-group separator space — always in base style.
            if !hex_buf.is_empty() {
                hex_spans.push(Span::styled(hex_buf.clone(), hex_cur));
                hex_buf.clear();
            }
            hex_cur = hex_style;
            hex_buf.push(' ');
        }
        let (hs, _) = byte_styles(offset + i);
        if hs != hex_cur {
            if !hex_buf.is_empty() {
                hex_spans.push(Span::styled(hex_buf.clone(), hex_cur));
                hex_buf.clear();
            }
            hex_cur = hs;
        }
        hex_buf.push_str(&format!("{:02X} ", byte));
    }
    // Padding for short last row.
    let n = bytes.len();
    let padding = (16 - n) * 3 + if n <= 8 { 1 } else { 0 };
    hex_buf.push_str(&" ".repeat(padding));
    if !hex_buf.is_empty() {
        hex_spans.push(Span::styled(hex_buf, hex_cur));
    }

    // ── ASCII section (encoding-aware) ───────────────────────────────────────
    let ascii_chars = encoding.decode_row_chars(bytes);
    let mut ascii_spans: Vec<Span<'static>> = Vec::new();
    let mut ascii_buf = String::new();
    let mut ascii_cur = ascii_style;
    let mut displayed_cols = 0usize;

    for (ch, byte_start, byte_end) in ascii_chars {
        let abs_start = offset + byte_start;
        let abs_end = offset + byte_end;
        let as_ = if current_match.is_some_and(|(ms, me)| ms < abs_end && me > abs_start) {
            ascii_current_style
        } else if match_ranges
            .iter()
            .any(|&(ms, me)| ms < abs_end && me > abs_start)
        {
            ascii_match_style
        } else {
            ascii_style
        };
        let ch_width = UnicodeWidthChar::width_cjk(ch).unwrap_or(1);
        if displayed_cols + ch_width > 16 {
            break;
        }
        if as_ != ascii_cur {
            if !ascii_buf.is_empty() {
                ascii_spans.push(Span::styled(ascii_buf.clone(), ascii_cur));
                ascii_buf.clear();
            }
            ascii_cur = as_;
        }
        ascii_buf.push(ch);
        displayed_cols += ch_width;
    }
    if !ascii_buf.is_empty() {
        ascii_spans.push(Span::styled(ascii_buf, ascii_cur));
    }
    let pad = 16usize.saturating_sub(displayed_cols);
    if pad > 0 {
        ascii_spans.push(Span::styled(" ".repeat(pad), ascii_style));
    }

    // ── Assemble ──────────────────────────────────────────────────────────────
    let addr_str = format!("{:08X}", offset);
    let mut spans: Vec<Span<'static>> = Vec::new();
    if let Some((hl_s, hl_e)) = addr_highlight {
        // Partial highlight: only the matching digit suffix of the address.
        if hl_s > 0 {
            spans.push(Span::styled(addr_str[..hl_s].to_string(), addr_style));
        }
        spans.push(Span::styled(
            addr_str[hl_s..hl_e].to_string(),
            hex_current_style,
        ));
        spans.push(Span::styled(format!("{}  ", &addr_str[hl_e..]), addr_style));
    } else {
        spans.push(Span::styled(format!("{}  ", addr_str), addr_style));
    }
    spans.extend(hex_spans);
    spans.push(Span::styled(" |", sep_style));
    spans.extend(ascii_spans);
    spans.push(Span::styled("|", sep_style));
    Line::from(spans)
}

// Control-character sanitization moved to `super::sanitize_for_display`
// (rwf-bin/src/ui/unicode_utils.rs) so the File Info dialog's header-bytes
// text-mode view can share the exact same pipeline. See its doc comment
// there for details.

// ── Unicode helpers ───────────────────────────────────────────────────────────

/// Produce ratatui spans for one viewport line with per-character match
/// highlighting. `match_ranges` are all match byte ranges on this line;
/// `current_match` is the active match byte range (rendered differently).
/// Characters in [column_offset, column_offset+max_cols) display columns are
/// included; the rest are clipped.
#[allow(clippy::too_many_arguments)]
fn highlight_spans(
    decoded: &str,
    match_ranges: &[(usize, usize)],
    current_match: Option<(usize, usize)>,
    column_offset: usize,
    max_cols: usize,
    normal_style: Style,
    match_style: Style,
    current_style: Style,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut cur_style = normal_style;
    let mut skipped = 0usize;
    let mut taken = 0usize;
    let mut byte_pos = 0usize;

    let style_for = |bp: usize| -> Style {
        if current_match.is_some_and(|(s, e)| bp >= s && bp < e) {
            current_style
        } else if match_ranges.iter().any(|&(s, e)| bp >= s && bp < e) {
            match_style
        } else {
            normal_style
        }
    };

    for ch in decoded.chars() {
        let ch_bytes = ch.len_utf8();
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        let ch_style = style_for(byte_pos);

        if skipped < column_offset {
            skipped += ch_width;
            byte_pos += ch_bytes;
            continue;
        }
        if taken + ch_width > max_cols {
            break;
        }

        if ch_style != cur_style {
            if !buf.is_empty() {
                spans.push(Span::styled(buf.clone(), cur_style));
                buf.clear();
            }
            cur_style = ch_style;
        }
        buf.push(ch);
        taken += ch_width;
        byte_pos += ch_bytes;
    }

    if !buf.is_empty() {
        spans.push(Span::styled(buf, cur_style));
    }

    // Pad to full content width so the background fills the row.
    let remaining = max_cols.saturating_sub(taken);
    if remaining > 0 {
        spans.push(Span::styled(" ".repeat(remaining), normal_style));
    }
    spans
}

// ── Directory preview ─────────────────────────────────────────────────────────

/// Rendered in the viewer area when the cursor is on a directory.
/// `dir_counts`: `Some((file_count, folder_count))` when the directory is cached.
pub fn render_dir_preview(
    frame: &mut Frame,
    area: Rect,
    location: &Location,
    dir_counts: Option<(usize, usize)>,
    colors: &ColorScheme,
    sbs_focused: bool,
) {
    let fg = parse_color(&colors.text_viewer_foreground_color);
    let bg = parse_color(&colors.text_viewer_background_color);
    let status_bg = parse_color(&colors.text_viewer_status_background_color);
    let dim_fg = Color::DarkGray;

    let title = if sbs_focused {
        " [Directory] "
    } else {
        " Directory "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(fg).bg(bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let path_str = location.display_path();
    let path_display = smart_truncate(&path_str, inner.width.saturating_sub(2) as usize, "…");

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        format!("  {}", path_display),
        Style::default().fg(fg).bg(bg),
    )));
    lines.push(Line::from(Span::raw("")));

    match dir_counts {
        Some((files, folders)) => {
            lines.push(Line::from(Span::styled(
                format!("  Folders : {}", folders),
                Style::default().fg(fg).bg(bg),
            )));
            lines.push(Line::from(Span::styled(
                format!("  Files   : {}", files),
                Style::default().fg(fg).bg(bg),
            )));
        }
        None => {
            lines.push(Line::from(Span::styled(
                "  (unreadable)",
                Style::default().fg(dim_fg).bg(bg),
            )));
        }
    }

    // Bottom hint
    if inner.height as usize > lines.len() + 1 {
        while lines.len() < inner.height.saturating_sub(1) as usize {
            lines.push(Line::from(Span::raw("")));
        }
        lines.push(Line::from(Span::styled(
            "  Enter: navigate in   Esc: close viewer",
            Style::default().fg(dim_fg).bg(status_bg),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().fg(fg).bg(bg)),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rwf_lib::model::{FileBytes, LineIndex, Location, ViewerBuffer};

    /// A loaded text-mode viewer over a small fixed in-memory buffer.
    fn smoke_viewer() -> ViewerState {
        let text = b"line one\nline two\nline three\n".to_vec();
        let line_index = LineIndex {
            offsets: vec![0, 9, 18],
            is_complete: true,
        };
        let buffer = ViewerBuffer::new(FileBytes::InMemory(text), line_index);
        let mut viewer = ViewerState::new(Location::Local(std::path::PathBuf::from(
            "/test/viewer_smoke.txt",
        )));
        viewer.buffer = Some(buffer);
        viewer.is_loading = false;
        viewer
    }

    /// M7 S2-2: render_viewer must not panic in text mode (loaded, no search active).
    #[test]
    fn test_render_viewer_text_mode_does_not_panic() {
        let viewer = smoke_viewer();
        let colors = ColorScheme::default();
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_viewer(frame, area, &viewer, &colors, UIMode::Viewer, "", "", false);
            })
            .expect("draw");
    }

    /// M7 S2-2: render_viewer must not panic in hex mode.
    #[test]
    fn test_render_viewer_hex_mode_does_not_panic() {
        let mut viewer = smoke_viewer();
        viewer.mode = ViewerMode::Hex;
        let colors = ColorScheme::default();
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_viewer(frame, area, &viewer, &colors, UIMode::Viewer, "", "", false);
            })
            .expect("draw");
    }

    /// M7 S2-2: render_viewer must not panic with the search input bar active.
    #[test]
    fn test_render_viewer_search_mode_does_not_panic() {
        let viewer = smoke_viewer();
        let colors = ColorScheme::default();
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_viewer(
                    frame,
                    area,
                    &viewer,
                    &colors,
                    UIMode::ViewerSearch,
                    "two",
                    "",
                    false,
                );
            })
            .expect("draw");
    }

    /// M7 S2-2: representative snapshot of a loaded text-mode viewer.
    #[test]
    fn test_render_viewer_snapshot() {
        let viewer = smoke_viewer();
        let colors = ColorScheme::default();
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_viewer(frame, area, &viewer, &colors, UIMode::Viewer, "", "", false);
            })
            .expect("draw");
        let output = format!("{:?}", terminal.backend().buffer());
        insta::assert_snapshot!("render_viewer_smoke", output);
    }

    // ── hex_row_spans ─────────────────────────────────────────────────────────

    fn hex_styles() -> (Style, Style, Style, Style, Style, Style, Style, Style) {
        (
            Style::default().fg(Color::Cyan),     // addr_style
            Style::default().fg(Color::White),    // hex_style
            Style::default().fg(Color::Yellow),   // hex_match_style
            Style::default().fg(Color::Red),      // hex_current_style
            Style::default().fg(Color::Gray),     // ascii_style
            Style::default().fg(Color::Yellow),   // ascii_match_style
            Style::default().fg(Color::Red),      // ascii_current_style
            Style::default().fg(Color::DarkGray), // sep_style
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn call_hex_row_spans(
        offset: usize,
        bytes: &[u8],
        match_ranges: &[(usize, usize)],
        current_match: Option<(usize, usize)>,
        addr_highlight: Option<(usize, usize)>,
    ) -> Line<'static> {
        let (
            addr_style,
            hex_style,
            hex_match_style,
            hex_current_style,
            ascii_style,
            ascii_match_style,
            ascii_current_style,
            sep_style,
        ) = hex_styles();
        hex_row_spans(
            offset,
            bytes,
            match_ranges,
            current_match,
            addr_style,
            hex_style,
            hex_match_style,
            hex_current_style,
            ascii_style,
            ascii_match_style,
            ascii_current_style,
            sep_style,
            TextEncoding::Utf8,
            addr_highlight,
        )
    }

    /// Concatenate all span contents in a Line, for easy substring assertions.
    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn hex_row_spans_full_row_formats_two_groups_and_ascii() {
        let bytes: Vec<u8> = (0u8..16).collect();
        let line = call_hex_row_spans(0, &bytes, &[], None, None);
        let text = line_text(&line);

        // Address, two 8-byte hex groups (with mid separator), then " |" + ascii + "|".
        let expected_hex = "00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F ";
        assert!(
            text.contains(expected_hex),
            "expected hex groups in {:?}",
            text
        );
        assert!(
            text.starts_with("00000000  "),
            "unexpected address: {:?}",
            text
        );
        assert!(text.contains(" |"), "missing separator: {:?}", text);
        assert!(
            text.ends_with('|'),
            "missing trailing separator: {:?}",
            text
        );

        // ASCII column: bytes 0x00-0x0F are non-printable control chars — decoding
        // depends on TextEncoding::Utf8's control-char handling, so just check width.
        let ascii_start = text.find(" |").unwrap() + 2;
        let ascii_part = &text[ascii_start..text.len() - 1];
        assert_eq!(
            ascii_part.chars().count(),
            16,
            "ascii column should be padded to 16 cols: {:?}",
            ascii_part
        );
    }

    #[test]
    fn hex_row_spans_short_row_pads_hex_and_ascii_columns() {
        let bytes: Vec<u8> = vec![0x41, 0x42, 0x43]; // "ABC"
        let full = call_hex_row_spans(0, &bytes, &[], None, None);
        let full_text = line_text(&full);

        let expected_hex = "41 42 43 ";
        assert!(
            full_text.contains(expected_hex),
            "expected partial hex group in {:?}",
            full_text
        );

        // Same total row width as a full 16-byte row (hex column + ascii column).
        let full16 = call_hex_row_spans(0, &(0u8..16).collect::<Vec<u8>>(), &[], None, None);
        assert_eq!(
            line_text(&full).chars().count(),
            line_text(&full16).chars().count(),
            "short row should pad to same width as full row"
        );

        // ASCII column decodes "ABC" and pads the remaining 13 columns with spaces.
        let ascii_start = full_text.find(" |").unwrap() + 2;
        let ascii_part = &full_text[ascii_start..full_text.len() - 1];
        assert!(
            ascii_part.starts_with("ABC"),
            "expected ABC prefix in ascii column: {:?}",
            ascii_part
        );
        assert_eq!(ascii_part.chars().count(), 16);
    }

    #[test]
    fn hex_row_spans_short_row_at_8_bytes_gets_extra_padding() {
        // n <= 8 gets one extra padding char to account for the mid-group separator
        // that a full row would have but this row never emits.
        let bytes: Vec<u8> = vec![0x41; 8];
        let row8 = call_hex_row_spans(0, &bytes, &[], None, None);
        let full16 = call_hex_row_spans(0, &(0u8..16).collect::<Vec<u8>>(), &[], None, None);
        assert_eq!(
            line_text(&row8).chars().count(),
            line_text(&full16).chars().count(),
        );
    }

    #[test]
    fn hex_row_spans_applies_current_and_match_styles_per_byte() {
        let (
            addr_style,
            hex_style,
            hex_match_style,
            hex_current_style,
            ascii_style,
            ascii_match_style,
            ascii_current_style,
            _sep_style,
        ) = hex_styles();

        let bytes: Vec<u8> = (0x41u8..0x41 + 16).collect(); // 'A'..'P', all printable ASCII
                                                            // Byte offset 2 is an "other match"; byte offset 5 is the current match.
        let line = call_hex_row_spans(0, &bytes, &[(2, 3)], Some((5, 6)), None);

        // Find the hex span covering byte 2 ("43 ") and assert it uses hex_match_style,
        // and the span covering byte 5 ("45 ") uses hex_current_style.
        let hex_other_span = line
            .spans
            .iter()
            .find(|s| s.content.contains("43 "))
            .expect("span for matched byte 2 not found");
        assert_eq!(hex_other_span.style, hex_match_style);

        let hex_current_span = line
            .spans
            .iter()
            .find(|s| s.content.contains("46 "))
            .expect("span for current-match byte 5 not found");
        assert_eq!(hex_current_span.style, hex_current_style);

        // A byte outside both ranges (e.g. byte 0, "41 ") uses the base hex_style.
        let hex_normal_span = line
            .spans
            .iter()
            .find(|s| s.content.contains("41 "))
            .expect("span for normal byte 0 not found");
        assert_eq!(hex_normal_span.style, hex_style);

        // ASCII column starts right after the " |" separator span — scope the
        // remaining assertions to spans past that point, since hex digits can
        // themselves contain letters (e.g. "4A") and collide with ascii content.
        let sep_idx = line
            .spans
            .iter()
            .position(|s| s.content.as_ref() == " |")
            .expect("separator span not found");
        let ascii_spans = &line.spans[sep_idx + 1..];

        // ASCII column: char at index 2 ('C') is the other-match style, char at
        // index 5 ('F') is the current-match style, char at index 0 ('A') is normal.
        let ascii_c_span = ascii_spans
            .iter()
            .find(|s| s.content.as_ref() == "C")
            .expect("ascii span for byte 2 not found");
        assert_eq!(ascii_c_span.style, ascii_match_style);

        let ascii_f_span = ascii_spans
            .iter()
            .find(|s| s.content.as_ref() == "F")
            .expect("ascii span for byte 5 not found");
        assert_eq!(ascii_f_span.style, ascii_current_style);

        // Byte 0 ('A') is adjacent to another normal-style byte ('B'), so it merges
        // into a shared span rather than standing alone — check by substring instead.
        let ascii_a_span = ascii_spans
            .iter()
            .find(|s| s.content.contains('A'))
            .expect("ascii span for byte 0 not found");
        assert_eq!(ascii_a_span.style, ascii_style);

        let _ = addr_style;
    }

    #[test]
    fn hex_row_spans_addr_highlight_splits_address_span() {
        let bytes: Vec<u8> = vec![0x41; 4];
        let (addr_style, _hex_style, _hex_match_style, hex_current_style, ..) = hex_styles();

        // No highlight: the address is a single span in addr_style.
        let plain = call_hex_row_spans(0x1000, &bytes, &[], None, None);
        let addr_span = &plain.spans[0];
        assert_eq!(addr_span.style, addr_style);
        assert!(addr_span.content.starts_with("00001000"));

        // With addr_highlight = Some((4, 8)) — the last 4 hex digits of
        // "00001000" highlighted — the address should be split into three spans:
        // prefix (addr_style), highlighted digits (hex_current_style), suffix.
        let highlighted = call_hex_row_spans(0x1000, &bytes, &[], None, Some((4, 8)));
        assert_eq!(highlighted.spans[0].style, addr_style);
        assert_eq!(highlighted.spans[0].content.as_ref(), "0000");
        assert_eq!(highlighted.spans[1].style, hex_current_style);
        assert_eq!(highlighted.spans[1].content.as_ref(), "1000");
        assert_eq!(highlighted.spans[2].style, addr_style);
        assert!(highlighted.spans[2].content.starts_with("  "));

        // Sanity: compute_addr_highlight actually produces this range for query "1000".
        assert_eq!(compute_addr_highlight("1000", 0x1000), Some((4, 8)));
    }
}
