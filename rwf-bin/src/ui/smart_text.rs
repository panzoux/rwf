//! Reusable SmartText widget with advanced truncation and multi-line support
//!
//! Provides CJK-aware text rendering with multiple truncation modes
//! and smart multi-line wrapping (e.g., at path separators).

use ratatui::{
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Paragraph, Widget},
    Frame,
};
use crate::ui::unicode_utils::{truncate_to_width, shorten_path};

/// Truncation modes for SmartText
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TruncateMode {
    End,  // "Very long te..."
    Path, // "C:\...\dir\file.txt"
}

/// A widget that renders text with smart truncation and multi-line support
pub struct SmartText<'a> {
    text: &'a str,
    style: Style,
    mode: TruncateMode,
    max_lines: usize,
    ellipsis: &'a str,
}

impl<'a> SmartText<'a> {
    /// Create a new SmartText widget
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            style: Style::default(),
            mode: TruncateMode::End,
            max_lines: 1,
            ellipsis: "...",
        }
    }

    /// Set the style
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the truncation mode
    pub fn mode(mut self, mode: TruncateMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the maximum number of lines
    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = max_lines;
        self
    }

    /// Helper to split text into multiple lines smartly
    fn split_to_lines(&self, width: usize) -> Vec<String> {
        if self.text.is_empty() {
            return vec![String::new()];
        }

        if self.max_lines <= 1 {
            return vec![self.truncate_single_line(self.text, width)];
        }

        let mut lines = Vec::new();
        let mut remaining = self.text;

        while !remaining.is_empty() && lines.len() < self.max_lines {
            if lines.len() == self.max_lines - 1 {
                // Last line: truncate if necessary
                lines.push(self.truncate_single_line(remaining, width));
                break;
            }

            // Find best split point near width
            let split_idx = self.find_split_point(remaining, width);
            lines.push(remaining[..split_idx].to_string());
            remaining = &remaining[split_idx..];
            
            // Skip leading separators on new lines if they were at the split point
            if (remaining.starts_with('/') || remaining.starts_with('\\')) && lines.last().map_or(false, |l| !l.is_empty()) {
                // Optional: keep separator on the previous line or move to next
                // For paths, keeping separator as part of the next component is often clearer
            }
        }

        lines
    }

    /// Find a smart split point near the target width
    fn find_split_point(&self, s: &str, target_width: usize) -> usize {
        let mut current_width = 0;
        let mut last_separator_idx = 0;
        let mut last_byte_pos = 0;

        for (idx, ch) in s.char_indices() {
            let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
            if current_width + ch_width > target_width {
                if last_separator_idx > 0 {
                    return last_separator_idx;
                }
                return last_byte_pos;
            }

            current_width += ch_width;
            last_byte_pos = idx + ch.len_utf8();

            // Prefer splitting at separators
            if ch == '/' || ch == '\\' || ch == ' ' || ch == '_' || ch == '-' || ch == '.' {
                last_separator_idx = last_byte_pos;
            }
        }

        last_byte_pos
    }

    /// Truncate a single string according to mode
    fn truncate_single_line(&self, s: &str, width: usize) -> String {
        match self.mode {
            TruncateMode::End => truncate_to_width(s, width, self.ellipsis),
            TruncateMode::Path => shorten_path(s, width, self.ellipsis),
        }
    }

    /// Render using a Frame (convenience method)
    pub fn render(self, frame: &mut Frame, area: Rect) {
        let lines = self.split_to_lines(area.width as usize);
        let paragraph = Paragraph::new(lines.iter().map(|l| Line::from(l.as_str())).collect::<Vec<_>>())
            .style(self.style);
        frame.render_widget(paragraph, area);
    }
}

impl<'a> Widget for SmartText<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let lines = self.split_to_lines(area.width as usize);
        let paragraph = Paragraph::new(lines.iter().map(|l| Line::from(l.as_str())).collect::<Vec<_>>())
            .style(self.style);
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_text_single_line() {
        let widget = SmartText::new("very long text indeed").max_lines(1);
        let lines = widget.split_to_lines(10);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "very lo...");
    }

    #[test]
    fn test_smart_text_multi_line_split() {
        // Should split at space
        let widget = SmartText::new("hello world").max_lines(2);
        let lines = widget.split_to_lines(6);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "hello ");
        assert_eq!(lines[1], "world");
    }

    #[test]
    fn test_smart_text_path_split() {
        // Should split at path separator
        let widget = SmartText::new("C:\\Users\\user\\file.txt").max_lines(2);
        let lines = widget.split_to_lines(15);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with('\\') || lines[1].starts_with('\\'));
    }

}
