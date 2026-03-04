//! Terminal initialization and cleanup
//!
//! This module handles terminal setup and restoration using crossterm.

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

/// Terminal wrapper that ensures proper cleanup
pub struct TerminalManager {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalManager {
    /// Initialize the terminal with raw mode and alternate screen
    pub fn new() -> Result<Self> {
        // Enter alternate screen FIRST before any output
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        // Enable raw mode for character-by-character input
        enable_raw_mode()?;

        // Create ratatui terminal
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        
        // Clear the terminal to ensure clean state
        terminal.clear()?;

        Ok(Self { terminal })
    }

    /// Get a mutable reference to the terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Restore terminal to original state
    pub fn restore(&mut self) -> Result<()> {
        // Disable raw mode
        disable_raw_mode()?;

        // Leave alternate screen
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;

        // Show cursor
        self.terminal.show_cursor()?;

        Ok(())
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        // Ensure terminal is restored even if restore() wasn't called
        let _ = self.restore();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_terminal_manager_creation() {
        // This test is mainly to ensure the code compiles
        // Actual terminal initialization would require a TTY
        // which may not be available in CI environments
    }
}
