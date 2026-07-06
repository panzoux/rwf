//! Custom function selector and menu rendering and input handling.
//!
//! Rendering split from dialog/mod.rs in M3 (move-only; snapshot-protected).
//! Input handling moved from dialog/mod.rs in M4 S5.

use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};

use crate::ui::smart_truncate;

use crossterm::event::{KeyEvent, KeyModifiers};
use rwf_lib::model::dialog::{CustomFunctionMenuDialog, CustomFunctionSelectorContent};

use super::DialogAction;

/// Handle key input for the Custom Function Selector: incremental search + arrow navigation.
pub(super) fn handle_selector_input(
    dialog: &mut CustomFunctionSelectorContent,
    key: KeyEvent,
) -> DialogAction {
    let CustomFunctionSelectorContent {
        functions,
        selected_index,
        filter,
    } = dialog;
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
    DialogAction::None
}

/// Handle key input for the Custom Function Menu: second-level menu with separator
/// skipping and char-jump.
pub(super) fn handle_menu_input(
    dialog: &mut CustomFunctionMenuDialog,
    key: KeyEvent,
) -> DialogAction {
    let CustomFunctionMenuDialog {
        items,
        selected_index,
    } = dialog;
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
    DialogAction::None
}

pub(super) fn render_custom_function_selector(
    frame: &mut Frame,
    area: Rect,
    functions: &[rwf_lib::model::dialog::CustomFunction],
    selected_index: usize,
    filter: &str,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

    let item_width = area.width.saturating_sub(4) as usize;

    let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
        functions.iter().collect()
    } else {
        let lower = filter.to_lowercase();
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

    let clamped_sel = selected_index.min(filtered.len().saturating_sub(1));

    let hint_y = area.y + area.height.saturating_sub(2);
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
        if fi >= filtered.len() {
            break;
        }
        let func = filtered[fi];
        let name_w = item_width.saturating_sub(2);
        let label = if let Some(desc) = &func.description {
            let desc_w = name_w.saturating_sub(func.name.len() + 3);
            if desc_w > 4 {
                format!(
                    "{:<name_w$}",
                    format!("{}  {}", func.name, smart_truncate(desc, desc_w, "…")),
                    name_w = name_w
                )
            } else {
                smart_truncate(&func.name, name_w, "…")
            }
        } else {
            smart_truncate(&func.name, name_w, "…")
        };
        let style = if fi == clamped_sel {
            selected_style
        } else {
            base_style
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(style),
            Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
        );
    }
}

pub(super) fn render_custom_function_menu(
    frame: &mut Frame,
    area: Rect,
    items: &[rwf_lib::model::dialog::MenuItem],
    selected_index: usize,
) {
    let base_style = crate::ui::dialog::common::DIALOG_TEXT;
    let selected_style = crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD);
    let sep_style = crate::ui::dialog::common::DIALOG_DIM;
    let hint_style = crate::ui::dialog::common::DIALOG_DIM;

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
        if ii >= items.len() {
            break;
        }
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
            let style = if ii == selected_index {
                selected_style
            } else {
                base_style
            };
            frame.render_widget(
                Paragraph::new(format!(" {}", item.name)).style(style),
                Rect::new(area.x + 2, area.y + row as u16, item_width as u16, 1),
            );
        }
    }
}
