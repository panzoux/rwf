//! Shared dialog styling primitives.
//!
//! Single source of truth for the standard dialog palette (black-on-gray
//! dialogs with white focus highlights). New dialog rendering code must use
//! these constants instead of inline `Style::default().fg(..).bg(..)` chains;
//! existing render functions are converted incrementally (M3).
//!
//! Layout/frame helpers live in [`super::frame`]: `render_dialog_frame`,
//! `render_dialog_buttons`, `centered_rect_abs`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

/// Normal dialog text: black on the gray dialog background.
pub const DIALOG_TEXT: Style = Style::new().fg(Color::Black).bg(Color::Gray);

/// Dimmed/secondary dialog text (hints, shortcuts, disabled items).
pub const DIALOG_DIM: Style = Style::new().fg(Color::DarkGray).bg(Color::Gray);

/// Focused/selected item: black on white.
pub const DIALOG_SELECTED: Style = Style::new().fg(Color::Black).bg(Color::White);

/// Active text-input field: white on black.
pub const DIALOG_INPUT: Style = Style::new().fg(Color::White).bg(Color::Black);

/// Positive/success accent (e.g. completed jobs) on the dialog background.
pub const DIALOG_ACCENT_GREEN: Style = Style::new().fg(Color::Green).bg(Color::Gray);

/// Warning accent (e.g. running jobs) on the dialog background.
pub const DIALOG_ACCENT_YELLOW: Style = Style::new().fg(Color::Yellow).bg(Color::Gray);

/// Dialog background fill (no foreground override).
pub const DIALOG_BG: Style = Style::new().bg(Color::Gray);

/// Dialog border: black lines on the dialog background.
pub const DIALOG_BORDER: Style = Style::new().fg(Color::Black);

/// Dialog title: bold black.
pub const DIALOG_TITLE: Style = Style::new().fg(Color::Black).add_modifier(Modifier::BOLD);

/// Standard dialog block: full border, gray background, bold black title.
///
/// Equivalent to the block built inside `render_dialog_frame`; use this when
/// a dialog needs the standard chrome but manages clearing/inner layout
/// itself.
pub fn titled_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(DIALOG_BORDER)
        .title(title)
        .title_style(DIALOG_TITLE)
        .style(DIALOG_BG)
}
