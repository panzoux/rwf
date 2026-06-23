//! Pane rendering
//!
//! This module renders the two vertical panes side by side.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use rwf_lib::{model::ActivePane, AppState, FileEntry, config::ColorScheme};
use super::{parse_color, pad_to_width, smart_truncate};

/// Render only the anchored pane, filling the entire area (used in SideBySide viewer mode).
pub fn render_active_pane_only(frame: &mut Frame, area: Rect, state: &AppState, anchor: ActivePane) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    let ellipsis = &state.config.ellipsis;
    let symlink_sep = &state.config.display.symlink_separator;
    let (pane, marking) = match anchor {
        ActivePane::Left  => (&tab.left_pane,  &tab.left_pane.marking),
        ActivePane::Right => (&tab.right_pane, &tab.right_pane.marking),
    };
    render_pane(frame, area, pane, true, marking, colors, ellipsis, symlink_sep);
}

/// Render both panes side by side
pub fn render_panes(frame: &mut Frame, area: Rect, state: &AppState) {
    // Split area into two vertical panes
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    let ellipsis = &state.config.ellipsis;
    let symlink_sep = &state.config.display.symlink_separator;

    // Render left pane
    render_pane(
        frame,
        panes[0],
        &tab.left_pane,
        state.ui.active_pane == ActivePane::Left,
        &tab.left_pane.marking,
        colors,
        ellipsis,
        symlink_sep,
    );

    // Render right pane
    render_pane(
        frame,
        panes[1],
        &tab.right_pane,
        state.ui.active_pane == ActivePane::Right,
        &tab.right_pane.marking,
        colors,
        ellipsis,
        symlink_sep,
    );
}

/// Render a single pane
fn render_pane(
    frame: &mut Frame,
    area: Rect,
    pane: &rwf_lib::model::PaneModel,
    is_active: bool,
    marking: &rwf_lib::model::MarkingModel,
    colors: &ColorScheme,
    ellipsis: &str,
    symlink_sep: &str,
) {
    // NO BORDERS - render directly to area
    
    // If loading, show fetching message
    if pane.is_loading {
        tracing::error!("[UI::render_pane] STUCK: is_loading=true for pane={:?} at location={} address={:p}", pane.display_mode, pane.current_location.display_path(), pane as *const _);
        let loading_msg = Paragraph::new("(fetching file entries...)")
            .style(Style::default().fg(parse_color(&colors.foreground_color)));
        frame.render_widget(loading_msg, area);
        return;
    }
    
    tracing::info!("[UI::render_pane] OK: is_loading=false for pane at location={}, entries.len()={} address={:p}", pane.current_location.display_path(), pane.entries.len(), pane as *const _);

    // If no entries, show empty message
    if pane.entries.is_empty() {
        let empty_msg = Paragraph::new("(empty directory)")
            .style(Style::default().fg(parse_color(&colors.foreground_color)));
        frame.render_widget(empty_msg, area);
        return;
    }

    // Render based on display mode
    match pane.display_mode {
        rwf_lib::model::DisplayMode::Detailed => {
            render_detailed_mode(frame, area, pane, marking, colors, is_active, ellipsis, symlink_sep);
        }
        rwf_lib::model::DisplayMode::Columns(cols) => {
            render_column_mode(frame, area, pane, marking, cols, colors, is_active, ellipsis, symlink_sep);
        }
    }
}

/// Create a list item for a file entry with selection indicator
fn create_list_item(entry: &FileEntry, is_cursor: bool, is_marked: bool, colors: &ColorScheme, is_active: bool, name_width: usize, ellipsis: &str, symlink_sep: &str) -> ListItem<'static> {
    // Set base colors based on active state
    let mut style = if is_active {
        Style::default()
            .fg(parse_color(&colors.foreground_color))
            .bg(parse_color(&colors.background_color))
    } else {
        Style::default()
            .fg(parse_color(colors.get_inactive_foreground()))
            .bg(parse_color(colors.get_inactive_background()))
    };

    // Apply cursor highlighting
    if is_cursor {
        let bg_color = if is_active {
            colors.get_file_pane_cursor_background()
        } else {
            colors.get_inactive_file_pane_cursor_background()
        };
        let fg_color = if is_active {
            colors.get_file_pane_cursor_foreground()
        } else {
            colors.get_inactive_file_pane_cursor_foreground()
        };
        style = style
            .bg(parse_color(&bg_color))
            .fg(parse_color(&fg_color));
    }

    // Apply directory coloring
    if entry.is_dir {
        style = style.fg(if is_cursor {
            let fg_color = if is_active {
                colors.get_file_pane_cursor_foreground()
            } else {
                colors.get_inactive_file_pane_cursor_foreground()
            };
            parse_color(&fg_color)
        } else if is_active {
            parse_color(&colors.directory_color)
        } else {
            parse_color(&colors.inactive_directory_color)
        });
    }

    // Apply marked file coloring
    if is_marked {
        style = style.fg(if is_cursor {
            let fg_color = if is_active {
                colors.get_file_pane_cursor_foreground()
            } else {
                colors.get_inactive_file_pane_cursor_foreground()
            };
            parse_color(&fg_color)
        } else {
            parse_color(&colors.marked_file_color)
        });
    }

    // Selection indicator: "*" for marked files, " " for others
    let indicator = if is_marked { "*" } else { " " };

    // Format the entry
    let name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };
    let name = if entry.is_symlink {
        format!("{}{}", name, symlink_sep)
    } else {
        name
    };

    let size = if entry.is_dir {
        "<DIR>".to_string()
    } else {
        format_size(entry.size)
    };
    let date = format_date(&entry.modified);

    // Create line with indicator, name, size, and date
    // Format: "*info/   <DIR> 9999-12-31 21:47"
    let line = Line::from(vec![
        Span::styled(indicator, style),
        Span::styled(
            pad_to_width(&smart_truncate(&name, name_width, ellipsis), name_width),
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
    colors: &ColorScheme,
    is_active: bool,
    ellipsis: &str,
    symlink_sep: &str,
) {
    // Calculate visible range
    let visible_height = area.height as usize;
    let start_idx = pane.scroll_offset;
    let end_idx = (start_idx + visible_height).min(pane.entries.len());

    // Calculate dynamic name width based on terminal width
    // Reserve 30 chars for indicator(1) + size(10) + date(19), minimum 10 for name
    let name_width = (area.width as usize).saturating_sub(30).max(10);

    // Create list items for visible entries
    let items: Vec<ListItem> = pane.entries[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let global_idx = start_idx + idx;
            let is_cursor = global_idx == pane.cursor;
            let is_marked = marking.is_marked(&entry.location);

            create_list_item(entry, is_cursor, is_marked, colors, is_active, name_width, ellipsis, symlink_sep)
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
    colors: &ColorScheme,
    is_active: bool,
    ellipsis: &str,
    symlink_sep: &str,
) {
    let columns = columns.clamp(1, 8) as usize;
    
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
                
                // Set base colors based on active state
                let mut style = if is_active {
                    Style::default()
                        .fg(parse_color(&colors.foreground_color))
                        .bg(parse_color(&colors.background_color))
                } else {
                    Style::default()
                        .fg(parse_color(colors.get_inactive_foreground()))
                        .bg(parse_color(colors.get_inactive_background()))
                };
                
                // Apply cursor highlighting
                if is_cursor {
                    let bg_color = if is_active {
                        colors.get_file_pane_cursor_background()
                    } else {
                        colors.get_inactive_file_pane_cursor_background()
                    };
                    let fg_color = if is_active {
                        colors.get_file_pane_cursor_foreground()
                    } else {
                        colors.get_inactive_file_pane_cursor_foreground()
                    };
                    style = style
                        .bg(parse_color(&bg_color))
                        .fg(parse_color(&fg_color));
                }
                
                // Apply directory coloring
                if entry.is_dir {
                    style = style.fg(if is_cursor {
                        let fg_color = if is_active {
                            colors.get_file_pane_cursor_foreground()
                        } else {
                            colors.get_inactive_file_pane_cursor_foreground()
                        };
                        parse_color(&fg_color)
                    } else if is_active {
                        parse_color(&colors.directory_color)
                    } else {
                        parse_color(&colors.inactive_directory_color)
                    });
                }
                
                // Apply marked file coloring
                if is_marked {
                    style = style.fg(if is_cursor {
                        let fg_color = if is_active {
                            colors.get_file_pane_cursor_foreground()
                        } else {
                            colors.get_inactive_file_pane_cursor_foreground()
                        };
                        parse_color(&fg_color)
                    } else {
                        parse_color(&colors.marked_file_color)
                    });
                }
                
                // Selection indicator
                let indicator = if is_marked { "*" } else { " " };
                
                // Format the entry name
                let name = if entry.is_dir {
                    format!("{}/", entry.name)
                } else {
                    entry.name.clone()
                };
                let name = if entry.is_symlink {
                    format!("{}{}", name, symlink_sep)
                } else {
                    name
                };

                let truncated = smart_truncate(&name, col_width.saturating_sub(2), ellipsis);
                let combined = format!("{}{}", indicator, truncated);
                let padded = pad_to_width(&combined, col_width);
                
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
