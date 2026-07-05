//! Generic dialog content rendering and input for simple variants
//! (Confirmation / Input / Progress / Error / Version / selectors...).
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use crossterm::event::KeyEvent;
use rwf_lib::config::ViMode;
use rwf_lib::model::dialog::DialogContent;

use super::compression::{render_compression_dialog, CompressionDialogState};
use super::extract_confirm::ExtractionConfirmDialog;
use super::DialogAction;
use super::{archive_ext_for_format, compression};

pub(super) fn render_dialog_content(
    frame: &mut Frame,
    content: &DialogContent,
    area: Rect,
    focused: bool,
) {
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

        DialogContent::ExtractionConfirm {
            archive,
            dest,
            file_count,
        } => {
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
        DialogContent::Error {
            message, details, ..
        } => {
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
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
        }
        DialogContent::Input {
            prompt,
            input,
            cursor_pos,
            scroll_pos,
            ..
        } => {
            use ratatui::layout::Alignment;
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // prompt label
                    Constraint::Length(1), // textbox
                    Constraint::Length(1), // hint
                ])
                .split(area);
            let base_style = Style::default().fg(Color::Black).bg(Color::Gray);
            let hint_style = Style::default().fg(Color::DarkGray).bg(Color::Gray);
            let item_width = area.width.saturating_sub(4);
            frame.render_widget(
                Paragraph::new(prompt.as_str()).style(base_style),
                Rect::new(area.x + 2, chunks[0].y, item_width, 1),
            );
            {
                use crate::ui::text_input::TextInput;
                let mut ti = TextInput::new(Some(input.clone()), rwf_lib::config::EditMode::Emacs);
                ti.set_original_text(input.clone());
                ti.set_cursor(*cursor_pos);
                ti.set_scroll(*scroll_pos);
                ti.set_width(item_width);
                ti.render(
                    frame,
                    Rect::new(area.x + 2, chunks[1].y, item_width, 1),
                    focused,
                );
            }
            frame.render_widget(
                Paragraph::new("(Enter to confirm, Esc to cancel)")
                    .style(hint_style)
                    .alignment(Alignment::Left),
                Rect::new(area.x + 2, chunks[2].y, item_width, 1),
            );
        }
        _ => {}
    }
}

/// Handle content-specific input
pub(super) fn handle_content_input(content: &mut DialogContent, key: KeyEvent) -> DialogAction {
    match content {
        DialogContent::SortDialog {
            selected_mode_index,
            selected_order_index,
            focused_section,
        } => {
            use crossterm::event::KeyCode;
            match key.code {
                KeyCode::Up => {
                    match *focused_section {
                        0 => {
                            if *selected_mode_index > 0 {
                                *selected_mode_index -= 1;
                            }
                        }
                        1 => {
                            if *selected_order_index > 0 {
                                *selected_order_index -= 1;
                            }
                        }
                        _ => {}
                    }
                    DialogAction::None
                }
                KeyCode::Down => {
                    match *focused_section {
                        0 => {
                            if *selected_mode_index < 3 {
                                *selected_mode_index += 1;
                            }
                        }
                        1 => {
                            if *selected_order_index < 1 {
                                *selected_order_index += 1;
                            }
                        }
                        _ => {}
                    }
                    DialogAction::None
                }
                KeyCode::Left => {
                    if *focused_section > 0 {
                        *focused_section -= 1;
                    }
                    DialogAction::None
                }
                KeyCode::Right => {
                    if *focused_section < 3 {
                        *focused_section += 1;
                    }
                    DialogAction::None
                }
                _ => DialogAction::None,
            }
        }
        DialogContent::JobManager {
            selected_index,
            focused_field,
        } => {
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
            DialogAction::None
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
                                let base = if old_ext == "tar"
                                    && archive_name.to_lowercase().ends_with(".tar.gz")
                                {
                                    &archive_name[..archive_name.len() - ".tar.gz".len()]
                                } else if archive_name
                                    .to_lowercase()
                                    .ends_with(&format!(".{}", old_ext))
                                {
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
        DialogContent::DeleteConfirm {
            targets,
            scroll_offset,
        } => {
            let total = targets.len();
            // Mirror the render-side height formula exactly (now using centered_rect_abs, no rounding loss)
            // min_content = total.min(12) + 7; min_dialog = min_content + 2 = total.min(12) + 9
            // dialog_h = max(80% of screen, min_dialog), capped at screen - 2
            // layout fixed overhead: borders(2) + spacer(1) + hint(1) + buttons(3) + header(1) + blank(1) = 9
            let screen_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
            let min_content = total.min(12) as u16 + 7;
            let min_dialog = min_content + 2;
            let dialog_h = (screen_h * 80 / 100)
                .max(min_dialog)
                .min(screen_h.saturating_sub(2));
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
