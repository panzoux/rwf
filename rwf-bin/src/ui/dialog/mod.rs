//! Dialog system with centralized input handling
//!
//! This module provides a hybrid dialog system with:
//! - Common infrastructure (border, title, buttons, centering)
//! - Content-specific rendering via trait
//! - Centralized input handling with consistent shortcuts

mod basic;
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
mod pattern_rename;
mod registered_folder;
mod simple_rename;
#[cfg(test)]
mod snapshot_tests;
mod sort;
#[cfg(test)]
mod test_support;
mod wildcard_mark;

use basic::{handle_content_input, render_dialog_content};
use context_menu::render_context_menu_dialog;
use custom_function::{render_custom_function_menu, render_custom_function_selector};
use drive_selection::render_drive_selection_dialog;
use file_conflict::{handle_file_conflict_input, render_file_conflict_dialog};
use file_info::render_file_info_dialog;
use file_mask::render_file_mask_dialog;
use help::{help_filter_entries, render_help_dialog};
use history::render_history_dialog;
use jump_to_file::render_jump_to_file_dialog;
use jump_to_path::render_jump_to_path_dialog;
use pattern_rename::render_pattern_rename_dialog;
use registered_folder::render_registered_folder_selector;
use simple_rename::render_simple_rename_dialog;
use sort::render_sort_dialog;
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
    CloseTabWithActiveJobDialog, ContextMenuDialog, DeleteConfirmDialog, Dialog, DialogContent,
    DialogUiState, DriveSelectionDialog, ErrorDialog, FileInfoDialog, FileMaskDialog, HelpDialog,
    HistoryDialogContent, InputDialog, RegisteredFolderSelectorContent, SimpleRenameDialog,
    SortDialog, WildcardMarkDialog,
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

/// Render a dialog overlay centered on screen
pub fn render_dialog(frame: &mut Frame, dialog: &Dialog, state: &rwf_lib::AppState) {
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
        DialogContent::CustomFunctionSelector { functions, .. } => {
            // list + hint(1) + filter(1)
            (functions.len() as u16 + 2).max(6)
        }
        DialogContent::CustomFunctionMenu { items, .. } => {
            // items + hint(1)
            (items.len() as u16 + 1).max(4)
        }
        DialogContent::ContextMenu(ContextMenuDialog { options, .. }) => {
            // options list + hint(1)
            (options.len() as u16 + 1).max(4)
        }
        DialogContent::JumpToPath { suggestions, .. } => {
            // input(1) + sep(1) + list(up to 10) + sep(1) + preview(1) + hint(1) = list+5, min 8
            (suggestions.len().min(10) as u16 + 5).max(8)
        }
        DialogContent::JumpToFile { suggestions, .. } => {
            (suggestions.len().min(10) as u16 + 5).max(8)
        }
        DialogContent::FileInfo(FileInfoDialog { link_target, .. }) => {
            if link_target.is_some() {
                12u16
            } else {
                11u16
            }
        } // name+path+size+type+3×datetime + hint (+1 for link row)
        DialogContent::PatternRename { preview, .. } => {
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
    let screen_width = frame.area().width;

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
        | DialogContent::JumpToPath { .. }
        | DialogContent::JumpToFile { .. }
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
        DialogContent::CustomFunctionMenu { items, .. } => {
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
        DialogContent::ContextMenu(ContextMenuDialog { options, .. }) => {
            let max_label = options.iter().map(|o| o.label.len()).max().unwrap_or(10);
            ((max_label as u16 + 6).max(24)).min(screen_width.saturating_sub(2))
        }
        DialogContent::PatternRename { .. }
        | DialogContent::JumpToPath { .. }
        | DialogContent::JumpToFile { .. } => ((screen_width * 80) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
        DialogContent::Help { .. } => ((screen_width * 70) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
        _ => ((screen_width * 60) / 100)
            .max(40)
            .min(screen_width.saturating_sub(2)),
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
        DialogContent::JobManager {
            selected_index,
            focused_field,
        } => {
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
        DialogContent::FileConflict {
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
        } => {
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
        DialogContent::CustomFunctionSelector {
            functions,
            selected_index,
            filter,
        } => {
            render_custom_function_selector(
                frame,
                content_area,
                functions,
                *selected_index,
                filter,
            );
        }
        DialogContent::CustomFunctionMenu {
            items,
            selected_index,
        } => {
            render_custom_function_menu(frame, content_area, items, *selected_index);
        }
        DialogContent::ContextMenu(ContextMenuDialog {
            options,
            selected_index,
        }) => {
            render_context_menu_dialog(frame, content_area, options, *selected_index);
        }
        DialogContent::JumpToPath {
            query,
            cursor_pos,
            suggestions,
            selected_index,
            loading_job_id,
            ..
        } => {
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
        DialogContent::JumpToFile {
            query,
            cursor_pos,
            suggestions,
            selected_index,
            loading_job_id,
            ..
        } => {
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
            );
        }
        DialogContent::PatternRename {
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
        } => {
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
        if let DialogContent::JobManager { focused_field, .. } = &dialog.content {
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
    if let DialogContent::CloseTabWithActiveJob(CloseTabWithActiveJobDialog {
        focused_field, ..
    }) = &mut dialog.content
    {
        if key.code == crossterm::event::KeyCode::Enter {
            return DialogAction::Confirm;
        }
        if key.code == crossterm::event::KeyCode::Esc {
            return DialogAction::Cancel;
        }
        // Tab key cycles between OK (field 0) and Cancel (field 1) buttons
        if key.code == crossterm::event::KeyCode::Tab {
            // Cycle: 0→1→0 (OK→Cancel→OK)
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Tab: backwards
                *focused_field = if *focused_field == 0 { 1 } else { 0 };
            } else {
                // Tab: forwards
                *focused_field = if *focused_field == 0 { 1 } else { 0 };
            }
            return DialogAction::None;
        }
    }

    // FileMask dialog — text input with Tab navigation and Enter/Esc handling
    if let DialogContent::FileMask(FileMaskDialog {
        input,
        ui:
            DialogUiState {
                cursor_pos,
                scroll_pos,
                focused_field,
            },
    }) = &mut dialog.content
    {
        use crate::ui::text_input::{TextInput, TextInputAction};
        use crossterm::event::KeyCode;
        // Tab cycles: 0 (textbox) → 1 (OK) → 2 (Cancel) → 0
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 {
                    2
                } else {
                    *focused_field - 1
                };
            } else {
                *focused_field = (*focused_field + 1) % 3;
            }
            return DialogAction::None;
        }
        if key.code == KeyCode::Esc {
            return DialogAction::Cancel;
        }
        if key.code == KeyCode::Enter {
            return match *focused_field {
                2 => DialogAction::Cancel,  // Cancel button
                _ => DialogAction::Confirm, // textbox or OK button
            };
        }
        // Delegate text editing to TextInput widget only when textbox is focused
        if *focused_field == 0 {
            let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
            ti.set_original_text(input.clone()); // rule #1: for Vi U (revert) command
            ti.set_cursor(*cursor_pos);
            ti.set_scroll(*scroll_pos);
            let action = ti.handle_input(&key);
            *input = ti.text().to_string();
            *cursor_pos = ti.cursor();
            *scroll_pos = ti.scroll();
            match action {
                TextInputAction::Confirm => return DialogAction::Confirm,
                TextInputAction::Cancel => return DialogAction::Cancel,
                _ => return DialogAction::None,
            }
        }
        return DialogAction::None;
    }

    // WildcardMark dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::WildcardMark(WildcardMarkDialog {
        input,
        ui:
            DialogUiState {
                cursor_pos,
                scroll_pos,
                focused_field,
            },
    }) = &mut dialog.content
    {
        use crate::ui::text_input::{TextInput, TextInputAction};
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 {
                    2
                } else {
                    *focused_field - 1
                };
            } else {
                *focused_field = (*focused_field + 1) % 3;
            }
            return DialogAction::None;
        }
        if key.code == KeyCode::Esc {
            return DialogAction::Cancel;
        }
        if key.code == KeyCode::Enter {
            return match *focused_field {
                2 => DialogAction::Cancel,
                _ => DialogAction::Confirm,
            };
        }
        if *focused_field == 0 {
            let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
            ti.set_original_text(input.clone());
            ti.set_cursor(*cursor_pos);
            ti.set_scroll(*scroll_pos);
            let action = ti.handle_input(&key);
            *input = ti.text().to_string();
            *cursor_pos = ti.cursor();
            *scroll_pos = ti.scroll();
            match action {
                TextInputAction::Confirm => return DialogAction::Confirm,
                TextInputAction::Cancel => return DialogAction::Cancel,
                _ => return DialogAction::None,
            }
        }
        return DialogAction::None;
    }

    // Input dialog — generic text input (Create Directory, Register Folder, Custom Function Input, etc.)
    if let DialogContent::Input(InputDialog {
        input,
        cursor_pos,
        scroll_pos,
        ..
    }) = &mut dialog.content
    {
        use crate::ui::text_input::{TextInput, TextInputAction};
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Esc {
            return DialogAction::Cancel;
        }
        if key.code == KeyCode::Enter {
            return DialogAction::Confirm;
        }
        let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.clone());
        ti.set_cursor(*cursor_pos);
        ti.set_scroll(*scroll_pos);
        let action = ti.handle_input(&key);
        *input = ti.text().to_string();
        *cursor_pos = ti.cursor();
        *scroll_pos = ti.scroll();
        match action {
            TextInputAction::Confirm => return DialogAction::Confirm,
            TextInputAction::Cancel => return DialogAction::Cancel,
            _ => return DialogAction::None,
        }
    }

    // SimpleRename dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::SimpleRename(SimpleRenameDialog {
        input,
        ui:
            DialogUiState {
                cursor_pos,
                scroll_pos,
                focused_field,
            },
    }) = &mut dialog.content
    {
        use crate::ui::text_input::{TextInput, TextInputAction};
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 {
                    2
                } else {
                    *focused_field - 1
                };
            } else {
                *focused_field = (*focused_field + 1) % 3;
            }
            return DialogAction::None;
        }
        if key.code == KeyCode::Esc {
            return DialogAction::Cancel;
        }
        if key.code == KeyCode::Enter {
            return match *focused_field {
                2 => DialogAction::Cancel,
                _ => DialogAction::Confirm,
            };
        }
        if *focused_field == 0 {
            let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
            ti.set_original_text(input.clone());
            ti.set_cursor(*cursor_pos);
            ti.set_scroll(*scroll_pos);
            let action = ti.handle_input(&key);
            *input = ti.text().to_string();
            *cursor_pos = ti.cursor();
            *scroll_pos = ti.scroll();
            match action {
                TextInputAction::Confirm => return DialogAction::Confirm,
                TextInputAction::Cancel => return DialogAction::Cancel,
                _ => return DialogAction::None,
            }
        }
        return DialogAction::None;
    }

    // PatternRename dialog — Find/Replace textboxes + Alt+R/S flag toggles + preview scroll
    if let DialogContent::PatternRename {
        find,
        find_cursor_pos,
        find_scroll_pos,
        replace,
        replace_cursor_pos,
        replace_scroll_pos,
        use_regex,
        case_sensitive,
        focused_field,
        preview_scroll,
        preview_horizontal_scroll,
        preview,
        error_message,
        preview_mode,
        show_all,
    } = &mut dialog.content
    {
        use crate::ui::text_input::{TextInput, TextInputAction};
        use crossterm::event::KeyCode;

        // Alt+R → toggle regex mode
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('r') {
            *use_regex = !*use_regex;
            *error_message = None;
            return DialogAction::PatternChanged;
        }
        // Alt+S → toggle case sensitive
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('s') {
            *case_sensitive = !*case_sensitive;
            *error_message = None;
            return DialogAction::PatternChanged;
        }
        // Alt+P → cycle preview mode: 0=SIDE-BY-SIDE → 1=Preview → 2=Original → 0
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('p') {
            *preview_mode = (*preview_mode + 1) % 3;
            *preview_scroll = 0;
            *preview_horizontal_scroll = 0;
            return DialogAction::None;
        }
        // Alt+A → toggle show_all (MATCHES ↔ All)
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('a') {
            *show_all = !*show_all;
            *preview_scroll = 0;
            return DialogAction::None;
        }

        // Tab / BackTab: cycle find(0) → replace(1) → filelist(2) → find(0)
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 {
                    2
                } else {
                    *focused_field - 1
                };
            } else {
                *focused_field = (*focused_field + 1) % 3;
            }
            return DialogAction::None;
        }
        if key.code == KeyCode::BackTab {
            *focused_field = if *focused_field == 0 {
                2
            } else {
                *focused_field - 1
            };
            return DialogAction::None;
        }

        if key.code == KeyCode::Esc {
            return DialogAction::Cancel;
        }
        if key.code == KeyCode::Enter {
            // Detect duplicate target names before executing
            let mut seen = std::collections::HashSet::new();
            let has_collision = preview
                .iter()
                .any(|(orig, new_name)| orig != new_name && !seen.insert(new_name.clone()));
            if has_collision {
                *error_message =
                    Some("Multiple files would be renamed to the same name".to_string());
                return DialogAction::None;
            }
            return DialogAction::Confirm;
        }

        // Up/Down: always scroll the preview list (regardless of focused field)
        // Cap at filtered_count-1 so Down never goes past the last item
        let filtered_count = if *show_all {
            preview.len()
        } else {
            preview.iter().filter(|(a, b)| a != b).count()
        };
        let scroll_max = filtered_count.saturating_sub(1);
        if key.code == KeyCode::Up && key.modifiers == KeyModifiers::NONE {
            *preview_scroll = preview_scroll.saturating_sub(1);
            return DialogAction::None;
        }
        if key.code == KeyCode::Down && key.modifiers == KeyModifiers::NONE {
            *preview_scroll = (*preview_scroll + 1).min(scroll_max);
            return DialogAction::None;
        }

        // Page Up/Down scrolls the preview list
        if key.code == KeyCode::PageUp {
            *preview_scroll = preview_scroll.saturating_sub(5);
            return DialogAction::None;
        }
        if key.code == KeyCode::PageDown {
            *preview_scroll = (*preview_scroll + 5).min(scroll_max);
            return DialogAction::None;
        }

        // When filelist (2) is focused: Left/Right scroll horizontally, Ctrl+Left/Right jumps
        if *focused_field == 2 {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Left => {
                        *preview_horizontal_scroll = 0;
                    }
                    KeyCode::Right => {
                        *preview_horizontal_scroll = 500;
                    } // clamped at render
                    _ => {}
                }
            } else if key.modifiers == KeyModifiers::NONE {
                match key.code {
                    KeyCode::Left => {
                        *preview_horizontal_scroll = preview_horizontal_scroll.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        *preview_horizontal_scroll = preview_horizontal_scroll.saturating_add(1);
                    }
                    _ => {}
                }
            }
            return DialogAction::None;
        }

        // Text editing for focused textbox (0 or 1)
        if *focused_field == 0 || *focused_field == 1 {
            let (text, cursor, scroll) = if *focused_field == 0 {
                (find as &mut String, find_cursor_pos, find_scroll_pos)
            } else {
                (
                    replace as &mut String,
                    replace_cursor_pos,
                    replace_scroll_pos,
                )
            };
            let mut ti = TextInput::new(Some(text.clone()), rwf_lib::config::EditMode::Emacs);
            ti.set_original_text(text.clone());
            ti.set_cursor(*cursor);
            ti.set_scroll(*scroll);
            let action = ti.handle_input(&key);
            let new_text = ti.text().to_string();
            let changed = new_text != *text;
            *text = new_text;
            *cursor = ti.cursor();
            *scroll = ti.scroll();
            match action {
                TextInputAction::Confirm => return DialogAction::Confirm,
                TextInputAction::Cancel => return DialogAction::Cancel,
                _ => {
                    return if changed {
                        DialogAction::PatternChanged
                    } else {
                        DialogAction::None
                    }
                }
            }
        }
        return DialogAction::None;
    }

    // Help dialog — full input handler
    if let DialogContent::Help(HelpDialog {
        entries,
        query,
        regex_mode,
        show_unbound,
        active_tab,
        scroll_pos,
        ..
    }) = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        use rwf_lib::model::dialog::HelpTab;

        // Compute filtered count for scroll clamping
        let filtered_count =
            help_filter_entries(entries, active_tab, *show_unbound, query, *regex_mode).len();
        let list_height_estimate: usize = 20; // conservative; true height used in render

        match key.code {
            // Close
            KeyCode::Esc => return DialogAction::Cancel,

            // Tab switching by Ctrl+1-4
            KeyCode::Char('1') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = HelpTab::NormalMode;
                *scroll_pos = 0;
                query.clear();
            }
            KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = HelpTab::ViewerMode;
                *scroll_pos = 0;
                query.clear();
            }
            KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = HelpTab::LeapMode;
                *scroll_pos = 0;
                query.clear();
            }
            KeyCode::Char('4') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = HelpTab::DialogMode;
                *scroll_pos = 0;
                query.clear();
            }
            KeyCode::Char('5') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = HelpTab::CustomFunctions;
                *scroll_pos = 0;
                query.clear();
            }

            // Tab switching Ctrl+PageUp / Ctrl+PageDown
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = active_tab.prev();
                *scroll_pos = 0;
                query.clear();
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *active_tab = active_tab.next();
                *scroll_pos = 0;
                query.clear();
            }

            // Scroll — Up/Down arrow only (j/k are search input)
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                if *scroll_pos > 0 {
                    *scroll_pos -= 1;
                }
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                let max_scroll = filtered_count.saturating_sub(list_height_estimate);
                if *scroll_pos < max_scroll {
                    *scroll_pos += 1;
                }
            }
            KeyCode::PageUp => {
                *scroll_pos = scroll_pos.saturating_sub(list_height_estimate);
            }
            KeyCode::PageDown => {
                let max_scroll = filtered_count.saturating_sub(list_height_estimate);
                *scroll_pos = (*scroll_pos + list_height_estimate).min(max_scroll);
            }
            KeyCode::Home => {
                *scroll_pos = 0;
            }
            KeyCode::End => {
                *scroll_pos = filtered_count.saturating_sub(list_height_estimate);
            }

            // u: toggle show_unbound
            KeyCode::Char('u') if key.modifiers == KeyModifiers::NONE => {
                *show_unbound = !*show_unbound;
                *scroll_pos = 0;
            }

            // L: switch language
            KeyCode::Char('L')
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                return DialogAction::RotateLanguage;
            }

            // Ctrl+R: toggle regex mode
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *regex_mode = !*regex_mode;
                *scroll_pos = 0;
            }

            // Ctrl+K: clear query
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                query.clear();
                *scroll_pos = 0;
            }
            KeyCode::Char('\x0b') => {
                query.clear();
                *scroll_pos = 0;
            }

            // Backspace: remove last char from query
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if !query.is_empty() {
                    let mut chars = query.chars();
                    chars.next_back();
                    *query = chars.as_str().to_string();
                    *scroll_pos = 0;
                }
            }

            // Printable chars: append to query
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                query.push(c);
                *scroll_pos = 0;
            }

            _ => {}
        }
        return DialogAction::None;
    }

    // HistoryDialog — Up/Down/j/k: navigate, Tab/Left/Right/h/l: switch pane, Enter: jump, Esc: cancel
    // DriveSelection dialog — incremental search + arrow navigation
    if let DialogContent::DriveSelection(DriveSelectionDialog {
        drives,
        selected_index,
        filter,
    }) = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        let filtered_count = if filter.is_empty() {
            drives.len()
        } else {
            let lower = filter.to_lowercase();
            drives
                .iter()
                .filter(|d| {
                    d.display_label().to_lowercase().contains(&lower)
                        || d.path.to_lowercase().contains(&lower)
                })
                .count()
        };
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index + 1 < filtered_count {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => {
                *selected_index = 0;
            }
            KeyCode::End => {
                *selected_index = filtered_count.saturating_sub(1);
            }
            KeyCode::Backspace => {
                if !filter.is_empty() {
                    let mut chars = filter.chars();
                    chars.next_back();
                    *filter = chars.as_str().to_string();
                    *selected_index = 0;
                }
            }
            // Ctrl+K: clear search (also handle raw \x0b from Windows Console API)
            // Do NOT reset selected_index — cursor stays on current item.
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                filter.clear();
            }
            KeyCode::Char('\x0b') => {
                filter.clear();
            }
            // Printable chars: add to search filter (reset to top for new search)
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                filter.push(c);
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // JumpToPath — text input + AND-filter suggestions + arrow navigation
    if let DialogContent::JumpToPath {
        query,
        cursor_pos,
        suggestions,
        selected_index,
        candidates,
        ..
    } = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                }
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                if !suggestions.is_empty() && *selected_index + 1 < suggestions.len() {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => {
                *selected_index = 0;
            }
            KeyCode::End => {
                *selected_index = suggestions.len().saturating_sub(1);
            }
            KeyCode::PageUp => {
                *selected_index = selected_index.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if !suggestions.is_empty() {
                    *selected_index = (*selected_index + 10).min(suggestions.len() - 1);
                }
            }
            KeyCode::Backspace => {
                if !query.is_empty() {
                    let mut chars = query.chars();
                    chars.next_back();
                    *query = chars.as_str().to_string();
                    if *cursor_pos > 0 {
                        *cursor_pos -= 1;
                    }
                    *suggestions = if let Some(s) = search {
                        s.filter_paths(candidates, query)
                    } else {
                        rwf_lib::model::dialog::filter_jump_to_path_suggestions(candidates, query)
                    };
                    *selected_index = 0;
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                query.clear();
                *cursor_pos = 0;
                *suggestions = candidates.clone();
                *selected_index = 0;
            }
            KeyCode::Char('\x0b') => {
                query.clear();
                *cursor_pos = 0;
                *suggestions = candidates.clone();
                *selected_index = 0;
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                query.push(c);
                *cursor_pos += 1;
                *suggestions = if let Some(s) = search {
                    s.filter_paths(candidates, query)
                } else {
                    rwf_lib::model::dialog::filter_jump_to_path_suggestions(candidates, query)
                };
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // JumpToFile — text input + AND-filter suggestions (files + dirs) + arrow navigation
    if let DialogContent::JumpToFile {
        query,
        cursor_pos,
        suggestions,
        selected_index,
        candidates,
        ..
    } = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                }
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                if !suggestions.is_empty() && *selected_index + 1 < suggestions.len() {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => {
                *selected_index = 0;
            }
            KeyCode::End => {
                *selected_index = suggestions.len().saturating_sub(1);
            }
            KeyCode::PageUp => {
                *selected_index = selected_index.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if !suggestions.is_empty() {
                    *selected_index = (*selected_index + 10).min(suggestions.len() - 1);
                }
            }
            KeyCode::Backspace => {
                if !query.is_empty() {
                    let mut chars = query.chars();
                    chars.next_back();
                    *query = chars.as_str().to_string();
                    if *cursor_pos > 0 {
                        *cursor_pos -= 1;
                    }
                    *suggestions = if let Some(s) = search {
                        s.filter_paths(candidates, query)
                    } else {
                        rwf_lib::model::dialog::filter_jump_to_file_suggestions(candidates, query)
                    };
                    *selected_index = 0;
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                query.clear();
                *cursor_pos = 0;
                *suggestions = candidates.clone();
                *selected_index = 0;
            }
            KeyCode::Char('\x0b') => {
                query.clear();
                *cursor_pos = 0;
                *suggestions = candidates.clone();
                *selected_index = 0;
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                query.push(c);
                *cursor_pos += 1;
                *suggestions = if let Some(s) = search {
                    s.filter_paths(candidates, query)
                } else {
                    rwf_lib::model::dialog::filter_jump_to_file_suggestions(candidates, query)
                };
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // CustomFunctionSelector — incremental search + arrow navigation
    if let DialogContent::CustomFunctionSelector {
        functions,
        selected_index,
        filter,
    } = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        let lower = filter.to_lowercase();
        let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
            functions.iter().collect()
        } else {
            functions
                .iter()
                .filter(|f| {
                    f.name.to_lowercase().contains(&lower)
                        || f.description
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&lower)
                })
                .collect()
        };
        let filtered_count = filtered.len();
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => {
                if let Some(func) = filtered.get(*selected_index) {
                    if func.is_menu() {
                        let title = func.name.clone();
                        let items = func.menu_items().to_vec();
                        return DialogAction::OpenMenu { title, items };
                    }
                }
                return DialogAction::Confirm;
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index + 1 < filtered_count {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => {
                *selected_index = 0;
            }
            KeyCode::End => {
                *selected_index = filtered_count.saturating_sub(1);
            }
            KeyCode::Backspace => {
                if !filter.is_empty() {
                    let mut chars = filter.chars();
                    chars.next_back();
                    *filter = chars.as_str().to_string();
                    *selected_index = 0;
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                filter.clear();
            }
            KeyCode::Char('\x0b') => {
                filter.clear();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                filter.push(c);
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // ContextMenu — arrow navigation (skip separators)
    if let DialogContent::ContextMenu(ContextMenuDialog {
        options,
        selected_index,
    }) = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        use rwf_lib::model::dialog::ContextMenuAction;
        let selectable_count = options
            .iter()
            .filter(|o| !matches!(o.action, ContextMenuAction::Separator))
            .count();
        let _ = selectable_count;
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                // Move up, skip separators
                let mut idx = *selected_index;
                loop {
                    if idx == 0 {
                        break;
                    }
                    idx -= 1;
                    if !matches!(options[idx].action, ContextMenuAction::Separator) {
                        *selected_index = idx;
                        break;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                let mut idx = *selected_index;
                loop {
                    if idx + 1 >= options.len() {
                        break;
                    }
                    idx += 1;
                    if !matches!(options[idx].action, ContextMenuAction::Separator) {
                        *selected_index = idx;
                        break;
                    }
                }
            }
            KeyCode::Home => {
                // Jump to first selectable
                for (i, o) in options.iter().enumerate() {
                    if !matches!(o.action, ContextMenuAction::Separator) {
                        *selected_index = i;
                        break;
                    }
                }
            }
            KeyCode::End => {
                // Jump to last selectable
                for (i, o) in options.iter().enumerate().rev() {
                    if !matches!(o.action, ContextMenuAction::Separator) {
                        *selected_index = i;
                        break;
                    }
                }
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // CustomFunctionMenu — second-level menu with separator skipping and char-jump
    if let DialogContent::CustomFunctionMenu {
        items,
        selected_index,
    } = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                let mut idx = *selected_index;
                loop {
                    if idx == 0 {
                        break;
                    }
                    idx -= 1;
                    if items[idx].is_selectable() {
                        *selected_index = idx;
                        break;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                let mut idx = *selected_index;
                loop {
                    if idx + 1 >= items.len() {
                        break;
                    }
                    idx += 1;
                    if items[idx].is_selectable() {
                        *selected_index = idx;
                        break;
                    }
                }
            }
            KeyCode::Home => {
                for (i, item) in items.iter().enumerate() {
                    if item.is_selectable() {
                        *selected_index = i;
                        break;
                    }
                }
            }
            KeyCode::End => {
                for (i, item) in items.iter().enumerate().rev() {
                    if item.is_selectable() {
                        *selected_index = i;
                        break;
                    }
                }
            }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => {
                // Jump to next selectable item whose name starts with c (case-insensitive)
                let lower = c.to_lowercase().next().unwrap_or(c);
                let start = *selected_index + 1;
                let wrap_iter = (start..items.len()).chain(0..start);
                for i in wrap_iter {
                    let item = &items[i];
                    if item.is_selectable() && item.name.to_lowercase().starts_with(lower) {
                        *selected_index = i;
                        break;
                    }
                }
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // RegisteredFolderSelector — incremental search + arrow navigation
    if let DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
        folders,
        selected_index,
        filter,
    }) = &mut dialog.content
    {
        use crossterm::event::KeyCode;
        let filtered_count = if filter.is_empty() {
            folders.len()
        } else {
            let lower = filter.to_lowercase();
            folders
                .iter()
                .filter(|f| {
                    f.name.to_lowercase().contains(&lower) || f.path.to_lowercase().contains(&lower)
                })
                .count()
        };
        match key.code {
            KeyCode::Esc => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Delete => return DialogAction::DeleteSelected,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index + 1 < filtered_count {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => {
                *selected_index = 0;
            }
            KeyCode::End => {
                *selected_index = filtered_count.saturating_sub(1);
            }
            KeyCode::Backspace => {
                if !filter.is_empty() {
                    let mut chars = filter.chars();
                    chars.next_back();
                    *filter = chars.as_str().to_string();
                    *selected_index = 0;
                }
            }
            // Ctrl+K: clear search (also handle raw \x0b from Windows Console API)
            // Do NOT reset selected_index — cursor stays on current item.
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                filter.clear();
            }
            KeyCode::Char('\x0b') => {
                filter.clear();
            }
            // Printable chars: add to search filter (reset to top for new search)
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                filter.push(c);
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    if matches!(&dialog.content, DialogContent::HistoryDialog(_)) {
        use crossterm::event::KeyCode;
        use rwf_lib::model::ui::ActivePane;

        // ── Pane switch (Tab, Left arrow, Right arrow, h, l) ──────────────
        let switch_to: Option<ActivePane> = match key.code {
            KeyCode::Tab => {
                let cur = if let DialogContent::HistoryDialog(HistoryDialogContent {
                    active_pane,
                    ..
                }) = &dialog.content
                {
                    *active_pane
                } else {
                    unreachable!()
                };
                Some(match cur {
                    ActivePane::Left => ActivePane::Right,
                    ActivePane::Right => ActivePane::Left,
                })
            }
            KeyCode::Left | KeyCode::Char('h') => Some(ActivePane::Left),
            KeyCode::Right | KeyCode::Char('l') => Some(ActivePane::Right),
            _ => None,
        };

        if let Some(new_pane) = switch_to {
            // update content
            if let DialogContent::HistoryDialog(HistoryDialogContent { active_pane, .. }) =
                &mut dialog.content
            {
                *active_pane = new_pane;
            }
            // update title separately (no borrow conflict — different fields)
            let pane_label = match new_pane {
                ActivePane::Left => "Left",
                ActivePane::Right => "Right",
            };
            if let Some(bar) = dialog.title.rfind('|') {
                let prefix = dialog.title[..bar].to_string();
                dialog.title = format!("{}| {}]", prefix, pane_label);
            }
            return DialogAction::None;
        }

        // ── Cursor navigation ──────────────────────────────────────────────
        if let DialogContent::HistoryDialog(HistoryDialogContent {
            left_entries,
            right_entries,
            left_selected,
            right_selected,
            active_pane,
            ..
        }) = &mut dialog.content
        {
            let (sel, total) = match active_pane {
                ActivePane::Left => (left_selected, left_entries.len()),
                ActivePane::Right => (right_selected, right_entries.len()),
            };
            match key.code {
                KeyCode::Esc => return DialogAction::Cancel,
                KeyCode::Enter => return DialogAction::Confirm,
                KeyCode::Up | KeyCode::Char('k') => {
                    if *sel + 1 < total {
                        *sel += 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if *sel > 0 {
                        *sel -= 1;
                    }
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    *sel = total.saturating_sub(1);
                }
                KeyCode::End | KeyCode::Char('G') => {
                    *sel = 0;
                }
                _ => {}
            }
        }
        return DialogAction::None;
    }

    // FileConflict dialog - custom input handling with textbox
    if let DialogContent::FileConflict {
        conflicts,
        current_index,
        focused_button,
        rename_text,
        rename_cursor,
        rename_scroll,
        edit_mode,
        vi_mode,
        error_message,
        decisions,
        vi_pending_find_backward,
        vi_pending_operator,
        vi_pending_ctrl_x,
        history,
        history_index,
        ..
    } = &mut dialog.content
    {
        return handle_file_conflict_input(
            conflicts,
            current_index,
            focused_button,
            rename_text,
            rename_cursor,
            rename_scroll,
            edit_mode,
            vi_mode,
            error_message,
            decisions,
            vi_pending_find_backward,
            vi_pending_operator,
            vi_pending_ctrl_x,
            history,
            history_index,
            key,
        );
    }

    // Compression dialog - Vi mode support for Esc (when textbox not focused)
    if let DialogContent::Compression {
        edit_mode,
        vi_mode,
        focused_field,
        ..
    } = &mut dialog.content
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
        if let DialogContent::JobManager { focused_field, .. } = &mut dialog.content {
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
        if let DialogContent::Compression { focused_field, .. } = &mut dialog.content {
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
