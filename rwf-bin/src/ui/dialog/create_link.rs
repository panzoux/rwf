//! Create Link dialog rendering and input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::model::dialog::CreateLinkDialog;
use rwf_lib::model::LinkCreateKind;

use super::DialogAction;
use crate::ui::text_input::{TextInput, TextInputAction};

/// Handle key input for the Create Link dialog.
///
/// Focus order: Type, Link name, OK, Cancel. `target`/`dest_dir` are fixed at
/// open and never editable. `Left`/`Right` cycle the link type when Type is
/// focused (skipping options `unavailable_reason` flags).
pub(super) fn handle_input(dialog: &mut CreateLinkDialog, key: KeyEvent) -> DialogAction {
    let total_stops = dialog.cancel_index() + 1;

    if key.code == KeyCode::Esc {
        return DialogAction::Cancel;
    }
    // Shift+Tab arrives as `BackTab`, not `Tab` with a Shift modifier.
    if key.code == KeyCode::Tab || key.code == KeyCode::BackTab {
        if key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT) {
            dialog.focused_field = if dialog.focused_field == 0 {
                total_stops - 1
            } else {
                dialog.focused_field - 1
            };
        } else {
            dialog.focused_field = (dialog.focused_field + 1) % total_stops;
        }
        return DialogAction::None;
    }
    if key.code == KeyCode::Enter {
        return if dialog.focused_field == dialog.cancel_index() {
            DialogAction::Cancel
        } else {
            DialogAction::Confirm
        };
    }

    if dialog.focused_field == 0 {
        if matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
        ) {
            dialog.cycle_kind();
        }
        return DialogAction::None;
    }

    if dialog.focused_field == 1 {
        let mut ti = TextInput::new(
            Some(dialog.link_name.clone()),
            rwf_lib::config::EditMode::Emacs,
        );
        ti.set_cursor(dialog.link_name_cursor_pos);
        ti.set_scroll(dialog.link_name_scroll_pos);
        let action = ti.handle_input(&key);
        dialog.link_name = ti.text().to_string();
        dialog.link_name_cursor_pos = ti.cursor();
        dialog.link_name_scroll_pos = ti.scroll();
        return match action {
            TextInputAction::Confirm => DialogAction::Confirm,
            TextInputAction::Cancel => DialogAction::Cancel,
            _ => DialogAction::None,
        };
    }

    DialogAction::None
}

fn kind_label(kind: LinkCreateKind) -> &'static str {
    match kind {
        LinkCreateKind::Symlink => "Symlink",
        LinkCreateKind::Hardlink => "Hardlink",
        #[cfg(windows)]
        LinkCreateKind::Junction => "Junction",
    }
}

fn all_kinds() -> &'static [LinkCreateKind] {
    #[cfg(windows)]
    {
        &[
            LinkCreateKind::Symlink,
            LinkCreateKind::Hardlink,
            LinkCreateKind::Junction,
        ]
    }
    #[cfg(unix)]
    {
        &[LinkCreateKind::Symlink, LinkCreateKind::Hardlink]
    }
}

pub(super) fn render_create_link_dialog(frame: &mut Frame, area: Rect, dialog: &CreateLinkDialog) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1); 10])
        .split(area);

    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let dim_style = crate::ui::dialog::common::DIALOG_DIM;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED;
    let item_width = area.width.saturating_sub(4);
    let x = area.x + 2;

    // Row 0: Type selector
    let mut spans = vec![Span::styled("Type:  ", base_style)];
    for kind in all_kinds() {
        let is_selected = *kind == dialog.kind;
        let is_available = dialog.is_kind_available(*kind);
        let marker = if is_selected { "(*)" } else { "( )" };
        let style = if !is_available {
            // Grayed out: visually unselectable, regardless of focus state.
            dim_style
        } else if is_selected && dialog.focused_field == 0 {
            selected_style
        } else {
            base_style
        };
        spans.push(Span::styled(
            format!("{} {}   ", marker, kind_label(*kind)),
            style,
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(base_style),
        Rect::new(x, chunks[0].y, item_width, 1),
    );

    // Row 1: unavailable-option reasons
    let reasons: Vec<String> = all_kinds()
        .iter()
        .filter_map(|k| {
            dialog
                .unavailable_reason(*k)
                .map(|r| format!("\u{2717} {}: {}", kind_label(*k), r))
        })
        .collect();
    if !reasons.is_empty() {
        frame.render_widget(
            Paragraph::new(reasons.join("   ")).style(dim_style),
            Rect::new(x, chunks[1].y, item_width, 1),
        );
    }

    // Row 3: "Link name:" label
    frame.render_widget(
        Paragraph::new("Link name:").style(base_style),
        Rect::new(x, chunks[3].y, item_width, 1),
    );
    // Row 4: dest_dir (display-only)
    frame.render_widget(
        Paragraph::new(dialog.dest_dir.display().to_string()).style(dim_style),
        Rect::new(x, chunks[4].y, item_width, 1),
    );
    // Row 5: link_name (editable)
    {
        let mut ti = TextInput::new(
            Some(dialog.link_name.clone()),
            rwf_lib::config::EditMode::Emacs,
        );
        ti.set_cursor(dialog.link_name_cursor_pos);
        ti.set_scroll(dialog.link_name_scroll_pos);
        ti.set_width(item_width);
        ti.render(
            frame,
            Rect::new(x, chunks[5].y, item_width, 1),
            dialog.focused_field == 1,
        );
    }

    // Row 7: "Target (what it points to):" label
    frame.render_widget(
        Paragraph::new("Target (what it points to):").style(base_style),
        Rect::new(x, chunks[7].y, item_width, 1),
    );
    // Row 8: target (display-only)
    frame.render_widget(
        Paragraph::new(format!(
            "{}  ({})",
            dialog.target.display_path(),
            dialog.target_kind_label()
        ))
        .style(dim_style),
        Rect::new(x, chunks[8].y, item_width, 1),
    );

    // Row 9: buttons
    let ok_style = if dialog.focused_field == dialog.ok_index() {
        selected_style
    } else {
        base_style
    };
    let cancel_style = if dialog.focused_field == dialog.cancel_index() {
        selected_style
    } else {
        base_style
    };
    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line)
            .alignment(Alignment::Center)
            .style(base_style),
        chunks[9],
    );
}
