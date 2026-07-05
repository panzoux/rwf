//! Sort dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(super) fn render_sort_dialog(
    frame: &mut Frame,
    area: Rect,
    selected_mode_index: usize,
    selected_order_index: usize,
    focused_section: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // "Sort by:" label (1) + 4 items
            Constraint::Length(1), // spacer
            Constraint::Length(3), // "Order:" label (1) + 2 items
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let sort_keys = ["Name", "Size", "Date", "Extension"];
    let orders = ["Ascending", "Descending"];

    // Spec colors: focused item = Black/White, unfocused = Black/Gray
    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let focused_item = Style::default().fg(Color::Black).bg(Color::White); // spec: White bg
    let label_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let item_width = chunks[0].width.saturating_sub(4); // 2-char margin each side

    // --- Sort key section ---
    // Label on line 0 (no Block — avoids overlapping items)
    frame.render_widget(
        Paragraph::new("Sort by:").style(label_style),
        Rect::new(chunks[0].x + 2, chunks[0].y, item_width, 1),
    );
    // Items on lines 1-4
    for (i, label) in sort_keys.iter().enumerate() {
        let is_selected = i == selected_mode_index;
        let is_cursor = focused_section == 0 && i == selected_mode_index;
        let marker = if is_selected { "● " } else { "○ " };
        let text = format!("{}{}", marker, label);
        // Full-width paragraph so highlight covers entire row uniformly
        let row_style = if is_cursor { focused_item } else { base_style };
        let para = Paragraph::new(text).style(row_style);
        frame.render_widget(
            para,
            Rect::new(chunks[0].x + 2, chunks[0].y + 1 + i as u16, item_width, 1),
        );
    }

    // --- Order section ---
    frame.render_widget(
        Paragraph::new("Order:").style(label_style),
        Rect::new(chunks[2].x + 2, chunks[2].y, item_width, 1),
    );
    for (i, label) in orders.iter().enumerate() {
        let is_selected = i == selected_order_index;
        let is_cursor = focused_section == 1 && i == selected_order_index;
        let marker = if is_selected { "● " } else { "○ " };
        let text = format!("{}{}", marker, label);
        // Same item_width → identical highlight width for "Ascending" and "Descending"
        let row_style = if is_cursor { focused_item } else { base_style };
        let para = Paragraph::new(text).style(row_style);
        frame.render_widget(
            para,
            Rect::new(chunks[2].x + 2, chunks[2].y + 1 + i as u16, item_width, 1),
        );
    }

    // --- Buttons [*OK*] [Cancel] ---
    // Base row is Gray; only the button text spans receive focus color (not padding)
    let ok_style = if focused_section == 2 {
        focused_item
    } else {
        base_style
    };
    let cancel_style = if focused_section == 3 {
        focused_item
    } else {
        base_style
    };

    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line)
            .alignment(Alignment::Center)
            .style(base_style), // Gray bg for the whole row
        chunks[4],
    );
}
