//! Dialog system with centralized input handling
//!
//! This module provides a hybrid dialog system with:
//! - Common infrastructure (border, title, buttons, centering)
//! - Content-specific rendering via trait
//! - Centralized input handling with consistent shortcuts

mod compression;
mod extract_confirm;
mod frame;
mod job_manager;

pub use compression::{render_compression_dialog, CompressionDialogState};
pub use extract_confirm::ExtractionConfirmDialog;
pub use frame::{centered_rect_abs, render_dialog_buttons, render_dialog_frame};
pub use job_manager::{
    render_job_manager_dialog, 
    JobManagerDialogState, 
    calculate_job_manager_dialog_min_height,
};

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::model::dialog::{Dialog, DialogContent};
use rwf_lib::config::ViMode;
use tracing::debug;

use super::{smart_truncate, SmartText, TruncateMode};

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
    OpenMenu { title: String, items: Vec<rwf_lib::model::dialog::MenuItem> },
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
    if text.is_empty() || w == 0 { return vec![]; }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < text.len() && out.len() < max_lines as usize {
        let prefix = if out.is_empty() { 1usize } else { 0 };
        let avail = w.saturating_sub(prefix);
        let mut cols = 0usize;
        let mut end = pos;
        for c in text[pos..].chars() {
            let cw = c.width().unwrap_or(1);
            if cols + cw > avail { break; }
            cols += cw;
            end += c.len_utf8();
        }
        if end == pos { break; }
        let chunk = &text[pos..end];
        let s = if out.is_empty() { format!(" {chunk}") } else { chunk.to_string() };
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
        DialogContent::ExtractionConfirm { .. } => {
            // Extraction dialog: ~6 lines content
            6u16
        }
        DialogContent::DeleteConfirm { targets, .. } => {
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
        DialogContent::HistoryDialog { left_entries, right_entries, active_pane, .. } => {
            use rwf_lib::model::ui::ActivePane;
            let len = match active_pane {
                ActivePane::Left  => left_entries.len(),
                ActivePane::Right => right_entries.len(),
            };
            (len as u16 + 2).max(5)
        }
        DialogContent::DriveSelection { drives, .. } => {
            // list + hint(1) + search(1)
            (drives.len() as u16 + 2).max(6)
        }
        DialogContent::RegisteredFolderSelector { folders, .. } => {
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
        DialogContent::ContextMenu { options, .. } => {
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
        DialogContent::FileInfo { .. } => 11u16,  // name+path+size+type+3×datetime + hint
        DialogContent::PatternRename { preview, .. } => {
            // find(1) + replace(1) + flags(1) + mode-row(1) + separator(1) + preview rows + status(1) = 6 + preview count, min 8
            (preview.len() as u16 + 6).max(8)
        }
        DialogContent::Help { .. } => {
            // tab bar(1) + search(1) + entries + hint(1), min 8
            20u16.max(8)
        }
        DialogContent::Error { message, .. } => {
            // message lines + blank(1) + buttons(3), min 5
            (message.lines().count() as u16 + 4).max(5)
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
        DialogContent::HistoryDialog { .. } | DialogContent::DriveSelection { .. } | DialogContent::PatternRename { .. } | DialogContent::Help { .. } | DialogContent::RegisteredFolderSelector { .. } | DialogContent::CustomFunctionSelector { .. } | DialogContent::JumpToPath { .. } | DialogContent::JumpToFile { .. } | DialogContent::DeleteConfirm { .. } => {
            let percent_height = (screen_height * 80) / 100;
            percent_height.max(min_dialog_height).min(screen_height.saturating_sub(2))
        }
        DialogContent::CustomFunctionMenu { .. } => {
            // Exact size, same as ContextMenu
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::ContextMenu { .. } => {
            // Exact size for context menu
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::FileConflict { .. } | DialogContent::SortDialog { .. } | DialogContent::FileMask { .. } | DialogContent::WildcardMark { .. } | DialogContent::SimpleRename { .. } | DialogContent::FileInfo { .. } | DialogContent::ExtractionConfirm { .. } | DialogContent::Error { .. } => {
            // Use exact minimum height for compact dialogs
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        _ => {
            // Use 70% of screen or minimum, whichever is larger
            let percent_height = (screen_height * 70) / 100;
            percent_height.max(min_dialog_height).min(screen_height.saturating_sub(2))
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
        DialogContent::DriveSelection { .. } | DialogContent::RegisteredFolderSelector { .. } => {
            60u16.min(screen_width.saturating_sub(2)).max(40)
        }
        DialogContent::CustomFunctionSelector { .. } => {
            ((screen_width * 70) / 100).max(50).min(screen_width.saturating_sub(2))
        }
        DialogContent::CustomFunctionMenu { items, .. } => {
            // label fits with outer_width = max_label + 8 (2 border + 4 indent + 2 margin)
            // hint "[Enter] Execute  [Esc] Close" (29 chars) fits at offset+1 with width-2 when outer>=34
            let max_label = items.iter().filter(|i| i.is_selectable()).map(|i| i.name.len()).max().unwrap_or(10);
            ((max_label as u16 + 8).max(34)).min(screen_width.saturating_sub(2))
        }
        DialogContent::ContextMenu { options, .. } => {
            let max_label = options.iter().map(|o| o.label.len()).max().unwrap_or(10);
            ((max_label as u16 + 6).max(24)).min(screen_width.saturating_sub(2))
        }
        DialogContent::PatternRename { .. } | DialogContent::JumpToPath { .. } | DialogContent::JumpToFile { .. } => {
            ((screen_width * 80) / 100).max(40).min(screen_width.saturating_sub(2))
        }
        DialogContent::Help { .. } => {
            ((screen_width * 70) / 100).max(40).min(screen_width.saturating_sub(2))
        }
        _ => ((screen_width * 60) / 100).max(40).min(screen_width.saturating_sub(2)),
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
        DialogContent::JobManager { selected_index, focused_field } => {
            // Render Job Manager dialog with its own layout (Part 6.2)
            let dialog_state = JobManagerDialogState {
                selected_index: *selected_index,
                focused_field: *focused_field,
                job_list_focus_index: *selected_index,
            };
            render_job_manager_dialog(frame, content_area, state, &dialog_state);
        }
        DialogContent::CloseTabWithActiveJob { tab_name, job_ids, focused_field, .. } => {
            // Render Close Tab confirmation dialog with buttons (compact layout)
            let job_list = if job_ids.len() == 1 {
                format!("Job #{} is still running.", job_ids[0])
            } else {
                let job_strs: Vec<String> = job_ids.iter().map(|id| format!("#{}", id)).collect();
                format!("Jobs {} are still running.", job_strs.join(", "))
            };
            let message = format!("{} {}\nClose this tab and cancel the job(s)?", tab_name, job_list);

            // Use compact layout: message takes remaining space, buttons fixed at 3 lines
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(2),  // Message (compact)
                    Constraint::Length(3), // Buttons
                ])
                .split(content_area);

            let confirmation = Paragraph::new(message)
                .style(Style::default()
                    .fg(Color::Black)
                    .bg(Color::Gray));

            frame.render_widget(confirmation, chunks[0]);

            // Render buttons (OK/Cancel) with proper focus
            render_dialog_buttons(frame, chunks[1], &dialog.content, *focused_field);
        }
        DialogContent::FileConflict { conflicts, current_index, focused_button, rename_text, rename_cursor, rename_scroll, edit_mode, vi_mode, error_message, .. } => {
            // Render File Conflict dialog with TextInput widget
            render_file_conflict_dialog(frame, content_area, conflicts, *current_index, *focused_button, rename_text, *rename_cursor, *rename_scroll, *edit_mode, *vi_mode, error_message);
        }
        DialogContent::SortDialog { selected_mode_index, selected_order_index, focused_section } => {
            render_sort_dialog(frame, content_area, *selected_mode_index, *selected_order_index, *focused_section);
        }
        DialogContent::FileMask { input, cursor_pos, scroll_pos, focused_field } => {
            render_file_mask_dialog(frame, content_area, input, *cursor_pos, *scroll_pos, *focused_field);
        }
        DialogContent::WildcardMark { input, cursor_pos, scroll_pos, focused_field } => {
            render_wildcard_mark_dialog(frame, content_area, input, *cursor_pos, *scroll_pos, *focused_field);
        }
        DialogContent::SimpleRename { input, cursor_pos, scroll_pos, focused_field } => {
            render_simple_rename_dialog(frame, content_area, input, *cursor_pos, *scroll_pos, *focused_field);
        }
        DialogContent::HistoryDialog {
            left_entries, right_entries,
            left_selected, right_selected,
            left_current_pos, right_current_pos,
            active_pane,
        } => {
            use rwf_lib::model::ui::ActivePane;
            let (entries, selected, current_pos) = match active_pane {
                ActivePane::Left  => (left_entries.as_slice(),  *left_selected,  *left_current_pos),
                ActivePane::Right => (right_entries.as_slice(), *right_selected, *right_current_pos),
            };
            render_history_dialog(frame, content_area, entries, selected, current_pos);
        }
        DialogContent::DriveSelection { drives, selected_index, filter } => {
            render_drive_selection_dialog(frame, content_area, drives, *selected_index, filter);
        }
        DialogContent::RegisteredFolderSelector { folders, selected_index, filter } => {
            render_registered_folder_selector(frame, content_area, folders, *selected_index, filter);
        }
        DialogContent::CustomFunctionSelector { functions, selected_index, filter } => {
            render_custom_function_selector(frame, content_area, functions, *selected_index, filter);
        }
        DialogContent::CustomFunctionMenu { items, selected_index } => {
            render_custom_function_menu(frame, content_area, items, *selected_index);
        }
        DialogContent::ContextMenu { options, selected_index } => {
            render_context_menu_dialog(frame, content_area, options, *selected_index);
        }
        DialogContent::JumpToPath { query, cursor_pos, suggestions, selected_index, loading_job_id, .. } => {
            render_jump_to_path_dialog(frame, content_area, query, *cursor_pos, suggestions, *selected_index, loading_job_id.is_some());
        }
        DialogContent::JumpToFile { query, cursor_pos, suggestions, selected_index, loading_job_id, .. } => {
            render_jump_to_file_dialog(frame, content_area, query, *cursor_pos, suggestions, *selected_index, loading_job_id.is_some());
        }
        DialogContent::FileInfo {
            file_name, file_path, size, created, modified, accessed,
            is_dir, is_readonly,
            #[cfg(unix)] permissions,
            #[cfg(unix)] owner,
            #[cfg(unix)] group,
        } => {
            render_file_info_dialog(
                frame, content_area,
                file_name, file_path, *size, *created, *modified, *accessed,
                *is_dir, *is_readonly,
                #[cfg(unix)] *permissions,
                #[cfg(unix)] owner.as_deref(),
                #[cfg(unix)] group.as_deref(),
            );
        }
        DialogContent::PatternRename {
            find, find_cursor_pos, find_scroll_pos,
            replace, replace_cursor_pos, replace_scroll_pos,
            use_regex, case_sensitive,
            preview, focused_field, preview_scroll, preview_horizontal_scroll,
            error_message, preview_mode, show_all,
        } => {
            render_pattern_rename_dialog(
                frame, content_area,
                find, *find_cursor_pos, *find_scroll_pos,
                replace, *replace_cursor_pos, *replace_scroll_pos,
                *use_regex, *case_sensitive,
                preview, *focused_field, *preview_scroll, *preview_horizontal_scroll,
                error_message.as_deref(),
                *preview_mode, *show_all,
            );
        }
        DialogContent::Help { entries, query, regex_mode, show_unbound, active_tab, scroll_pos, language, .. } => {
            render_help_dialog(frame, content_area, area, entries, query, *regex_mode, *show_unbound, active_tab, *scroll_pos, language);
        }
        DialogContent::DeleteConfirm { targets, scroll_offset } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // header + blank + list items
                    Constraint::Length(1), // spacer (hint 1 line down)
                    Constraint::Length(1), // hint
                    Constraint::Length(3), // buttons
                ])
                .split(content_area);

            let base  = Style::default().fg(Color::Black).bg(Color::Gray);
            let dir_s = Style::default().fg(Color::Black).bg(Color::Gray).add_modifier(Modifier::BOLD);
            let hint  = Style::default().fg(Color::DarkGray).bg(Color::Gray);
            let w = content_area.width.saturating_sub(2) as usize;
            let total = targets.len();

            // Header
            let header_txt = if total == 1 { "Delete this item?".to_string() }
                             else { format!("Delete these {} items?", total) };
            frame.render_widget(
                Paragraph::new(smart_truncate(&header_txt, w, "…")).style(base),
                Rect::new(chunks[0].x + 1, chunks[0].y, w as u16, 1),
            );

            // List items (chunks[0]: row 0 = header, row 1 = up-indicator or blank, rows 2+ = items)
            let list_h = (chunks[0].height as usize).saturating_sub(2);
            let max_scroll = total.saturating_sub(list_h);
            let scroll = (*scroll_offset).min(max_scroll);
            let remaining_below = total.saturating_sub(scroll + list_h);
            let show_up   = scroll > 0;
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
                if item_idx >= total { break; }
                let y = chunks[0].y + 2 + row as u16;
                let (loc, is_dir) = &targets[item_idx];
                let raw_name = loc.path()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| loc.display_path());
                let label = if *is_dir { format!("  {}/", raw_name) } else { format!("  {}", raw_name) };
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
            let hint_txt = if total > 1 { "↑↓:scroll  Enter:delete  Esc:cancel" }
                           else { "Enter:delete  Esc:cancel" };
            frame.render_widget(
                Paragraph::new(smart_truncate(hint_txt, w, "…")).style(hint),
                Rect::new(chunks[2].x + 1, chunks[2].y, w as u16, 1),
            );

            // Buttons (chunks[3])
            render_dialog_buttons(frame, chunks[3], &dialog.content, 0);
        }
        _ => {
            // Split content area for buttons (generic layout)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5),  // Content
                    Constraint::Length(3), // Buttons
                ])
                .split(content_area);

            // Render content-specific widgets
            render_dialog_content(frame, &dialog.content, chunks[0], true);
            render_dialog_buttons(frame, chunks[1], &dialog.content, 0);
        }
    }
}

/// Render File Conflict dialog (compact layout with vertical buttons)
fn render_file_conflict_dialog(
    frame: &mut Frame,
    area: Rect,
    conflicts: &[rwf_lib::model::dialog::ConflictPair],
    current_index: usize,
    focused_button: usize,
    rename_text: &str,
    rename_cursor: usize,
    rename_scroll: usize,
    edit_mode: rwf_lib::config::EditMode,
    vi_mode: Option<rwf_lib::config::ViMode>,
    error_message: &Option<String>,
) {
    let current = &conflicts[current_index];
    let (indicator, message) = current.get_status_message();
    let content_width = area.width.saturating_sub(4) as usize; // 2 chars margin on each side
    let textbox_width = area.width.saturating_sub(20) as usize;  // Leave space for button

    // Line 0: Filename
    let filename_line = format!("Filename: {}", smart_truncate(&current.source.name, content_width.saturating_sub(10), "..."));
    let filename_para = Paragraph::new(filename_line)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(filename_para, Rect::new(area.x + 2, area.y, content_width as u16, 1));

    // --- FROM SECTION (Lines 1-4) ---
    // Line 1: "From:" label
    let from_label = Paragraph::new("From:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(from_label, Rect::new(area.x + 2, area.y + 1, content_width as u16, 1));

    // Line 2-3: From path (2 lines) using SmartText
    let from_full_path = current.source_path.display_path();
    let from_path_widget = SmartText::new(&from_full_path)
        .style(Style::default().fg(Color::Black).bg(Color::Gray))
        .max_lines(2)
        .mode(TruncateMode::Path);
    from_path_widget.render(frame, Rect::new(area.x + 4, area.y + 2, content_width.saturating_sub(2) as u16, 2));

    // Line 4: From size,date
    let from_info = format!("  Size,Date: {} Bytes, {}",
        current.source.size,
        chrono::DateTime::<chrono::Local>::from(current.source.modified).format("%Y-%m-%d %H:%M:%S"));
    let from_info_para = Paragraph::new(from_info)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(from_info_para, Rect::new(area.x + 2, area.y + 4, content_width as u16, 1));

    // Line 5: Blank

    // --- TO SECTION (Lines 6-9) ---
    // Line 6: "To:" label
    let to_label = Paragraph::new("To:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(to_label, Rect::new(area.x + 2, area.y + 6, content_width as u16, 1));

    // Line 7-8: To path (2 lines) using SmartText
    let to_full_path = current.dest_path.display_path();
    let to_path_widget = SmartText::new(&to_full_path)
        .style(Style::default().fg(Color::Black).bg(Color::Gray))
        .max_lines(2)
        .mode(TruncateMode::Path);
    to_path_widget.render(frame, Rect::new(area.x + 4, area.y + 7, content_width.saturating_sub(2) as u16, 2));

    // Line 9: To size,date
    let to_info = format!("  Size,Date: {} Bytes, {}",
        current.dest.size,
        chrono::DateTime::<chrono::Local>::from(current.dest.modified).format("%Y-%m-%d %H:%M:%S"));
    let to_info_para = Paragraph::new(to_info)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(to_info_para, Rect::new(area.x + 2, area.y + 9, content_width as u16, 1));

    // Line 10: Blank

    // Line 11: Status indicator
    let status_line = format!("{} {}", indicator, message);
    let status_style = if indicator == "✓" {
        Style::default().fg(Color::Green).bg(Color::Gray)
    } else {
        Style::default().fg(Color::Yellow).bg(Color::Gray)
    };
    let status_para = Paragraph::new(status_line)
        .style(status_style);
    frame.render_widget(status_para, Rect::new(area.x + 2, area.y + 11, content_width as u16, 1));

    // Line 12: Blank

    // Lines 13-16: Buttons (vertical layout)
    // Focus fields: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename (Textbox), 4=Cancel
    let button_texts = [
        ("Force Overwrite", false),
        ("Overwrite if New", false),
        ("Skip", false),
        ("Rename:", false), // Label for textbox
        ("Cancel", false),
    ];

    let mut button_y = area.y + 13;
    for (i, (label, is_default)) in button_texts.iter().enumerate() {
        let button_text = if *is_default {
            format!("[*{}*]", label)
        } else {
            format!("[{}]", label)
        };

        let button_is_focused = focused_button == i;

        if i == 3 {
            // Rename label + Textbox
            let label_style = if button_is_focused {
                Style::default().fg(Color::Yellow).bg(Color::Gray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black).bg(Color::Gray)
            };

            // Render "Rename:" label
            let label_para = Paragraph::new(label.to_string()).style(label_style);
            frame.render_widget(label_para, Rect::new(area.x + 2, button_y, label.len() as u16, 1));

            // Render textbox using TextInput widget (dedicated focus field 3)
            let textbox_x = area.x + 2 + label.len() as u16 + 1;
            let textbox_width_u16 = textbox_width.saturating_sub(label.len()) as u16;

            // Create TextInput widget for rendering
            let mut text_input = crate::ui::text_input::TextInput::new(Some(rename_text.to_string()), edit_mode);
            // Restore Vi mode state
            if let Some(vm) = vi_mode {
                text_input.set_vi_mode(vm);
            }
            text_input.set_width(textbox_width_u16);
            text_input.set_cursor(rename_cursor);
            text_input.set_scroll(rename_scroll);
            text_input.render(frame, Rect::new(textbox_x, button_y, textbox_width_u16, 1), button_is_focused);
        } else {
            let button_style = if button_is_focused {
                Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black).bg(Color::Gray)
            };
            let button_para = Paragraph::new(button_text.clone()).style(button_style);
            frame.render_widget(button_para, Rect::new(area.x + 2, button_y, button_text.len() as u16, 1));
        }

        button_y += 1;
    }

    // Hint text (shown when Force, Overwrite if New, or Skip focused)
    if focused_button == 0 || focused_button == 1 || focused_button == 2 {
        let hint = Paragraph::new("(Shift+Enter for the rest)")
            .style(Style::default().fg(Color::DarkGray).bg(Color::Gray));
        frame.render_widget(hint, Rect::new(area.x + 2, button_y, content_width as u16, 1));
    }

    // Error message line - ALWAYS AT BOTTOM OF DIALOG AREA
    if let Some(error) = error_message {
        let error_para = Paragraph::new(format!("Error: {}", error))
            .style(Style::default().fg(Color::Red).bg(Color::Gray).add_modifier(Modifier::BOLD));
        // Place at the last line of the provided area
        frame.render_widget(error_para, Rect::new(area.x + 2, area.y + area.height - 1, content_width as u16, 1));
    }
}

/// Render dialog content based on type
/// Render the Sort dialog (sort key + order + OK/Cancel)
///
/// Layout (11 lines total, per DIALOG_DESIGN_SPEC.md):
///   5 = label "Sort by:" + 4 items
///   1 = spacer
///   3 = label "Order:" + 2 items
///   1 = spacer
///   1 = buttons [*OK*] [Cancel]
fn render_file_mask_dialog(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    cursor_pos: usize,
    scroll_pos: usize,
    focused_field: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // blank
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint1: Multiple patterns
            Constraint::Length(1), // hint2: Exclusion
            Constraint::Length(1), // hint3: Regexp
            Constraint::Length(1), // blank/spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let item_width = area.width.saturating_sub(4);

    // Prompt
    frame.render_widget(
        Paragraph::new("Enter file mask (* = any chars, ? = single char):").style(base_style),
        Rect::new(area.x + 2, chunks[1].y, item_width, 1),
    );

    // Textbox
    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(frame, Rect::new(area.x + 2, chunks[2].y, item_width, 1), focused_field == 0);
    }

    // Hint lines
    frame.render_widget(
        Paragraph::new("Multiple patterns: *.txt *.doc").style(hint_style),
        Rect::new(area.x + 2, chunks[3].y, item_width, 1),
    );
    frame.render_widget(
        Paragraph::new("Exclusion: :*.txt :temp*").style(hint_style),
        Rect::new(area.x + 2, chunks[4].y, item_width, 1),
    );
    frame.render_widget(
        Paragraph::new("Regexp: /.*\\.json$/ /TEST/i /Test/").style(hint_style),
        Rect::new(area.x + 2, chunks[5].y, item_width, 1),
    );

    // Buttons [*OK*] [Cancel]
    let focused_item = Style::default().fg(Color::Black).bg(Color::White);
    let ok_style     = if focused_field == 1 { focused_item } else { base_style };
    let cancel_style = if focused_field == 2 { focused_item } else { base_style };
    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line).alignment(Alignment::Center).style(base_style),
        chunks[7],
    );
}

fn render_wildcard_mark_dialog(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    cursor_pos: usize,
    scroll_pos: usize,
    focused_field: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let item_width = area.width.saturating_sub(4);

    frame.render_widget(
        Paragraph::new("Enter pattern to mark:").style(base_style),
        Rect::new(area.x + 2, chunks[0].y, item_width, 1),
    );

    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(frame, Rect::new(area.x + 2, chunks[1].y, item_width, 1), focused_field == 0);
    }

    frame.render_widget(
        Paragraph::new("(* = any chars, ? = one char)").style(hint_style),
        Rect::new(area.x + 2, chunks[2].y, item_width, 1),
    );

    let focused_item = Style::default().fg(Color::Black).bg(Color::White);
    let ok_style     = if focused_field == 1 { focused_item } else { base_style };
    let cancel_style = if focused_field == 2 { focused_item } else { base_style };
    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line).alignment(Alignment::Center).style(base_style),
        chunks[4],
    );
}

fn render_simple_rename_dialog(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    cursor_pos: usize,
    scroll_pos: usize,
    focused_field: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // textbox
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let item_width = area.width.saturating_sub(4);

    frame.render_widget(
        Paragraph::new("New name:").style(base_style),
        Rect::new(area.x + 2, chunks[0].y, item_width, 1),
    );

    {
        use crate::ui::text_input::TextInput;
        let mut ti = TextInput::new(Some(input.to_string()), rwf_lib::config::EditMode::Emacs);
        ti.set_original_text(input.to_string());
        ti.set_cursor(cursor_pos);
        ti.set_scroll(scroll_pos);
        ti.set_width(item_width);
        ti.render(frame, Rect::new(area.x + 2, chunks[1].y, item_width, 1), focused_field == 0);
    }

    frame.render_widget(
        Paragraph::new("(Enter to confirm, Esc to cancel)").style(hint_style),
        Rect::new(area.x + 2, chunks[2].y, item_width, 1),
    );

    let focused_item = Style::default().fg(Color::Black).bg(Color::White);
    let ok_style     = if focused_field == 1 { focused_item } else { base_style };
    let cancel_style = if focused_field == 2 { focused_item } else { base_style };
    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line).alignment(Alignment::Center).style(base_style),
        chunks[4],
    );
}

fn render_history_dialog(
    frame: &mut Frame,
    area: Rect,
    entries: &[rwf_lib::model::Location],
    selected_index: usize,
    current_pos: usize,
) {
    let base_style   = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint_style   = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let current_marker_style = Style::default().fg(Color::Yellow).bg(Color::Gray).add_modifier(Modifier::BOLD);

    let item_width = area.width.saturating_sub(4) as usize;

    // Hint line at the bottom
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("Enter: jump  Esc: cancel  ↑↓: navigate").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    // Entries (oldest at bottom, newest at top — reversed display)
    // visible_area: all rows except the last hint line
    let list_height = area.height.saturating_sub(1) as usize;
    let total = entries.len();

    // Compute scroll window so selected_index stays visible (reversed display)
    // Display index = total - 1 - entry_index (newest at row 0)
    let display_selected = total.saturating_sub(1).saturating_sub(selected_index);
    let scroll_start = if display_selected >= list_height {
        display_selected + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let display_idx = scroll_start + row;
        if display_idx >= total {
            break;
        }
        // Convert display index back to stack index (reversed)
        let entry_idx = total - 1 - display_idx;
        let entry = &entries[entry_idx];
        let path_str = smart_truncate(&entry.display_path(), item_width.saturating_sub(3), "…");

        let (prefix, row_style) = if entry_idx == selected_index {
            (">", selected_style)
        } else if entry_idx == current_pos {
            ("*", current_marker_style)
        } else {
            (" ", base_style)
        };

        let line = format!("{} {}", prefix, path_str);
        frame.render_widget(
            Paragraph::new(line).style(row_style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}

fn render_drive_selection_dialog(
    frame: &mut Frame,
    area: Rect,
    drives: &[rwf_lib::model::dialog::DriveInfo],
    selected_index: usize,
    filter: &str,
) {
    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let search_style   = Style::default().fg(Color::Black).bg(Color::Gray);

    let item_width = area.width.saturating_sub(4) as usize;

    // Compute filtered list
    let filtered: Vec<&rwf_lib::model::dialog::DriveInfo> = if filter.is_empty() {
        drives.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        drives.iter().filter(|d| {
            d.display_label().to_lowercase().contains(&lower)
                || d.path.to_lowercase().contains(&lower)
        }).collect()
    };

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    // Hint line (second-to-last row) and search line (last row)
    let hint_y   = area.y + area.height.saturating_sub(2);
    let search_y = area.y + area.height.saturating_sub(1);

    frame.render_widget(
        Paragraph::new("Enter: go  Esc: cancel  ↑↓: select  Bksp: del char  ^K: clear").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("/{}", filter)).style(search_style),
        Rect::new(area.x + 2, search_y, item_width as u16, 1),
    );

    // List area (all rows except hint + search)
    let list_height = area.height.saturating_sub(2) as usize;
    let scroll_start = if clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let fi = scroll_start + row;
        if fi >= filtered.len() { break; }
        let drive = filtered[fi];
        let label = smart_truncate(&drive.display_label(), item_width.saturating_sub(2), "…");
        let style = if fi == clamped_sel { selected_style } else { base_style };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}

fn render_registered_folder_selector(
    frame: &mut Frame,
    area: Rect,
    folders: &[rwf_lib::model::dialog::RegisteredFolder],
    selected_index: usize,
    filter: &str,
) {
    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);

    let item_width = area.width.saturating_sub(4) as usize;

    // Compute filtered list
    let filtered: Vec<&rwf_lib::model::dialog::RegisteredFolder> = if filter.is_empty() {
        folders.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        folders.iter().filter(|f| {
            f.name.to_lowercase().contains(&lower)
                || f.path.to_lowercase().contains(&lower)
        }).collect()
    };

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    // Hint line (second-to-last row) and search line (last row)
    let hint_y   = area.y + area.height.saturating_sub(2);
    let search_y = area.y + area.height.saturating_sub(1);

    frame.render_widget(
        Paragraph::new("[Enter] Jump to folder [Delete] Remove selected [Esc] Cancel").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("/{}", filter)).style(base_style),
        Rect::new(area.x + 2, search_y, item_width as u16, 1),
    );

    // List area (all rows except hint + search)
    let list_height = area.height.saturating_sub(2) as usize;
    let scroll_start = if clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let fi = scroll_start + row;
        if fi >= filtered.len() { break; }
        let folder = filtered[fi];
        let label = if folder.name.is_empty() {
            smart_truncate(&folder.path, item_width.saturating_sub(2), "…")
        } else {
            smart_truncate(&format!("{} — {}", folder.name, folder.path), item_width.saturating_sub(2), "…")
        };
        let style = if fi == clamped_sel { selected_style } else { base_style };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}

fn render_custom_function_selector(
    frame: &mut Frame,
    area: Rect,
    functions: &[rwf_lib::model::dialog::CustomFunction],
    selected_index: usize,
    filter: &str,
) {
    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);

    let item_width = area.width.saturating_sub(4) as usize;

    let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
        functions.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        functions.iter().filter(|f| {
            f.name.to_lowercase().contains(&lower)
                || f.description.as_deref().unwrap_or("").to_lowercase().contains(&lower)
        }).collect()
    };

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    let hint_y   = area.y + area.height.saturating_sub(2);
    let search_y = area.y + area.height.saturating_sub(1);

    frame.render_widget(
        Paragraph::new("[Enter] Execute  [Esc] Cancel").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("/{}", filter)).style(base_style),
        Rect::new(area.x + 2, search_y, item_width as u16, 1),
    );

    let list_height = area.height.saturating_sub(2) as usize;
    let scroll_start = if clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let fi = scroll_start + row;
        if fi >= filtered.len() { break; }
        let func = filtered[fi];
        let name_w = item_width.saturating_sub(2);
        let label = if let Some(desc) = &func.description {
            let desc_w = name_w.saturating_sub(func.name.len() + 3);
            if desc_w > 4 {
                format!("{:<name_w$}", format!("{}  {}", func.name, smart_truncate(desc, desc_w, "…")), name_w = name_w)
            } else {
                smart_truncate(&func.name, name_w, "…")
            }
        } else {
            smart_truncate(&func.name, name_w, "…")
        };
        let style = if fi == clamped_sel { selected_style } else { base_style };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}

fn render_custom_function_menu(
    frame: &mut Frame,
    area: Rect,
    items: &[rwf_lib::model::dialog::MenuItem],
    selected_index: usize,
) {
    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let sep_style      = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);

    // item_width = inner width - 4 (2 left-indent + 2 right-margin for items)
    let item_width = area.width.saturating_sub(4) as usize;

    // Hint at offset+1 with full inner width-2 — avoids the 1-char right clip from offset+2
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("[Enter] Execute  [Esc] Close").style(hint_style),
        Rect::new(area.x + 1, hint_y, area.width.saturating_sub(2), 1),
    );

    let list_height = area.height.saturating_sub(1) as usize;
    let scroll_start = if selected_index >= list_height {
        selected_index + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let ii = scroll_start + row;
        if ii >= items.len() { break; }
        let item = &items[ii];
        if item.is_separator() {
            // Separator spans item_width
            let sep = "─".repeat(item_width.saturating_sub(1));
            frame.render_widget(
                Paragraph::new(sep).style(sep_style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        } else {
            // No truncation: dialog is sized to fit the longest label
            let style = if ii == selected_index { selected_style } else { base_style };
            frame.render_widget(
                Paragraph::new(format!(" {}", item.name)).style(style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        }
    }
}

fn render_context_menu_dialog(
    frame: &mut Frame,
    area: Rect,
    options: &[rwf_lib::model::dialog::ContextMenuOption],
    selected_index: usize,
) {
    use rwf_lib::model::dialog::ContextMenuAction;

    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let sep_style      = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);

    let item_width = area.width.saturating_sub(4) as usize;

    // hint on last row
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("[Enter] Select  [Esc] Cancel").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    let list_height = area.height.saturating_sub(1) as usize;
    // compute scroll so selected item is visible
    let scroll_start = if selected_index >= list_height {
        selected_index + 1 - list_height
    } else {
        0
    };

    for row in 0..list_height {
        let oi = scroll_start + row;
        if oi >= options.len() { break; }
        let opt = &options[oi];
        let is_sep = matches!(opt.action, ContextMenuAction::Separator);
        if is_sep {
            let sep_text = "─".repeat(item_width.saturating_sub(2));
            frame.render_widget(
                Paragraph::new(format!(" {}", sep_text)).style(sep_style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        } else {
            let label = smart_truncate(&opt.label, item_width.saturating_sub(2), "…");
            let style = if oi == selected_index { selected_style } else { base_style };
            frame.render_widget(
                Paragraph::new(format!(" {}", label)).style(style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        }
    }
}

fn render_jump_to_path_dialog(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    cursor_pos: usize,
    suggestions: &[String],
    selected_index: usize,
    is_loading: bool,
) {
    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let input_style    = Style::default().fg(Color::White).bg(Color::Black);
    let sep_style      = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let preview_style  = Style::default().fg(Color::White).bg(Color::Black);

    let clamped_sel = if suggestions.is_empty() { 0 } else { selected_index.min(suggestions.len() - 1) };
    let item_width = area.width.saturating_sub(4) as usize;

    // ── Row 0: input field + hit count ────────────────────────────────────
    let status = if is_loading {
        if suggestions.is_empty() { "searching…".to_string() } else { format!("{}+ hits", suggestions.len()) }
    } else if suggestions.is_empty() { "No match".to_string() }
    else { format!("{} hits", suggestions.len()) };
    let status_width: u16 = 10;
    let input_width = area.width.saturating_sub(status_width + 3).max(4);
    // Horizontal scroll: show the end of the query when cursor is near it
    let q_chars: Vec<char> = query.chars().collect();
    let visible_chars = input_width as usize;
    let scroll = if cursor_pos > visible_chars { cursor_pos - visible_chars } else { 0 };
    let visible_query: String = q_chars.iter().skip(scroll).take(visible_chars).collect();
    let input_text = format!("{:<width$}", visible_query, width = visible_chars);
    frame.render_widget(
        Paragraph::new(input_text).style(input_style),
        Rect::new(area.x + 1, area.y, input_width, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("{:>width$}", status, width = status_width as usize)).style(base_style),
        Rect::new(area.x + 1 + input_width, area.y, status_width, 1),
    );

    // ── Row 1: separator ──────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new("─".repeat(item_width)).style(sep_style),
        Rect::new(area.x + 2, area.y + 1, item_width as u16, 1),
    );

    // ── Rows 2..height-6: suggestion list ────────────────────────────────
    // Footer = sep(1) + preview(4) + hint(1) = 6 rows
    let header_rows: u16 = 2;
    let footer_rows: u16 = 6;
    let list_height = area.height.saturating_sub(header_rows + footer_rows) as usize;
    let scroll_start = if list_height > 0 && clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };
    for row in 0..list_height {
        let si = scroll_start + row;
        if si >= suggestions.len() { break; }
        let path = &suggestions[si];
        let label = smart_truncate(path, item_width.saturating_sub(2), "…");
        let style = if si == clamped_sel { selected_style } else { base_style };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(area.x + 2, area.y + header_rows + row as u16, item_width as u16, 1),
        );
    }

    // ── Row height-6: separator before preview ────────────────────────────
    let sep2_y = area.y + area.height.saturating_sub(6);
    frame.render_widget(
        Paragraph::new("─".repeat(item_width)).style(sep_style),
        Rect::new(area.x + 2, sep2_y, item_width as u16, 1),
    );

    // ── Rows height-5..height-2: full-path preview (4 lines, char-chunked) ─
    let preview_y = area.y + area.height.saturating_sub(5);
    let preview_w = area.width.saturating_sub(2);
    let preview_lines = if !suggestions.is_empty() && clamped_sel < suggestions.len() {
        let raw = &suggestions[clamped_sel];
        let text = if raw.len() > 1024 { &raw[..1024] } else { raw.as_str() };
        chunk_path_preview(text, preview_w, 4)
    } else { vec![] };
    frame.render_widget(
        Paragraph::new(preview_lines).style(preview_style),
        Rect::new(area.x + 1, preview_y, preview_w, 4),
    );

    // ── Row height-1: hint ────────────────────────────────────────────────
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("↑↓:select  Enter:jump  Esc:cancel  Bksp:del  ^K:clear").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    // ── Cursor rendering on input row ─────────────────────────────────────
    let cursor_x_in_visible = cursor_pos.saturating_sub(scroll);
    let cursor_screen_x = (area.x + 1 + cursor_x_in_visible as u16).min(area.x + input_width);
    frame.set_cursor_position((cursor_screen_x, area.y));
}

fn render_jump_to_file_dialog(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    cursor_pos: usize,
    suggestions: &[String],
    selected_index: usize,
    is_loading: bool,
) {
    let base_style     = Style::default().fg(Color::Black).bg(Color::Gray);
    let selected_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let hint_style     = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let input_style    = Style::default().fg(Color::White).bg(Color::Black);
    let sep_style      = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let preview_style  = Style::default().fg(Color::White).bg(Color::Black);

    let clamped_sel = if suggestions.is_empty() { 0 } else { selected_index.min(suggestions.len() - 1) };
    let item_width = area.width.saturating_sub(4) as usize;

    // ── Row 0: input field + hit count ────────────────────────────────────
    let status = if is_loading {
        if suggestions.is_empty() { "searching…".to_string() } else { format!("{}+ hits", suggestions.len()) }
    } else if suggestions.is_empty() { "No match".to_string() }
    else { format!("{} hits", suggestions.len()) };
    let status_width: u16 = 10;
    let input_width = area.width.saturating_sub(status_width + 3).max(4);
    let q_chars: Vec<char> = query.chars().collect();
    let visible_chars = input_width as usize;
    let scroll = if cursor_pos > visible_chars { cursor_pos - visible_chars } else { 0 };
    let visible_query: String = q_chars.iter().skip(scroll).take(visible_chars).collect();
    let input_text = format!("{:<width$}", visible_query, width = visible_chars);
    frame.render_widget(
        Paragraph::new(input_text).style(input_style),
        Rect::new(area.x + 1, area.y, input_width, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("{:>width$}", status, width = status_width as usize)).style(base_style),
        Rect::new(area.x + 1 + input_width, area.y, status_width, 1),
    );

    // ── Row 1: separator ──────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new("─".repeat(item_width)).style(sep_style),
        Rect::new(area.x + 2, area.y + 1, item_width as u16, 1),
    );

    // ── Rows 2..height-6: suggestion list ────────────────────────────────
    // Footer = sep(1) + preview(4) + hint(1) = 6 rows
    let header_rows: u16 = 2;
    let footer_rows: u16 = 6;
    let list_height = area.height.saturating_sub(header_rows + footer_rows) as usize;
    let scroll_start = if list_height > 0 && clamped_sel >= list_height {
        clamped_sel + 1 - list_height
    } else {
        0
    };
    for row in 0..list_height {
        let si = scroll_start + row;
        if si >= suggestions.len() { break; }
        let path = &suggestions[si];
        // Show a trailing '/' hint for directories
        let is_dir = std::path::Path::new(path.as_str()).is_dir();
        let display = if is_dir {
            format!("{}/", smart_truncate(path, item_width.saturating_sub(3), "…"))
        } else {
            smart_truncate(path, item_width.saturating_sub(2), "…")
        };
        let style = if si == clamped_sel { selected_style } else { base_style };
        frame.render_widget(
            Paragraph::new(format!(" {}", display)).style(style),
            Rect::new(area.x + 2, area.y + header_rows + row as u16, item_width as u16, 1),
        );
    }

    // ── Row height-6: separator before preview ────────────────────────────
    let sep2_y = area.y + area.height.saturating_sub(6);
    frame.render_widget(
        Paragraph::new("─".repeat(item_width)).style(sep_style),
        Rect::new(area.x + 2, sep2_y, item_width as u16, 1),
    );

    // ── Rows height-5..height-2: full-path preview (4 lines, char-chunked) ─
    let preview_y = area.y + area.height.saturating_sub(5);
    let preview_w = area.width.saturating_sub(2);
    let preview_lines = if !suggestions.is_empty() && clamped_sel < suggestions.len() {
        let raw = &suggestions[clamped_sel];
        let text = if raw.len() > 1024 { &raw[..1024] } else { raw.as_str() };
        chunk_path_preview(text, preview_w, 4)
    } else { vec![] };
    frame.render_widget(
        Paragraph::new(preview_lines).style(preview_style),
        Rect::new(area.x + 1, preview_y, preview_w, 4),
    );

    // ── Row height-1: hint ────────────────────────────────────────────────
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("↑↓:select  Enter:open  Esc:cancel  Bksp:del  ^K:clear").style(hint_style),
        Rect::new(area.x + 2, hint_y, item_width as u16, 1),
    );

    // ── Cursor rendering on input row ─────────────────────────────────────
    let cursor_x_in_visible = cursor_pos.saturating_sub(scroll);
    let cursor_screen_x = (area.x + 1 + cursor_x_in_visible as u16).min(area.x + input_width);
    frame.set_cursor_position((cursor_screen_x, area.y));
}

fn fmt_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    if bytes >= GB {
        format!("{:.2} GB ({} bytes)", bytes as f64 / GB as f64, bytes)
    } else if bytes >= MB {
        format!("{:.2} MB ({} bytes)", bytes as f64 / MB as f64, bytes)
    } else if bytes >= KB {
        format!("{:.1} KB ({} bytes)", bytes as f64 / KB as f64, bytes)
    } else {
        format!("{} bytes", bytes)
    }
}

fn fmt_time(t: Option<std::time::SystemTime>) -> String {
    match t {
        None => "N/A".to_string(),
        Some(st) => {
            let dt: chrono::DateTime<chrono::Local> = st.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
    }
}

#[allow(unused_variables, unused_mut)]
fn render_file_info_dialog(
    frame: &mut Frame,
    area: Rect,
    file_name: &str,
    file_path: &str,
    size: u64,
    created: Option<std::time::SystemTime>,
    modified: std::time::SystemTime,
    accessed: Option<std::time::SystemTime>,
    is_dir: bool,
    is_readonly: bool,
    #[cfg(unix)] permissions: Option<u32>,
    #[cfg(unix)] owner: Option<&str>,
    #[cfg(unix)] group: Option<&str>,
) {
    let base  = Style::default().fg(Color::Black).bg(Color::Gray);
    let label = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let hint  = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let w = area.width.saturating_sub(4) as usize;

    let rows: &[(&str, String)] = &[
        ("Name",     smart_truncate(file_name, w.saturating_sub(8), "…")),
        ("Path",     smart_truncate(file_path, w.saturating_sub(8), "…")),
        ("Size",     fmt_size(size)),
        ("Type",     {
            let t = if is_dir { "Directory" } else { "File" };
            if is_readonly { format!("{} (Read-only)", t) } else { t.to_string() }
        }),
        ("",         String::new()),
        ("Created",  fmt_time(created)),
        ("Modified", fmt_time(Some(modified))),
        ("Accessed", fmt_time(accessed)),
    ];

    let col_w = 9u16; // label column width ("Modified" = 8 chars + space)
    for (row_i, (lbl, val)) in rows.iter().enumerate() {
        let y = area.y + row_i as u16;
        if y + 1 >= area.y + area.height { break; }
        if lbl.is_empty() { continue; }
        frame.render_widget(
            Paragraph::new(format!("{:<col_w$}", lbl, col_w = col_w as usize)).style(label),
            Rect::new(area.x + 2, y, col_w, 1),
        );
        frame.render_widget(
            Paragraph::new(val.as_str()).style(base),
            Rect::new(area.x + 2 + col_w, y, w.saturating_sub(col_w as usize) as u16, 1),
        );
    }

    // Hint line
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("Enter/Esc: close").style(hint),
        Rect::new(area.x + 2, hint_y, w as u16, 1),
    );
}

fn render_pattern_rename_dialog(
    frame: &mut Frame,
    area: Rect,
    find: &str,
    find_cursor_pos: usize,
    find_scroll_pos: usize,
    replace: &str,
    replace_cursor_pos: usize,
    replace_scroll_pos: usize,
    use_regex: bool,
    case_sensitive: bool,
    preview: &[(String, String)],
    focused_field: usize,
    preview_scroll: usize,
    preview_horizontal_scroll: usize,
    error_message: Option<&str>,
    preview_mode: u8,
    show_all: bool,
) {
    let base   = Style::default().fg(Color::Black).bg(Color::Gray);
    let hint   = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let active = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let w = area.width as usize;

    // Helper: render one labeled textbox row
    let render_textbox = |frame: &mut Frame, y: u16, label: &str, text: &str, cursor: usize, scroll: usize, focused: bool| {
        let label_len = label.len() as u16;
        let tw = area.width.saturating_sub(label_len + 2) as usize;
        frame.render_widget(Paragraph::new(label.to_string()).style(base), Rect::new(area.x, y, label_len, 1));
        let visible: String = text.chars().skip(scroll).take(tw).collect();
        let input_style = if focused {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::Black).bg(Color::White)
        };
        frame.render_widget(Paragraph::new(visible).style(input_style), Rect::new(area.x + label_len, y, tw as u16, 1));
        if focused {
            let cx = area.x + label_len + cursor.saturating_sub(scroll) as u16;
            frame.set_cursor_position((cx.min(area.x + area.width.saturating_sub(1)), y));
        }
    };

    // Row 0: Find field
    render_textbox(frame, area.y, "Find:    ", find, find_cursor_pos, find_scroll_pos, focused_field == 0);

    // Row 1: Replace field
    render_textbox(frame, area.y + 1, "Replace: ", replace, replace_cursor_pos, replace_scroll_pos, focused_field == 1);

    // Row 2: regex/case flags + expert syntax hint
    let regex_mark = if use_regex { "[●]" } else { "[○]" };
    let case_mark  = if case_sensitive { "[●]" } else { "[○]" };
    let flags_line = format!("{} Regex (Alt+R) {} Case (Alt+S) | s/find/repl/[gi] tr/from/to/", regex_mark, case_mark);
    frame.render_widget(
        Paragraph::new(smart_truncate(&flags_line, w.saturating_sub(1), "…")).style(hint),
        Rect::new(area.x, area.y + 2, area.width, 1),
    );

    // Row 3: preview-mode selector + filter selector
    {
        let modes = ["SIDE-BY-SIDE", "Preview", "Original"];
        let mut spans: Vec<Span> = Vec::new();
        for (i, &name) in modes.iter().enumerate() {
            if i > 0 { spans.push(Span::styled("  ", hint)); }
            if preview_mode == i as u8 {
                spans.push(Span::styled(format!("[{}]", name), active));
            } else {
                spans.push(Span::styled(name.to_string(), hint));
            }
        }
        spans.push(Span::styled("  (Alt+P)", hint));
        spans.push(Span::styled("   Filter: ", hint));
        if !show_all {
            spans.push(Span::styled("[MATCHES]", active));
            spans.push(Span::styled("  ", hint));
            spans.push(Span::styled("All", hint));
        } else {
            spans.push(Span::styled("MATCHES", hint));
            spans.push(Span::styled("  ", hint));
            spans.push(Span::styled("[All]", active));
        }
        spans.push(Span::styled("  (Alt+A)", hint));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(hint),
            Rect::new(area.x, area.y + 3, area.width, 1),
        );
    }

    // Row 4: horizontal separator — highlighted when filelist (focused_field==2) has focus
    let (sep_style, sep_line) = if focused_field == 2 {
        let dashes: String = std::iter::repeat('─').take(w.saturating_sub(9)).collect();
        (
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD),
            format!("▶ LIST {}", dashes),
        )
    } else {
        (hint, std::iter::repeat('─').take(w).collect())
    };
    frame.render_widget(
        Paragraph::new(sep_line).style(sep_style),
        Rect::new(area.x, area.y + 4, area.width, 1),
    );

    // Rows 5..area.height-1: filtered preview list
    let preview_area_h = area.height.saturating_sub(6); // find+replace+flags+mode-row+separator+status
    let filtered: Vec<&(String, String)> = if show_all {
        preview.iter().collect()
    } else {
        preview.iter().filter(|(a, b)| a != b).collect()
    };
    let max_scroll = filtered.len().saturating_sub(preview_area_h as usize);
    let effective_scroll = preview_scroll.min(max_scroll);
    let col_w = w.saturating_sub(5) / 2; // 2=indicator+space, 3=" ║ "

    for (i, (original, renamed)) in filtered.iter().skip(effective_scroll).take(preview_area_h as usize).enumerate() {
        let y = area.y + 5 + i as u16;
        if y >= area.y + area.height.saturating_sub(1) { break; }
        let changed = original != renamed;
        let indicator = if changed { "√" } else { "╴" };
        // Horizontal scroll applied to content only; indicator and ║ stay fixed
        let line = match preview_mode {
            0 => {
                // Side-by-side: scroll both columns independently, separator stays put
                let orig: String = original.chars().skip(preview_horizontal_scroll).take(col_w).collect();
                let new_name: String = renamed.chars().skip(preview_horizontal_scroll).take(col_w).collect();
                format!("{} {:<col_w$} ║ {}", indicator, orig, new_name, col_w = col_w)
            }
            1 => {
                let content_w = area.width.saturating_sub(2) as usize;
                let scrolled: String = renamed.chars().skip(preview_horizontal_scroll).take(content_w).collect();
                format!("{} {}", indicator, scrolled)
            }
            _ => {
                let content_w = area.width.saturating_sub(2) as usize;
                let scrolled: String = original.chars().skip(preview_horizontal_scroll).take(content_w).collect();
                format!("{} {}", indicator, scrolled)
            }
        };
        let row_style = if changed {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Gray).bg(Color::DarkGray)
        };
        frame.render_widget(
            Paragraph::new(line).style(row_style),
            Rect::new(area.x, y, area.width, 1),
        );
    }

    // Last row: error or status
    let status_y = area.y + area.height.saturating_sub(1);
    if let Some(err) = error_message {
        frame.render_widget(
            Paragraph::new(smart_truncate(err, w.saturating_sub(1), "…"))
                .style(Style::default().fg(Color::Red).bg(Color::DarkGray)),
            Rect::new(area.x, status_y, area.width, 1),
        );
    } else {
        let match_count = preview.iter().filter(|(a, b)| a != b).count();
        let status = format!(" Matches: {}  Alt+R/S: regex/case  Enter: OK  Esc: cancel", match_count);
        frame.render_widget(
            Paragraph::new(smart_truncate(&status, w.saturating_sub(1), "…")).style(hint),
            Rect::new(area.x, status_y, area.width, 1),
        );
    }
}

fn help_filter_entries<'a>(
    entries: &'a [rwf_lib::model::dialog::HelpEntry],
    active_tab: &rwf_lib::model::dialog::HelpTab,
    show_unbound: bool,
    query: &str,
    regex_mode: bool,
) -> Vec<&'a rwf_lib::model::dialog::HelpEntry> {
    let tab_filtered: Vec<&rwf_lib::model::dialog::HelpEntry> = entries.iter()
        .filter(|e| e.tab == *active_tab)
        .filter(|e| show_unbound || !e.keys.is_empty())
        .collect();

    if query.is_empty() {
        return tab_filtered;
    }

    if regex_mode {
        if let Ok(re) = regex::Regex::new(&format!("(?i){}", query)) {
            tab_filtered.into_iter().filter(|e| {
                let haystack = format!("{} {} {}", e.category, e.description, e.keys.join(" "));
                re.is_match(&haystack)
            }).collect()
        } else {
            tab_filtered
        }
    } else {
        // AND search: each space-separated token must appear in the row text
        let tokens: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
        tab_filtered.into_iter().filter(|e| {
            let haystack = format!("{} {} {}", e.category, e.description, e.keys.join(" ")).to_lowercase();
            tokens.iter().all(|tok| haystack.contains(tok.as_str()))
        }).collect()
    }
}

fn render_help_dialog(
    frame: &mut Frame,
    area: Rect,
    _dialog_area: Rect,
    entries: &[rwf_lib::model::dialog::HelpEntry],
    query: &str,
    regex_mode: bool,
    show_unbound: bool,
    active_tab: &rwf_lib::model::dialog::HelpTab,
    scroll_pos: usize,
    language: &str,
) {
    use unicode_width::UnicodeWidthStr;
    use rwf_lib::model::dialog::HelpTab;

    let base  = Style::default().fg(Color::Black).bg(Color::Gray);
    let tab_active  = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
    let tab_inactive = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let search_style = Style::default().fg(Color::White).bg(Color::DarkGray);
    let unbound_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);

    let w = area.width.saturating_sub(2) as usize; // 1-char margin each side

    // ── Row 0: Tab bar ──────────────────────────────────────────────────────
    if area.height >= 1 {
        let tabs = [
            (HelpTab::NormalMode,      "^1:Normal"),
            (HelpTab::ViewerMode,      "^2:Viewer"),
            (HelpTab::LeapMode,        "^3:Leap"),
            (HelpTab::DialogMode,      "^4:Dialog"),
            (HelpTab::CustomFunctions, "^5:Custom"),
        ];
        let mut spans: Vec<Span> = Vec::new();
        for (i, (tab, label)) in tabs.iter().enumerate() {
            if i > 0 { spans.push(Span::styled("  ", base)); }
            if tab == active_tab {
                spans.push(Span::styled(format!("[{}]", label), tab_active));
            } else {
                spans.push(Span::styled(label.to_string(), tab_inactive));
            }
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(base),
            Rect::new(area.x + 1, area.y, w as u16, 1),
        );
    }

    // ── Row 1: Search field ─────────────────────────────────────────────────
    if area.height >= 2 {
        let search_text = if regex_mode {
            format!("[regex] {}", query)
        } else {
            format!("/{}", query)
        };
        frame.render_widget(
            Paragraph::new(smart_truncate(&search_text, w, "…")).style(search_style),
            Rect::new(area.x + 1, area.y + 1, w as u16, 1),
        );
    }

    if area.height < 3 {
        return;
    }

    // ── Filter and compute column widths ────────────────────────────────────
    let filtered = help_filter_entries(entries, active_tab, show_unbound, query, regex_mode);
    let count = filtered.len();

    // Compute column widths from visible entries (unicode display width)
    let min_cat_w: usize = 10;
    let min_desc_w: usize = 20;
    let min_keys_w: usize = 8;

    let (max_cat_w, _max_desc_w, max_keys_w) = filtered.iter().fold(
        (min_cat_w, min_desc_w, min_keys_w),
        |(mc, md, mk), e| {
            let kw = if e.keys.is_empty() { "(unbound)".len() } else { e.keys.join(", ").len() };
            (mc.max(UnicodeWidthStr::width(e.category.as_str())),
             md.max(UnicodeWidthStr::width(e.description.as_str())),
             mk.max(kw))
        },
    );

    // Distribute space: 2 chars separator between columns
    // Total = cat_w + 2 + desc_w + 2 + keys_w; cap at w
    let avail = w.saturating_sub(4); // 2 separators of 2 chars each
    // Keys column is smallest — let description flex, cap category
    let cat_w  = max_cat_w.min(avail / 4).max(min_cat_w);
    let keys_w = max_keys_w.min(avail / 4).max(min_keys_w);
    let desc_w = avail.saturating_sub(cat_w).saturating_sub(keys_w).max(min_desc_w);

    // ── Rows 2..height-2: entry list ─────────────────────────────────────────
    let list_start_y = area.y + 2;
    let list_height = area.height.saturating_sub(3) as usize; // -tab -search -hint

    // Clamp scroll so the last entry is always at the bottom (no trailing blank rows)
    let effective_scroll = if filtered.len() > list_height {
        scroll_pos.min(filtered.len() - list_height)
    } else {
        0
    };

    for (row, entry) in filtered.iter().skip(effective_scroll).take(list_height).enumerate() {
        let y = list_start_y + row as u16;
        if y >= area.y + area.height.saturating_sub(1) { break; }

        let keys_str = if entry.keys.is_empty() { "(unbound)".to_string() } else { entry.keys.join(", ") };
        let is_unbound = entry.keys.is_empty();

        // Truncate each column to its width
        let cat_s  = smart_truncate(&entry.category,    cat_w,  "…");
        let desc_s = smart_truncate(&entry.description, desc_w, "…");
        let keys_s = smart_truncate(&keys_str,          keys_w, "…");

        let row_style = if is_unbound { unbound_style } else { base };

        let line = Line::from(vec![
            Span::styled(format!("{:<cat_w$}", cat_s, cat_w = cat_w), row_style),
            Span::styled("  ", row_style),
            Span::styled(format!("{:<desc_w$}", desc_s, desc_w = desc_w), row_style),
            Span::styled("  ", row_style),
            Span::styled(keys_s, row_style),
        ]);
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(area.x + 1, y, w as u16, 1),
        );
    }

    // ── Last row: hint line ──────────────────────────────────────────────────
    let hint_y = area.y + area.height.saturating_sub(1);
    let unbound_indicator = if show_unbound { "u:hide unbound" } else { "u:show unbound" };
    let hint_text = format!("({})  {}  L:lang({})  Ctrl+R:regex", count, unbound_indicator, language);
    frame.render_widget(
        Paragraph::new(smart_truncate(&hint_text, w, "…")).style(hint_style),
        Rect::new(area.x + 1, hint_y, w as u16, 1),
    );
}

fn render_sort_dialog(
    frame: &mut Frame,
    area: Rect,
    selected_mode_index: usize,
    selected_order_index: usize,
    focused_section: usize,
) {
    use ratatui::layout::Alignment;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // "Sort by:" label (1) + 4 items
            Constraint::Length(1), // spacer
            Constraint::Length(3), // "Order:" label (1) + 2 items
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
        ])
        .split(area);

    let sort_keys = ["Name", "Size", "Date", "Extension"];
    let orders    = ["Ascending", "Descending"];

    // Spec colors: focused item = Black/White, unfocused = Black/Gray
    let base_style    = Style::default().fg(Color::Black).bg(Color::Gray);
    let focused_item  = Style::default().fg(Color::Black).bg(Color::White);  // spec: White bg
    let label_style   = Style::default().fg(Color::Black).bg(Color::Gray);
    let item_width    = chunks[0].width.saturating_sub(4); // 2-char margin each side

    // --- Sort key section ---
    // Label on line 0 (no Block — avoids overlapping items)
    frame.render_widget(
        Paragraph::new("Sort by:").style(label_style),
        Rect::new(chunks[0].x + 2, chunks[0].y, item_width, 1),
    );
    // Items on lines 1-4
    for (i, label) in sort_keys.iter().enumerate() {
        let is_selected  = i == selected_mode_index;
        let is_cursor    = focused_section == 0 && i == selected_mode_index;
        let marker = if is_selected { "● " } else { "○ " };
        let text   = format!("{}{}", marker, label);
        // Full-width paragraph so highlight covers entire row uniformly
        let row_style = if is_cursor { focused_item } else { base_style };
        let para = Paragraph::new(text).style(row_style);
        frame.render_widget(
            para,
            Rect::new(chunks[0].x + 2, chunks[0].y + 1 + i as u16, item_width, 1),
        );
    }

    // --- Order section ---
    frame.render_widget(
        Paragraph::new("Order:").style(label_style),
        Rect::new(chunks[2].x + 2, chunks[2].y, item_width, 1),
    );
    for (i, label) in orders.iter().enumerate() {
        let is_selected = i == selected_order_index;
        let is_cursor   = focused_section == 1 && i == selected_order_index;
        let marker = if is_selected { "● " } else { "○ " };
        let text   = format!("{}{}", marker, label);
        // Same item_width → identical highlight width for "Ascending" and "Descending"
        let row_style = if is_cursor { focused_item } else { base_style };
        let para = Paragraph::new(text).style(row_style);
        frame.render_widget(
            para,
            Rect::new(chunks[2].x + 2, chunks[2].y + 1 + i as u16, item_width, 1),
        );
    }

    // --- Buttons [*OK*] [Cancel] ---
    // Base row is Gray; only the button text spans receive focus color (not padding)
    let ok_style     = if focused_section == 2 { focused_item } else { base_style };
    let cancel_style = if focused_section == 3 { focused_item } else { base_style };

    let btn_line = Line::from(vec![
        Span::styled("[*OK*]", ok_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(btn_line)
            .alignment(Alignment::Center)
            .style(base_style),  // Gray bg for the whole row
        chunks[4],
    );
}

fn render_dialog_content(frame: &mut Frame, content: &DialogContent, area: Rect, focused: bool) {
    match content {
        DialogContent::Compression {
            archive_name,
            selected_format_index,
            selected_compression_index,
            focused_field,
            format_focus_index,
            compression_focus_index,
            cursor_pos,
            scroll_pos,
            edit_mode,
            vi_mode,
            ..
        } => {
            // Create state from embedded dialog state
            let state = CompressionDialogState {
                archive_name: archive_name.clone(),
                selected_format_index: *selected_format_index,
                selected_compression_index: *selected_compression_index,
                focused_field: *focused_field,
                format_focus_index: *format_focus_index,
                compression_focus_index: *compression_focus_index,
                cursor_pos: *cursor_pos,
                scroll_pos: *scroll_pos,
                edit_mode: *edit_mode,
                vi_mode: *vi_mode,
            };
            render_compression_dialog(frame, area, &state, focused);
        }

        DialogContent::ExtractionConfirm { archive, dest, file_count } => {
            let dialog = ExtractionConfirmDialog {
                archive_name: archive.display_path(),
                dest_path: dest.display_path(),
                file_count: *file_count,
            };
            dialog.render(frame, area, focused);
        }
        DialogContent::DeleteConfirm { .. } => {
            // Rendered by the dedicated arm in render_dialog — not reached via render_dialog_content.
        }
        DialogContent::Error { message, details, .. } => {
            use ratatui::text::{Line, Span};
            use ratatui::widgets::{Paragraph, Wrap};
            let mut lines: Vec<Line> = message
                .lines()
                .map(|l| Line::from(Span::raw(l.to_string())))
                .collect();
            if let Some(d) = details {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled(
                    d.as_str(),
                    Style::default().add_modifier(ratatui::style::Modifier::DIM),
                )));
            }
            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap { trim: false }),
                area,
            );
        }
        _ => {}
    }
}

/// Validate filename for cross-platform compatibility
/// Returns error message if invalid, None if valid
fn validate_filename(rename_text: &str, original_name: &str) -> Option<String> {
    // Check if empty
    if rename_text.is_empty() {
        return Some("Filename cannot be empty".to_string());
    }
    
    // Check if same as original
    if rename_text == original_name {
        return Some("Same filename - no change needed".to_string());
    }
    
    // Windows invalid characters: < > : " / \ | ? *
    let win_invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if rename_text.chars().any(|c| win_invalid.contains(&c)) {
        return Some("Invalid chars for Windows: < > : \" / \\ | ? *".to_string());
    }
    
    // Mac invalid character: :
    let mac_invalid = [':'];
    if rename_text.chars().any(|c| mac_invalid.contains(&c)) {
        return Some("Invalid char for Mac: :".to_string());
    }
    
    // Linux invalid character: /
    let linux_invalid = ['/'];
    if rename_text.chars().any(|c| linux_invalid.contains(&c)) {
        return Some("Invalid char for Linux: /".to_string());
    }
    
    // Check reserved names (Windows)
    let reserved_names = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
    let name_without_ext = std::path::Path::new(rename_text)
        .file_stem()
        .map(|s| s.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    if reserved_names.contains(&name_without_ext.as_str()) {
        return Some("Reserved system name".to_string());
    }
    
    None  // Valid
}

/// Handle File Conflict dialog input with TextInput widget
/// Focus fields: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename button, 4=Textbox, 5=Cancel
fn handle_file_conflict_input(
    conflicts: &mut Vec<rwf_lib::model::dialog::ConflictPair>,
    current_index: &mut usize,
    focused_button: &mut usize,
    rename_text: &mut String,
    rename_cursor: &mut usize,
    rename_scroll: &mut usize,
    edit_mode: &mut rwf_lib::config::EditMode,
    vi_mode: &mut Option<rwf_lib::config::ViMode>,
    error_message: &mut Option<String>,
    decisions: &mut Vec<rwf_lib::model::dialog::ConflictAction>,
    pending_find_backward: &mut Option<bool>,
    pending_operator: &mut Option<u8>,
    pending_ctrl_x: &mut bool,
    history: &mut Vec<String>,
    history_index: &mut usize,
    key: KeyEvent,
) -> DialogAction {
    use crossterm::event::KeyCode;
    use crate::ui::text_input::{TextInput, TextInputAction};
    use rwf_lib::config::ViMode;

    let is_textbox_focused = *focused_button == 3;

    // Clear error on focus change
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        *error_message = None;
    }

    // If textbox is focused, delegate to TextInput
    if is_textbox_focused {
        let mut text_input = TextInput::new(Some(rename_text.clone()), *edit_mode);
        // Set original text for Vi U command
        text_input.set_original_text(rename_text.clone());
        // Restore Vi mode state
        if let Some(vm) = vi_mode {
            text_input.set_vi_mode(*vm);
        }
        // Restore pending states (convert u8 to ViOperator)
        text_input.set_pending_find_backward(*pending_find_backward);
        text_input.set_pending_operator(match pending_operator {
            Some(1) => Some(crate::ui::text_input::ViOperator::Change),
            Some(2) => Some(crate::ui::text_input::ViOperator::Delete),
            _ => None,
        });
        text_input.set_pending_ctrl_x(*pending_ctrl_x);
        text_input.set_history(history.clone());
        text_input.set_history_index(*history_index);
        // set_cursor/set_scroll AFTER set_history_index to prevent cursor reset to end
        text_input.set_cursor(*rename_cursor);
        text_input.set_scroll(*rename_scroll);

        let action = text_input.handle_input(&key);

        // Always sync state from TextInput (DRY - widget owns all state)
        *rename_text = text_input.text().to_string();
        *rename_cursor = text_input.cursor();
        *rename_scroll = text_input.scroll();
        *edit_mode = text_input.mode();
        // Sync Vi mode state
        *vi_mode = text_input.vi_mode();
        // Sync pending states (convert ViOperator to u8)
        *pending_find_backward = text_input.pending_find_backward();
        *pending_operator = match text_input.pending_operator() {
            Some(crate::ui::text_input::ViOperator::Change) => Some(1),
            Some(crate::ui::text_input::ViOperator::Delete) => Some(2),
            None => None,
        };
        *pending_ctrl_x = text_input.pending_ctrl_x();
        *history = text_input.history().clone();
        *history_index = text_input.history_index();

        match action {
            TextInputAction::TextChanged => {
                // Validate immediately on text change
                let original_name = if !conflicts.is_empty() {
                    &conflicts[*current_index].source.name
                } else {
                    ""
                };
                *error_message = validate_filename(rename_text, original_name);
                return DialogAction::None;
            }
            TextInputAction::CursorMoved => {
                // Cursor moved, clear error if it was a validation error
                // (user might be navigating to fix the issue)
                return DialogAction::None;
            }
            TextInputAction::ModeToggled | TextInputAction::ModeChanged => {
                // Mode changed, just continue
                debug!("TextInput mode changed in FileConflict dialog (textbox focused), edit_mode={:?}, vi_mode={:?}", edit_mode, vi_mode);
                return DialogAction::None;
            }
            TextInputAction::Confirm => {
                // Validate and confirm
                let original_name = if !conflicts.is_empty() {
                    &conflicts[*current_index].source.name
                } else {
                    ""
                };
                match validate_filename(rename_text, original_name) {
                    Some(err) => {
                        *error_message = Some(err);
                        return DialogAction::None;
                    }
                    None => {
                        *error_message = None;
                        decisions.push(rwf_lib::model::dialog::ConflictAction::Rename {
                            new_name: rename_text.clone()
                        });
                        return DialogAction::Confirm;
                    }
                }
            }
            TextInputAction::Cancel => {
                // In Vi mode, behavior depends on current vi_mode state
                debug!("TextInput Cancel action in FileConflict dialog (textbox focused), edit_mode={:?}, vi_mode={:?}", edit_mode, vi_mode);
                if *edit_mode == rwf_lib::config::EditMode::Vi {
                    match vi_mode {
                        Some(ViMode::Normal) => {
                            // Already in Normal mode - cancel the dialog
                            debug!("Vi-Normal mode, canceling dialog");
                            return DialogAction::Cancel;
                        }
                        Some(ViMode::Insert) | None => {
                            // In Insert mode - switch to Normal mode
                            *vi_mode = Some(ViMode::Normal);
                            debug!("Switching to Normal mode from textbox Cancel");
                            return DialogAction::None;
                        }
                    }
                }
                debug!("Emacs mode, returning Cancel action");
                return DialogAction::Cancel;
            }
            TextInputAction::NextField => {
                *focused_button = (*focused_button + 1) % 5;
                return DialogAction::None;
            }
            TextInputAction::PrevField => {
                *focused_button = if *focused_button == 0 { 4 } else { *focused_button - 1 };
                return DialogAction::None;
            }
            TextInputAction::None => return DialogAction::None,
        }
    }

    // Handle non-textbox focus
    match key.code {
        KeyCode::Esc => {
            // In Vi mode, behavior depends on current vi_mode state
            debug!("Esc pressed in FileConflict dialog (non-textbox), edit_mode={:?}, current vi_mode={:?}", edit_mode, vi_mode);
            if *edit_mode == rwf_lib::config::EditMode::Vi {
                match vi_mode {
                    Some(ViMode::Normal) => {
                        // Already in Normal mode - Esc cancels the dialog
                        debug!("Vi-Normal mode, returning Cancel action");
                        DialogAction::Cancel
                    }
                    Some(ViMode::Insert) | None => {
                        // In Insert mode - switch to Normal mode
                        debug!("Switching to Normal mode (Vi Insert mode active)");
                        *vi_mode = Some(ViMode::Normal);
                        DialogAction::None
                    }
                }
            } else {
                debug!("Emacs mode active, returning Cancel action");
                DialogAction::Cancel
            }
        }
        KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+[: In Vi mode, switches to Normal mode
            if *edit_mode == rwf_lib::config::EditMode::Vi {
                *vi_mode = Some(ViMode::Normal);
                DialogAction::None
            } else {
                DialogAction::Cancel
            }
            }

            KeyCode::Tab => {
            *focused_button = (*focused_button + 1) % 5;  // 5 fields now
            DialogAction::None
            }

            KeyCode::BackTab => {
            *focused_button = if *focused_button == 0 { 4 } else { *focused_button - 1 }; 
            DialogAction::None
            }

            KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Enter: Apply to ALL remaining
                let action = button_index_to_action(*focused_button);
                for _ in 0..(conflicts.len() - *current_index) {
                    decisions.push(action.clone());
                }
                DialogAction::ConfirmAll
            } else {
                // Enter: Apply to current only
                match *focused_button {
                    0 | 1 | 2 | 4 => {
                        // Force, OverwriteIfNew, Skip, Cancel buttons
                        let action = button_index_to_action(*focused_button);
                        decisions.push(action);
                        DialogAction::Confirm
                    }
                    3 => {
                        // Textbox: handled above in the is_textbox_focused block
                        DialogAction::None
                    }
                    _ => DialogAction::None,
                }
            }
            }
        _ => DialogAction::None,
    }
}

/// Convert button index to ConflictAction
fn button_index_to_action(index: usize) -> rwf_lib::model::dialog::ConflictAction {
    match index {
        0 => rwf_lib::model::dialog::ConflictAction::Force,
        1 => rwf_lib::model::dialog::ConflictAction::OverwriteIfNewer,
        2 => rwf_lib::model::dialog::ConflictAction::Skip,
        3 => rwf_lib::model::dialog::ConflictAction::Rename { new_name: String::new() },  // Placeholder, name synced from textbox
        4 => rwf_lib::model::dialog::ConflictAction::Skip,  // Cancel = skip this file
        _ => rwf_lib::model::dialog::ConflictAction::Skip,
    }
}


/// Handle dialog input centrally
pub fn handle_dialog_input(dialog: &mut Dialog, key: KeyEvent, search: Option<&rwf_lib::model::SearchModel>) -> DialogAction {
    // Note: Esc handling is delegated to individual dialog handlers
    // - FileConflict: Esc cancels (Emacs) or switches to Normal mode (Vi)
    // - Other dialogs: Esc cancels

    // Enter = Confirm (but depends on focused field for JobManager / SortDialog)
    if key.code == crossterm::event::KeyCode::Enter {
        // SortDialog: Enter confirms only when OK (2) or Cancel (3) section is focused
        if let DialogContent::SortDialog { focused_section, .. } = &dialog.content {
            match *focused_section {
                2 => return DialogAction::Confirm,  // OK button
                3 => return DialogAction::Cancel,   // Cancel button
                _ => return DialogAction::None,     // List section — Enter does nothing
            }
        }
        // For JobManager dialog, check which field has focus
        if let DialogContent::JobManager { focused_field, .. } = &dialog.content {
            match *focused_field {
                1 => return DialogAction::Confirm,  // Close button focused
                2 => return DialogAction::Confirm,  // Cancel Job button focused
                _ => {}                              // Job List focused, Enter does nothing
            }
        } else if let DialogContent::FileConflict { .. } = &dialog.content {
            // FileConflict dialog handles Enter internally (for buttons and textbox)
            // Don't return here, let it be handled below
        } else {
            return DialogAction::Confirm;
        }
    }

    // CloseTabWithActiveJob dialog - Enter confirms, Escape cancels, Tab cycles
    if let DialogContent::CloseTabWithActiveJob { focused_field, .. } = &mut dialog.content {
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
    if let DialogContent::FileMask { input, cursor_pos, scroll_pos, focused_field } = &mut dialog.content {
        use crossterm::event::KeyCode;
        use crate::ui::text_input::{TextInput, TextInputAction};
        // Tab cycles: 0 (textbox) → 1 (OK) → 2 (Cancel) → 0
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 { 2 } else { *focused_field - 1 };
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
                TextInputAction::Cancel  => return DialogAction::Cancel,
                _ => return DialogAction::None,
            }
        }
        return DialogAction::None;
    }

    // WildcardMark dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::WildcardMark { input, cursor_pos, scroll_pos, focused_field } = &mut dialog.content {
        use crossterm::event::KeyCode;
        use crate::ui::text_input::{TextInput, TextInputAction};
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 { 2 } else { *focused_field - 1 };
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
                TextInputAction::Cancel  => return DialogAction::Cancel,
                _ => return DialogAction::None,
            }
        }
        return DialogAction::None;
    }

    // SimpleRename dialog — identical Tab/Enter/Esc/TextInput logic as FileMask
    if let DialogContent::SimpleRename { input, cursor_pos, scroll_pos, focused_field } = &mut dialog.content {
        use crossterm::event::KeyCode;
        use crate::ui::text_input::{TextInput, TextInputAction};
        if key.code == KeyCode::Tab {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                *focused_field = if *focused_field == 0 { 2 } else { *focused_field - 1 };
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
                TextInputAction::Cancel  => return DialogAction::Cancel,
                _ => return DialogAction::None,
            }
        }
        return DialogAction::None;
    }

    // PatternRename dialog — Find/Replace textboxes + Alt+R/S flag toggles + preview scroll
    if let DialogContent::PatternRename {
        find, find_cursor_pos, find_scroll_pos,
        replace, replace_cursor_pos, replace_scroll_pos,
        use_regex, case_sensitive,
        focused_field, preview_scroll, preview_horizontal_scroll,
        preview, error_message, preview_mode, show_all,
    } = &mut dialog.content {
        use crossterm::event::KeyCode;
        use crate::ui::text_input::{TextInput, TextInputAction};

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
                *focused_field = if *focused_field == 0 { 2 } else { *focused_field - 1 };
            } else {
                *focused_field = (*focused_field + 1) % 3;
            }
            return DialogAction::None;
        }
        if key.code == KeyCode::BackTab {
            *focused_field = if *focused_field == 0 { 2 } else { *focused_field - 1 };
            return DialogAction::None;
        }

        if key.code == KeyCode::Esc { return DialogAction::Cancel; }
        if key.code == KeyCode::Enter {
            // Detect duplicate target names before executing
            let mut seen = std::collections::HashSet::new();
            let has_collision = preview.iter().any(|(orig, new_name)| {
                orig != new_name && !seen.insert(new_name.clone())
            });
            if has_collision {
                *error_message = Some("Multiple files would be renamed to the same name".to_string());
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
                    KeyCode::Left  => { *preview_horizontal_scroll = 0; }
                    KeyCode::Right => { *preview_horizontal_scroll = 500; } // clamped at render
                    _ => {}
                }
            } else if key.modifiers == KeyModifiers::NONE {
                match key.code {
                    KeyCode::Left  => { *preview_horizontal_scroll = preview_horizontal_scroll.saturating_sub(1); }
                    KeyCode::Right => { *preview_horizontal_scroll = preview_horizontal_scroll.saturating_add(1); }
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
                (replace as &mut String, replace_cursor_pos, replace_scroll_pos)
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
                TextInputAction::Cancel  => return DialogAction::Cancel,
                _ => return if changed { DialogAction::PatternChanged } else { DialogAction::None },
            }
        }
        return DialogAction::None;
    }

    // Help dialog — full input handler
    if let DialogContent::Help {
        entries, query, regex_mode, show_unbound, active_tab, scroll_pos, ..
    } = &mut dialog.content {
        use crossterm::event::KeyCode;
        use rwf_lib::model::dialog::HelpTab;

        // Compute filtered count for scroll clamping
        let filtered_count = help_filter_entries(entries, active_tab, *show_unbound, query, *regex_mode).len();
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
                if *scroll_pos > 0 { *scroll_pos -= 1; }
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                let max_scroll = filtered_count.saturating_sub(list_height_estimate);
                if *scroll_pos < max_scroll { *scroll_pos += 1; }
            }
            KeyCode::PageUp => {
                *scroll_pos = scroll_pos.saturating_sub(list_height_estimate);
            }
            KeyCode::PageDown => {
                let max_scroll = filtered_count.saturating_sub(list_height_estimate);
                *scroll_pos = (*scroll_pos + list_height_estimate).min(max_scroll);
            }
            KeyCode::Home => { *scroll_pos = 0; }
            KeyCode::End  => { *scroll_pos = filtered_count.saturating_sub(list_height_estimate); }

            // u: toggle show_unbound
            KeyCode::Char('u') if key.modifiers == KeyModifiers::NONE => {
                *show_unbound = !*show_unbound;
                *scroll_pos = 0;
            }

            // L: switch language
            KeyCode::Char('L') if key.modifiers == KeyModifiers::NONE
                                || key.modifiers == KeyModifiers::SHIFT =>
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
                && !key.modifiers.contains(KeyModifiers::SUPER)
                =>
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
    if let DialogContent::DriveSelection { drives, selected_index, filter } = &mut dialog.content {
        use crossterm::event::KeyCode;
        let filtered_count = if filter.is_empty() {
            drives.len()
        } else {
            let lower = filter.to_lowercase();
            drives.iter().filter(|d| {
                d.display_label().to_lowercase().contains(&lower)
                    || d.path.to_lowercase().contains(&lower)
            }).count()
        };
        match key.code {
            KeyCode::Esc  => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 { *selected_index -= 1; }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index + 1 < filtered_count { *selected_index += 1; }
            }
            KeyCode::Home => { *selected_index = 0; }
            KeyCode::End  => { *selected_index = filtered_count.saturating_sub(1); }
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
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                              && !key.modifiers.contains(KeyModifiers::ALT)
                              && !key.modifiers.contains(KeyModifiers::SUPER) => {
                filter.push(c);
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // JumpToPath — text input + AND-filter suggestions + arrow navigation
    if let DialogContent::JumpToPath { query, cursor_pos, suggestions, selected_index, candidates, .. } = &mut dialog.content {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc   => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up    if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 { *selected_index -= 1; }
            }
            KeyCode::Down  if key.modifiers == KeyModifiers::NONE => {
                if !suggestions.is_empty() && *selected_index + 1 < suggestions.len() {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => { *selected_index = 0; }
            KeyCode::End  => {
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
                    if *cursor_pos > 0 { *cursor_pos -= 1; }
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
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                              && !key.modifiers.contains(KeyModifiers::ALT)
                              && !key.modifiers.contains(KeyModifiers::SUPER) => {
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
    if let DialogContent::JumpToFile { query, cursor_pos, suggestions, selected_index, candidates, .. } = &mut dialog.content {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc   => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up    if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 { *selected_index -= 1; }
            }
            KeyCode::Down  if key.modifiers == KeyModifiers::NONE => {
                if !suggestions.is_empty() && *selected_index + 1 < suggestions.len() {
                    *selected_index += 1;
                }
            }
            KeyCode::Home => { *selected_index = 0; }
            KeyCode::End  => {
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
                    if *cursor_pos > 0 { *cursor_pos -= 1; }
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
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                              && !key.modifiers.contains(KeyModifiers::ALT)
                              && !key.modifiers.contains(KeyModifiers::SUPER) => {
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
    if let DialogContent::CustomFunctionSelector { functions, selected_index, filter } = &mut dialog.content {
        use crossterm::event::KeyCode;
        let lower = filter.to_lowercase();
        let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
            functions.iter().collect()
        } else {
            functions.iter().filter(|f| {
                f.name.to_lowercase().contains(&lower)
                    || f.description.as_deref().unwrap_or("").to_lowercase().contains(&lower)
            }).collect()
        };
        let filtered_count = filtered.len();
        match key.code {
            KeyCode::Esc   => return DialogAction::Cancel,
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
                if *selected_index > 0 { *selected_index -= 1; }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index + 1 < filtered_count { *selected_index += 1; }
            }
            KeyCode::Home => { *selected_index = 0; }
            KeyCode::End  => { *selected_index = filtered_count.saturating_sub(1); }
            KeyCode::Backspace => {
                if !filter.is_empty() {
                    let mut chars = filter.chars();
                    chars.next_back();
                    *filter = chars.as_str().to_string();
                    *selected_index = 0;
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => { filter.clear(); }
            KeyCode::Char('\x0b') => { filter.clear(); }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                              && !key.modifiers.contains(KeyModifiers::ALT)
                              && !key.modifiers.contains(KeyModifiers::SUPER) => {
                filter.push(c);
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    // ContextMenu — arrow navigation (skip separators)
    if let DialogContent::ContextMenu { options, selected_index } = &mut dialog.content {
        use crossterm::event::KeyCode;
        use rwf_lib::model::dialog::ContextMenuAction;
        let selectable_count = options.iter().filter(|o| !matches!(o.action, ContextMenuAction::Separator)).count();
        let _ = selectable_count;
        match key.code {
            KeyCode::Esc   => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                // Move up, skip separators
                let mut idx = *selected_index;
                loop {
                    if idx == 0 { break; }
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
                    if idx + 1 >= options.len() { break; }
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
    if let DialogContent::CustomFunctionMenu { items, selected_index } = &mut dialog.content {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc   => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                let mut idx = *selected_index;
                loop {
                    if idx == 0 { break; }
                    idx -= 1;
                    if items[idx].is_selectable() { *selected_index = idx; break; }
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                let mut idx = *selected_index;
                loop {
                    if idx + 1 >= items.len() { break; }
                    idx += 1;
                    if items[idx].is_selectable() { *selected_index = idx; break; }
                }
            }
            KeyCode::Home => {
                for (i, item) in items.iter().enumerate() {
                    if item.is_selectable() { *selected_index = i; break; }
                }
            }
            KeyCode::End => {
                for (i, item) in items.iter().enumerate().rev() {
                    if item.is_selectable() { *selected_index = i; break; }
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
    if let DialogContent::RegisteredFolderSelector { folders, selected_index, filter } = &mut dialog.content {
        use crossterm::event::KeyCode;
        let filtered_count = if filter.is_empty() {
            folders.len()
        } else {
            let lower = filter.to_lowercase();
            folders.iter().filter(|f| {
                f.name.to_lowercase().contains(&lower)
                    || f.path.to_lowercase().contains(&lower)
            }).count()
        };
        match key.code {
            KeyCode::Esc  => return DialogAction::Cancel,
            KeyCode::Enter => return DialogAction::Confirm,
            KeyCode::Delete => return DialogAction::DeleteSelected,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index > 0 { *selected_index -= 1; }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                if *selected_index + 1 < filtered_count { *selected_index += 1; }
            }
            KeyCode::Home => { *selected_index = 0; }
            KeyCode::End  => { *selected_index = filtered_count.saturating_sub(1); }
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
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                              && !key.modifiers.contains(KeyModifiers::ALT)
                              && !key.modifiers.contains(KeyModifiers::SUPER) => {
                filter.push(c);
                *selected_index = 0;
            }
            _ => {}
        }
        return DialogAction::None;
    }

    if matches!(&dialog.content, DialogContent::HistoryDialog { .. }) {
        use crossterm::event::KeyCode;
        use rwf_lib::model::ui::ActivePane;

        // ── Pane switch (Tab, Left arrow, Right arrow, h, l) ──────────────
        let switch_to: Option<ActivePane> = match key.code {
            KeyCode::Tab => {
                let cur = if let DialogContent::HistoryDialog { active_pane, .. } = &dialog.content {
                    *active_pane
                } else { unreachable!() };
                Some(match cur {
                    ActivePane::Left  => ActivePane::Right,
                    ActivePane::Right => ActivePane::Left,
                })
            }
            KeyCode::Left  | KeyCode::Char('h') => Some(ActivePane::Left),
            KeyCode::Right | KeyCode::Char('l') => Some(ActivePane::Right),
            _ => None,
        };

        if let Some(new_pane) = switch_to {
            // update content
            if let DialogContent::HistoryDialog { active_pane, .. } = &mut dialog.content {
                *active_pane = new_pane;
            }
            // update title separately (no borrow conflict — different fields)
            let pane_label = match new_pane { ActivePane::Left => "Left", ActivePane::Right => "Right" };
            if let Some(bar) = dialog.title.rfind('|') {
                let prefix = dialog.title[..bar].to_string();
                dialog.title = format!("{}| {}]", prefix, pane_label);
            }
            return DialogAction::None;
        }

        // ── Cursor navigation ──────────────────────────────────────────────
        if let DialogContent::HistoryDialog {
            left_entries, right_entries,
            left_selected, right_selected,
            active_pane, ..
        } = &mut dialog.content {
            let (sel, total) = match active_pane {
                ActivePane::Left  => (left_selected,  left_entries.len()),
                ActivePane::Right => (right_selected, right_entries.len()),
            };
            match key.code {
                KeyCode::Esc            => return DialogAction::Cancel,
                KeyCode::Enter          => return DialogAction::Confirm,
                KeyCode::Up   | KeyCode::Char('k') => { if *sel + 1 < total { *sel += 1; } }
                KeyCode::Down | KeyCode::Char('j') => { if *sel > 0 { *sel -= 1; } }
                KeyCode::Home | KeyCode::Char('g') => { *sel = total.saturating_sub(1); }
                KeyCode::End  | KeyCode::Char('G') => { *sel = 0; }
                _ => {}
            }
        }
        return DialogAction::None;
    }

    // FileConflict dialog - custom input handling with textbox
    if let DialogContent::FileConflict { conflicts, current_index, focused_button, rename_text, rename_cursor, rename_scroll, edit_mode, vi_mode, error_message, decisions, vi_pending_find_backward, vi_pending_operator, vi_pending_ctrl_x, history, history_index, .. } = &mut dialog.content {
        return handle_file_conflict_input(conflicts, current_index, focused_button, rename_text, rename_cursor, rename_scroll, edit_mode, vi_mode, error_message, decisions, vi_pending_find_backward, vi_pending_operator, vi_pending_ctrl_x, history, history_index, key);
    }

    // Compression dialog - Vi mode support for Esc (when textbox not focused)
    if let DialogContent::Compression { edit_mode, vi_mode, focused_field, .. } = &mut dialog.content {
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
    if matches!(&dialog.content, DialogContent::Error { .. }) {
        return DialogAction::Confirm;
    }

    // Tab navigation - cycles through dialog fields
    if key.code == crossterm::event::KeyCode::Tab || key.code == crossterm::event::KeyCode::BackTab {
        let backward = key.code == crossterm::event::KeyCode::BackTab
            || key.modifiers.contains(KeyModifiers::SHIFT);

        // SortDialog: Tab cycles 0→1→2→3→0 (sort-key list→order list→OK→Cancel)
        if let DialogContent::SortDialog { focused_section, .. } = &mut dialog.content {
            if backward {
                *focused_section = if *focused_section == 0 { 3 } else { *focused_section - 1 };
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
                    0 => 2,  // Job List → Cancel
                    1 => 0,  // Close → Job List
                    2 => 1,  // Cancel → Close
                    _ => 0,
                };
            } else {
                *focused_field = match *focused_field {
                    0 => 1,  // Job List → Close
                    1 => 2,  // Close → Cancel
                    2 => 0,  // Cancel → Job List
                    _ => 0,
                };
            }
            return DialogAction::None;
        }

        // Handle Compression dialog Tab navigation
        if let DialogContent::Compression { focused_field, .. } = &mut dialog.content {
            // Cycle: 0→1→2→3→4→0 (format→compression→name→OK→Cancel→format)
            if backward {
                *focused_field = if *focused_field == 0 { 4 } else { *focused_field - 1 };
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

/// Handle content-specific input
fn handle_content_input(content: &mut DialogContent, key: KeyEvent) -> DialogAction {
    match content {
        DialogContent::SortDialog { selected_mode_index, selected_order_index, focused_section } => {
            use crossterm::event::KeyCode;
            match key.code {
                KeyCode::Up => {
                    match *focused_section {
                        0 => { if *selected_mode_index > 0 { *selected_mode_index -= 1; } }
                        1 => { if *selected_order_index > 0 { *selected_order_index -= 1; } }
                        _ => {}
                    }
                    DialogAction::None
                }
                KeyCode::Down => {
                    match *focused_section {
                        0 => { if *selected_mode_index < 3 { *selected_mode_index += 1; } }
                        1 => { if *selected_order_index < 1 { *selected_order_index += 1; } }
                        _ => {}
                    }
                    DialogAction::None
                }
                KeyCode::Left => {
                    if *focused_section > 0 { *focused_section -= 1; }
                    DialogAction::None
                }
                KeyCode::Right => {
                    if *focused_section < 3 { *focused_section += 1; }
                    DialogAction::None
                }
                _ => DialogAction::None,
            }
        }
        DialogContent::JobManager { selected_index, focused_field } => {
            // Job Manager dialog input handling (Part 6.6, 6.7)

            // Up/Down navigation in Job List (only when Job List is focused)
            if *focused_field == 0 {
                match key.code {
                    crossterm::event::KeyCode::Up => {
                        if *selected_index > 0 {
                            *selected_index -= 1;
                        }
                        return DialogAction::None;
                    }
                    crossterm::event::KeyCode::Down => {
                        *selected_index += 1;
                        return DialogAction::None;
                    }
                    // C key: Cancel selected job directly (Part 6.6)
                    crossterm::event::KeyCode::Char('c') | crossterm::event::KeyCode::Char('C') => {
                        // Return Confirm to trigger cancellation (focused_field will be checked by caller)
                        // We temporarily set focused_field to 2 (Cancel Job button) to trigger cancellation
                        *focused_field = 2;
                        return DialogAction::Confirm;
                    }
                    _ => {}
                }
            }
            return DialogAction::None;
        }
        DialogContent::Compression {
            focused_field,
            format_focus_index,
            compression_focus_index,
            selected_format_index,
            selected_compression_index,
            archive_name,
            format,
            cursor_pos,
            scroll_pos,
            edit_mode,
            vi_mode,
            vi_pending_find_backward,
            vi_pending_operator,
            vi_pending_ctrl_x,
            history,
            history_index,
            ..
        } => {
            // If archive name is focused, delegate to TextInput
            if *focused_field == 2 {
                use crate::ui::text_input::{TextInput, TextInputAction};
                let mut text_input = TextInput::new(Some(archive_name.clone()), *edit_mode);
                // Set original text for Vi U command (could be stored but archive_name is usually fine)
                text_input.set_original_text(archive_name.clone());
                // Restore all state
                if let Some(vm) = vi_mode {
                    text_input.set_vi_mode(*vm);
                }
                text_input.set_pending_find_backward(*vi_pending_find_backward);
                text_input.set_pending_operator(match vi_pending_operator {
                    Some(1) => Some(crate::ui::text_input::ViOperator::Change),
                    Some(2) => Some(crate::ui::text_input::ViOperator::Delete),
                    _ => None,
                });
                text_input.set_pending_ctrl_x(*vi_pending_ctrl_x);
                text_input.set_history(history.clone());
                text_input.set_history_index(*history_index);
                // set_cursor/set_scroll AFTER set_history_index to prevent cursor reset to end
                text_input.set_cursor(*cursor_pos);
                text_input.set_scroll(*scroll_pos);

                let action = text_input.handle_input(&key);

                // Sync state back
                *archive_name = text_input.text().to_string();
                *cursor_pos = text_input.cursor();
                *scroll_pos = text_input.scroll();
                *edit_mode = text_input.mode();
                *vi_mode = text_input.vi_mode();
                *vi_pending_find_backward = text_input.pending_find_backward();
                *vi_pending_operator = match text_input.pending_operator() {
                    Some(crate::ui::text_input::ViOperator::Change) => Some(1),
                    Some(crate::ui::text_input::ViOperator::Delete) => Some(2),
                    None => None,
                };
                *vi_pending_ctrl_x = text_input.pending_ctrl_x();
                *history = text_input.history().clone();
                *history_index = text_input.history_index();

                match action {
                    TextInputAction::Cancel => {
                        if *edit_mode == rwf_lib::config::EditMode::Vi {
                            match vi_mode {
                                Some(ViMode::Normal) => return DialogAction::Cancel,
                                Some(ViMode::Insert) | None => {
                                    *vi_mode = Some(ViMode::Normal);
                                    return DialogAction::None;
                                }
                            }
                        } else {
                            return DialogAction::Cancel;
                        }
                    }
                    TextInputAction::Confirm => return DialogAction::Confirm,
                    _ => return DialogAction::None,
                }
            }

            match *focused_field {
                0 => {
                    // Format list has focus - Up/Down moves focus, Space sets selection
                    match key.code {
                        crossterm::event::KeyCode::Up => {
                            if *format_focus_index > 0 {
                                *format_focus_index -= 1;
                            }
                        }
                        crossterm::event::KeyCode::Down => {
                            if *format_focus_index < 7 {
                                *format_focus_index += 1;
                            }
                        }
                        crossterm::event::KeyCode::Char(' ') => {
                            *selected_format_index = *format_focus_index;
                            // Sync the format enum and update archive_name extension
                            let new_fmt = compression::ARCHIVE_FORMATS
                                .get(*format_focus_index)
                                .map(|(_, f)| *f)
                                .unwrap_or(rwf_lib::ArchiveFormat::ZIP);
                            if new_fmt != *format {
                                let new_ext = archive_ext_for_format(new_fmt);
                                let old_ext = archive_ext_for_format(*format);
                                // Handle double extension .tar.gz
                                let base = if old_ext == "tar" && archive_name.to_lowercase().ends_with(".tar.gz") {
                                    &archive_name[..archive_name.len() - ".tar.gz".len()]
                                } else if archive_name.to_lowercase().ends_with(&format!(".{}", old_ext)) {
                                    &archive_name[..archive_name.len() - old_ext.len() - 1]
                                } else {
                                    archive_name.as_str()
                                };
                                *archive_name = format!("{}.{}", base, new_ext);
                                *cursor_pos = archive_name.chars().count();
                                *format = new_fmt;
                            }
                        }
                        _ => {}
                    }
                }
                1 => {
                    // Compression list has focus - Up/Down moves focus, Space sets selection
                    match key.code {
                        crossterm::event::KeyCode::Up => {
                            if *compression_focus_index > 0 {
                                *compression_focus_index -= 1;
                            }
                        }
                        crossterm::event::KeyCode::Down => {
                            if *compression_focus_index < 5 {
                                *compression_focus_index += 1;
                            }
                        }
                        crossterm::event::KeyCode::Char(' ') => {
                            // Set selection to current focus position
                            *selected_compression_index = *compression_focus_index;
                        }
                        _ => {}
                    }
                }
                _ => {} // Buttons and Name handled above
            }
            DialogAction::None
        }

        DialogContent::ExtractionConfirm { .. } => {
            // Simple confirmation - only global shortcuts apply
            DialogAction::None
        }
        DialogContent::DeleteConfirm { targets, scroll_offset } => {
            let total = targets.len();
            // Mirror the render-side height formula exactly (now using centered_rect_abs, no rounding loss)
            // min_content = total.min(12) + 7; min_dialog = min_content + 2 = total.min(12) + 9
            // dialog_h = max(80% of screen, min_dialog), capped at screen - 2
            // layout fixed overhead: borders(2) + spacer(1) + hint(1) + buttons(3) + header(1) + blank(1) = 9
            let screen_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
            let min_content = total.min(12) as u16 + 7;
            let min_dialog = min_content + 2;
            let dialog_h = (screen_h * 80 / 100).max(min_dialog).min(screen_h.saturating_sub(2));
            let list_h = dialog_h.saturating_sub(9).max(1) as usize;
            let visible_rows = total.min(list_h);
            let max_scroll = total.saturating_sub(visible_rows);
            match key.code {
                crossterm::event::KeyCode::Up => {
                    *scroll_offset = scroll_offset.saturating_sub(1);
                }
                crossterm::event::KeyCode::Down => {
                    if *scroll_offset < max_scroll {
                        *scroll_offset += 1;
                    }
                }
                _ => {}
            }
            DialogAction::None
        }
        _ => DialogAction::None,
    }
}

/// Remove the selected entry from a RegisteredFolderSelector dialog.
/// Updates both state.registered_folders and the dialog's own folder list.
/// Returns a log message on success.
pub fn process_dialog_delete(state: &mut rwf_lib::AppState) -> Option<String> {
    // Step 1: resolve actual folder index from the filtered selection (borrow ends at block close).
    let folder_index: Option<usize> = {
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::RegisteredFolderSelector { folders, selected_index, filter } = &dialog.content {
                let lower = filter.to_lowercase();
                let filtered_indices: Vec<usize> = if filter.is_empty() {
                    (0..folders.len()).collect()
                } else {
                    folders.iter().enumerate()
                        .filter(|(_, f)| {
                            f.name.to_lowercase().contains(&lower)
                                || f.path.to_lowercase().contains(&lower)
                        })
                        .map(|(i, _)| i)
                        .collect()
                };
                filtered_indices.get(*selected_index).copied()
            } else { None }
        } else { None }
    };

    let idx = folder_index?;

    // Step 2: remove from authoritative state (different field — no borrow conflict).
    let removed = state.registered_folders.remove(idx);
    let save_path = rwf_lib::model::dialog::RegisteredFolderManager::default_path();
    let _ = state.registered_folders.save_to_file(&save_path);

    // Step 3: mirror removal in the dialog's own snapshot.
    if let Some(dialog) = state.dialogs.current_mut() {
        if let DialogContent::RegisteredFolderSelector { folders, selected_index, .. } = &mut dialog.content {
            if idx < folders.len() {
                folders.remove(idx);
            }
            let count = folders.len();
            if count > 0 && *selected_index >= count {
                *selected_index = count - 1;
            }
        }
    }

    removed.map(|f| format!("[Folder] Removed \"{}\" → {}", f.name, f.path))
}

/// Build a human-readable job name for a delete operation showing file names.
pub fn delete_job_name(targets: &[rwf_lib::Location]) -> String {
    let file_name = |loc: &rwf_lib::Location| -> String {
        loc.path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| loc.display_path())
    };
    match targets.len() {
        0 => "Delete".to_string(),
        1 => format!("Delete '{}'", file_name(&targets[0])),
        2 => format!("Delete '{}', '{}'", file_name(&targets[0]), file_name(&targets[1])),
        n => format!("Delete {} files: '{}', '{}'...", n, file_name(&targets[0]), file_name(&targets[1])),
    }
}

/// Process dialog confirmation and create transitions
/// Returns the job spec if a job was created, so it can be submitted to the worker pool
pub fn process_dialog_confirmation(state: &mut rwf_lib::AppState) -> Option<rwf_lib::job::JobSpec> {
    debug!("process_dialog_confirmation called");

    // Input dialogs: extract title first so the borrow on state.dialogs ends before we
    // access state.dialogs.input_buffer or call update_state.
    let input_dialog_title: Option<String> = state.dialogs.current()
        .filter(|d| matches!(d.content, DialogContent::Input { .. }))
        .map(|d| d.title.clone());

    if let Some(title) = input_dialog_title {
        let input = state.dialogs.input_buffer.clone();
        match title.as_str() {
            "Register Folder" if !input.is_empty() => {
                let path = state.active_pane().current_location.display_path();
                rwf_lib::state::update_state(state, rwf_lib::state::Transition::RegisterCurrentFolder { name: input, path });
            }
            "Create Directory" if !input.is_empty() => {
                let current_location = state.active_pane().current_location.clone();
                let new_dir_loc = current_location.join(&input);
                return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::Mkdir { location: new_dir_loc }));
            }
            "Custom Function Input" => {
                if let Some(func) = state.pending_custom_function_input.take() {
                    let expander = rwf_lib::macro_expander::MacroExpander::new();
                    if let Ok(command) = expander.expand_with_user_input(state, &func, &input) {
                        let working_dir = state.active_pane().current_location.clone();
                        let shell = func.shell.clone();
                        // Pop the CustomFunctionSelector sitting below this Input dialog;
                        // app.rs will pop the Input dialog itself after we return.
                        state.dialogs.pop_below_top();
                        return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExecuteCustomFunction {
                            command,
                            working_dir,
                            pipe_to_action: func.pipe_to_action.clone(),
                            shell,
                        }));
                    }
                }
            }
            _ => {}
        }
        return None;
    }

    if let Some(dialog) = state.dialogs.current() {
        debug!("Dialog content type: {:?}", std::mem::discriminant(&dialog.content));
        match &dialog.content {
            DialogContent::SortDialog { selected_mode_index, selected_order_index, .. } => {
                use rwf_lib::model::{SortMode, SortOrder};
                let mode = match *selected_mode_index {
                    0 => SortMode::Name,
                    1 => SortMode::Size,
                    2 => SortMode::Date,
                    _ => SortMode::Extension,
                };
                let order = if *selected_order_index == 0 { SortOrder::Ascending } else { SortOrder::Descending };
                let pane = state.ui.active_pane;
                // Apply both mode and order directly (no job needed)
                rwf_lib::state::update_state(state, rwf_lib::state::Transition::ChangeSortMode { pane, mode });
                rwf_lib::state::update_state(state, rwf_lib::state::Transition::ChangeSortOrder { pane, order });
                return None;
            }
            DialogContent::FileMask { input, .. } => {
                let mask = if input.is_empty() { None } else { Some(input.clone()) };
                let pane = state.ui.active_pane;
                // Do NOT pop here — app.rs pops after process_dialog_confirmation returns
                rwf_lib::state::update_state(state, rwf_lib::state::Transition::SetFileMask { pane, mask });
                return None;
            }
            DialogContent::WildcardMark { input, .. } => {
                if !input.is_empty() {
                    let pattern = input.clone();
                    rwf_lib::state::update_state(state, rwf_lib::state::Transition::MarkPattern { pattern });
                }
                return None;
            }
            DialogContent::SimpleRename { input, .. } => {
                let new_name = input.clone();
                if !new_name.is_empty() {
                    if let Some(entry) = state.active_pane().current_entry() {
                        let from = entry.location.clone();
                        let to = from.parent()
                            .map(|parent| parent.join(&new_name))
                            .unwrap_or_else(|| from.clone());
                        let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::Rename { from, to });
                        return Some(job_spec);
                    }
                }
                return None;
            }
            DialogContent::DriveSelection { drives, selected_index, filter } => {
                let lower = filter.to_lowercase();
                let filtered: Vec<&rwf_lib::model::dialog::DriveInfo> = if filter.is_empty() {
                    drives.iter().collect()
                } else {
                    drives.iter().filter(|d| {
                        d.display_label().to_lowercase().contains(&lower)
                            || d.path.to_lowercase().contains(&lower)
                    }).collect()
                };
                if let Some(drive) = filtered.get(*selected_index) {
                    let path = drive.path.clone();
                    let pane = state.ui.active_pane;
                    let location = rwf_lib::Location::Local(std::path::PathBuf::from(&path));
                    let result = rwf_lib::state::update_state(state, rwf_lib::state::Transition::ChangeLocation { pane, location });
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::HistoryDialog {
                left_entries, right_entries,
                left_selected, right_selected,
                active_pane, ..
            } => {
                use rwf_lib::model::ui::ActivePane;
                let (entries, selected_index, pane) = match active_pane {
                    ActivePane::Left  => (left_entries.as_slice(),  *left_selected,  ActivePane::Left),
                    ActivePane::Right => (right_entries.as_slice(), *right_selected, ActivePane::Right),
                };
                if entries.get(selected_index).is_some() {
                    let result = rwf_lib::state::update_state(state, rwf_lib::state::Transition::NavigateToHistoryIndex {
                        pane,
                        index: selected_index,
                    });
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::JumpToPath { suggestions, selected_index, query, search_root, loading_job_id, .. } => {
                let path_str: Option<String> = if !suggestions.is_empty() && *selected_index < suggestions.len() {
                    Some(suggestions[*selected_index].clone())
                } else if !query.is_empty() {
                    // Fallback: interpret typed text as a direct path
                    let candidate = std::path::PathBuf::from(query.as_str());
                    if candidate.is_absolute() && candidate.is_dir() {
                        Some(query.clone())
                    } else {
                        let combined = std::path::PathBuf::from(search_root.as_str()).join(query.as_str());
                        if combined.is_dir() { Some(combined.to_string_lossy().into_owned()) }
                        else { None }
                    }
                } else {
                    None
                };
                let pending_job = *loading_job_id;
                if let Some(path) = path_str {
                    let location = rwf_lib::Location::Local(std::path::PathBuf::from(&path));
                    let pane = state.ui.active_pane;
                    state.dialogs.pop();
                    if let Some(job_id) = pending_job {
                        state.jobs.request_cancel(job_id);
                    }
                    let result = rwf_lib::state::update_state(state, rwf_lib::state::Transition::ChangeLocation { pane, location });
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::JumpToFile { suggestions, selected_index, query, search_root, loading_job_id, .. } => {
                let path_str: Option<String> = if !suggestions.is_empty() && *selected_index < suggestions.len() {
                    Some(suggestions[*selected_index].clone())
                } else if !query.is_empty() {
                    // Fallback: interpret typed text as a direct path
                    let candidate = std::path::PathBuf::from(query.as_str());
                    if candidate.is_absolute() && (candidate.is_file() || candidate.is_dir()) {
                        Some(query.clone())
                    } else {
                        let combined = std::path::PathBuf::from(search_root.as_str()).join(query.as_str());
                        if combined.is_file() || combined.is_dir() {
                            Some(combined.to_string_lossy().into_owned())
                        } else {
                            None
                        }
                    }
                } else {
                    None
                };
                let pending_job = *loading_job_id;
                // For a file selection, record the filename to position cursor after navigation.
                let target_file_name: Option<String> = path_str.as_ref().and_then(|p| {
                    let pb = std::path::Path::new(p);
                    if pb.is_file() {
                        pb.file_name().map(|n| n.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                });
                if let Some(path) = path_str {
                    // For files: navigate to the parent directory. For dirs: navigate into them.
                    let nav_path = {
                        let pb = std::path::PathBuf::from(&path);
                        if pb.is_dir() {
                            path.clone()
                        } else {
                            pb.parent()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or(path.clone())
                        }
                    };
                    let location = rwf_lib::Location::Local(std::path::PathBuf::from(&nav_path));
                    let pane = state.ui.active_pane;
                    state.dialogs.pop();
                    if let Some(job_id) = pending_job {
                        state.jobs.request_cancel(job_id);
                    }
                    let result = rwf_lib::state::update_state(state, rwf_lib::state::Transition::ChangeLocation { pane, location });
                    if let Some(name) = target_file_name {
                        let pane_height = state.ui.layout.pane_height;
                        let scroll_margin = state.config.ui.scroll_offset;
                        let tab = state.current_tab_mut();
                        let pane_model = match pane {
                            rwf_lib::model::ActivePane::Left  => &mut tab.left_pane,
                            rwf_lib::model::ActivePane::Right => &mut tab.right_pane,
                        };
                        if pane_model.is_loading {
                            pane_model.pending_cursor_name = Some(name);
                        } else if let Some(pos) = pane_model.entries.iter().position(|e| e.name == name) {
                            pane_model.cursor = pos;
                            pane_model.update_scroll(pane_height, scroll_margin);
                        }
                    }
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::Compression {
                sources,
                archive_name,
                format,
                selected_format_index,
                compression_level,
                ..
            } => {
                debug!("Compression dialog confirmed: {} sources, archive_name='{}'", sources.len(), archive_name);
                debug!("Selected format index: {}, compression level: {}", selected_format_index, compression_level);

                // Ensure archive name has the correct extension for the selected format
                let ext = archive_ext_for_format(*format);
                let archive_name_with_ext = if archive_name.to_lowercase().ends_with(&format!(".{}", ext)) {
                    archive_name.clone()
                } else {
                    // Strip any mismatched extension before adding the correct one
                    let base = ["zip", "7z", "tar", "tgz"].iter().find_map(|old_ext| {
                        archive_name.to_lowercase()
                            .ends_with(&format!(".{}", old_ext))
                            .then(|| &archive_name[..archive_name.len() - old_ext.len() - 1])
                    }).unwrap_or(archive_name.as_str());
                    format!("{}.{}", base, ext)
                };
                debug!("Archive name with extension: '{}'", archive_name_with_ext);

                // Build destination path in opposite pane
                let dest_path = state.opposite_pane().current_location.path()
                    .unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
                let dest = rwf_lib::Location::Local(dest_path.join(&archive_name_with_ext));
                debug!("Destination path: {:?}", dest_path.join(&archive_name_with_ext));

                // Calculate original size for compression ratio
                let original_size: u64 = sources.iter()
                    .filter_map(|loc| {
                        state.active_pane()
                            .entries
                            .iter()
                            .find(|e| &e.location == loc)
                    })
                    .filter(|e| !e.is_dir)
                    .map(|e| e.size)
                    .sum();
                debug!("Original size: {} bytes", original_size);

                let job_spec = rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::CreateArchive {
                        sources: sources.clone(),
                        dest,
                        original_size,
                    }
                );
                debug!("Job spec created: {:?}", job_spec.kind);

                return Some(job_spec);
            }
            DialogContent::ExtractionConfirm { archive, dest, .. } => {
                // Create extraction job - dest is already a Location
                let job_spec = rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::ExtractArchive {
                        archive: archive.clone(),
                        dest: dest.clone(),
                    }
                );

                return Some(job_spec);
            }
            DialogContent::DeleteConfirm { targets, .. } => {
                let locations: Vec<rwf_lib::Location> = targets.iter().map(|(loc, _)| loc.clone()).collect();
                return Some(rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::Delete { targets: locations }
                ));
            }
            DialogContent::RegisteredFolderSelector { folders, selected_index, filter } => {
                let lower = filter.to_lowercase();
                let filtered_indices: Vec<usize> = if filter.is_empty() {
                    (0..folders.len()).collect()
                } else {
                    folders.iter().enumerate()
                        .filter(|(_, f)| {
                            f.name.to_lowercase().contains(&lower)
                                || f.path.to_lowercase().contains(&lower)
                        })
                        .map(|(i, _)| i)
                        .collect()
                };
                if let Some(&folder_index) = filtered_indices.get(*selected_index) {
                    if state.active_pane().marking.count() > 0 {
                        rwf_lib::state::update_state(state, rwf_lib::state::Transition::MoveToRegisteredFolder { folder_index });
                    } else {
                        rwf_lib::state::update_state(state, rwf_lib::state::Transition::NavigateToRegisteredFolder { folder_index });
                    }
                }
                return None;
            }
            DialogContent::PatternRename { find, replace, use_regex, case_sensitive, .. } => {
                if find.is_empty() { return None; }
                let (find, replace, use_regex, case_sensitive) = (find.clone(), replace.clone(), *use_regex, *case_sensitive);
                let pane = state.active_pane();
                let targets: Vec<rwf_lib::Location> = if pane.marking.count() > 0 {
                    pane.entries.iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.location.clone())
                        .collect()
                } else {
                    pane.entries.iter().map(|e| e.location.clone()).collect()
                };
                if targets.is_empty() { return None; }
                let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::PatternRename {
                    targets, find, replace, use_regex, case_sensitive,
                });
                return Some(job_spec);
            }
            DialogContent::CustomFunctionSelector { functions, selected_index, filter } => {
                let lower = filter.to_lowercase();
                let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
                    functions.iter().collect()
                } else {
                    functions.iter().filter(|f| {
                        f.name.to_lowercase().contains(&lower)
                            || f.description.as_deref().unwrap_or("").to_lowercase().contains(&lower)
                    }).collect()
                };
                if let Some(&func) = filtered.get(*selected_index) {
                    let func = func.clone();
                    let expander = rwf_lib::macro_expander::MacroExpander::new();
                    match expander.expand(state, &func) {
                        Ok(command) => {
                            let working_dir = state.active_pane().current_location.clone();
                            let shell = func.shell.clone();
                            return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExecuteCustomFunction {
                                command,
                                working_dir,
                                pipe_to_action: func.pipe_to_action.clone(),
                                shell,
                            }));
                        }
                        Err(_) => {
                            // Command requires $I user input — push an Input dialog.
                            let prompt = rwf_lib::macro_expander::MacroExpander::extract_i_prompt(
                                func.get_command().unwrap_or("")
                            ).unwrap_or_else(|| "Enter input".to_string());
                            state.dialogs.push(rwf_lib::model::Dialog::input(
                                "Custom Function Input", &prompt, ""
                            ));
                            state.pending_custom_function_input = Some(func);
                            state.suppress_next_dialog_pop = true;
                            return None;
                        }
                    }
                }
                return None;
            }
            DialogContent::ContextMenu { options, selected_index } => {
                use rwf_lib::model::dialog::ContextMenuAction;
                if let Some(opt) = options.get(*selected_index) {
                    match opt.action.clone() {
                        ContextMenuAction::Copy => {
                            let transitions = rwf_lib::input::action_to_transitions(state, &rwf_lib::input::Action::Copy);
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    return Some(job);
                                }
                            }
                        }
                        ContextMenuAction::Move => {
                            let transitions = rwf_lib::input::action_to_transitions(state, &rwf_lib::input::Action::Move);
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    return Some(job);
                                }
                            }
                        }
                        ContextMenuAction::Delete => {
                            let transitions = rwf_lib::input::action_to_transitions(state, &rwf_lib::input::Action::Delete);
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    return Some(job);
                                }
                            }
                        }
                        ContextMenuAction::Rename => {
                            let transitions = rwf_lib::input::action_to_transitions(state, &rwf_lib::input::Action::Rename);
                            for t in transitions {
                                rwf_lib::state::update_state(state, t);
                            }
                        }
                        ContextMenuAction::View => {
                            if let Some(entry) = state.active_pane().current_entry() {
                                if !entry.is_dir {
                                    let loc = entry.location.clone();
                                    rwf_lib::state::update_state(state, rwf_lib::state::Transition::OpenTextViewer { location: loc });
                                }
                            }
                        }
                        ContextMenuAction::CustomFunction(name) => {
                            let func = state.custom_functions.iter().find(|f| f.name == name).cloned();
                            if let Some(func) = func {
                                let expander = rwf_lib::macro_expander::MacroExpander::new();
                                if let Ok(command) = expander.expand(state, &func) {
                                    let working_dir = state.active_pane().current_location.clone();
                                    let shell = func.shell.clone();
                                    return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExecuteCustomFunction {
                                        command,
                                        working_dir,
                                        pipe_to_action: func.pipe_to_action.clone(),
                                        shell,
                                    }));
                                }
                            }
                        }
                        ContextMenuAction::Separator => {}
                    }
                }
                return None;
            }
            DialogContent::CustomFunctionMenu { items, selected_index } => {
                let items = items.clone();
                let idx = *selected_index;
                if let Some(item) = items.get(idx) {
                    if item.is_selectable() {
                        return resolve_menu_item_action(state, &item.action);
                    }
                }
                return None;
            }
            _ => {
                debug!("Unknown dialog content type");
            }
        }
    } else {
        debug!("No dialog found");
    }

    None
}

/// Resolve a menu item's `Action` string to a job spec.
/// First tries built-in action names, then looks up a custom function by name.
fn resolve_menu_item_action(state: &mut rwf_lib::AppState, action_name: &str) -> Option<rwf_lib::job::JobSpec> {
    let builtin: Option<rwf_lib::input::Action> = match action_name {
        "DeleteFile" | "Delete" => Some(rwf_lib::input::Action::Delete),
        "MoveFile"   | "Move"   => Some(rwf_lib::input::Action::Move),
        "CopyFile"   | "Copy"   => Some(rwf_lib::input::Action::Copy),
        "ViewFileAsText" | "View" => Some(rwf_lib::input::Action::OpenTextViewer),
        "ViewFileAsHex"  => Some(rwf_lib::input::Action::OpenHexViewer),
        "ReloadConfiguration"        => Some(rwf_lib::input::Action::ReloadConfig),
        "EditConfigFile" => Some(rwf_lib::input::Action::EditConfigFile),
        _ => None,
    };

    if let Some(action) = builtin {
        let transitions = rwf_lib::input::action_to_transitions(state, &action);
        for t in transitions {
            let result = rwf_lib::state::update_state(state, t);
            // Collect logs and reload-keybindings flag into staging fields on AppState
            state.pending_confirmation_logs.extend(result.task_panel_logs);
            if result.reload_keybindings {
                state.confirmation_needs_keybinding_reload = true;
            }
            if let Some(job) = result.jobs_to_start.into_iter().next() {
                return Some(job);
            }
        }
        return None;
    }

    // Fall back: find by custom function name and execute its command
    let func = state.custom_functions.iter().find(|f| f.name == action_name).cloned();
    if let Some(func) = func {
        if func.is_command() {
            let expander = rwf_lib::macro_expander::MacroExpander::new();
            if let Ok(command) = expander.expand(state, &func) {
                let working_dir = state.active_pane().current_location.clone();
                let shell = func.shell.clone();
                return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExecuteCustomFunction {
                    command,
                    working_dir,
                    pipe_to_action: func.pipe_to_action.clone(),
                    shell,
                }));
            }
        }
    }
    None
}

#[cfg(test)]
mod conflict_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use rwf_lib::model::dialog::{ConflictAction, ConflictPair};
    use rwf_lib::model::{FileEntry, Location};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_conflict(src_name: &str, dst_name: &str) -> ConflictPair {
        let src = FileEntry {
            name: src_name.to_string(),
            location: Location::Local(PathBuf::from(format!("/src/{}", src_name))),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };
        let dst = FileEntry {
            name: dst_name.to_string(),
            location: Location::Local(PathBuf::from(format!("/dst/{}", dst_name))),
            size: 200,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };
        ConflictPair {
            source: src.clone(),
            dest: dst.clone(),
            source_path: src.location.clone(),
            dest_path: dst.location.clone(),
            is_directory: false,
        }
    }

    fn enter_key() -> KeyEvent { KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE) }
    fn shift_enter_key() -> KeyEvent { KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT) }
    fn esc_key() -> KeyEvent { KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE) }

    // ---- validate_filename tests -------------------------------------------

    #[test]
    fn test_validate_filename_empty() {
        assert!(validate_filename("", "original.txt").is_some());
    }

    #[test]
    fn test_validate_filename_same_as_original() {
        assert!(validate_filename("file.txt", "file.txt").is_some());
    }

    #[test]
    fn test_validate_filename_windows_invalid_chars() {
        for ch in ['<', '>', ':', '"', '|', '?', '*'] {
            let bad = format!("fi{}le.txt", ch);
            assert!(validate_filename(&bad, "orig.txt").is_some(), "char '{}' should be rejected", ch);
        }
    }

    #[test]
    fn test_validate_filename_reserved_name() {
        assert!(validate_filename("CON", "orig.txt").is_some());
        assert!(validate_filename("NUL.txt", "orig.txt").is_some());
    }

    #[test]
    fn test_validate_filename_valid() {
        assert!(validate_filename("new_name.txt", "old_name.txt").is_none());
    }

    // ---- handle_file_conflict_input: Force button (index 0) ----------------

    #[test]
    fn test_force_button_enter_pushes_force_decision() {
        let mut conflicts = vec![make_conflict("a.txt", "a.txt")];
        let mut current_index = 0usize;
        let mut focused_button = 0usize;  // Force
        let mut rename_text = "a.txt".to_string();
        let mut rename_cursor = 5usize;
        let mut rename_scroll = 0usize;
        let mut edit_mode = rwf_lib::config::EditMode::Emacs;
        let mut vi_mode = None;
        let mut error_message = None;
        let mut decisions = Vec::new();
        let mut pending_fwd = None;
        let mut pending_op = None;
        let mut pending_cx = false;
        let mut history = vec!["a.txt".to_string()];
        let mut history_index = 0usize;

        let action = handle_file_conflict_input(
            &mut conflicts, &mut current_index, &mut focused_button,
            &mut rename_text, &mut rename_cursor, &mut rename_scroll,
            &mut edit_mode, &mut vi_mode, &mut error_message, &mut decisions,
            &mut pending_fwd, &mut pending_op, &mut pending_cx,
            &mut history, &mut history_index, enter_key(),
        );

        assert_eq!(action, DialogAction::Confirm);
        assert_eq!(decisions.len(), 1);
        assert!(matches!(decisions[0], ConflictAction::Force));
    }

    // ---- Skip button (index 2) ---------------------------------------------

    #[test]
    fn test_skip_button_enter_pushes_skip_decision() {
        let mut conflicts = vec![make_conflict("b.txt", "b.txt")];
        let mut current_index = 0usize;
        let mut focused_button = 2usize;  // Skip
        let mut rename_text = "b.txt".to_string();
        let mut rename_cursor = 5usize;
        let mut rename_scroll = 0usize;
        let mut edit_mode = rwf_lib::config::EditMode::Emacs;
        let mut vi_mode = None;
        let mut error_message = None;
        let mut decisions = Vec::new();
        let mut pending_fwd = None;
        let mut pending_op = None;
        let mut pending_cx = false;
        let mut history = vec!["b.txt".to_string()];
        let mut history_index = 0usize;

        let action = handle_file_conflict_input(
            &mut conflicts, &mut current_index, &mut focused_button,
            &mut rename_text, &mut rename_cursor, &mut rename_scroll,
            &mut edit_mode, &mut vi_mode, &mut error_message, &mut decisions,
            &mut pending_fwd, &mut pending_op, &mut pending_cx,
            &mut history, &mut history_index, enter_key(),
        );

        assert_eq!(action, DialogAction::Confirm);
        assert!(matches!(decisions[0], ConflictAction::Skip));
    }

    // ---- Cancel button (index 4) -------------------------------------------

    #[test]
    fn test_cancel_button_enter_returns_confirm_with_skip_decision() {
        let mut conflicts = vec![make_conflict("c.txt", "c.txt")];
        let mut current_index = 0usize;
        let mut focused_button = 4usize;  // Cancel
        let mut rename_text = "c.txt".to_string();
        let mut rename_cursor = 5usize;
        let mut rename_scroll = 0usize;
        let mut edit_mode = rwf_lib::config::EditMode::Emacs;
        let mut vi_mode = None;
        let mut error_message = None;
        let mut decisions = Vec::new();
        let mut pending_fwd = None;
        let mut pending_op = None;
        let mut pending_cx = false;
        let mut history = vec!["c.txt".to_string()];
        let mut history_index = 0usize;

        let action = handle_file_conflict_input(
            &mut conflicts, &mut current_index, &mut focused_button,
            &mut rename_text, &mut rename_cursor, &mut rename_scroll,
            &mut edit_mode, &mut vi_mode, &mut error_message, &mut decisions,
            &mut pending_fwd, &mut pending_op, &mut pending_cx,
            &mut history, &mut history_index, enter_key(),
        );

        assert_eq!(action, DialogAction::Confirm);
        assert!(matches!(decisions[0], ConflictAction::Skip));
    }

    // ---- Esc cancels dialog ------------------------------------------------

    #[test]
    fn test_esc_cancels_dialog() {
        let mut conflicts = vec![make_conflict("d.txt", "d.txt")];
        let mut current_index = 0usize;
        let mut focused_button = 0usize;
        let mut rename_text = "d.txt".to_string();
        let mut rename_cursor = 5usize;
        let mut rename_scroll = 0usize;
        let mut edit_mode = rwf_lib::config::EditMode::Emacs;
        let mut vi_mode = None;
        let mut error_message = None;
        let mut decisions = Vec::new();
        let mut pending_fwd = None;
        let mut pending_op = None;
        let mut pending_cx = false;
        let mut history = vec!["d.txt".to_string()];
        let mut history_index = 0usize;

        let action = handle_file_conflict_input(
            &mut conflicts, &mut current_index, &mut focused_button,
            &mut rename_text, &mut rename_cursor, &mut rename_scroll,
            &mut edit_mode, &mut vi_mode, &mut error_message, &mut decisions,
            &mut pending_fwd, &mut pending_op, &mut pending_cx,
            &mut history, &mut history_index, esc_key(),
        );

        assert_eq!(action, DialogAction::Cancel);
        assert!(decisions.is_empty(), "Cancel should not push a decision");
    }

    // ---- Shift+Enter applies to all remaining ------------------------------

    #[test]
    fn test_shift_enter_applies_to_all_remaining() {
        let mut conflicts = vec![
            make_conflict("e1.txt", "e1.txt"),
            make_conflict("e2.txt", "e2.txt"),
            make_conflict("e3.txt", "e3.txt"),
        ];
        let mut current_index = 1usize;  // at second conflict
        let mut focused_button = 2usize; // Skip
        let mut rename_text = "e2.txt".to_string();
        let mut rename_cursor = 6usize;
        let mut rename_scroll = 0usize;
        let mut edit_mode = rwf_lib::config::EditMode::Emacs;
        let mut vi_mode = None;
        let mut error_message = None;
        let mut decisions = vec![ConflictAction::Force]; // first conflict already decided
        let mut pending_fwd = None;
        let mut pending_op = None;
        let mut pending_cx = false;
        let mut history = vec!["e2.txt".to_string()];
        let mut history_index = 0usize;

        let action = handle_file_conflict_input(
            &mut conflicts, &mut current_index, &mut focused_button,
            &mut rename_text, &mut rename_cursor, &mut rename_scroll,
            &mut edit_mode, &mut vi_mode, &mut error_message, &mut decisions,
            &mut pending_fwd, &mut pending_op, &mut pending_cx,
            &mut history, &mut history_index, shift_enter_key(),
        );

        assert_eq!(action, DialogAction::ConfirmAll);
        // decisions: 1 (pre-existing) + 2 (remaining from current_index=1 to end)
        assert_eq!(decisions.len(), 3, "all 3 decisions must be present");
        assert!(matches!(decisions[1], ConflictAction::Skip));
        assert!(matches!(decisions[2], ConflictAction::Skip));
    }

    // ---- Tab cycle stays within 0..4 ---------------------------------------

    #[test]
    fn test_tab_cycles_0_to_4() {
        let mut conflicts = vec![make_conflict("f.txt", "f.txt")];
        let mut current_index = 0usize;
        let mut focused_button = 4usize; // last field
        let mut rename_text = "f.txt".to_string();
        let mut rename_cursor = 5usize;
        let mut rename_scroll = 0usize;
        let mut edit_mode = rwf_lib::config::EditMode::Emacs;
        let mut vi_mode = None;
        let mut error_message = None;
        let mut decisions = Vec::new();
        let mut pending_fwd = None;
        let mut pending_op = None;
        let mut pending_cx = false;
        let mut history = vec!["f.txt".to_string()];
        let mut history_index = 0usize;

        let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        handle_file_conflict_input(
            &mut conflicts, &mut current_index, &mut focused_button,
            &mut rename_text, &mut rename_cursor, &mut rename_scroll,
            &mut edit_mode, &mut vi_mode, &mut error_message, &mut decisions,
            &mut pending_fwd, &mut pending_op, &mut pending_cx,
            &mut history, &mut history_index, tab_key,
        );

        assert_eq!(focused_button, 0, "Tab from last field (4) should wrap to 0");
    }
}
