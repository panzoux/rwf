//! Snapshots for `DialogContent::TabSelector`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn tab_selector_single_tab() {
    let state = test_state();
    let dialog = Dialog::tab_selector(vec!["Tab 1".to_string()]);
    snapshot_dialog("tab_selector_single", &dialog, &state);
}

#[test]
fn tab_selector_multiple_tabs() {
    let state = test_state();
    let dialog = Dialog::tab_selector(vec![
        "Tab 1".to_string(),
        "Tab 2".to_string(),
        "Tab 3".to_string(),
    ]);
    snapshot_dialog("tab_selector_multiple", &dialog, &state);
}

#[test]
fn tab_selector_many_tabs() {
    let state = test_state();
    let tabs = (1..=10).map(|i| format!("Tab {}", i)).collect::<Vec<_>>();
    let dialog = Dialog::tab_selector(tabs);
    snapshot_dialog("tab_selector_many", &dialog, &state);
}
