//! Simple rename dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(super) fn render_simple_rename_dialog(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    cursor_pos: usize,
    scroll_pos: usize,
    focused_field: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let item_width = area.width.saturating_sub(4);

    frame.render_widget(
        Paragraph::new("New name:").style(base_style),
        Rect::new(area.x + 2, chunks[0].y, item_width, 1),
    );

    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(
            frame,
            Rect::new(area.x + 2, chunks[1].y, item_width, 1),
            focused_field == 0,
        );
    }

    frame.render_widget(
        Paragraph::new("(Enter to confirm, Esc to cancel)").style(hint_style),
        Rect::new(area.x + 2, chunks[2].y, item_width, 1),
    );

    let focused_item = Style::default().fg(Color::Black).bg(Color::White);
    let ok_style = if focused_field == 1 {
        focused_item
    } else {
        base_style
    };
    let cancel_style = if focused_field == 2 {
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
            .style(base_style),
        chunks[4],
    );
}
