//! Type mismatch warning dialog rendering (Phase 7.3, magic-byte detection).
//!
//! Shown before running an `ExtensionAssociation` command when the target
//! file's leading bytes look like an executable but the extension says
//! otherwise. Confirm proceeds with the original command; Cancel does nothing
//! — both are handled generically (see `confirm.rs` / `app.rs`), this module
//! only renders the content.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use super::common::{DIALOG_DIM, DIALOG_TEXT};
use super::render_dialog_buttons;

/// Render the type mismatch warning dialog's content (message + buttons).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_type_mismatch_warning_dialog(
    frame: &mut Frame,
    content: &rwf_lib::model::dialog::DialogContent,
    area: Rect,
    path: &std::path::Path,
    detected_label: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),    // message
            Constraint::Length(3), // buttons
        ])
        .split(area);

    let lines = vec![
        Line::from(Span::styled(path.display().to_string(), DIALOG_TEXT)),
        Line::default(),
        Line::from(Span::styled(
            format!("Detected type: {}", detected_label),
            DIALOG_DIM,
        )),
        Line::default(),
        Line::from(Span::styled(
            format!(
                "This looks like a {} but the extension suggests otherwise. Run the command anyway?",
                detected_label
            ),
            DIALOG_TEXT,
        )),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[0]);

    render_dialog_buttons(frame, chunks[1], content, 0);
}
