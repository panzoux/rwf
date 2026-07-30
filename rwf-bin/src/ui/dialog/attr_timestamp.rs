//! Attribute/timestamp change dialog rendering and input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::model::dialog::AttrTimestampDialog;

use super::DialogAction;
#[cfg(unix)]
use crate::ui::text_input::{TextInput, TextInputAction};

/// Handle key input for the Attribute/Timestamp dialog.
///
/// Focus order: platform fields (checkboxes on Windows / mode+timestamps on
/// Unix)..., OK, Cancel. `Space` toggles a focused checkbox; `t` stamps "now"
/// into a focused timestamp field; `Tab`/`Shift+Tab` cycles focus; `Enter`
/// confirms unless Cancel is focused; `Esc` always cancels.
pub(super) fn handle_input(dialog: &mut AttrTimestampDialog, key: KeyEvent) -> DialogAction {
    let total_stops = dialog.cancel_index() + 1;

    if key.code == KeyCode::Esc {
        return DialogAction::Cancel;
    }
    // Shift+Tab arrives as `BackTab`, not `Tab` with a Shift modifier — most
    // terminals never report Tab+Shift as such (see `TextInput`'s own
    // `(KeyCode::BackTab, _) => PrevField` handling for the same reason).
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

    #[cfg(windows)]
    {
        match dialog.focused_field {
            0 => return handle_checkbox_key(key, &mut dialog.readonly),
            1 => return handle_checkbox_key(key, &mut dialog.hidden),
            2 => return handle_checkbox_key(key, &mut dialog.system),
            3 => return handle_checkbox_key(key, &mut dialog.archive),
            4 => return handle_timestamp_field_key(key, &mut dialog.modified),
            5 => return handle_timestamp_field_key(key, &mut dialog.accessed),
            6 => return handle_timestamp_field_key(key, &mut dialog.created),
            _ => {}
        }
    }
    #[cfg(unix)]
    {
        match dialog.focused_field {
            0 => return handle_text_field_key(key, &mut dialog.mode),
            1 => return handle_timestamp_field_key(key, &mut dialog.modified),
            2 => return handle_timestamp_field_key(key, &mut dialog.accessed),
            _ => {}
        }
    }

    DialogAction::None
}

fn handle_checkbox_key(
    key: KeyEvent,
    field: &mut rwf_lib::model::dialog::TriToggle,
) -> DialogAction {
    if key.code == KeyCode::Char(' ') {
        field.toggle();
    }
    DialogAction::None
}

/// Free-form text editing (used only for the Unix octal `mode` field).
#[cfg(unix)]
fn handle_text_field_key(
    key: KeyEvent,
    field: &mut rwf_lib::model::dialog::AttrTextField,
) -> DialogAction {
    if key.code == KeyCode::Char('t') && key.modifiers.is_empty() {
        field.set_now();
        return DialogAction::None;
    }
    let mut ti = TextInput::new(Some(field.text.clone()), rwf_lib::config::EditMode::Emacs);
    ti.set_cursor(field.cursor_pos);
    ti.set_scroll(field.scroll_pos);
    let action = ti.handle_input(&key);
    field.text = ti.text().to_string();
    field.cursor_pos = ti.cursor();
    field.scroll_pos = ti.scroll();
    match action {
        TextInputAction::Confirm => DialogAction::Confirm,
        TextInputAction::Cancel => DialogAction::Cancel,
        _ => DialogAction::None,
    }
}

/// Segmented digit-overwrite editing for a `"YYYY-MM-DD HH:MM:SS"` field.
/// Only `0`-`9` (overwrite-and-advance), `t`/`T` (stamp now), arrow keys,
/// Backspace, and Home/End are accepted; everything else is a true no-op —
/// no character ever gets inserted only to be silently dropped at submit
/// time (the previous free-text editor's failure mode).
fn handle_timestamp_field_key(
    key: KeyEvent,
    field: &mut rwf_lib::model::dialog::AttrTextField,
) -> DialogAction {
    match key.code {
        KeyCode::Char(c @ '0'..='9') => field.apply_timestamp_digit(c),
        KeyCode::Char('t' | 'T') if key.modifiers.is_empty() => field.set_now(),
        KeyCode::Left | KeyCode::Backspace => field.move_timestamp_cursor_left(),
        KeyCode::Right => field.move_timestamp_cursor_right(),
        KeyCode::Home => field.move_timestamp_cursor_home(),
        KeyCode::End => field.move_timestamp_cursor_end(),
        _ => {}
    }
    DialogAction::None
}

pub(super) fn render_attr_timestamp_dialog(
    frame: &mut Frame,
    area: Rect,
    dialog: &AttrTimestampDialog,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1); 8])
        .split(area);

    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    #[cfg(unix)]
    let dim_style = crate::ui::dialog::common::DIALOG_DIM;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED;
    let item_width = area.width.saturating_sub(4);
    let x = area.x + 2;

    #[cfg(windows)]
    {
        frame.render_widget(
            Paragraph::new("Attributes").style(base_style),
            Rect::new(x, chunks[0].y, item_width, 1),
        );

        let checkbox_span =
            |label: &str, field: &rwf_lib::model::dialog::TriToggle, focused: bool| {
                let style = if focused { selected_style } else { base_style };
                Span::styled(format!("{} {}  ", field.label(), label), style)
            };
        let row = Line::from(vec![
            checkbox_span("ReadOnly", &dialog.readonly, dialog.focused_field == 0),
            checkbox_span("Hidden", &dialog.hidden, dialog.focused_field == 1),
            checkbox_span("System", &dialog.system, dialog.focused_field == 2),
            checkbox_span("Archive", &dialog.archive, dialog.focused_field == 3),
        ]);
        frame.render_widget(
            Paragraph::new(row).style(base_style),
            Rect::new(x, chunks[1].y, item_width, 1),
        );

        frame.render_widget(
            Paragraph::new("Timestamps").style(base_style),
            Rect::new(x, chunks[3].y, item_width, 1),
        );

        render_timestamp_row(
            frame,
            x,
            chunks[4],
            item_width,
            "Modified",
            &dialog.modified,
            dialog.focused_field == 4,
        );
        render_timestamp_row(
            frame,
            x,
            chunks[5],
            item_width,
            "Accessed",
            &dialog.accessed,
            dialog.focused_field == 5,
        );

        render_timestamp_row(
            frame,
            x,
            chunks[6],
            item_width,
            "Created",
            &dialog.created,
            dialog.focused_field == 6,
        );
    }
    #[cfg(unix)]
    {
        frame.render_widget(
            Paragraph::new("Permissions").style(base_style),
            Rect::new(x, chunks[0].y, item_width, 1),
        );

        {
            let label_width = 10u16;
            frame.render_widget(
                Paragraph::new("Mode (octal)").style(base_style),
                Rect::new(x, chunks[1].y, label_width, 1),
            );
            let mut ti = TextInput::new(
                Some(dialog.mode.text.clone()),
                rwf_lib::config::EditMode::Emacs,
            );
            ti.set_cursor(dialog.mode.cursor_pos);
            ti.set_scroll(dialog.mode.scroll_pos);
            ti.set_width(8);
            ti.render(
                frame,
                Rect::new(x + label_width, chunks[1].y, 8, 1),
                dialog.focused_field == 0,
            );
        }
        frame.render_widget(
            Paragraph::new(dialog.mode_rwx_preview()).style(dim_style),
            Rect::new(x, chunks[2].y, item_width, 1),
        );

        frame.render_widget(
            Paragraph::new("Timestamps").style(base_style),
            Rect::new(x, chunks[4].y, item_width, 1),
        );

        render_timestamp_row(
            frame,
            x,
            chunks[5],
            item_width,
            "Modified",
            &dialog.modified,
            dialog.focused_field == 1,
        );
        render_timestamp_row(
            frame,
            x,
            chunks[6],
            item_width,
            "Accessed",
            &dialog.accessed,
            dialog.focused_field == 2,
        );
    }

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
        chunks[7],
    );
}

fn render_timestamp_row(
    frame: &mut Frame,
    x: u16,
    row: Rect,
    item_width: u16,
    label: &str,
    field: &rwf_lib::model::dialog::AttrTextField,
    focused: bool,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED;
    let label_width = 10u16;
    frame.render_widget(
        Paragraph::new(format!("{:<9}", label)).style(base_style),
        Rect::new(x, row.y, label_width, 1),
    );
    let text_width = item_width.saturating_sub(label_width + 6);
    // Segmented overwrite editing (see `apply_timestamp_digit`): highlight
    // the digit under the cursor with a reverse-video block instead of an
    // insertion-point cursor, matching the "pick a segment, overwrite it"
    // interaction rather than free-text insertion.
    let display = if field.text.is_empty() {
        "<mixed>".to_string()
    } else {
        field.text.clone()
    };
    let spans: Vec<Span> = display
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let style = if focused
                && i == field
                    .cursor_pos
                    .min(display.chars().count().saturating_sub(1))
            {
                selected_style
            } else {
                base_style
            };
            Span::styled(c.to_string(), style)
        })
        .collect();
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect::new(x + label_width, row.y, text_width, 1),
    );
    frame.render_widget(
        Paragraph::new("[t:now]").style(crate::ui::dialog::common::DIALOG_DIM),
        Rect::new(x + label_width + text_width + 1, row.y, 8, 1),
    );
}
