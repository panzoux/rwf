//! Task Panel with state management
//!
//! This module provides a task panel with state for displaying job logs,
//! supporting expansion, scrolling, and spinner animation.

use std::time::SystemTime;
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

/// Log entry with timestamp and level
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub message: String,
    pub level: LogLevel,
}

/// Task panel with state
pub struct TaskPanel {
    /// In-memory log buffer
    log_entries: Vec<LogEntry>,
    /// Pending logs from background thread
    pending_logs: Vec<String>,
    /// Panel expansion state
    is_expanded: bool,
    /// Expanded height in lines
    expanded_height: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Spinner animation frames (TWF reference: "|", "/", "-", "\\")
    spinner_frames: [&'static str; 4],
    /// Current spinner frame index
    spinner_index: usize,
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
            is_expanded: false,
            expanded_height: 10,  // Default height
            scroll_offset: 0,
            spinner_frames: ["|", "/", "-", "\\"],
            spinner_index: 0,
        }
    }
    
    /// Add a log entry with level
    pub fn add_log(&mut self, message: String, level: LogLevel) {
        let timestamp = SystemTime::now();
        self.log_entries.push(LogEntry {
            timestamp,
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
                timestamp: SystemTime::now(),
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
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
    
    /// Scroll down
    pub fn scroll_down(&mut self, visible_height: usize) {
        let max_scroll = self.log_entries.len().saturating_sub(visible_height);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }
    
    /// Scroll to end
    pub fn scroll_to_end(&mut self, visible_height: usize) {
        self.scroll_offset = self.log_entries.len().saturating_sub(visible_height);
    }
    
    /// Toggle expand/collapse
    pub fn toggle_expanded(&mut self) {
        self.is_expanded = !self.is_expanded;
    }
    
    /// Check if panel is expanded
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }
    
    /// Get current panel height
    pub fn current_height(&self) -> usize {
        if self.is_expanded {
            self.expanded_height
        } else {
            1
        }
    }
    
    /// Resize panel up
    pub fn resize_up(&mut self, max_height: usize) {
        if self.expanded_height < max_height {
            self.expanded_height += 1;
        }
    }
    
    /// Resize panel down
    pub fn resize_down(&mut self, min_height: usize) {
        if self.expanded_height > min_height {
            self.expanded_height -= 1;
        }
    }
    
    /// Get expanded height
    pub fn expanded_height(&self) -> usize {
        self.expanded_height
    }
    
    /// Advance spinner animation (TWF Tick() equivalent)
    pub fn tick(&mut self) {
        self.spinner_index = (self.spinner_index + 1) % self.spinner_frames.len();
    }
    
    /// Get current spinner frame
    pub fn current_spinner(&self) -> &'static str {
        self.spinner_frames[self.spinner_index]
    }
    
    /// Get all log entries (for rendering)
    pub fn log_entries(&self) -> &[LogEntry] {
        &self.log_entries
    }
    
    /// Get scroll offset (for rendering)
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
    
    /// Get log entry at index (for rendering with scroll)
    pub fn get_log_entry(&self, index: usize) -> Option<&LogEntry> {
        self.log_entries.get(index + self.scroll_offset)
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
        assert!(!panel.is_expanded());
        assert_eq!(panel.expanded_height(), 10);
        assert_eq!(panel.current_height(), 1);
        assert_eq!(panel.log_count(), 0);
    }
    
    #[test]
    fn test_task_panel_toggle_expanded() {
        let mut panel = TaskPanel::new();
        assert!(!panel.is_expanded());
        
        panel.toggle_expanded();
        assert!(panel.is_expanded());
        assert_eq!(panel.current_height(), 10);
        
        panel.toggle_expanded();
        assert!(!panel.is_expanded());
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
        assert_eq!(panel.scroll_offset(), 10);  // 20 - 10 = 10
    }
    
    #[test]
    fn test_task_panel_resize() {
        let mut panel = TaskPanel::new();
        assert_eq!(panel.expanded_height(), 10);
        
        panel.resize_up(20);
        assert_eq!(panel.expanded_height(), 11);
        
        panel.resize_down(5);
        assert_eq!(panel.expanded_height(), 10);
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

/// Render the task panel
pub fn render_task_panel(
    frame: &mut Frame,
    area: Rect,
    task_panel: &TaskPanel,
    colors: &ColorScheme,
) {
    let mut items = Vec::new();
    
    // Get visible log entries based on scroll offset
    let visible_height = area.height as usize;
    let start_idx = task_panel.scroll_offset().min(task_panel.log_count().saturating_sub(1));
    let end_idx = (start_idx + visible_height).min(task_panel.log_count());
    
    for i in start_idx..end_idx {
        if let Some(entry) = task_panel.get_log_entry(i) {
            let style = match entry.level {
                LogLevel::Info => Style::default().fg(parse_color(&colors.foreground_color)),
                LogLevel::Ok => Style::default().fg(parse_color(&colors.ok_color)),
                LogLevel::Fail => Style::default().fg(parse_color(&colors.error_color)),
                LogLevel::Warn => Style::default().fg(parse_color(&colors.warning_color)),
            };
            
            let line = Line::from(Span::styled(&entry.message, style));
            items.push(ListItem::new(line));
        }
    }
    
    // If no items, show empty message
    if items.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "No active tasks",
            Style::default().fg(parse_color(&colors.foreground_color)),
        ));
        items.push(ListItem::new(empty_msg));
    }

    let list = List::new(items);
    frame.render_widget(list, area);
}
