//! Common dialog frame rendering
//!
//! Provides reusable functions for rendering dialog borders, titles, and buttons.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use rwf_lib::model::dialog::DialogContent;

/// Render dialog border and title, return content area
pub fn render_dialog_frame(frame: &mut Frame, title: &str, area: Rect) -> Rect {
    // Clear the area first so nothing shows through
    frame.render_widget(Clear, area);
    
    // Create block with border and title
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Black))
        .title(title)
        .title_style(Style::default().bold().fg(Color::Black))
        .style(Style::default().bg(Color::Gray));

    // Render block and get inner area
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    inner_area
}

/// Render OK/Cancel buttons at bottom of dialog
/// Renders buttons as TEXT: [*OK*] for default, [Cancel] for normal
/// Focus indicated by white background
pub fn render_dialog_buttons(frame: &mut Frame, area: Rect, content: &DialogContent, focused_button: usize) {
    let buttons = get_button_labels(content);

    // Calculate button positions
    let total_width: u16 = buttons.iter().map(|b| b.len() as u16 + 4).sum(); // +4 for spacing
    let start_x = area.x + (area.width - total_width) / 2;

    let mut current_x = start_x as u16;

    for (i, button_label) in buttons.iter().enumerate() {
        let is_focused = i == focused_button;
        let is_default = i == 0; // First button (OK) is default

        // Format button text:
        // [*Label*] for default button (asterisks denote Enter shortcut)
        // [Label] for other buttons
        let button_text = if is_default {
            format!("[*{}*]", button_label)
        } else {
            format!("[{}]", button_label)
        };

        // Focused button: black text on white background
        // Unfocused: black text on gray (transparent)
        let button_style = if is_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
        } else {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
        };

        let button_width = button_text.len() as u16;
        let button = Paragraph::new(button_text)
            .style(button_style)
            .alignment(Alignment::Center);

        let button_area = Rect::new(current_x, area.y + 1, button_width, 1);
        frame.render_widget(button, button_area);

        current_x += button_width + 2; // +2 for spacing between buttons
    }
}

/// Get button labels based on dialog content
fn get_button_labels(content: &DialogContent) -> Vec<&'static str> {
    match content {
        DialogContent::Compression { .. } => vec!["OK", "Cancel"],
        DialogContent::ExtractionConfirm { .. } => vec!["Extract", "Cancel"],
        DialogContent::CloseTabWithActiveJob { .. } => vec!["OK", "Cancel"],
        _ => vec!["OK", "Cancel"],
    }
}

/// Center a rectangle within another rectangle
/// 
/// # Arguments
/// * `percent_x` - Width as percentage of parent (0-100)
/// * `percent_y` - Height as percentage of parent (0-100)
/// * `area` - Parent rectangle
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let width = (area.width * percent_x / 100).max(40);
    let height = (area.height * percent_y / 100).max(10);
    
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    
    Rect::new(x, y, width, height)
}

/// Render a simple text line with optional highlighting
pub fn render_text_line(frame: &mut Frame, text: &str, area: Rect, highlighted: bool) {
    let style = if highlighted {
        Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    
    let paragraph = Paragraph::new(text).style(style);
    frame.render_widget(paragraph, area);
}

/// Render a labeled input field
pub fn render_labeled_field(frame: &mut Frame, label: &str, value: &str, area: Rect, focused: bool) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(label.len() as u16 + 2),
            Constraint::Min(10),
        ])
        .split(area);
    
    // Render label
    let label_paragraph = Paragraph::new(label).style(Style::default().fg(Color::Yellow));
    frame.render_widget(label_paragraph, chunks[0]);
    
    // Render value with border if focused
    let value_style = if focused {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default().fg(Color::White)
    };
    
    let value_block = Block::default()
        .borders(Borders::ALL)
        .border_style(value_style);
    
    let value_inner = value_block.inner(chunks[1]);
    frame.render_widget(value_block, chunks[1]);
    
    let value_paragraph = Paragraph::new(value).style(value_style);
    frame.render_widget(value_paragraph, value_inner);
}
