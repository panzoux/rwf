//! Dialog system with centralized input handling
//!
//! This module provides a hybrid dialog system with:
//! - Common infrastructure (border, title, buttons, centering)
//! - Content-specific rendering via trait
//! - Centralized input handling with consistent shortcuts

mod attr_timestamp;
mod basic;
mod close_tab_with_active_job;
pub mod common;
mod compression;
mod confirm;
mod context_menu;
mod create_link;
mod custom_function;
mod drive_selection;
mod extract_confirm;
mod file_conflict;
mod file_info;
mod file_mask;
mod frame;
mod help;
mod history;
mod job_manager;
mod jump_to_file;
mod jump_to_path;
mod multiline_input;
mod open_with_picker;
mod operation_report;
mod pattern_rename;
mod registered_folder;
mod simple_rename;
#[cfg(test)]
mod snapshot_tests;
mod sort;
#[cfg(test)]
mod test_support;
mod trash_browser;
mod type_mismatch_warning;
mod wildcard_mark;

use attr_timestamp::render_attr_timestamp_dialog;
use basic::{handle_content_input, render_dialog_content, CONFIRMATION_MESSAGE_MAX_LINES};
use context_menu::render_context_menu_dialog;
use create_link::render_create_link_dialog;
use custom_function::{render_custom_function_menu, render_custom_function_selector};
use drive_selection::render_drive_selection_dialog;
use file_conflict::render_file_conflict_dialog;
use file_info::render_file_info_dialog;
use file_mask::render_file_mask_dialog;
use help::render_help_dialog;
use history::render_history_dialog;
use jump_to_file::render_jump_to_file_dialog;
use jump_to_path::render_jump_to_path_dialog;
use multiline_input::render_multiline_input_dialog;
use open_with_picker::{candidate_label, render_open_with_picker};
use operation_report::{
    calculate_operation_report_dialog_min_height, render_operation_report_dialog,
};
use pattern_rename::render_pattern_rename_dialog;
use registered_folder::render_registered_folder_selector;
use simple_rename::render_simple_rename_dialog;
use sort::render_sort_dialog;
use trash_browser::render_trash_browser_dialog;
use type_mismatch_warning::render_type_mismatch_warning_dialog;
use wildcard_mark::render_wildcard_mark_dialog;

use common::{DIALOG_DIM, DIALOG_TEXT};

pub use confirm::{
    delete_job_name, process_dialog_confirmation, process_dialog_delete, restore_job_name,
    trash_job_name,
};
pub use frame::{centered_rect_abs, render_dialog_buttons, render_dialog_frame};
pub use job_manager::{
    calculate_job_manager_dialog_min_height, render_job_manager_dialog, JobManagerDialogState,
};

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::Line,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::model::dialog::{
    ActionConfirmDialog, CloseTabWithActiveJobDialog, CompressionDialog, ContextMenuDialog,
    CustomFunctionMenuDialog, CustomFunctionSelectorContent, DeleteConfirmDialog, Dialog,
    DialogContent, DialogUiState, DriveSelectionDialog, ErrorDialog, FileConflictDialog,
    FileInfoDialog, FileMaskDialog, HelpDialog, HistoryDialogContent, JobManagerContent,
    JumpToFileDialog, JumpToPathDialog, OpenWithPickerDialog, OperationReportDialogContent,
    PatternRenameContent, RegisteredFolderSelectorContent, SimpleRenameDialog, SortDialog,
    TrashBrowserDialog, TypeMismatchWarningDialog, WildcardMarkDialog,
};
use tracing::debug;

use super::smart_truncate;

/// Result of dialog input handling
#[derive(Debug, Clone, PartialEq)]
pub enum DialogAction {
    None,
    Confirm,
    ConfirmAll,
    Cancel,
    NextField,
    PrevField,
    PatternChanged,
    RotateLanguage,
    DeleteSelected,
    /// Open a second-level menu dialog for a menu-type custom function
    OpenMenu {
        title: String,
        items: Vec<rwf_lib::model::dialog::MenuItem>,
    },
    /// Toggle the File Information dialog's header-bytes view between hex
    /// and raw text (Phase 7.3b, Task 10). The dialog stays open; the app
    /// loop dispatches `Transition::ToggleFileInfoHeaderView`.
    ToggleHeaderView,
    /// Cycle the File Information dialog's manual text-encoding override for
    /// the header-bytes text-mode view (Phase 7.3b, Task 12). The dialog
    /// stays open; the app loop dispatches
    /// `Transition::CycleFileInfoHeaderEncoding`.
    CycleHeaderEncoding,
    /// Move the Operation Report dialog's view to an older/newer report in
    /// history (Left/Right in `operation_report::handle_input`). The app
    /// loop dispatches `Transition::NavigateOperationReportHistory`.
    NavigateReportHistory {
        older: bool,
    },
}

fn archive_ext_for_format(fmt: rwf_lib::ArchiveFormat) -> &'static str {
    match fmt {
        rwf_lib::ArchiveFormat::ZIP => "zip",
        rwf_lib::ArchiveFormat::SevenZip => "7z",
        rwf_lib::ArchiveFormat::Tar => "tar",
        rwf_lib::ArchiveFormat::TarGz => "tgz",
    }
}

/// Chunk `text` into at most `max_lines` rows of `width` terminal columns, respecting double-width chars.
fn chunk_path_preview(text: &str, width: u16, max_lines: u16) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthChar;
    let w = width as usize;
    if text.is_empty() || w == 0 {
        return vec![];
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < text.len() && out.len() < max_lines as usize {
        let prefix = if out.is_empty() { 1usize } else { 0 };
        let avail = w.saturating_sub(prefix);
        let mut cols = 0usize;
        let mut end = pos;
        for c in text[pos..].chars() {
            let cw = c.width().unwrap_or(1);
            if cols + cw > avail {
                break;
            }
            cols += cw;
            end += c.len_utf8();
        }
        if end == pos {
            break;
        }
        let chunk = &text[pos..end];
        let s = if out.is_empty() {
            format!(" {chunk}")
        } else {
            chunk.to_string()
        };
        out.push(Line::from(s));
        pos = end;
    }
    out
}

/// Default dialog width: 60% of the screen, at least 40, capped to fit the
/// screen minus its border columns. Shared by the width match's `_ => ...`
/// arm below and by any dialog (like TypeMismatchWarning) whose height
/// calculation needs to know its own eventual width before the width match
/// runs — a single definition so the two can't drift apart.
fn default_dialog_width(screen_width: u16) -> u16 {
    ((screen_width * 60) / 100)
        .max(40)
        .min(screen_width.saturating_sub(2))
}

/// Minimum dialog border-to-border width (in columns) needed so the File
/// Info dialog's hex-mode preview can show one full 16-byte row without
/// clipping the ASCII column — derived directly from
/// `render_file_info_dialog`'s hex-row line, not guessed:
///
/// ```text
/// let line = format!("{:06X}  {} {}", offset, hex_str, ascii_str);
/// ```
///
/// - `{:06X}` offset               ->  6 columns
/// - `"  "` separator              ->  2 columns
/// - `hex_str` (`format_hex_row`, a full 16-byte row: 16×"XX " = 48 cols,
///   plus 1 extra mid-row gap space at the 8-byte boundary) -> 49 columns
///   (see `format_hex_row_full_16_byte_row` in `rwf-lib/src/model/viewer.rs`,
///   which pins this exact string).
/// - `" "` separator before the ASCII column -> 1 column
/// - `ascii_str` (`TextEncoding::decode_row_chars`, at most one char per
///   input byte, so at most 16 columns for a 16-byte row) -> 16 columns
///
/// Total hex-row content = 6 + 2 + 49 + 1 + 16 = 74 columns.
///
/// That content is rendered into `w = area.width - 4` (2-column left/right
/// margin applied inside `render_file_info_dialog`), where `area` is already
/// the dialog's *inner* content area (`Block::inner` subtracts 1 column of
/// border on each side from the outer dialog width). So:
///
/// ```text
/// dialog_width - 2 (borders) - 4 (render_file_info_dialog's margin) >= 74
/// dialog_width >= 80
/// ```
///
/// The other rows that share this width ("Text encoding: <name>" and the
/// "(showing first N of M bytes)" truncation indicator) are both well under
/// 74 columns even in worst-case cases (longest encoding name is
/// "Windows-1252" at 12 chars; the truncation line tops out around 50 chars
/// even for a `u64::MAX`-sized file), so the hex row is the true binding
/// constraint, not those lines. The Name/Path rows are excluded entirely:
/// they already gracefully ellipsis-truncate via `smart_truncate`, so they
/// never force extra width.
const FILE_INFO_HEX_ROW_DIALOG_WIDTH: u16 = 80;

/// File Info dialog width: exactly enough columns to show a full hex row
/// (see `FILE_INFO_HEX_ROW_DIALOG_WIDTH`) with no wasted stretch beyond that
/// — the user's complaint was clipping, not a desire for extra blank space,
/// so unlike `TypeMismatchWarning`/`Help`/etc. this is NOT a "stretch to N%
/// of screen" formula. It's capped at 90% of screen width (the widest the
/// user asked any dialog to go) and floored at the same 40-column minimum
/// every other dialog uses, so on a narrow terminal where even 90% can't fit
/// the ideal 80 columns, the dialog still uses up to that 90%/40-floor
/// range and simply clips the hex row — an accepted, inherent limit of a
/// 16-byte-per-row hex format in a genuinely narrow terminal.
fn file_info_dialog_width(screen_width: u16) -> u16 {
    let cap_90 = (screen_width * 90) / 100;
    FILE_INFO_HEX_ROW_DIALOG_WIDTH
        .min(cap_90)
        .max(40)
        .min(screen_width.saturating_sub(2))
}

/// Render a dialog overlay centered on screen
pub fn render_dialog(frame: &mut Frame, dialog: &Dialog, state: &rwf_lib::AppState) {
    let screen_width = frame.area().width;

    // Calculate minimum dialog height based on content type BEFORE rendering (Part 1.1, 1.2)
    let min_content_height = match &dialog.content {
        DialogContent::Compression { .. } => {
            // Calculate from actual layout constraints
            crate::ui::dialog::compression::calculate_compression_dialog_min_height()
        }
        DialogContent::ExtractionConfirm(_) => {
            // Extraction dialog: ~6 lines content
            6u16
        }
        DialogContent::DeleteConfirm(DeleteConfirmDialog { targets, .. }) => {
            // layout: header(1)+blank(1)+list(N≤12) | spacer(1) | hint(1) | buttons(3)
            // min_content = (N+2) + 1 + 1 + 3 = N + 7
            (targets.len().min(12) as u16 + 7).max(10)
        }
        DialogContent::TypeMismatchWarning(TypeMismatchWarningDialog { path, .. }) => {
            // blank(1) + detected-type(1) + blank(1) + message (up to 3 wrapped rows) +
            // buttons(3) = 9, plus however many rows the path itself wraps into (usually
            // 1, but a long/deeply nested path can take more — without this the warning
            // message gets silently clipped off the bottom of the dialog).
            //
            // TypeMismatchWarning has no width arm of its own, so it uses
            // default_dialog_width (same formula as the width match's `_ => ...` arm)
            // to know how many columns the path has to wrap into.
            let content_width =
                default_dialog_width(screen_width).saturating_sub(2).max(1) as usize;
            let path_cols = {
                use unicode_width::UnicodeWidthStr;
                path.display().to_string().width()
            };
            let path_rows = path_cols
                .div_ceil(content_width)
                .min(u16::MAX as usize)
                .max(1) as u16;
            path_rows + 9
        }
        DialogContent::JobManager { .. } => {
            // Job Manager dialog: calculate from constraints (Part 6.2)
            calculate_job_manager_dialog_min_height()
        }
        DialogContent::FileConflict { .. } => {
            // File Conflict dialog: 19 lines content + 5 for buttons = 24 lines
            24u16
        }
        DialogContent::SortDialog { .. } => {
            // Sort key list (4) + spacer (1) + order list (2) + spacer (1) + buttons (3) = 11
            11u16
        }
        DialogContent::FileMask { .. } => {
            // blank(1) + prompt(1) + textbox(1) + hint1(1) + hint2(1) + hint3(1) + blank(1) + buttons(1) = 8
            8u16
        }
        DialogContent::WildcardMark { .. } | DialogContent::SimpleRename { .. } => {
            // prompt(1) + textbox(1) + hint(1) + spacer(1) + buttons(1) = 5
            5u16
        }
        DialogContent::AttrTimestamp { .. } => {
            // label(1) + fields(1) + spacer/preview(1) + label(1) + 2 timestamp rows(2) +
            // created/spacer(1) + buttons(1) = 8 (same row count on both platforms)
            8u16
        }
        DialogContent::CreateLink { .. } => {
            // type(1) + reasons(1) + spacer(1) + label(1) + dest_dir(1) + link_name(1) +
            // spacer(1) + label(1) + target(1) + buttons(1) = 10
            10u16
        }
        DialogContent::HistoryDialog(HistoryDialogContent {
            left_entries,
            right_entries,
            active_pane,
            ..
        }) => {
            use rwf_lib::model::ui::ActivePane;
            let len = match active_pane {
                ActivePane::Left => left_entries.len(),
                ActivePane::Right => right_entries.len(),
            };
            (len as u16 + 2).max(5)
        }
        DialogContent::DriveSelection(DriveSelectionDialog { drives, .. }) => {
            // list + hint(1) + search(1)
            (drives.len() as u16 + 2).max(6)
        }
        DialogContent::TrashBrowser(TrashBrowserDialog { records, .. }) => {
            // list + restore-destination(1) + hint(1)
            (records.len() as u16 + 2).max(6)
        }
        DialogContent::OperationReportView(OperationReportDialogContent { report, .. }) => {
            calculate_operation_report_dialog_min_height(report)
        }
        DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
            folders,
            ..
        }) => {
            // list + hint(1) + search(1)
            (folders.len() as u16 + 2).max(6)
        }
        DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
            functions, ..
        }) => {
            // list + hint(1) + filter(1)
            (functions.len() as u16 + 2).max(6)
        }
        DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog { items, .. }) => {
            // items + hint(1)
            (items.len() as u16 + 1).max(4)
        }
        DialogContent::OpenWithPicker(OpenWithPickerDialog { candidates, .. }) => {
            // candidates + hint(1)
            (candidates.len() as u16 + 1).max(4)
        }
        DialogContent::ContextMenu(ContextMenuDialog { options, .. }) => {
            // options list + hint(1)
            (options.len() as u16 + 1).max(4)
        }
        DialogContent::JumpToPath(JumpToPathDialog { suggestions, .. }) => {
            // input(1) + sep(1) + list(up to 10) + sep(1) + preview(1) + hint(1) = list+5, min 8
            (suggestions.len().min(10) as u16 + 5).max(8)
        }
        DialogContent::JumpToFile(JumpToFileDialog { suggestions, .. }) => {
            (suggestions.len().min(10) as u16 + 5).max(8)
        }
        DialogContent::FileInfo(FileInfoDialog {
            size,
            link_target,
            detecting,
            detected_type,
            header_bytes,
            header_encoding,
            ..
        }) => {
            let base = if link_target.is_some() { 12u16 } else { 11u16 };
            let base = if *detecting || detected_type.is_some() {
                base + 1
            } else {
                base
            };
            // Up to 4 rows (64 bytes / 16 per row, or 4×~16-char text-wrap
            // lines) for the header-bytes hex/text view (Task 10).
            let base = if header_bytes.is_some() {
                base + 4
            } else {
                base
            };
            // +1 for the "Text encoding: ..." row (Task 12), shown whenever
            // `header_encoding` is set — which is always alongside
            // `header_bytes` (see job.rs's FileInfoDisplay success arm), same
            // conditioning as that field's own +4 above. Without this, the
            // encoding row silently eats the one row of slack the old
            // formula happened to leave before the hint line.
            let base = if header_encoding.is_some() {
                base + 1
            } else {
                base
            };
            // +1 for the "(showing first N of M bytes)" truncation indicator
            // (Phase 7.3b, Task 14), shown whenever the file is bigger than
            // the captured `header_bytes` window — same condition
            // `render_file_info_dialog` uses. Without this, the indicator row
            // silently eats the last row of slack before the hint line, just
            // like the encoding row did before Task 12's height fix.
            match header_bytes {
                Some(bytes) if *size > bytes.len() as u64 => base + 1,
                _ => base,
            }
        } // name+path+size+type+3×datetime + hint (+1 for link row, +1 for detected type, +4 for header bytes, +1 for encoding row)
        DialogContent::PatternRename(PatternRenameContent { preview, .. }) => {
            // find(1) + replace(1) + flags(1) + mode-row(1) + separator(1) + preview rows + status(1) = 6 + preview count, min 8
            (preview.len() as u16 + 6).max(8)
        }
        DialogContent::Help { .. } => {
            // tab bar(1) + search(1) + entries + hint(1), min 8
            20u16
        }
        DialogContent::Error(ErrorDialog {
            message, details, ..
        }) => {
            // The generic render path splits this interior into
            // `Constraint::Min(5)` for content and `Constraint::Length(3)` for
            // the button row, so it needs **8** rows however short the message
            // is. The old formula gave `(lines + 4).max(5)` — 5 for a one-line
            // message — and the button row was pushed past the bottom border and
            // drawn outside the box. Reported from a diagnostic bundle on
            // 2026-08-12, where "[*OK*]" sat on the row below `└────┘`.
            //
            // `details`, when present, renders as a blank line plus its own
            // lines (see `basic.rs`) and must be counted too — otherwise a long
            // detail string overflows the same way.
            let detail_rows = details.as_ref().map_or(0, |d| d.lines().count() as u16 + 1);
            (message.lines().count() as u16 + detail_rows + 3).max(8)
        }
        DialogContent::Input { .. } => {
            // prompt(1) + textbox(1) + hint(1) = 3
            3u16
        }
        DialogContent::Confirmation(ActionConfirmDialog { message, stats, .. }) => {
            // Same shape as the Error arm above: generic render path splits
            // the interior into Constraint::Min(5) content + Constraint::
            // Length(3) buttons, so it needs the message's rendered line
            // count + 3. The message is open-ended (Phase 7.6's Undo/Redo
            // blocked-rows summary adds one line per blocked row) so it's
            // capped at CONFIRMATION_MESSAGE_MAX_LINES, +1 for a "... N more"
            // indicator when truncated — this must track what
            // `render_dialog_content`'s Confirmation arm (basic.rs) actually
            // draws, or the dialog either wastes space or clips content.
            let msg_lines = message.lines().count() as u16;
            let visible_lines = msg_lines.min(CONFIRMATION_MESSAGE_MAX_LINES);
            let truncated_indicator = if msg_lines > CONFIRMATION_MESSAGE_MAX_LINES {
                1
            } else {
                0
            };
            // stats renders as a blank separator line + one summary line.
            let stats_lines = if stats.is_some() { 2 } else { 0 };
            (visible_lines + truncated_indicator + stats_lines + 3).max(8)
        }
        _ => 8u16, // Default
    };

    // Add 2 for borders (top + bottom)
    let min_dialog_height = min_content_height + 2;

    let screen_height = frame.area().height;

    // For compression and job manager dialogs, use exact minimum height (no extra space)
    // For other dialogs, use 70% of screen or minimum, whichever is larger
    let dialog_height = match &dialog.content {
        DialogContent::Compression { .. } | DialogContent::JobManager { .. } => {
            // Use exact minimum height, but ensure it fits on screen
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::HistoryDialog(_)
        | DialogContent::DriveSelection(_)
        | DialogContent::PatternRename { .. }
        | DialogContent::Help { .. }
        | DialogContent::RegisteredFolderSelector(_)
        | DialogContent::CustomFunctionSelector { .. }
        | DialogContent::JumpToPath(_)
        | DialogContent::JumpToFile(_)
        | DialogContent::DeleteConfirm(_) => {
            let percent_height = (screen_height * 80) / 100;
            percent_height
                .max(min_dialog_height)
                .min(screen_height.saturating_sub(2))
        }
        DialogContent::CustomFunctionMenu { .. } => {
            // Exact size, same as ContextMenu
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::OpenWithPicker { .. } => {
            // Exact size, same as ContextMenu/CustomFunctionMenu
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::ContextMenu(_) => {
            // Exact size for context menu
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::FileConflict { .. }
        | DialogContent::SortDialog { .. }
        | DialogContent::FileMask { .. }
        | DialogContent::WildcardMark { .. }
        | DialogContent::SimpleRename { .. }
        | DialogContent::AttrTimestamp { .. }
        | DialogContent::CreateLink { .. }
        | DialogContent::FileInfo { .. }
        | DialogContent::ExtractionConfirm(_)
        | DialogContent::Error(_)
        | DialogContent::TypeMismatchWarning(_)
        | DialogContent::Confirmation(_)
        | DialogContent::Input { .. } => {
            // Use exact minimum height for compact dialogs
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        _ => {
            // Use 70% of screen or minimum, whichever is larger
            let percent_height = (screen_height * 70) / 100;
            percent_height
                .max(min_dialog_height)
                .min(screen_height.saturating_sub(2))
        }
    };

    // Compute dialog width in absolute pixels to avoid double-floor rounding
    let dialog_width: u16 = match &dialog.content {
        DialogContent::JobManager { .. } => {
            // Fixed 64 columns, capped to screen
            64u16.min(screen_width.saturating_sub(2)).max(40)
        }
        DialogContent::FileConflict { .. } => {
            // Min 64 chars, up to 80% of terminal width
            let w80 = (screen_width * 80) / 100;
            64u16.max(w80).min(screen_width.saturating_sub(2)).max(40)
        }
        DialogContent::DriveSelection(_) | DialogContent::RegisteredFolderSelector(_) => {
            60u16.min(screen_width.saturating_sub(2)).max(40)
        }
        DialogContent::CustomFunctionSelector { .. } => ((screen_width * 70) / 100)
            .max(50)
            .min(screen_width.saturating_sub(2)),
        DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog { items, .. }) => {
            // label fits with outer_width = max_label + 8 (2 border + 4 indent + 2 margin)
            // hint "[Enter] Execute  [Esc] Close" (29 chars) fits at offset+1 with width-2 when outer>=34
            let max_label = items
                .iter()
                .filter(|i| i.is_selectable())
                .map(|i| i.name.len())
                .max()
                .unwrap_or(10);
            ((max_label as u16 + 8).max(34)).min(screen_width.saturating_sub(2))
        }
        DialogContent::OpenWithPicker(OpenWithPickerDialog { candidates, .. }) => {
            // label fits with outer_width = max_label + 8 (2 border + 4 indent + 2 margin)
            // hint "[Enter] Open  [Esc] Cancel" fits at offset+1 with width-2 when outer>=34
            let max_label = candidates
                .iter()
                .map(|c| candidate_label(c).len())
                .max()
                .unwrap_or(10);
            ((max_label as u16 + 8).max(34)).min(screen_width.saturating_sub(2))
        }
        DialogContent::ContextMenu(ContextMenuDialog {
            options,
            detected_type_label,
            detected_type_job_id,
            ..
        }) => {
            // The OpenWith row's rendered label gets a " (<type>)" or
            // " (detecting...)" suffix appended at render time (Phase 7.3b,
            // Task 9) that isn't part of `option.label` — account for it here
            // too, or a detected type longer than every other row's label
            // just gets truncated even though there's room to show it.
            let suffix_len = if detected_type_job_id.is_some() {
                " (detecting...)".len()
            } else if let Some(t) = detected_type_label {
                format!(" ({t})").len()
            } else {
                0
            };
            let max_label = options
                .iter()
                .map(|o| {
                    let extra = if matches!(
                        o.action,
                        rwf_lib::model::dialog::ContextMenuAction::OpenWith
                    ) {
                        suffix_len
                    } else {
                        0
                    };
                    o.label.len() + extra
                })
                .max()
                .unwrap_or(10);
            // +8 (not the base formula's +6) once a suffix is in play: the
            // suffix isn't part of any `option.label`, so the extra couple of
            // columns keeps it from immediately re-truncating against the
            // render loop's own indent/margin (see `render_context_menu_dialog`).
            let padding = if suffix_len > 0 { 8 } else { 6 };
            ((max_label as u16 + padding).max(24)).min(screen_width.saturating_sub(2))
        }
        DialogContent::PatternRename { .. }
        | DialogContent::JumpToPath(_)
        | DialogContent::JumpToFile(_) => ((screen_width * 80) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
        DialogContent::Help { .. } => ((screen_width * 70) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
        DialogContent::FileInfo { .. } => file_info_dialog_width(screen_width),
        DialogContent::AttrTimestamp { .. } => {
            // Wide enough for the 4-checkbox attribute row (~51 chars) and a
            // full "YYYY-MM-DD HH:MM:SS" timestamp field + "[t:now]" hint.
            64u16.min(screen_width.saturating_sub(2)).max(50)
        }
        DialogContent::CreateLink { .. } => {
            // Wide enough for the Type row (3 options) and the unavailable-
            // reasons row when more than one option is disabled.
            76u16.min(screen_width.saturating_sub(2)).max(50)
        }
        _ => default_dialog_width(screen_width),
    };

    let area = centered_rect_abs(dialog_width, dialog_height, frame.area());

    // Render common frame (border, title)
    let content_area = render_dialog_frame(frame, &dialog.title, area);

    // Render dialog based on type
    match &dialog.content {
        DialogContent::Compression { .. } => {
            // Render compression dialog using exact content area (buttons rendered within)
            render_dialog_content(frame, &dialog.content, content_area, true);
        }
        DialogContent::JobManager(JobManagerContent {
            selected_index,
            focused_field,
        }) => {
            // Render Job Manager dialog with its own layout (Part 6.2)
            let dialog_state = JobManagerDialogState {
                selected_index: *selected_index,
                focused_field: *focused_field,
                job_list_focus_index: *selected_index,
            };
            render_job_manager_dialog(frame, content_area, state, &dialog_state);
        }
        DialogContent::CloseTabWithActiveJob(CloseTabWithActiveJobDialog {
            tab_name,
            job_ids,
            focused_field,
            ..
        }) => {
            // Render Close Tab confirmation dialog with buttons (compact layout)
            let job_list = if job_ids.len() == 1 {
                format!("Job #{} is still running.", job_ids[0])
            } else {
                let job_strs: Vec<String> = job_ids.iter().map(|id| format!("#{}", id)).collect();
                format!("Jobs {} are still running.", job_strs.join(", "))
            };
            let message = format!(
                "{} {}\nClose this tab and cancel the job(s)?",
                tab_name, job_list
            );

            // Use compact layout: message takes remaining space, buttons fixed at 3 lines
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(2),    // Message (compact)
                    Constraint::Length(3), // Buttons
                ])
                .split(content_area);

            let confirmation =
                Paragraph::new(message).style(crate::ui::dialog::common::DIALOG_TEXT);

            frame.render_widget(confirmation, chunks[0]);

            // Render buttons (OK/Cancel) with proper focus
            render_dialog_buttons(frame, chunks[1], &dialog.content, *focused_field);
        }
        DialogContent::FileConflict(FileConflictDialog {
            conflicts,
            current_index,
            focused_button,
            rename_text,
            rename_cursor,
            rename_scroll,
            edit_mode,
            vi_mode,
            error_message,
            ..
        }) => {
            // Render File Conflict dialog with TextInput widget
            render_file_conflict_dialog(
                frame,
                content_area,
                conflicts,
                *current_index,
                *focused_button,
                rename_text,
                *rename_cursor,
                *rename_scroll,
                *edit_mode,
                *vi_mode,
                error_message,
            );
        }
        DialogContent::SortDialog(SortDialog {
            selected_mode_index,
            selected_order_index,
            focused_section,
        }) => {
            render_sort_dialog(
                frame,
                content_area,
                *selected_mode_index,
                *selected_order_index,
                *focused_section,
            );
        }
        DialogContent::FileMask(FileMaskDialog {
            input,
            ui:
                DialogUiState {
                    cursor_pos,
                    scroll_pos,
                    focused_field,
                },
        }) => {
            render_file_mask_dialog(
                frame,
                content_area,
                input,
                *cursor_pos,
                *scroll_pos,
                *focused_field,
            );
        }
        DialogContent::WildcardMark(WildcardMarkDialog {
            input,
            ui:
                DialogUiState {
                    cursor_pos,
                    scroll_pos,
                    focused_field,
                },
        }) => {
            render_wildcard_mark_dialog(
                frame,
                content_area,
                input,
                *cursor_pos,
                *scroll_pos,
                *focused_field,
            );
        }
        DialogContent::SimpleRename(SimpleRenameDialog {
            input,
            ui:
                DialogUiState {
                    cursor_pos,
                    scroll_pos,
                    focused_field,
                },
        }) => {
            render_simple_rename_dialog(
                frame,
                content_area,
                input,
                *cursor_pos,
                *scroll_pos,
                *focused_field,
            );
        }
        DialogContent::AttrTimestamp(d) => {
            render_attr_timestamp_dialog(frame, content_area, d);
        }
        DialogContent::CreateLink(d) => {
            render_create_link_dialog(frame, content_area, d);
        }
        DialogContent::HistoryDialog(HistoryDialogContent {
            left_entries,
            right_entries,
            left_selected,
            right_selected,
            left_current_pos,
            right_current_pos,
            active_pane,
        }) => {
            use rwf_lib::model::ui::ActivePane;
            let (entries, selected, current_pos) = match active_pane {
                ActivePane::Left => (left_entries.as_slice(), *left_selected, *left_current_pos),
                ActivePane::Right => (
                    right_entries.as_slice(),
                    *right_selected,
                    *right_current_pos,
                ),
            };
            render_history_dialog(frame, content_area, entries, selected, current_pos);
        }
        DialogContent::DriveSelection(DriveSelectionDialog {
            drives,
            selected_index,
            filter,
        }) => {
            render_drive_selection_dialog(frame, content_area, drives, *selected_index, filter);
        }
        DialogContent::TrashBrowser(TrashBrowserDialog {
            records,
            selected_index,
        }) => {
            render_trash_browser_dialog(frame, content_area, records, *selected_index);
        }
        DialogContent::OperationReportView(content) => {
            render_operation_report_dialog(frame, content_area, content);
        }
        DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
            folders,
            selected_index,
            filter,
        }) => {
            render_registered_folder_selector(
                frame,
                content_area,
                folders,
                *selected_index,
                filter,
            );
        }
        DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
            functions,
            selected_index,
            filter,
        }) => {
            render_custom_function_selector(
                frame,
                content_area,
                functions,
                *selected_index,
                filter,
            );
        }
        DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog {
            items,
            selected_index,
        }) => {
            render_custom_function_menu(frame, content_area, items, *selected_index);
        }
        DialogContent::OpenWithPicker(OpenWithPickerDialog {
            candidates,
            selected_index,
            ..
        }) => {
            render_open_with_picker(frame, content_area, candidates, *selected_index);
        }
        DialogContent::ContextMenu(ContextMenuDialog {
            options,
            selected_index,
            detected_type_label,
            detected_type_job_id,
        }) => {
            render_context_menu_dialog(
                frame,
                content_area,
                options,
                *selected_index,
                detected_type_label.as_deref(),
                detected_type_job_id.is_some(),
            );
        }
        DialogContent::JumpToPath(JumpToPathDialog {
            query,
            cursor_pos,
            suggestions,
            selected_index,
            loading_job_id,
            ..
        }) => {
            render_jump_to_path_dialog(
                frame,
                content_area,
                query,
                *cursor_pos,
                suggestions,
                *selected_index,
                loading_job_id.is_some(),
            );
        }
        DialogContent::JumpToFile(JumpToFileDialog {
            query,
            cursor_pos,
            suggestions,
            selected_index,
            loading_job_id,
            ..
        }) => {
            render_jump_to_file_dialog(
                frame,
                content_area,
                query,
                *cursor_pos,
                suggestions,
                *selected_index,
                loading_job_id.is_some(),
            );
        }
        DialogContent::FileInfo(FileInfoDialog {
            file_name,
            file_path,
            size,
            created,
            modified,
            accessed,
            is_dir,
            is_readonly,
            #[cfg(unix)]
            permissions,
            #[cfg(unix)]
            owner,
            #[cfg(unix)]
            group,
            link_target,
            link_kind,
            detecting,
            detected_type,
            header_bytes,
            header_hex_mode,
            header_encoding,
            ..
        }) => {
            render_file_info_dialog(
                frame,
                content_area,
                file_name,
                file_path,
                *size,
                *created,
                *modified,
                *accessed,
                *is_dir,
                *is_readonly,
                #[cfg(unix)]
                *permissions,
                #[cfg(unix)]
                owner.as_deref(),
                #[cfg(unix)]
                group.as_deref(),
                link_target.as_deref(),
                link_kind.as_ref(),
                *detecting,
                detected_type.as_deref(),
                header_bytes.as_deref(),
                *header_hex_mode,
                *header_encoding,
                &state.config.display.spinner_frames,
                state.config.display.spinner_frame_ms,
            );
        }
        DialogContent::PatternRename(PatternRenameContent {
            find,
            find_cursor_pos,
            find_scroll_pos,
            replace,
            replace_cursor_pos,
            replace_scroll_pos,
            use_regex,
            case_sensitive,
            preview,
            focused_field,
            preview_scroll,
            preview_horizontal_scroll,
            error_message,
            preview_mode,
            show_all,
        }) => {
            render_pattern_rename_dialog(
                frame,
                content_area,
                find,
                *find_cursor_pos,
                *find_scroll_pos,
                replace,
                *replace_cursor_pos,
                *replace_scroll_pos,
                *use_regex,
                *case_sensitive,
                preview,
                *focused_field,
                *preview_scroll,
                *preview_horizontal_scroll,
                error_message.as_deref(),
                *preview_mode,
                *show_all,
            );
        }
        DialogContent::Help(HelpDialog {
            entries,
            query,
            regex_mode,
            show_unbound,
            active_tab,
            scroll_pos,
            language,
            ..
        }) => {
            render_help_dialog(
                frame,
                content_area,
                area,
                entries,
                query,
                *regex_mode,
                *show_unbound,
                active_tab,
                *scroll_pos,
                language,
            );
        }
        DialogContent::DeleteConfirm(DeleteConfirmDialog {
            targets,
            scroll_offset,
            to_trash,
            ..
        }) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // header + blank + list items
                    Constraint::Length(1), // spacer (hint 1 line down)
                    Constraint::Length(1), // hint
                    Constraint::Length(3), // buttons
                ])
                .split(content_area);

            let base = DIALOG_TEXT;
            let dir_s = DIALOG_TEXT.add_modifier(Modifier::BOLD);
            let hint = DIALOG_DIM;
            let w = content_area.width.saturating_sub(2) as usize;
            let total = targets.len();

            // Header
            let header_txt = match (*to_trash, total == 1) {
                (true, true) => "Move this item to Trash?".to_string(),
                (true, false) => format!("Move these {} items to Trash?", total),
                (false, true) => "Delete this item?".to_string(),
                (false, false) => format!("Delete these {} items?", total),
            };
            frame.render_widget(
                Paragraph::new(smart_truncate(&header_txt, w, "…")).style(base),
                Rect::new(chunks[0].x + 1, chunks[0].y, w as u16, 1),
            );

            // List items (chunks[0]: row 0 = header, row 1 = up-indicator or blank, rows 2+ = items)
            let list_h = (chunks[0].height as usize).saturating_sub(2);
            let max_scroll = total.saturating_sub(list_h);
            let scroll = (*scroll_offset).min(max_scroll);
            let remaining_below = total.saturating_sub(scroll + list_h);
            let show_up = scroll > 0;
            let show_down = remaining_below > 0;

            // Up indicator in row 1 (the blank line), never displaces items
            if show_up {
                frame.render_widget(
                    Paragraph::new(format!("  ↑ {} more above", scroll)).style(hint),
                    Rect::new(chunks[0].x + 1, chunks[0].y + 1, w as u16, 1),
                );
            }

            // Items always start at row 2, no offset for indicators
            for row in 0..list_h {
                let item_idx = scroll + row;
                if item_idx >= total {
                    break;
                }
                let y = chunks[0].y + 2 + row as u16;
                let (loc, is_dir) = &targets[item_idx];
                let raw_name = loc
                    .path()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| loc.display_path());
                let label = if *is_dir {
                    format!("  {}/", raw_name)
                } else {
                    format!("  {}", raw_name)
                };
                let truncated = smart_truncate(&label, w, "…");
                frame.render_widget(
                    Paragraph::new(truncated).style(if *is_dir { dir_s } else { base }),
                    Rect::new(chunks[0].x + 1, y, w as u16, 1),
                );
            }

            // Down indicator in chunks[1] (spacer row), never displaces items
            if show_down {
                frame.render_widget(
                    Paragraph::new(format!("  ↓ {} more below", remaining_below)).style(hint),
                    Rect::new(chunks[1].x + 1, chunks[1].y, w as u16, 1),
                );
            }

            // Hint (chunks[2])
            let action_word = if *to_trash { "move to trash" } else { "delete" };
            let hint_txt = if total > 1 {
                format!("↑↓:scroll  Enter:{action_word}  Esc:cancel")
            } else {
                format!("Enter:{action_word}  Esc:cancel")
            };
            frame.render_widget(
                Paragraph::new(smart_truncate(&hint_txt, w, "…")).style(hint),
                Rect::new(chunks[2].x + 1, chunks[2].y, w as u16, 1),
            );

            // Buttons (chunks[3])
            render_dialog_buttons(frame, chunks[3], &dialog.content, 0);
        }
        DialogContent::TypeMismatchWarning(TypeMismatchWarningDialog {
            path, detected, ..
        }) => {
            render_type_mismatch_warning_dialog(
                frame,
                &dialog.content,
                content_area,
                path,
                detected.label(),
            );
        }
        DialogContent::Input { .. } => {
            // No separate button row; Enter/Esc are the controls
            render_dialog_content(frame, &dialog.content, content_area, true);
        }
        DialogContent::MultiLineInput(d) => {
            // No separate button row; Ctrl+Enter/Enter/Esc are the controls
            // (Enter inserts a newline instead of confirming — see the
            // exclusion in `handle_dialog_input`'s Enter interception).
            render_multiline_input_dialog(frame, content_area, d);
        }
        _ => {
            // Split content area for buttons (generic layout)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5),    // Content
                    Constraint::Length(3), // Buttons
                ])
                .split(content_area);

            // Render content-specific widgets
            render_dialog_content(frame, &dialog.content, chunks[0], true);
            render_dialog_buttons(frame, chunks[1], &dialog.content, 0);
        }
    }
}

/// Handle dialog input centrally
pub fn handle_dialog_input(
    dialog: &mut Dialog,
    key: KeyEvent,
    search: Option<&rwf_lib::model::SearchModel>,
) -> DialogAction {
    // Note: Esc handling is delegated to individual dialog handlers
    // - FileConflict: Esc cancels (Emacs) or switches to Normal mode (Vi)
    // - Other dialogs: Esc cancels

    // Enter = Confirm (but depends on focused field for JobManager / SortDialog)
    if key.code == crossterm::event::KeyCode::Enter {
        // SortDialog: Enter confirms only when OK (2) or Cancel (3) section is focused
        if let DialogContent::SortDialog(SortDialog {
            focused_section, ..
        }) = &dialog.content
        {
            match *focused_section {
                2 => return DialogAction::Confirm, // OK button
                3 => return DialogAction::Cancel,  // Cancel button
                _ => return DialogAction::None,    // List section — Enter does nothing
            }
        }
        // For JobManager dialog, check which field has focus
        if let DialogContent::JobManager(JobManagerContent { focused_field, .. }) = &dialog.content
        {
            match *focused_field {
                1 => return DialogAction::Confirm, // Close button focused
                2 => return DialogAction::Confirm, // Cancel Job button focused
                _ => {}                            // Job List focused, Enter does nothing
            }
        } else if let DialogContent::FileConflict { .. } = &dialog.content {
            // FileConflict dialog handles Enter internally (for buttons and textbox)
            // Don't return here, let it be handled below
        } else if let DialogContent::MultiLineInput { .. } = &dialog.content {
            // Phase 7.17: Enter inserts a newline here, not confirm — the
            // dedicated handler below owns Enter/Ctrl+Enter/Esc. Don't return
            // here, let it be handled below.
        } else {
            return DialogAction::Confirm;
        }
    }

    // CloseTabWithActiveJob dialog - Enter confirms, Escape cancels, Tab cycles
    if let DialogContent::CloseTabWithActiveJob(d) = &mut dialog.content {
        return close_tab_with_active_job::handle_input(d, key);
    }

    // FileMask dialog — text input with Tab navigation and Enter/Esc handling
    if let DialogContent::FileMask(d) = &mut dialog.content {
        return file_mask::handle_input(d, key);
    }

    // WildcardMark dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::WildcardMark(d) = &mut dialog.content {
        return wildcard_mark::handle_input(d, key);
    }

    // Input dialog — generic text input (Create Directory, Register Folder, Custom Function Input, etc.)
    if let DialogContent::Input(d) = &mut dialog.content {
        return basic::handle_input(d, key);
    }

    // MultiLineInput dialog (Phase 7.17) — free-form multi-line text entry
    // (currently the diagnostic report prompt). Owns Enter/Ctrl+Enter/Esc
    // itself; see the exclusion above in the Enter interception.
    if let DialogContent::MultiLineInput(d) = &mut dialog.content {
        return multiline_input::handle_input(d, key);
    }

    // SimpleRename dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::SimpleRename(d) = &mut dialog.content {
        return simple_rename::handle_input(d, key);
    }

    // AttrTimestamp dialog — checkbox/text-field focus cycling, Space toggles,
    // `t` stamps "now" into a focused timestamp field.
    if let DialogContent::AttrTimestamp(d) = &mut dialog.content {
        return attr_timestamp::handle_input(d, key);
    }

    // CreateLink dialog — Type/link-name focus cycling, Left/Right cycles
    // the link type.
    if let DialogContent::CreateLink(d) = &mut dialog.content {
        return create_link::handle_input(d, key);
    }

    // PatternRename dialog — Find/Replace textboxes + Alt+R/S flag toggles + preview scroll
    if let DialogContent::PatternRename(d) = &mut dialog.content {
        return pattern_rename::handle_input(d, key);
    }

    // Help dialog — full input handler
    if let DialogContent::Help(d) = &mut dialog.content {
        return help::handle_input(d, key);
    }

    // HistoryDialog — Up/Down/j/k: navigate, Tab/Left/Right/h/l: switch pane, Enter: jump, Esc: cancel
    // DriveSelection dialog — incremental search + arrow navigation
    if let DialogContent::DriveSelection(d) = &mut dialog.content {
        return drive_selection::handle_input(d, key);
    }

    // TrashBrowser — Up/Down/Home/End navigate, Enter confirms restore
    if let DialogContent::TrashBrowser(d) = &mut dialog.content {
        return trash_browser::handle_input(d, key);
    }

    // OperationReportView — Up/Down navigate, Space toggle, a select-all/none,
    // Enter triggers Undo/Redo on the selection, Esc closes.
    if let DialogContent::OperationReportView(d) = &mut dialog.content {
        return operation_report::handle_input(d, key);
    }

    // JumpToPath — text input + AND-filter suggestions + arrow navigation
    if let DialogContent::JumpToPath(d) = &mut dialog.content {
        return jump_to_path::handle_input(d, key, search);
    }

    // JumpToFile — text input + AND-filter suggestions (files + dirs) + arrow navigation
    if let DialogContent::JumpToFile(d) = &mut dialog.content {
        return jump_to_file::handle_input(d, key, search);
    }

    // CustomFunctionSelector — incremental search + arrow navigation
    if let DialogContent::CustomFunctionSelector(d) = &mut dialog.content {
        return custom_function::handle_selector_input(d, key);
    }

    // ContextMenu — arrow navigation (skip separators)
    if let DialogContent::ContextMenu(d) = &mut dialog.content {
        return context_menu::handle_input(d, key);
    }

    // CustomFunctionMenu — second-level menu with separator skipping and char-jump
    if let DialogContent::CustomFunctionMenu(d) = &mut dialog.content {
        return custom_function::handle_menu_input(d, key);
    }

    // OpenWithPicker — plain index navigation (Phase 7.3, no separators)
    if let DialogContent::OpenWithPicker(d) = &mut dialog.content {
        return open_with_picker::handle_input(d, key);
    }

    // RegisteredFolderSelector — incremental search + arrow navigation
    if let DialogContent::RegisteredFolderSelector(d) = &mut dialog.content {
        return registered_folder::handle_input(d, key);
    }

    if matches!(&dialog.content, DialogContent::HistoryDialog(_)) {
        return history::handle_input(dialog, key);
    }

    // FileConflict dialog - custom input handling with textbox
    if let DialogContent::FileConflict(d) = &mut dialog.content {
        return file_conflict::handle_input(d, key);
    }

    // Compression dialog - Vi mode support for Esc (when textbox not focused)
    if let DialogContent::Compression(CompressionDialog {
        edit_mode,
        vi_mode,
        focused_field,
        ..
    }) = &mut dialog.content
    {
        if key.code == crossterm::event::KeyCode::Esc && *focused_field != 2 {
            debug!("Esc pressed in Compression dialog (non-textbox), edit_mode={:?}, current vi_mode={:?}", edit_mode, vi_mode);
            if *edit_mode == rwf_lib::config::EditMode::Vi {
                debug!("Switching to Normal mode in Compression dialog");
                *vi_mode = Some(rwf_lib::config::ViMode::Normal);
                return DialogAction::None;
            } else {
                debug!("Emacs mode active in Compression dialog, returning Cancel");
                return DialogAction::Cancel;
            }
        }
    }

    // Error dialog — any key dismisses (only OK button, Cancel removed)
    if matches!(&dialog.content, DialogContent::Error(_)) {
        return DialogAction::Confirm;
    }

    // Tab navigation - cycles through dialog fields
    if key.code == crossterm::event::KeyCode::Tab || key.code == crossterm::event::KeyCode::BackTab
    {
        let backward = key.code == crossterm::event::KeyCode::BackTab
            || key.modifiers.contains(KeyModifiers::SHIFT);

        // SortDialog: Tab cycles 0→1→2→3→0 (sort-key list→order list→OK→Cancel)
        if let DialogContent::SortDialog(SortDialog {
            focused_section, ..
        }) = &mut dialog.content
        {
            if backward {
                *focused_section = if *focused_section == 0 {
                    3
                } else {
                    *focused_section - 1
                };
            } else {
                *focused_section = (*focused_section + 1) % 4;
            }
            return DialogAction::None;
        }

        // Handle JobManager dialog Tab navigation (Part 6.6, 6.7)
        if let DialogContent::JobManager(JobManagerContent { focused_field, .. }) =
            &mut dialog.content
        {
            // Cycle: 0→1→2→0 (Job List→Close→Cancel→Job List)
            if backward {
                *focused_field = match *focused_field {
                    0 => 2, // Job List → Cancel
                    1 => 0, // Close → Job List
                    2 => 1, // Cancel → Close
                    _ => 0,
                };
            } else {
                *focused_field = match *focused_field {
                    0 => 1, // Job List → Close
                    1 => 2, // Close → Cancel
                    2 => 0, // Cancel → Job List
                    _ => 0,
                };
            }
            return DialogAction::None;
        }

        // Handle Compression dialog Tab navigation
        if let DialogContent::Compression(CompressionDialog { focused_field, .. }) =
            &mut dialog.content
        {
            // Cycle: 0→1→2→3→4→0 (format→compression→name→OK→Cancel→format)
            if backward {
                *focused_field = if *focused_field == 0 {
                    4
                } else {
                    *focused_field - 1
                };
            } else {
                *focused_field = (*focused_field + 1) % 5;
            }
            return DialogAction::None;
        }
        return if backward {
            DialogAction::PrevField
        } else {
            DialogAction::NextField
        };
    }

    // Esc cancels dialog (for dialogs that don't handle it themselves)
    // Note: FileConflict handles Esc in its own handler (Vi mode → Normal mode)
    if key.code == crossterm::event::KeyCode::Esc {
        return DialogAction::Cancel;
    }

    // Delegate to content-specific handler
    handle_content_input(&mut dialog.content, key)
}

#[cfg(test)]
mod width_calc_tests {
    use super::*;

    /// The exact content-width requirement derived from
    /// `render_file_info_dialog`'s hex-row format string — see the doc
    /// comment on `FILE_INFO_HEX_ROW_DIALOG_WIDTH` for the full derivation.
    #[test]
    fn file_info_dialog_width_is_content_driven_not_stretched() {
        // Wide enough that 90% comfortably clears the 80-column requirement:
        // the dialog should land exactly on the content-driven minimum, not
        // stretch out to fill the extra room.
        assert_eq!(file_info_dialog_width(200), FILE_INFO_HEX_ROW_DIALOG_WIDTH);
        assert_eq!(file_info_dialog_width(120), FILE_INFO_HEX_ROW_DIALOG_WIDTH);
    }

    /// Repro of the user's screenshot bug: at a terminal width where the OLD
    /// 60%-based `default_dialog_width` formula clips a full hex row (needs
    /// 80 columns) but the NEW 90%-capped, content-driven formula does not.
    /// 100 columns: old 60% => 60 (< 80, clips); new min(80, 90%=90) => 80
    /// (>= 80, no clip).
    #[test]
    fn file_info_dialog_width_fixes_the_old_60_percent_clipping_case() {
        let screen_width = 100;
        let old_width = default_dialog_width(screen_width);
        let new_width = file_info_dialog_width(screen_width);

        assert!(
            old_width < FILE_INFO_HEX_ROW_DIALOG_WIDTH,
            "expected the OLD formula to clip at this width (old={old_width}, needed={FILE_INFO_HEX_ROW_DIALOG_WIDTH})"
        );
        assert!(
            new_width >= FILE_INFO_HEX_ROW_DIALOG_WIDTH,
            "expected the NEW formula to fit a full hex row (new={new_width}, needed={FILE_INFO_HEX_ROW_DIALOG_WIDTH})"
        );
    }

    /// Narrow terminals: the dialog shouldn't blow past 90% of the screen,
    /// and should stay comfortably within the standard 80x24 size already
    /// used by this project's snapshot tests, floored at the same 40-column
    /// minimum every other dialog uses.
    #[test]
    fn file_info_dialog_width_stays_sane_at_80x24() {
        let width = file_info_dialog_width(80);
        assert!(width >= 40, "must respect the 40-column floor: {width}");
        assert!(
            width <= 78,
            "must never exceed screen_width - 2 at 80 columns: {width}"
        );
        // 90% of 80 = 72, which is less than the 80-column ideal, so the
        // dialog is expected to still clip the hex row here — that's the
        // accepted, inherent narrow-terminal limitation the task calls out.
        assert_eq!(width, 72);
    }

    /// Even at a genuinely tiny terminal, the formula must not panic or
    /// underflow (u16 saturating arithmetic) and must respect the 40-column
    /// floor without exceeding the screen.
    #[test]
    fn file_info_dialog_width_does_not_panic_on_tiny_screens() {
        for w in [0u16, 1, 10, 39, 40, 41] {
            let width = file_info_dialog_width(w);
            assert!(
                width <= w.max(40),
                "width {width} unreasonable for screen {w}"
            );
        }
    }
}
