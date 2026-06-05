//! Task Panel with state management

use rwf_lib::config::ColorScheme;
use super::colors::parse_color;

/// Log level for colored tags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,    // Normal text
    Ok,      // [OK] - Green
    Fail,    // [FAIL] - Red
    Warn,    // [WARN] - Yellow
}

/// Log entry with level
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub message: String,
    pub level: LogLevel,
}

/// Task panel with state
pub struct TaskPanel {
    log_entries: Vec<LogEntry>,
    pending_logs: Vec<String>,
    /// Scroll offset in visual lines (not log entries)
    scroll_offset: usize,
    /// Spinner animation frames (TWF reference: "|", "/", "-", "\\")
    spinner_frames: [&'static str; 4],
    /// Current spinner frame index
    spinner_index: usize,
    /// Last computed max scroll (visual lines), updated by render; used to normalize scroll_up
    last_max_visual_scroll: std::cell::Cell<usize>,
}

impl Default for TaskPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskPanel {
    /// Create a new TaskPanel
    pub fn new() -> Self {
        Self {
            log_entries: Vec::with_capacity(256),
            pending_logs: Vec::new(),
            scroll_offset: 0,
            spinner_frames: ["|", "/", "-", "\\"],
            spinner_index: 0,
            last_max_visual_scroll: std::cell::Cell::new(0),
        }
    }
    
    /// Add a log entry with level
    pub fn add_log(&mut self, message: String, level: LogLevel) {
        self.log_entries.push(LogEntry {
            message,
            level,
        });
    }
    
    /// Add a pending log (thread-safe, processed later)
    pub fn add_pending_log(&mut self, message: String) {
        self.pending_logs.push(message);
    }
    
    /// Process pending logs with max lines limit
    pub fn process_pending_logs(&mut self, max_lines: usize) {
        // Collect messages first to avoid borrow checker issues
        let messages: Vec<String> = self.pending_logs.drain(..).collect();
        
        for message in messages {
            // Parse log level from message tags
            let level = if message.contains("[OK]") {
                LogLevel::Ok
            } else if message.contains("[FAIL]") {
                LogLevel::Fail
            } else if message.contains("[WARN]") {
                LogLevel::Warn
            } else {
                LogLevel::Info
            };

            self.log_entries.push(LogEntry {
                message,
                level,
            });
        }

        // Trim old logs if exceeding max
        if self.log_entries.len() > max_lines {
            let excess = self.log_entries.len() - max_lines;
            self.log_entries.drain(0..excess);

            // Adjust scroll offset
            if self.scroll_offset >= excess {
                self.scroll_offset -= excess;
            } else {
                self.scroll_offset = 0;
            }
        }
    }
    
    /// Scroll up
    pub fn scroll_up(&mut self) {
        // Normalize first: scroll_to_end sets a large sentinel; snap to actual max before decrementing
        let cached_max = self.last_max_visual_scroll.get();
        if cached_max > 0 {
            self.scroll_offset = self.scroll_offset.min(cached_max);
        }
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down
    pub fn scroll_down(&mut self, visible_height: usize) {
        let max_scroll = {
            let cached = self.last_max_visual_scroll.get();
            if cached > 0 { cached } else { self.log_entries.len().saturating_sub(visible_height) }
        };
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }
    
    /// Scroll to end (sets offset to a large value; rendering will clamp it)
    pub fn scroll_to_end(&mut self, _visible_height: usize) {
        self.scroll_offset = usize::MAX;
    }
    
    /// Advance spinner animation (TWF Tick() equivalent)
    pub fn tick(&mut self) {
        self.spinner_index = (self.spinner_index + 1) % self.spinner_frames.len();
    }
    
    /// Get current spinner frame
    pub fn current_spinner(&self) -> &'static str {
        self.spinner_frames[self.spinner_index]
    }
    
    /// Get scroll offset (for rendering)
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
    
    /// Get log entry by absolute index
    pub fn get_log_entry(&self, index: usize) -> Option<&LogEntry> {
        self.log_entries.get(index)
    }
    
    /// Get number of log entries
    pub fn log_count(&self) -> usize {
        self.log_entries.len()
    }

    /// Get number of pending logs (not yet processed)
    pub fn pending_log_count(&self) -> usize {
        self.pending_logs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_task_panel_new() {
        let panel = TaskPanel::new();
        assert_eq!(panel.log_count(), 0);
    }

    #[test]
    fn test_task_panel_add_log() {
        let mut panel = TaskPanel::new();
        panel.add_log("Test message".to_string(), LogLevel::Info);
        assert_eq!(panel.log_count(), 1);
    }
    
    #[test]
    fn test_task_panel_pending_logs() {
        let mut panel = TaskPanel::new();
        panel.add_pending_log("Pending [OK]".to_string());
        panel.add_pending_log("Pending [FAIL]".to_string());
        assert_eq!(panel.log_count(), 0);
        
        panel.process_pending_logs(1000);
        assert_eq!(panel.log_count(), 2);
    }
    
    #[test]
    fn test_task_panel_scroll() {
        let mut panel = TaskPanel::new();
        
        // Add some logs
        for i in 0..20 {
            panel.add_log(format!("Log {}", i), LogLevel::Info);
        }
        
        // Scroll up
        panel.scroll_up();
        assert_eq!(panel.scroll_offset(), 0);  // Can't scroll up from 0
        
        // Scroll down
        panel.scroll_down(10);
        assert_eq!(panel.scroll_offset(), 1);
        
        // Scroll to end
        panel.scroll_to_end(10);
        assert_eq!(panel.scroll_offset(), usize::MAX);  // Sets to max; rendering clamps it
    }
    
    #[test]
    fn test_task_panel_spinner() {
        let mut panel = TaskPanel::new();
        assert_eq!(panel.current_spinner(), "|");
        
        panel.tick();
        assert_eq!(panel.current_spinner(), "/");
        
        panel.tick();
        assert_eq!(panel.current_spinner(), "-");
        
        panel.tick();
        assert_eq!(panel.current_spinner(), "\\");
        
        panel.tick();
        assert_eq!(panel.current_spinner(), "|");  // Wraps around
    }
    
    #[test]
    fn test_task_panel_max_lines() {
        let mut panel = TaskPanel::new();
        
        // Add 15 logs with max of 10
        for i in 0..15 {
            panel.add_log(format!("Log {}", i), LogLevel::Info);
        }
        
        panel.process_pending_logs(10);
        assert_eq!(panel.log_count(), 10);
        
        // Oldest logs should be removed
        assert_eq!(panel.get_log_entry(0).map(|e| e.message.as_str()), Some("Log 5"));
        assert_eq!(panel.get_log_entry(9).map(|e| e.message.as_str()), Some("Log 14"));
    }
}

// ============================================================================
// Rendering
// ============================================================================

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem},
    Frame,
};

/// Wrap a message into multiple lines at ` | ` separators or spaces, staying within `width` chars.
fn wrap_to_lines<'a>(message: &'a str, width: usize, style: Style) -> Vec<Line<'a>> {
    if width == 0 || message.chars().count() <= width {
        return vec![Line::from(Span::styled(message, style))];
    }

    let mut lines = Vec::new();
    let mut remaining = message;

    while !remaining.is_empty() {
        if remaining.chars().count() <= width {
            lines.push(Line::from(Span::styled(remaining, style)));
            break;
        }

        // byte index of the char at position `width`
        let slice_end = remaining
            .char_indices()
            .nth(width)
            .map(|(i, _)| i)
            .unwrap_or(remaining.len());
        let slice = &remaining[..slice_end];

        // Prefer breaking after " | ", then after a space
        let break_byte = if let Some(pos) = slice.rfind(" | ") {
            pos + " | ".len()
        } else if let Some(pos) = slice.rfind(' ') {
            pos + 1
        } else {
            slice_end
        };

        lines.push(Line::from(Span::styled(&remaining[..break_byte], style)));
        remaining = remaining[break_byte..].trim_start();
    }

    lines
}

/// Render the task panel with visual line scrolling
pub fn render_task_panel(
    frame: &mut Frame,
    area: Rect,
    task_panel: &TaskPanel,
    colors: &ColorScheme,
) {
    let mut items = Vec::new();
    let visible_height = area.height as usize;
    let panel_width = area.width as usize;
    let visual_scroll = task_panel.scroll_offset();

    // Build list of all entries with their visual line counts
    let mut entry_visual_lines: Vec<(usize, usize)> = Vec::new(); // (entry_idx, visual_line_count)
    let mut total_visual_lines = 0;

    for i in 0..task_panel.log_count() {
        if let Some(entry) = task_panel.get_log_entry(i) {
            let style = Style::default().fg(parse_color(&colors.foreground_color));
            let wrapped = wrap_to_lines(&entry.message, panel_width, style);
            let line_count = wrapped.len();
            entry_visual_lines.push((i, line_count));
            total_visual_lines += line_count;
        }
    }

    // Calculate max scroll to keep content at bottom
    let max_scroll = total_visual_lines.saturating_sub(visible_height);
    task_panel.last_max_visual_scroll.set(max_scroll);
    let actual_scroll = visual_scroll.min(max_scroll);

    // Find starting entry and line offset based on visual scroll
    let mut current_visual_line = 0;
    let mut start_entry_idx = 0;
    let mut start_line_offset = 0;

    for &(entry_idx, line_count) in &entry_visual_lines {
        if current_visual_line + line_count > actual_scroll {
            start_entry_idx = entry_idx;
            start_line_offset = actual_scroll.saturating_sub(current_visual_line);
            break;
        }
        current_visual_line += line_count;
    }

    // Render entries from start position until visible height is filled
    let mut rendered_lines = 0;
    let mut current_entry_idx = start_entry_idx;

    while rendered_lines < visible_height && current_entry_idx < task_panel.log_count() {
        if let Some(entry) = task_panel.get_log_entry(current_entry_idx) {
            let style = match entry.level {
                LogLevel::Info => Style::default().fg(parse_color(&colors.foreground_color)),
                LogLevel::Ok => Style::default().fg(parse_color(&colors.ok_color)),
                LogLevel::Fail => Style::default().fg(parse_color(&colors.error_color)),
                LogLevel::Warn => Style::default().fg(parse_color(&colors.warning_color)),
            };

            let wrapped = wrap_to_lines(&entry.message, panel_width, style);

            // Skip lines if this is the first entry (due to offset)
            let start_line = if current_entry_idx == start_entry_idx { start_line_offset } else { 0 };

            for (idx, line) in wrapped.iter().enumerate() {
                if idx < start_line {
                    continue;
                }
                if rendered_lines >= visible_height {
                    break;
                }
                items.push(ListItem::new(line.clone()));
                rendered_lines += 1;
            }
        }
        current_entry_idx += 1;
    }

    let list = List::new(items);
    frame.render_widget(list, area);
}
