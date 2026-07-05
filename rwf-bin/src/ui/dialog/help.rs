//! Help dialog rendering and entry filtering.
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

pub(super) fn help_filter_entries<'a>(
    entries: &'a [rwf_lib::model::dialog::HelpEntry],
    active_tab: &rwf_lib::model::dialog::HelpTab,
    show_unbound: bool,
    query: &str,
    regex_mode: bool,
) -> Vec<&'a rwf_lib::model::dialog::HelpEntry> {
    let tab_filtered: Vec<&rwf_lib::model::dialog::HelpEntry> = entries
        .iter()
        .filter(|e| e.tab == *active_tab)
        .filter(|e| show_unbound || !e.keys.is_empty())
        .collect();

    if query.is_empty() {
        return tab_filtered;
    }

    if regex_mode {
        if let Ok(re) = regex::Regex::new(&format!("(?i){}", query)) {
            tab_filtered
                .into_iter()
                .filter(|e| {
                    let haystack = format!("{} {} {}", e.category, e.description, e.keys.join(" "));
                    re.is_match(&haystack)
                })
                .collect()
        } else {
            tab_filtered
        }
    } else {
        // AND search: each space-separated token must appear in the row text
        let tokens: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
        tab_filtered
            .into_iter()
            .filter(|e| {
                let haystack =
                    format!("{} {} {}", e.category, e.description, e.keys.join(" ")).to_lowercase();
                tokens.iter().all(|tok| haystack.contains(tok.as_str()))
            })
            .collect()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_help_dialog(
    frame: &mut Frame,
    area: Rect,
    _dialog_area: Rect,
    entries: &[rwf_lib::model::dialog::HelpEntry],
    query: &str,
    regex_mode: bool,
    show_unbound: bool,
    active_tab: &rwf_lib::model::dialog::HelpTab,
    scroll_pos: usize,
    language: &str,
) {
    use rwf_lib::model::dialog::HelpTab;
    use unicode_width::UnicodeWidthStr;

    let base = crate::ui::dialog::common::DIALOG_TEXT;
    let tab_active = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let tab_inactive = crate::ui::dialog::common::DIALOG_DIM;
    let search_style = Style::default().fg(Color::White).bg(Color::DarkGray);
    let unbound_style = crate::ui::dialog::common::DIALOG_DIM;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let w = area.width.saturating_sub(2) as usize; // 1-char margin each side

    // ── Row 0: Tab bar ──────────────────────────────────────────────────────
    if area.height >= 1 {
        let tabs = [
            (HelpTab::NormalMode, "^1:Normal"),
            (HelpTab::ViewerMode, "^2:Viewer"),
            (HelpTab::LeapMode, "^3:Leap"),
            (HelpTab::DialogMode, "^4:Dialog"),
            (HelpTab::CustomFunctions, "^5:Custom"),
        ];
        let mut spans: Vec<Span> = Vec::new();
        for (i, (tab, label)) in tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", base));
            }
            if tab == active_tab {
                spans.push(Span::styled(format!("[{}]", label), tab_active));
            } else {
                spans.push(Span::styled(label.to_string(), tab_inactive));
            }
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(base),
            Rect::new(area.x + 1, area.y, w as u16, 1),
        );
    }

    // ── Row 1: Search field ─────────────────────────────────────────────────
    if area.height >= 2 {
        let search_text = if regex_mode {
            format!("[regex] {}", query)
        } else {
            format!("/{}", query)
        };
        frame.render_widget(
            Paragraph::new(smart_truncate(&search_text, w, "…")).style(search_style),
            Rect::new(area.x + 1, area.y + 1, w as u16, 1),
        );
    }

    if area.height < 3 {
        return;
    }

    // ── Filter and compute column widths ────────────────────────────────────
    let filtered = help_filter_entries(entries, active_tab, show_unbound, query, regex_mode);
    let count = filtered.len();

    // Compute column widths from visible entries (unicode display width)
    let min_cat_w: usize = 10;
    let min_desc_w: usize = 20;
    let min_keys_w: usize = 8;

    let (max_cat_w, _max_desc_w, max_keys_w) =
        filtered
            .iter()
            .fold((min_cat_w, min_desc_w, min_keys_w), |(mc, md, mk), e| {
                let kw = if e.keys.is_empty() {
                    "(unbound)".len()
                } else {
                    e.keys.join(", ").len()
                };
                (
                    mc.max(UnicodeWidthStr::width(e.category.as_str())),
                    md.max(UnicodeWidthStr::width(e.description.as_str())),
                    mk.max(kw),
                )
            });

    // Distribute space: 2 chars separator between columns
    // Total = cat_w + 2 + desc_w + 2 + keys_w; cap at w
    let avail = w.saturating_sub(4); // 2 separators of 2 chars each
                                     // Keys column is smallest — let description flex, cap category
    let cat_w = max_cat_w.min(avail / 4).max(min_cat_w);
    let keys_w = max_keys_w.min(avail / 4).max(min_keys_w);
    let desc_w = avail
        .saturating_sub(cat_w)
        .saturating_sub(keys_w)
        .max(min_desc_w);

    // ── Rows 2..height-2: entry list ─────────────────────────────────────────
    let list_start_y = area.y + 2;
    let list_height = area.height.saturating_sub(3) as usize; // -tab -search -hint

    // Clamp scroll so the last entry is always at the bottom (no trailing blank rows)
    let effective_scroll = if filtered.len() > list_height {
        scroll_pos.min(filtered.len() - list_height)
    } else {
        0
    };

    for (row, entry) in filtered
        .iter()
        .skip(effective_scroll)
        .take(list_height)
        .enumerate()
    {
        let y = list_start_y + row as u16;
        if y >= area.y + area.height.saturating_sub(1) {
            break;
        }

        let keys_str = if entry.keys.is_empty() {
            "(unbound)".to_string()
        } else {
            entry.keys.join(", ")
        };
        let is_unbound = entry.keys.is_empty();

        // Truncate each column to its width
        let cat_s = smart_truncate(&entry.category, cat_w, "…");
        let desc_s = smart_truncate(&entry.description, desc_w, "…");
        let keys_s = smart_truncate(&keys_str, keys_w, "…");

        let row_style = if is_unbound { unbound_style } else { base };

        let line = Line::from(vec![
            Span::styled(format!("{:<cat_w$}", cat_s, cat_w = cat_w), row_style),
            Span::styled("  ", row_style),
            Span::styled(format!("{:<desc_w$}", desc_s, desc_w = desc_w), row_style),
            Span::styled("  ", row_style),
            Span::styled(keys_s, row_style),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x + 1, y, w as u16, 1));
    }

    // ── Last row: hint line ──────────────────────────────────────────────────
    let hint_y = area.y + area.height.saturating_sub(1);
    let unbound_indicator = if show_unbound {
        "u:hide unbound"
    } else {
        "u:show unbound"
    };
    let hint_text = format!(
        "({})  {}  L:lang({})  Ctrl+R:regex",
        count, unbound_indicator, language
    );
    frame.render_widget(
        Paragraph::new(smart_truncate(&hint_text, w, "…")).style(hint_style),
        Rect::new(area.x + 1, hint_y, w as u16, 1),
    );
}
