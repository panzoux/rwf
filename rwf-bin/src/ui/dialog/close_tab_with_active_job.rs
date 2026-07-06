//! Close-tab-with-active-job confirmation dialog input handling.
//!
//! Moved from dialog/mod.rs in M4 S5.

use crossterm::event::KeyEvent;
use rwf_lib::model::dialog::CloseTabWithActiveJobDialog;

use super::DialogAction;

/// Handle key input: Enter confirms, Escape cancels, Tab cycles OK (0) / Cancel (1).
pub(super) fn handle_input(
    dialog: &mut CloseTabWithActiveJobDialog,
    key: KeyEvent,
) -> DialogAction {
    let CloseTabWithActiveJobDialog { focused_field, .. } = dialog;
    if key.code == crossterm::event::KeyCode::Enter {
        return DialogAction::Confirm;
    }
    if key.code == crossterm::event::KeyCode::Esc {
        return DialogAction::Cancel;
    }
    // Tab key cycles between OK (field 0) and Cancel (field 1) buttons.
    // Cycle: 0→1→0 (OK→Cancel→OK) regardless of Shift — matches original behavior.
    if key.code == crossterm::event::KeyCode::Tab {
        *focused_field = if *focused_field == 0 { 1 } else { 0 };
        return DialogAction::None;
    }
    DialogAction::None
}
