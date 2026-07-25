//! Snapshot tests for dialog rendering (M3 safety net).
//!
//! Every `DialogContent` variant is rendered through the real `render_dialog`
//! dispatch at two fixed terminal sizes and snapshotted with `insta`
//! (buffer text + style runs). These snapshots pin dialog behavior across the
//! M3 file split — any diff during the split is a regression.
//!
//! Conventions (see docs/TESTING.md):
//! - Build dialogs with fixed, ASCII, deterministic data only
//!   (`SystemTime::UNIX_EPOCH`, fixed sizes, `/test/...` paths).
//!   Timestamps rendered via the local timezone are redacted by a filter.
//! - One module per dialog variant; use [`snapshot_dialog`] so every dialog
//!   is captured at both sizes with consistent naming.
//! - Review intentional changes with `cargo insta review`.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rwf_lib::model::dialog::Dialog;
use rwf_lib::{AppConfig, AppState};

mod close_tab_with_active_job;
mod comparison_view;
mod compression;
mod confirmation;
mod context_menu;
mod custom_function_menu;
mod custom_function_selector;
mod delete_confirm;
mod drive_selection;
mod error;
mod extraction_confirm;
mod file_conflict;
mod file_info;
mod file_mask;
mod help;
mod history;
mod input;
mod job_manager;
mod jump_to_file;
mod jump_to_path;
mod open_with_picker;
mod pattern_rename;
mod progress;
mod registered_folder_selector;
mod simple_rename;
mod sort;
mod split_join;
mod tab_selector;
mod type_mismatch_warning;
mod version;
mod wildcard_mark;

/// Standard snapshot sizes: (name suffix, width, height).
pub const SIZES: [(&str, u16, u16); 2] = [("80x24", 80, 24), ("120x40", 120, 40)];

/// An `AppState` with default config — enough for most dialogs.
pub fn test_state() -> AppState {
    AppState::new(AppConfig::default())
}

/// Render `dialog` over `state` at the given size and return the buffer dump
/// (text + style runs).
pub fn render_dialog_to_string(
    dialog: &Dialog,
    state: &AppState,
    width: u16,
    height: u16,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| super::render_dialog(frame, dialog, state))
        .expect("draw");
    format!("{:?}", terminal.backend().buffer())
}

/// Snapshot `dialog` at both standard sizes as `<name>_<size>`.
///
/// Locally-rendered wall-clock timestamps (YYYY-MM-DD HH:MM:SS) are redacted
/// so snapshots are timezone-independent (CI runs UTC, dev machines may not).
pub fn snapshot_dialog(name: &str, dialog: &Dialog, state: &AppState) {
    insta::with_settings!({filters => vec![
        (r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}", "[TIMESTAMP]"),
        (r"\d{2}:\d{2}:\d{2}", "[TIME]"),
        (
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            "[UUID]",
        ),
    ]}, {
        for (suffix, width, height) in SIZES {
            insta::assert_snapshot!(
                format!("{name}_{suffix}"),
                render_dialog_to_string(dialog, state, width, height)
            );
        }
    });
}
