//! Operation Report dialog rendering (Phase 7.6).
//!
//! List + detail-pane layout: modelled on `trash_browser.rs`'s scrollable
//! row-list approach (header row + selection highlight), extended with a
//! `job_manager.rs`-style detail pane for the currently-highlighted row.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use rwf_lib::model::dialog::OperationReportDialogContent;
use rwf_lib::model::{Location, OperationRecord, OperationReport, UndoAvailability};

use crate::ui::smart_truncate;

fn undo_symbol(undo: &UndoAvailability) -> &'static str {
    match undo {
        UndoAvailability::Available(_) => "\u{2713}", // check mark
        UndoAvailability::Unavailable(_) => "\u{d7}", // multiplication sign (x)
        UndoAvailability::NotApplicable => "*",
    }
}

fn result_symbol(record: &OperationRecord) -> &'static str {
    if record.succeeded {
        "OK"
    } else {
        "Fail"
    }
}

fn location_text(loc: &Option<Location>) -> String {
    loc.as_ref()
        .map(|l| l.display_path())
        .unwrap_or_else(|| "-".to_string())
}

/// Constraints: [0]=list, [1]=detail-label, [2]=detail-view, [3]=hint.
fn constraints() -> Vec<Constraint> {
    vec![
        Constraint::Min(4),
        Constraint::Length(1),
        Constraint::Length(6),
        Constraint::Length(1),
    ]
}

/// Height needed for the row list (header + up to 15 data rows), the
/// detail label, the detail view, the hint line, and the surrounding
/// border pair.
pub fn calculate_operation_report_dialog_min_height(report: &OperationReport) -> u16 {
    let list_rows = (report.records.len() as u16 + 1).clamp(4, 15); // +1 header row
    list_rows + 1 + 6 + 1 + 2 // detail-label + detail-view + hint + borders
}

pub fn render_operation_report_dialog(
    frame: &mut Frame,
    area: Rect,
    content: &OperationReportDialogContent,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let report = &content.report;
    let records = &report.records;
    let action_label = report.action_column_label();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints())
        .split(area);

    render_row_list(
        frame,
        chunks[0],
        records,
        &content.selected,
        content.cursor,
        action_label,
        base_style,
        selected_style,
    );

    let detail_label = Paragraph::new("Details:").style(base_style);
    frame.render_widget(detail_label, chunks[1]);

    render_detail(frame, chunks[2], records.get(content.cursor), base_style);

    let hint = Paragraph::new(format!(
        "Space: toggle  a: all/none  {}: run  Esc: close  \u{2191}\u{2193}: select",
        action_label.to_lowercase()
    ))
    .style(hint_style);
    frame.render_widget(hint, chunks[3]);
}

#[allow(clippy::too_many_arguments)]
fn render_row_list(
    frame: &mut Frame,
    area: Rect,
    records: &[OperationRecord],
    selected: &[bool],
    cursor: usize,
    action_label: &str,
    base_style: ratatui::style::Style,
    selected_style: ratatui::style::Style,
) {
    let item_width = area.width.saturating_sub(2) as usize;

    let header = format!(
        " {:<4}{:<28}{:<28}{:<6}{}",
        "Mark", "Source", "Destination", "Res.", action_label
    );
    frame.render_widget(
        Paragraph::new(smart_truncate(&header, item_width, "")).style(base_style),
        Rect::new(area.x, area.y, area.width, 1),
    );

    if records.is_empty() {
        let empty = Paragraph::new(" (no rows)").style(base_style);
        frame.render_widget(empty, Rect::new(area.x, area.y + 1, area.width, 1));
        return;
    }

    let list_height = area.height.saturating_sub(1) as usize;
    if list_height == 0 {
        return;
    }
    let clamped_cursor = cursor.min(records.len().saturating_sub(1));
    let scroll_start = if clamped_cursor >= list_height {
        clamped_cursor + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let idx = scroll_start + row;
        if idx >= records.len() {
            break;
        }
        let record = &records[idx];
        let mark = if selected.get(idx).copied().unwrap_or(false) {
            "[x] "
        } else {
            "[ ] "
        };
        let source = smart_truncate(&location_text(&record.source), 26, "\u{2026}");
        let dest = smart_truncate(&location_text(&record.destination), 26, "\u{2026}");
        let line = format!(
            " {:<4}{:<28}{:<28}{:<6}{}",
            mark,
            source,
            dest,
            result_symbol(record),
            undo_symbol(&record.undo)
        );
        let style = if idx == clamped_cursor {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(smart_truncate(&line, item_width, "")).style(style),
            Rect::new(area.x, area.y + 1 + row as u16, area.width, 1),
        );
    }
}

fn render_detail(
    frame: &mut Frame,
    area: Rect,
    record: Option<&OperationRecord>,
    base_style: ratatui::style::Style,
) {
    let text = match record {
        None => "No rows".to_string(),
        Some(r) => {
            let undo_line = match &r.undo {
                UndoAvailability::Available(_) => "Available".to_string(),
                UndoAvailability::Unavailable(reason) => format!("Unavailable: {reason}"),
                UndoAvailability::NotApplicable => "Not applicable".to_string(),
            };
            format!(
                "Source: {}\nDestination: {}\nResult: {}\n{}\nUndo: {}",
                location_text(&r.source),
                location_text(&r.destination),
                if r.succeeded {
                    "OK".to_string()
                } else {
                    "Fail".to_string()
                },
                r.failure_reason
                    .as_deref()
                    .map(|reason| format!("Reason: {reason}"))
                    .unwrap_or_default(),
                undo_line,
            )
        }
    };

    let detail = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(ratatui::style::Color::Black)),
        )
        .style(base_style)
        .wrap(Wrap { trim: false });

    frame.render_widget(detail, area);
}
