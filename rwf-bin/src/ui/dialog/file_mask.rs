//! File mask dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render dialog content based on type
/// Render the Sort dialog (sort key + order + OK/Cancel)
///
/// Layout (11 lines total, per DIALOG_DESIGN_SPEC.md):
///   5 = label "Sort by:" + 4 items
///   1 = spacer
///   3 = label "Order:" + 2 items
///   1 = spacer
///   1 = buttons [*OK*] [Cancel]
pub(super) fn render_file_mask_dialog(
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
            Constraint::Length(1), // blank
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint1: Multiple patterns
            Constraint::Length(1), // hint2: Exclusion
            Constraint::Length(1), // hint3: Regexp
            Constraint::Length(1), // blank/spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;
    let item_width = area.width.saturating_sub(4);

    // Prompt
    frame.render_widget(
        Paragraph::new("Enter file mask (* = any chars, ? = single char):").style(base_style),
        Rect::new(area.x + 2, chunks[1].y, item_width, 1),
    );

    // Textbox
    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(
            frame,
            Rect::new(area.x + 2, chunks[2].y, item_width, 1),
            focused_field == 0,
        );
    }

    // Hint lines
    frame.render_widget(
        Paragraph::new("Multiple patterns: *.txt *.doc").style(hint_style),
        Rect::new(area.x + 2, chunks[3].y, item_width, 1),
    );
    frame.render_widget(
        Paragraph::new("Exclusion: :*.txt :temp*").style(hint_style),
        Rect::new(area.x + 2, chunks[4].y, item_width, 1),
    );
    frame.render_widget(
        Paragraph::new("Regexp: /.*\\.json$/ /TEST/i /Test/").style(hint_style),
        Rect::new(area.x + 2, chunks[5].y, item_width, 1),
    );

    // Buttons [*OK*] [Cancel]
    let focused_item = crate::ui::dialog::common::DIALOG_SELECTED;
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
        chunks[7],
    );
}
