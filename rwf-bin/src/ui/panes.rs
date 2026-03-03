//! Pane rendering
//!
//! This module renders the two vertical panes side by side.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use rwf_lib::{model::ActivePane, AppState, FileEntry};

/// Render both panes side by side
pub fn render_panes(frame: &mut Frame, area: Rect, state: &AppState) {
    // Split area into two vertical panes
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let tab = state.current_tab();

    // Render left pane
    render_pane(
        frame,
        panes[0],
        &tab.left_pane,
        state.ui.active_pane == ActivePane::Left,
        &state.marking,
    );

    // Render right pane
    render_pane(
        frame,
        panes[1],
        &tab.right_pane,
        state.ui.active_pane == ActivePane::Right,
        &state.marking,
    );
}

/// Render a single pane
fn render_pane(
    frame: &mut Frame,
    area: Rect,
    pane: &rwf_lib::model::PaneModel,
    is_active: bool,
    marking: &rwf_lib::model::MarkingModel,
) {
    // Create border with active indicator
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(" {} ", pane.current_location.display_path());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // If no entries, show empty message
    if pane.entries.is_empty() {
        let empty_msg = Paragraph::new("(empty directory)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty_msg, inner_area);
        return;
    }

    // Render based on display mode
    match pane.display_mode {
        rwf_lib::model::DisplayMode::Detailed => {
            render_detailed_mode(frame, inner_area, pane, marking);
        }
        rwf_lib::model::DisplayMode::Columns(cols) => {
            render_column_mode(frame, inner_area, pane, marking, cols);
        }
    }
}

/// Create a list item for a file entry
fn create_list_item(entry: &FileEntry, is_cursor: bool, is_marked: bool) -> ListItem<'static> {
    let mut style = Style::default();

    // Apply cursor highlighting
    if is_cursor {
        style = style.bg(Color::Cyan).fg(Color::Black);
    }

    // Apply directory coloring
    if entry.is_dir {
        style = style.fg(if is_cursor {
            Color::Black
        } else {
            Color::LightCyan
        });
    }

    // Apply marked file coloring
    if is_marked {
        style = style.fg(if is_cursor {
            Color::Black
        } else {
            Color::Yellow
        });
    }

    // Format the entry
    let name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    let size = format_size(entry.size);
    let date = format_date(&entry.modified);

    // Create line with name, size, and date
    let line = Line::from(vec![
        Span::styled(
            format!("{:<30}", truncate_string(&name, 30)),
            style,
        ),
        Span::styled(format!("{:>10}", size), style),
        Span::styled(format!("  {}", date), style),
    ]);

    ListItem::new(line)
}

/// Render detailed mode (full metadata)
fn render_detailed_mode(
    frame: &mut Frame,
    area: Rect,
    pane: &rwf_lib::model::PaneModel,
    marking: &rwf_lib::model::MarkingModel,
) {
    // Calculate visible range
    let visible_height = area.height as usize;
    let start_idx = pane.scroll_offset;
    let end_idx = (start_idx + visible_height).min(pane.entries.len());

    // Create list items for visible entries
    let items: Vec<ListItem> = pane.entries[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let global_idx = start_idx + idx;
            let is_cursor = global_idx == pane.cursor;
            let is_marked = marking.is_marked(&entry.location);

            create_list_item(entry, is_cursor, is_marked)
        })
        .collect();

    // Render list
    let list = List::new(items);
    frame.render_widget(list, area);
}

/// Render column mode (1-8 columns)
fn render_column_mode(
    frame: &mut Frame,
    area: Rect,
    pane: &rwf_lib::model::PaneModel,
    marking: &rwf_lib::model::MarkingModel,
    columns: u8,
) {
    let columns = columns.max(1).min(8) as usize;
    
    // Calculate visible range
    let visible_height = area.height as usize;
    let start_idx = pane.scroll_offset;
    let end_idx = (start_idx + visible_height * columns).min(pane.entries.len());

    // Calculate column width
    let col_width = area.width as usize / columns;

    // Create lines for each row
    let mut lines = Vec::new();
    
    for row in 0..visible_height {
        let mut spans = Vec::new();
        
        for col in 0..columns {
            let idx = start_idx + row * columns + col;
            
            if idx >= end_idx {
                break;
            }
            
            if idx < pane.entries.len() {
                let entry = &pane.entries[idx];
                let is_cursor = idx == pane.cursor;
                let is_marked = marking.is_marked(&entry.location);
                
                let mut style = Style::default();
                
                // Apply cursor highlighting
                if is_cursor {
                    style = style.bg(Color::Cyan).fg(Color::Black);
                }
                
                // Apply directory coloring
                if entry.is_dir {
                    style = style.fg(if is_cursor {
                        Color::Black
                    } else {
                        Color::LightCyan
                    });
                }
                
                // Apply marked file coloring
                if is_marked {
                    style = style.fg(if is_cursor {
                        Color::Black
                    } else {
                        Color::Yellow
                    });
                }
                
                // Format the entry name
                let name = if entry.is_dir {
                    format!("{}/", entry.name)
                } else {
                    entry.name.clone()
                };
                
                let truncated = truncate_string(&name, col_width.saturating_sub(1));
                let padded = format!("{:<width$}", truncated, width = col_width.saturating_sub(1));
                
                spans.push(Span::styled(padded, style));
                spans.push(Span::raw(" "));
            }
        }
        
        if !spans.is_empty() {
            lines.push(Line::from(spans));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    }
}

/// Format modification date
fn format_date(time: &std::time::SystemTime) -> String {
    use chrono::{DateTime, Local};

    let datetime: DateTime<Local> = (*time).into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Truncate string to max length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
