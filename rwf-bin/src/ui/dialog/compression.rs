//! Compression dialog with improved layout

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use rwf_lib::ArchiveFormat;

/// Archive format options (name + format enum)
const ARCHIVE_FORMATS: &[(&str, ArchiveFormat)] = &[
    ("ZIP", ArchiveFormat::ZIP),
    ("7Z", ArchiveFormat::ZIP),   // Future: SevenZip
    ("BZ2", ArchiveFormat::ZIP),  // Future: BZ2
    ("TAR", ArchiveFormat::ZIP),  // Future: TAR
    ("LZH", ArchiveFormat::ZIP),  // Future: LZH
    ("CAB", ArchiveFormat::ZIP),  // Future: CAB
    ("XZ", ArchiveFormat::ZIP),   // Future: XZ
    ("LZMA", ArchiveFormat::ZIP), // Future: LZMA
];

/// Compression level options: (level, display name)
const COMPRESSION_LEVELS: &[(u32, &str)] = &[
    (0, "Store"),
    (1, "Fastest"),
    (3, "Fast"),
    (5, "Normal"),
    (7, "Maximum"),
    (9, "Ultra"),
];

/// Compression dialog state (embedded in DialogContent)
#[derive(Debug, Clone)]
pub struct CompressionDialogState {
    pub archive_name: String,
    pub selected_format_index: usize,
    pub selected_compression_index: usize,
    pub focused_field: usize,  // 0=format, 1=compression, 2=name, 3=OK, 4=Cancel
    pub format_focus_index: usize,      // Which format has focus (0-7)
    pub compression_focus_index: usize, // Which compression level has focus (0-5)
    pub cursor_pos: usize,
}

impl CompressionDialogState {

    fn render_format_list(&self, _area: Rect) -> Vec<ListItem<'_>> {
        ARCHIVE_FORMATS
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                let is_focused = (self.focused_field == 0) && (i == self.format_focus_index);
                let is_selected = i == self.selected_format_index;
                
                // Selection indicator: "●" vs "○"
                // Focus indicator: white background on focused item ONLY (when section has focus)
                let radio = if is_selected { "●" } else { "○" };
                
                // Focused item (only when format section has focus): black text on white background
                // Unfocused items: black text on gray (transparent)
                let style = if is_focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                } else {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Gray)
                };

                ListItem::new(format!("{}{}", radio, name)).style(style)
            })
            .collect()
    }

    fn render_compression_list(&self) -> Vec<ListItem<'_>> {
        COMPRESSION_LEVELS
            .iter()
            .enumerate()
            .map(|(i, (level, name))| {
                let is_focused = (self.focused_field == 1) && (i == self.compression_focus_index);
                let is_selected = i == self.selected_compression_index;

                // Selection indicator: "●" vs "○"
                // Focus indicator: white background on focused item ONLY (when section has focus)
                let radio = if is_selected { "●" } else { "○" };
                
                // Focused item (only when compression section has focus): black text on white background
                // Unfocused items: black text on gray (transparent)
                // No indent
                let style = if is_focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                } else {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Gray)
                };

                ListItem::new(format!("{}{} ({})", radio, name, level)).style(style)
            })
            .collect()
    }
}

/// Get the layout constraints for compression dialog
/// Returns the sum of all constraint lengths (minimum content height)
pub fn get_compression_dialog_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Length(9),   // Archive format: 1 label + 8 items
        Constraint::Length(1),   // Spacing
        Constraint::Length(7),   // Compression levels: 1 label + 6 items
        Constraint::Length(1),   // Spacing
        Constraint::Length(2),   // Archive name: 1 label + 1 input
        Constraint::Length(1),   // Spacing
        Constraint::Length(1),   // Buttons
    ]
}

/// Calculate minimum height needed for compression dialog (content only, no borders)
pub fn calculate_compression_dialog_min_height() -> u16 {
    get_compression_dialog_constraints()
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum()
}

pub fn render_compression_dialog(
    frame: &mut Frame,
    area: Rect,
    state: &CompressionDialogState,
    dialog_focused: bool,
) {
    // Layout: [Format] [Compression] [Name] [Buttons]
    // Only 1 line spacing between sections
    let constraints = get_compression_dialog_constraints();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Render archive format label (black text, no background)
    let format_label = Paragraph::new("Archive Format:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(format_label, chunks[0]);

    // Render archive format (vertical list, all 8 formats shown)
    let format_items = state.render_format_list(chunks[0]);
    let format_list = List::new(format_items);
    let format_area = Rect::new(chunks[0].x, chunks[0].y + 1, chunks[0].width, 8);
    frame.render_widget(format_list, format_area);

    // Render compression level label (black text, no background)
    let compression_label = Paragraph::new("Compression Level:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(compression_label, chunks[2]);

    // Render compression levels (vertical list)
    let compression_items = state.render_compression_list();
    let compression_list = List::new(compression_items);
    let compression_area = Rect::new(chunks[2].x, chunks[2].y + 1, chunks[2].width, 6);
    frame.render_widget(compression_list, compression_area);

    // Render archive name input (no border)
    let name_label = Paragraph::new("Archive Name:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(name_label, chunks[4]);

    let name_text = if state.focused_field == 2 {
        let mut text = state.archive_name.clone();
        if state.cursor_pos <= text.len() {
            text.insert(state.cursor_pos, '█');
        }
        text
    } else {
        state.archive_name.clone()
    };

    // When name field has focus (field 2): black text on white background with cursor
    // Otherwise: white text on dark gray background
    let name_style = if state.focused_field == 2 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
    };

    let name_input = Paragraph::new(name_text)
        .style(name_style);
    frame.render_widget(name_input, Rect::new(chunks[4].x, chunks[4].y + 1, chunks[4].width, 1));

    // Render buttons directly
    // OK is field 3, Cancel is field 4 in the dialog state
    let buttons = [
        ("OK", 3, true),    // (label, focused_field value, is_default)
        ("Cancel", 4, false),
    ];
    let total_width: u16 = buttons.iter().map(|(label, _, _)| label.len() as u16 + 4).sum();
    let start_x = chunks[6].x + (chunks[6].width.saturating_sub(total_width)) / 2;
    let mut current_x = start_x;

    for (label, field_idx, is_default) in buttons.iter() {
        let is_focused = dialog_focused && state.focused_field == *field_idx;
        
        // Format button text: [*Label*] for default, [Label] for others
        let button_text = if *is_default {
            format!("[*{}*]", label)
        } else {
            format!("[{}]", label)
        };

        // Focused button: black text on white background
        // Unfocused: black text on gray
        let button_style = if is_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
        } else {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
        };

        let button = Paragraph::new(format!(" {} ", button_text))
            .style(button_style);
        let button_width = button_text.len() as u16 + 2;
        let button_area = Rect::new(current_x, chunks[6].y, button_width, 1);
        frame.render_widget(button, button_area);

        current_x += button_width + 2;
    }
}
