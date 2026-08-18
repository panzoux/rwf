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
use unicode_width::UnicodeWidthStr;

use rwf_lib::model::dialog::OperationReportDialogContent;
use rwf_lib::model::{Location, OperationRecord, OperationReport, UndoAvailability};

use crate::ui::{pad_to_width, smart_truncate};

/// Width (in terminal columns) of the "[x] "/"[ ] " mark field, including
/// its own trailing space plus one more column of gap before Source — see
/// `render_row_list`. Must stay in sync between the header and data rows.
const MARK_WIDTH: usize = 5;

/// Width of the "OK"/"Fail" result column.
const RESULT_WIDTH: usize = 6;

/// Width of the history sidebar (including its own border), reserved from
/// the dialog's overall width whenever there's more than one entry to
/// browse — see `render_history_sidebar`.
const SIDEBAR_WIDTH: u16 = 20;

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
///
/// The detail view is 7 rows: 2 for its own top/bottom border plus 5 content
/// lines (Source/Destination/Result/Reason-or-blank/Undo — see
/// `render_detail`). 6 would clip the last line (Undo/Redo status) off the
/// bottom, which is the most important field in the dialog.
fn constraints() -> Vec<Constraint> {
    vec![
        Constraint::Min(4),
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(1),
    ]
}

/// Height needed for the row list (header + up to 15 data rows), the
/// detail label, the detail view, the hint line, and the surrounding
/// border pair.
pub fn calculate_operation_report_dialog_min_height(report: &OperationReport) -> u16 {
    let list_rows = (report.records.len() as u16 + 1).clamp(4, 15); // +1 header row
    list_rows + 1 + 7 + 1 + 2 // detail-label + detail-view + hint + borders
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

    // The sidebar only earns its keep once there's more than one entry to
    // browse — a single-report dialog gets the full width, same as before
    // the sidebar existed.
    let main_area = if content.history.len() > 1 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(20)])
            .split(area);
        render_history_sidebar(frame, cols[0], content, base_style, selected_style);
        cols[1]
    } else {
        area
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints())
        .split(main_area);

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

    // "(view only)" now lives in the dialog title (see
    // `Dialog::operation_report_view_at`) next to the operation name it
    // actually describes, rather than tacked onto "Details:" below.
    frame.render_widget(Paragraph::new("Details:").style(base_style), chunks[1]);

    render_detail(frame, chunks[2], records.get(content.cursor), base_style);

    let hint = if records.is_empty() {
        "Esc: close".to_string()
    } else if content.is_actionable() {
        format!(
            "Space: toggle  a: all/none  Enter: {}  Esc: close  \u{2191}\u{2193}: select{}",
            action_label.to_lowercase(),
            if content.history.len() > 1 {
                "  \u{2190}\u{2192}: history"
            } else {
                ""
            }
        )
    } else {
        // Kept to the same "key: action" list style as the actionable-report
        // hint above (not a full sentence explaining *why* — at 80 columns
        // a longer explanatory clause gets silently clipped by this
        // unwrapped Paragraph, and the title's "(view only)" suffix already
        // conveys the reason).
        "Esc: close  \u{2191}\u{2193}: select  \u{2190}\u{2192}: history".to_string()
    };
    let hint = Paragraph::new(hint).style(hint_style);
    frame.render_widget(hint, chunks[3]);
}

/// Renders the Undo/Redo history sidebar: one row per
/// `content.history` entry (newest at top), each prefixed with `u:`/`r:`
/// for its current direction (report.is_undo — see `action_column_label`)
/// and `* ` when it's the live target for that direction (both the
/// nearest-redo and nearest-undo entries get one; everything else is
/// browsable but view-only). Scrolls to keep the focused entry
/// (`content.history_cursor`) visible, same clamped-scroll approach as
/// `render_row_list`.
fn render_history_sidebar(
    frame: &mut Frame,
    area: Rect,
    content: &OperationReportDialogContent,
    base_style: ratatui::style::Style,
    selected_style: ratatui::style::Style,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(crate::ui::dialog::common::DIALOG_BORDER);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let list_height = inner.height as usize;
    if list_height == 0 || content.history.is_empty() {
        return;
    }
    let clamped_cursor = content.history_cursor.min(content.history.len() - 1);
    let scroll_start = if clamped_cursor >= list_height {
        clamped_cursor + 1 - list_height
    } else {
        0
    };
    let width = inner.width as usize;

    for row in 0..list_height {
        let idx = scroll_start + row;
        let Some(entry) = content.history.get(idx) else {
            break;
        };
        let marker = if content
            .history_actionable
            .get(idx)
            .copied()
            .unwrap_or(false)
        {
            "*"
        } else {
            " "
        };
        let direction = if entry.is_undo { "r" } else { "u" };
        let label = format!("{marker} {direction}:{}", entry.operation_name);
        let label = pad_to_width(&smart_truncate(&label, width, "\u{2026}"), width);
        let style = if idx == clamped_cursor {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(label).style(style),
            Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
        );
    }
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

    // Fixed-width columns (leading space, Mark, Result, and the trailing
    // Undo/Redo symbol column, which is exactly as wide as its header label)
    // leave whatever's left of `item_width` for Source/Destination, split
    // evenly between them. Each field is truncated and padded to an *exact*
    // display width via width-aware helpers (`smart_truncate`/`pad_to_width`,
    // not `{:<N}` char-count padding, which misaligns CJK content) before
    // the row string is assembled — so a narrow terminal clips within a
    // column instead of a whole column silently vanishing from the middle
    // of the line the way whole-line truncation with an empty ellipsis did.
    //
    // `GAP` columns of blank space are reserved between Source/Destination
    // and between Destination/Res. — without this, a Source/Destination
    // value that fills its budget exactly (a real case for long-but-not-too-
    // long ASCII paths, since `smart_truncate`/`pad_to_width` both produce
    // an *exact*-width result) would butt directly against the next column
    // with no visible separator, the same collision class MARK_WIDTH/
    // RESULT_WIDTH already avoid by reserving slack beyond their max content
    // width.
    const GAP: usize = 1;
    let action_width = action_label.width();
    let fixed_width = 1 /* leading space */ + MARK_WIDTH + RESULT_WIDTH + action_width + GAP * 2;
    let path_budget = item_width.saturating_sub(fixed_width);
    let source_width = path_budget / 2;
    let dest_width = path_budget - source_width;
    let gap = " ".repeat(GAP);

    let header = format!(
        " {}{}{gap}{}{gap}{}{}",
        pad_to_width("Mark", MARK_WIDTH),
        // Header labels are truncated too, not just padded: on an
        // extremely narrow terminal `source_width`/`dest_width` can shrink
        // below the label's own width ("Source"=6, "Destination"=11),
        // which would otherwise overflow the label into the next column.
        pad_to_width(&smart_truncate("Source", source_width, ""), source_width),
        pad_to_width(&smart_truncate("Destination", dest_width, ""), dest_width),
        pad_to_width("Res.", RESULT_WIDTH),
        action_label,
    );
    frame.render_widget(
        Paragraph::new(header).style(base_style),
        Rect::new(area.x, area.y, area.width, 1),
    );

    if records.is_empty() {
        let empty = Paragraph::new(" No operations recorded yet").style(base_style);
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
        let source = smart_truncate(&location_text(&record.source), source_width, "\u{2026}");
        let dest = smart_truncate(&location_text(&record.destination), dest_width, "\u{2026}");
        let line = format!(
            " {}{}{gap}{}{gap}{}{}",
            pad_to_width(mark, MARK_WIDTH),
            pad_to_width(&source, source_width),
            pad_to_width(&dest, dest_width),
            pad_to_width(result_symbol(record), RESULT_WIDTH),
            undo_symbol(&record.undo),
        );
        let style = if idx == clamped_cursor {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(line).style(style),
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
                .border_style(crate::ui::dialog::common::DIALOG_BORDER),
        )
        .style(base_style)
        .wrap(Wrap { trim: false });

    frame.render_widget(detail, area);
}

use crossterm::event::{KeyCode, KeyEvent};

use super::DialogAction;

/// Up/Down/j/k navigate the cursor; Space toggles the current row's
/// selection and `a` toggles all rows on/off together, but only while
/// viewing the latest report — view-only browsing disables both, matching
/// the "(view only)" label; Left/Right browse older/newer reports in
/// history (`DialogAction::NavigateReportHistory`, dispatched by the app
/// loop); Enter triggers Undo/Redo on the current selection, but only
/// while viewing the latest report (handled by `process_dialog_confirmation`
/// in Task 17, which reads `selected_reversal_actions()`); Esc closes.
pub(super) fn handle_input(
    content: &mut OperationReportDialogContent,
    key: KeyEvent,
) -> DialogAction {
    let len = content.report.records.len();
    match key.code {
        KeyCode::Esc => return DialogAction::Cancel,
        KeyCode::Enter
            if content.is_actionable() && !content.selected_reversal_actions().is_empty() =>
        {
            return DialogAction::Confirm
        }
        KeyCode::Left => return DialogAction::NavigateReportHistory { older: true },
        KeyCode::Right => return DialogAction::NavigateReportHistory { older: false },
        KeyCode::Up | KeyCode::Char('k') => {
            if content.cursor > 0 {
                content.cursor -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if content.cursor + 1 < len {
                content.cursor += 1;
            }
        }
        KeyCode::Home => content.cursor = 0,
        KeyCode::End => content.cursor = len.saturating_sub(1),
        // Marking exists to choose which rows a future Undo/Redo trigger
        // will act on — meaningless while viewing a non-actionable report,
        // since Enter is already blocked there (see the guard above). Left
        // disabled, these would silently change `selected` on a report the
        // user can never actually run, contradicting the "(view only)"
        // label.
        KeyCode::Char(' ') if content.is_actionable() => {
            if let Some(sel) = content.selected.get_mut(content.cursor) {
                *sel = !*sel;
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') if content.is_actionable() => {
            let all_selected = content.selected.iter().all(|s| *s);
            content.selected.iter_mut().for_each(|s| *s = !all_selected);
        }
        _ => {}
    }
    DialogAction::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use rwf_lib::model::{OperationRecord, OperationReport, ReversalAction, UndoAvailability};

    fn two_row_content() -> OperationReportDialogContent {
        let report = OperationReport {
            id: 1,
            operation_name: "Copy".to_string(),
            records: vec![
                OperationRecord {
                    source: Some(Location::Local("a.txt".into())),
                    destination: Some(Location::Local("b.txt".into())),
                    succeeded: true,
                    failure_reason: None,
                    undo: UndoAvailability::Available(ReversalAction::Delete {
                        target: Location::Local("b.txt".into()),
                        recreate: None,
                    }),
                },
                OperationRecord {
                    source: Some(Location::Local("c.txt".into())),
                    destination: Some(Location::Local("d.txt".into())),
                    succeeded: false,
                    failure_reason: Some("Access denied".to_string()),
                    undo: UndoAvailability::NotApplicable,
                },
            ],
            finished_at: std::time::SystemTime::now(),
            is_undo: false,
        };
        OperationReportDialogContent::new(report)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn space_toggles_current_row_selection() {
        let mut content = two_row_content();
        assert_eq!(content.selected, vec![true, true]);
        handle_input(&mut content, key(KeyCode::Char(' ')));
        assert_eq!(content.selected, vec![false, true]);
    }

    #[test]
    fn down_moves_cursor_and_space_toggles_second_row() {
        let mut content = two_row_content();
        handle_input(&mut content, key(KeyCode::Down));
        assert_eq!(content.cursor, 1);
        handle_input(&mut content, key(KeyCode::Char(' ')));
        assert_eq!(content.selected, vec![true, false]);
    }

    #[test]
    fn a_toggles_all_rows_together() {
        let mut content = two_row_content();
        handle_input(&mut content, key(KeyCode::Char('a')));
        assert_eq!(content.selected, vec![false, false]);
        handle_input(&mut content, key(KeyCode::Char('a')));
        assert_eq!(content.selected, vec![true, true]);
    }

    #[test]
    fn enter_confirms_only_when_a_row_is_actionable() {
        let mut content = two_row_content();
        // Row 0 is Available and selected by default -> Enter confirms.
        assert_eq!(
            handle_input(&mut content, key(KeyCode::Enter)),
            DialogAction::Confirm
        );

        // Deselect the only actionable row -> Enter does nothing.
        content.selected[0] = false;
        assert_eq!(
            handle_input(&mut content, key(KeyCode::Enter)),
            DialogAction::None
        );
    }

    #[test]
    fn enter_does_nothing_when_not_actionable() {
        let mut content = two_row_content();
        content.actionable = false; // browsed away from the stack top
        assert!(!content.is_actionable());

        assert_eq!(
            handle_input(&mut content, key(KeyCode::Enter)),
            DialogAction::None,
            "Enter must not confirm while browsing a non-actionable report, even with an actionable row selected"
        );
    }

    #[test]
    fn space_and_a_do_nothing_while_not_actionable() {
        let mut content = two_row_content();
        let original = content.selected.clone();
        content.actionable = false; // browsed away from the stack top
        assert!(!content.is_actionable());

        handle_input(&mut content, key(KeyCode::Char(' ')));
        assert_eq!(
            content.selected, original,
            "Space must not toggle selection while the report is view-only"
        );

        handle_input(&mut content, key(KeyCode::Char('a')));
        assert_eq!(
            content.selected, original,
            "'a' must not toggle selection while the report is view-only"
        );
    }

    #[test]
    fn left_and_right_return_navigate_report_history() {
        let mut content = two_row_content();
        assert_eq!(
            handle_input(&mut content, key(KeyCode::Left)),
            DialogAction::NavigateReportHistory { older: true }
        );
        assert_eq!(
            handle_input(&mut content, key(KeyCode::Right)),
            DialogAction::NavigateReportHistory { older: false }
        );
    }

    #[test]
    fn esc_cancels() {
        let mut content = two_row_content();
        assert_eq!(
            handle_input(&mut content, key(KeyCode::Esc)),
            DialogAction::Cancel
        );
    }
}
