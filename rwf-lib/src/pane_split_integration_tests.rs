//! 7.25: moving the left/right pane divider (`Ctrl+Left/Right`, `Ctrl+|`). The
//! divider belongs to the tab: each tab keeps, draws and saves its own.
//!
//! The pure scaling rule is tested next to `split_columns` in `model/ui.rs`; these
//! tests cover the transitions, their interplay with terminal resizes, and the
//! session round trip. See plan/7.25.pane_width_resize.md §3.2.

use crate::model::{split_columns, PaneSplit};
use crate::state::{update_state, AppState, Transition};
use crate::test_utils::test_state;

fn state_at_width(width: usize) -> AppState {
    let mut state = test_state();
    update_state(&mut state, Transition::UpdatePaneWidth { width });
    state
}

fn split(left: u16, at_total: u16) -> Option<PaneSplit> {
    Some(PaneSplit { left, at_total })
}

fn drawn(state: &AppState) -> (u16, u16) {
    split_columns(
        state.ui.layout.pane_width as u16,
        state.current_tab().left_pane_width,
    )
}

#[test]
fn widen_left_moves_the_divider_two_columns_right_from_the_even_split() {
    let mut state = state_at_width(120);
    assert_eq!(state.current_tab().left_pane_width, None);

    let result = update_state(&mut state, Transition::WidenLeftPane);
    assert!(result.ui_changed);
    assert_eq!(state.current_tab().left_pane_width, split(62, 120));

    update_state(&mut state, Transition::WidenLeftPane);
    assert_eq!(state.current_tab().left_pane_width, split(64, 120));
    assert_eq!(drawn(&state), (64, 56));
}

#[test]
fn widen_right_moves_the_divider_two_columns_left() {
    let mut state = state_at_width(120);
    update_state(&mut state, Transition::WidenRightPane);
    assert_eq!(state.current_tab().left_pane_width, split(58, 120));
    assert_eq!(drawn(&state), (58, 62));
}

#[test]
fn widen_on_an_odd_width_starts_from_the_drawn_even_split() {
    // 121 columns draw as 61 / 60.
    let mut state = state_at_width(121);
    update_state(&mut state, Transition::WidenLeftPane);
    assert_eq!(state.current_tab().left_pane_width, split(63, 121));
}

#[test]
fn resize_keeps_the_stored_split_and_scales_the_drawn_one() {
    let mut state = state_at_width(120);
    update_state(&mut state, Transition::WidenLeftPane);
    update_state(&mut state, Transition::WidenLeftPane);

    for (width, expected) in [
        (160, (85, 75)),
        (34, (18, 16)),
        (33, (17, 16)),
        (120, (64, 56)),
    ] {
        update_state(&mut state, Transition::UpdatePaneWidth { width });
        assert_eq!(
            state.current_tab().left_pane_width,
            split(64, 120),
            "a resize to {width} must not rewrite the stored split"
        );
        assert_eq!(drawn(&state), expected, "drawn split at {width} columns");
    }
}

#[test]
fn widen_after_a_resize_starts_from_the_drawn_width_and_rebases() {
    let mut state = state_at_width(120);
    state.current_tab_mut().left_pane_width = split(64, 120);
    update_state(&mut state, Transition::UpdatePaneWidth { width: 160 });

    update_state(&mut state, Transition::WidenLeftPane);
    assert_eq!(state.current_tab().left_pane_width, split(87, 160));
    assert_eq!(drawn(&state), (87, 73));
}

#[test]
fn widen_stops_at_the_minimum_width_of_the_other_side() {
    let mut state = state_at_width(160);
    for _ in 0..100 {
        update_state(&mut state, Transition::WidenLeftPane);
    }
    assert_eq!(state.current_tab().left_pane_width, split(144, 160));

    let result = update_state(&mut state, Transition::WidenLeftPane);
    assert!(!result.ui_changed, "a press at the limit changes nothing");
    assert_eq!(state.current_tab().left_pane_width, split(144, 160));

    for _ in 0..100 {
        update_state(&mut state, Transition::WidenRightPane);
    }
    assert_eq!(state.current_tab().left_pane_width, split(16, 160));
}

/// A press that cannot move the drawn divider must not change the stored value
/// either — otherwise the split would jump on the next resize for no visible reason.
#[test]
fn widen_on_a_narrow_terminal_does_not_change_the_value_invisibly() {
    let mut state = state_at_width(120);
    state.current_tab_mut().left_pane_width = split(64, 120);
    update_state(&mut state, Transition::UpdatePaneWidth { width: 33 });
    assert_eq!(drawn(&state), (17, 16));

    let result = update_state(&mut state, Transition::WidenLeftPane);
    assert!(!result.ui_changed);
    assert_eq!(state.current_tab().left_pane_width, split(64, 120));

    // Too narrow for two 16-column panes: both directions are no-ops.
    update_state(&mut state, Transition::UpdatePaneWidth { width: 31 });
    for t in [Transition::WidenLeftPane, Transition::WidenRightPane] {
        assert!(!update_state(&mut state, t).ui_changed);
    }
    assert_eq!(state.current_tab().left_pane_width, split(64, 120));
}

#[test]
fn reset_returns_to_the_even_split() {
    let mut state = state_at_width(120);
    update_state(&mut state, Transition::WidenLeftPane);

    let result = update_state(&mut state, Transition::ResetPaneSplit);
    assert!(result.ui_changed);
    assert_eq!(state.current_tab().left_pane_width, None);
    assert_eq!(drawn(&state), (60, 60));

    let again = update_state(&mut state, Transition::ResetPaneSplit);
    assert!(!again.ui_changed);
}

/// Each tab keeps its own divider: moving it in one tab leaves the others alone, and
/// switching back finds it where it was left.
#[test]
fn each_tab_keeps_its_own_split() {
    let mut state = state_at_width(120);
    update_state(&mut state, Transition::WidenLeftPane);
    update_state(&mut state, Transition::WidenLeftPane);
    assert_eq!(state.current_tab().left_pane_width, split(64, 120));

    // A new tab starts even and is where the next key press lands.
    state.last_tab_created = None;
    update_state(&mut state, Transition::CreateTab);
    assert_eq!(state.tabs.active_index, 1);
    assert_eq!(state.current_tab().left_pane_width, None);
    assert_eq!(drawn(&state), (60, 60));
    update_state(&mut state, Transition::WidenRightPane);
    assert_eq!(state.current_tab().left_pane_width, split(58, 120));

    update_state(&mut state, Transition::PrevTab);
    assert_eq!(drawn(&state), (64, 56), "tab 0 lost its divider");

    // Reset only touches the active tab.
    update_state(&mut state, Transition::ResetPaneSplit);
    assert_eq!(state.current_tab().left_pane_width, None);
    assert_eq!(state.tabs.tabs[1].left_pane_width, split(58, 120));
}

/// The search-jump width follows the active tab's divider.
#[test]
fn viewer_text_width_follows_the_active_tabs_split() {
    let mut state = state_at_width(120);
    state.current_tab_mut().left_pane_width = split(74, 120);
    state.ui.layout.viewer_layout = crate::model::ViewerLayout::SideBySide;
    state.ui.layout.viewer_anchor_pane = crate::model::ActivePane::Left;
    // No viewer: 3-digit prefix. Viewer side 46 - 2 - 7 = 37.
    assert_eq!(state.viewer_text_width(), 37);
    state.current_tab_mut().left_pane_width = None;
    assert_eq!(state.viewer_text_width(), 51);
}

#[test]
fn session_round_trip_restores_each_tabs_split_proportionally() {
    let mut state = state_at_width(120);
    state.current_tab_mut().left_pane_width = split(64, 120);
    state.last_tab_created = None;
    update_state(&mut state, Transition::CreateTab);
    // Tab 1 stays even.

    let session = crate::session::save_session(
        &state.tabs.tabs,
        state.tabs.active_index,
        state.ui.active_pane,
        &std::collections::HashSet::new(),
        state.ui.layout.show_task_panel,
        state.ui.layout.task_panel_height,
    );
    let json = serde_json::to_string(&session).unwrap();
    let loaded: crate::session::SessionState = serde_json::from_str(&json).unwrap();

    // Restart on a wider terminal.
    let mut restored = state_at_width(160);
    restored.apply_session(&loaded);
    assert_eq!(restored.tabs.tabs[0].left_pane_width, split(64, 120));
    assert_eq!(restored.tabs.tabs[1].left_pane_width, None);
    restored.tabs.active_index = 0;
    assert_eq!(drawn(&restored), (85, 75));
    restored.tabs.active_index = 1;
    assert_eq!(drawn(&restored), (80, 80));
}

#[test]
fn session_tab_without_the_field_restores_the_even_split() {
    let json = r#"{
        "tabs": [{
            "id": 0,
            "left_location": {"Local": "/a"},
            "right_location": {"Local": "/b"},
            "left_cursor": 0,
            "right_cursor": 0
        }],
        "active_tab_index": 0,
        "active_pane": "Left",
        "marked_locations": []
    }"#;
    let session: crate::session::SessionState = serde_json::from_str(json).unwrap();
    assert_eq!(session.tabs[0].left_pane_width, None);
    assert_eq!(
        crate::session::restore_tabs(&session)[0].left_pane_width,
        None
    );
}
