//! Dialog system with centralized input handling
//!
//! This module provides a hybrid dialog system with:
//! - Common infrastructure (border, title, buttons, centering)
//! - Content-specific rendering via trait
//! - Centralized input handling with consistent shortcuts

mod basic;
mod close_tab_with_active_job;
pub mod common;
mod compression;
mod confirm;
mod context_menu;
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
mod open_with_picker;
mod pattern_rename;
mod registered_folder;
mod simple_rename;
#[cfg(test)]
mod snapshot_tests;
mod sort;
#[cfg(test)]
mod test_support;
mod type_mismatch_warning;
mod wildcard_mark;

use basic::{handle_content_input, render_dialog_content};
use context_menu::render_context_menu_dialog;
use custom_function::{render_custom_function_menu, render_custom_function_selector};
use drive_selection::render_drive_selection_dialog;
use file_conflict::render_file_conflict_dialog;
use file_info::render_file_info_dialog;
use file_mask::render_file_mask_dialog;
use help::render_help_dialog;
use history::render_history_dialog;
use jump_to_file::render_jump_to_file_dialog;
use jump_to_path::render_jump_to_path_dialog;
use open_with_picker::{candidate_label, render_open_with_picker};
use pattern_rename::render_pattern_rename_dialog;
use registered_folder::render_registered_folder_selector;
use simple_rename::render_simple_rename_dialog;
use sort::render_sort_dialog;
use type_mismatch_warning::render_type_mismatch_warning_dialog;
use wildcard_mark::render_wildcard_mark_dialog;

use common::{DIALOG_DIM, DIALOG_TEXT};

pub use confirm::{delete_job_name, process_dialog_confirmation, process_dialog_delete};
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
    CloseTabWithActiveJobDialog, CompressionDialog, ContextMenuDialog, CustomFunctionMenuDialog,
    CustomFunctionSelectorContent, DeleteConfirmDialog, Dialog, DialogContent, DialogUiState,
    DriveSelectionDialog, ErrorDialog, FileConflictDialog, FileInfoDialog, FileMaskDialog,
    HelpDialog, HistoryDialogContent, JobManagerContent, JumpToFileDialog, JumpToPathDialog,
    OpenWithPickerDialog, PatternRenameContent, RegisteredFolderSelectorContent,
    SimpleRenameDialog, SortDialog, TypeMismatchWarningDialog, WildcardMarkDialog,
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
    /// On-demand content-type detection requested from the open File
    /// Information dialog (Phase 7.3 §7). The dialog stays open; the app
    /// loop dispatches `Transition::DetectFileInfoType` for its `file_path`.
    DetectFileType,
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
            link_target,
            detecting,
            detected_type,
            ..
        }) => {
            let base = if link_target.is_some() { 12u16 } else { 11u16 };
            if *detecting || detected_type.is_some() {
                base + 1
            } else {
                base
            }
        } // name+path+size+type+3×datetime + hint (+1 for link row, +1 for detected type)
        DialogContent::PatternRename(PatternRenameContent { preview, .. }) => {
            // find(1) + replace(1) + flags(1) + mode-row(1) + separator(1) + preview rows + status(1) = 6 + preview count, min 8
            (preview.len() as u16 + 6).max(8)
        }
        DialogContent::Help { .. } => {
            // tab bar(1) + search(1) + entries + hint(1), min 8
            20u16
        }
        DialogContent::Error(ErrorDialog { message, .. }) => {
            // message lines + blank(1) + buttons(3), min 5
            (message.lines().count() as u16 + 4).max(5)
        }
        DialogContent::Input { .. } => {
            // prompt(1) + textbox(1) + hint(1) = 3
            3u16
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
        | DialogContent::FileInfo { .. }
        | DialogContent::ExtractionConfirm(_)
        | DialogContent::Error(_)
        | DialogContent::TypeMismatchWarning(_)
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
        DialogContent::ContextMenu(ContextMenuDialog { options, .. }) => {
            let max_label = options.iter().map(|o| o.label.len()).max().unwrap_or(10);
            ((max_label as u16 + 6).max(24)).min(screen_width.saturating_sub(2))
        }
        DialogContent::PatternRename { .. }
        | DialogContent::JumpToPath(_)
        | DialogContent::JumpToFile(_) => ((screen_width * 80) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
        DialogContent::Help { .. } => ((screen_width * 70) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
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
        }) => {
            render_context_menu_dialog(frame, content_area, options, *selected_index);
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
            let header_txt = if total == 1 {
                "Delete this item?".to_string()
            } else {
                format!("Delete these {} items?", total)
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
            let hint_txt = if total > 1 {
                "↑↓:scroll  Enter:delete  Esc:cancel"
            } else {
                "Enter:delete  Esc:cancel"
            };
            frame.render_widget(
                Paragraph::new(smart_truncate(hint_txt, w, "…")).style(hint),
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

    // SimpleRename dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::SimpleRename(d) = &mut dialog.content {
        return simple_rename::handle_input(d, key);
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
