//! Snapshots for `DialogContent::Help`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent, HelpDialog, HelpEntry, HelpTab};

#[test]
fn help_empty() {
    let state = test_state();
    let dialog = Dialog::help_with_language("en");
    snapshot_dialog("help_empty", &dialog, &state);
}

#[test]
fn help_populated() {
    let state = test_state();
    let entries = vec![
        HelpEntry {
            category: "Navigation".to_string(),
            description: "Move up one line".to_string(),
            keys: vec!["k".to_string()],
            action_name: "move_up".to_string(),
            tab: HelpTab::NormalMode,
        },
        HelpEntry {
            category: "Navigation".to_string(),
            description: "Move down one line".to_string(),
            keys: vec!["j".to_string()],
            action_name: "move_down".to_string(),
            tab: HelpTab::NormalMode,
        },
        HelpEntry {
            category: "File Operations".to_string(),
            description: "Copy selected file".to_string(),
            keys: vec!["c".to_string()],
            action_name: "copy".to_string(),
            tab: HelpTab::NormalMode,
        },
    ];
    let dialog = Dialog {
        title: "Help".to_string(),
        content: DialogContent::Help(HelpDialog {
            entries,
            query: String::new(),
            regex_mode: false,
            show_unbound: true,
            active_tab: HelpTab::NormalMode,
            scroll_pos: 0,
            language: "en".to_string(),
            last_query_change: None,
        }),
    };
    snapshot_dialog("help_populated", &dialog, &state);
}

#[test]
fn help_with_search() {
    let state = test_state();
    let entries = vec![
        HelpEntry {
            category: "Navigation".to_string(),
            description: "Move up one line".to_string(),
            keys: vec!["k".to_string()],
            action_name: "move_up".to_string(),
            tab: HelpTab::NormalMode,
        },
        HelpEntry {
            category: "Navigation".to_string(),
            description: "Move down one line".to_string(),
            keys: vec!["j".to_string()],
            action_name: "move_down".to_string(),
            tab: HelpTab::NormalMode,
        },
    ];
    let dialog = Dialog {
        title: "Help".to_string(),
        content: DialogContent::Help(HelpDialog {
            entries,
            query: "move".to_string(),
            regex_mode: false,
            show_unbound: true,
            active_tab: HelpTab::NormalMode,
            scroll_pos: 0,
            language: "en".to_string(),
            last_query_change: None,
        }),
    };
    snapshot_dialog("help_with_search", &dialog, &state);
}
