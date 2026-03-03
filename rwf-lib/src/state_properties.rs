//! Property-based tests for state transitions
//!
//! **Validates: Requirements 26.4, 26.9**

use crate::state::{AppState, AppConfig, Transition, update_state, StateUpdateResult, HistoryDirection};
use crate::model::{ActivePane, Location, SortMode, DisplayMode, FileEntry};
use crate::job::JobKind;
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;

/// Strategy for generating simple Location values
fn arb_location() -> impl Strategy<Value = Location> {
    prop_oneof![
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/tmp/{}", s)))),
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/home/{}", s)))),
    ]
}

/// Strategy for generating ActivePane
fn arb_active_pane() -> impl Strategy<Value = ActivePane> {
    prop_oneof![
        Just(ActivePane::Left),
        Just(ActivePane::Right),
    ]
}

/// Strategy for generating SortMode
fn arb_sort_mode() -> impl Strategy<Value = SortMode> {
    prop_oneof![
        Just(SortMode::Name),
        Just(SortMode::Size),
        Just(SortMode::Date),
        Just(SortMode::Extension),
    ]
}

/// Strategy for generating DisplayMode
fn arb_display_mode() -> impl Strategy<Value = DisplayMode> {
    prop_oneof![
        (1u8..=8).prop_map(DisplayMode::Columns),
        Just(DisplayMode::Detailed),
    ]
}

/// Strategy for generating HistoryDirection
fn arb_history_direction() -> impl Strategy<Value = HistoryDirection> {
    prop_oneof![
        Just(HistoryDirection::Back),
        Just(HistoryDirection::Forward),
    ]
}

/// Strategy for generating Transition values
/// We focus on deterministic transitions (no I/O, no randomness)
/// We also avoid transitions that could panic (like CloseTab with invalid index)
fn arb_transition() -> impl Strategy<Value = Transition> {
    prop_oneof![
        // Navigation transitions
        (arb_active_pane(), -10isize..10isize).prop_map(|(pane, delta)| Transition::CursorMove { pane, delta }),
        (arb_active_pane(), 0usize..100).prop_map(|(pane, position)| Transition::CursorJump { pane, position }),
        Just(Transition::SwitchPane),
        (arb_active_pane()).prop_map(|pane| Transition::NavigateUp { pane }),
        (arb_active_pane(), arb_history_direction()).prop_map(|(pane, direction)| Transition::NavigateHistory { pane, direction }),
        
        // Tab management (avoid CloseTab and SwitchTab with potentially invalid indices)
        Just(Transition::CreateTab),
        Just(Transition::NextTab),
        Just(Transition::PrevTab),
        
        // Marking operations
        arb_location().prop_map(|location| Transition::ToggleMark { location }),
        Just(Transition::MarkAll),
        Just(Transition::UnmarkAll),
        
        // View operations
        (arb_active_pane(), arb_sort_mode()).prop_map(|(pane, mode)| Transition::ChangeSortMode { pane, mode }),
        (arb_active_pane(), arb_display_mode()).prop_map(|(pane, mode)| Transition::ChangeDisplayMode { pane, mode }),
        (arb_active_pane(), prop::option::of("[a-z*?]{1,10}")).prop_map(|(pane, mask)| Transition::SetFileMask { pane, mask }),
    ]
}

/// Create a snapshot of AppState for comparison
/// We only capture the parts that should be deterministic
#[derive(Debug, Clone, PartialEq)]
struct StateSnapshot {
    active_tab_index: usize,
    tab_count: usize,
    active_pane: ActivePane,
    left_cursor: usize,
    left_scroll: usize,
    left_location: String,
    left_sort_mode: SortMode,
    left_display_mode: DisplayMode,
    left_file_mask: Option<String>,
    right_cursor: usize,
    right_scroll: usize,
    right_location: String,
    right_sort_mode: SortMode,
    right_display_mode: DisplayMode,
    right_file_mask: Option<String>,
    marked_count: usize,
    job_queue_len: usize,
    job_active_len: usize,
}

impl StateSnapshot {
    fn from_state(state: &AppState) -> Self {
        let tab = state.current_tab();
        Self {
            active_tab_index: state.tabs.active_index,
            tab_count: state.tabs.tabs.len(),
            active_pane: state.ui.active_pane,
            left_cursor: tab.left_pane.cursor,
            left_scroll: tab.left_pane.scroll_offset,
            left_location: tab.left_pane.current_location.display_path(),
            left_sort_mode: tab.left_pane.sort_mode,
            left_display_mode: tab.left_pane.display_mode,
            left_file_mask: tab.left_pane.file_mask.clone(),
            right_cursor: tab.right_pane.cursor,
            right_scroll: tab.right_pane.scroll_offset,
            right_location: tab.right_pane.current_location.display_path(),
            right_sort_mode: tab.right_pane.sort_mode,
            right_display_mode: tab.right_pane.display_mode,
            right_file_mask: tab.right_pane.file_mask.clone(),
            marked_count: state.marking.marked_locations.len(),
            job_queue_len: state.jobs.queue.len(),
            job_active_len: state.jobs.active.len(),
        }
    }
}

/// Compare StateUpdateResult for equality
/// We focus on the deterministic parts
fn results_equivalent(r1: &StateUpdateResult, r2: &StateUpdateResult) -> bool {
    // Check if both have the same number of jobs to start
    if r1.jobs_to_start.len() != r2.jobs_to_start.len() {
        return false;
    }
    
    // Check if both have the same number of jobs to cancel
    if r1.jobs_to_cancel.len() != r2.jobs_to_cancel.len() {
        return false;
    }
    
    // Check if both have the same number of panes to refresh
    if r1.panes_to_refresh.len() != r2.panes_to_refresh.len() {
        return false;
    }
    
    // For jobs, we compare the JobKind (not the ID or cancel token which may differ)
    for (job1, job2) in r1.jobs_to_start.iter().zip(r2.jobs_to_start.iter()) {
        if !job_kinds_equivalent(&job1.kind, &job2.kind) {
            return false;
        }
    }
    
    // For panes to refresh, compare tab_id and pane
    for (refresh1, refresh2) in r1.panes_to_refresh.iter().zip(r2.panes_to_refresh.iter()) {
        if refresh1.tab_id != refresh2.tab_id || refresh1.pane != refresh2.pane {
            return false;
        }
    }
    
    true
}

/// Compare JobKind for equivalence (ignoring non-deterministic parts)
fn job_kinds_equivalent(k1: &JobKind, k2: &JobKind) -> bool {
    match (k1, k2) {
        (JobKind::ReadDirectory { location: l1 }, JobKind::ReadDirectory { location: l2 }) => {
            l1.display_path() == l2.display_path()
        }
        (JobKind::Copy { sources: s1, dest: d1 }, JobKind::Copy { sources: s2, dest: d2 }) => {
            s1.len() == s2.len() && 
            s1.iter().zip(s2.iter()).all(|(a, b)| a.display_path() == b.display_path()) &&
            d1.display_path() == d2.display_path()
        }
        (JobKind::Move { sources: s1, dest: d1 }, JobKind::Move { sources: s2, dest: d2 }) => {
            s1.len() == s2.len() && 
            s1.iter().zip(s2.iter()).all(|(a, b)| a.display_path() == b.display_path()) &&
            d1.display_path() == d2.display_path()
        }
        (JobKind::Delete { targets: t1 }, JobKind::Delete { targets: t2 }) => {
            t1.len() == t2.len() && 
            t1.iter().zip(t2.iter()).all(|(a, b)| a.display_path() == b.display_path())
        }
        (JobKind::Mkdir { location: l1 }, JobKind::Mkdir { location: l2 }) => {
            l1.display_path() == l2.display_path()
        }
        (JobKind::Rename { from: f1, to: t1 }, JobKind::Rename { from: f2, to: t2 }) => {
            f1.display_path() == f2.display_path() && t1.display_path() == t2.display_path()
        }
        (JobKind::CalculateSize { location: l1 }, JobKind::CalculateSize { location: l2 }) => {
            l1.display_path() == l2.display_path()
        }
        _ => false,
    }
}

proptest! {
    /// **Property 22: State Transition Determinism**
    ///
    /// For any AppState and Transition, applying the same transition to the same state
    /// should always produce the same resulting state and StateUpdateResult.
    ///
    /// This test verifies that:
    /// 1. The update_state function is pure (no side effects)
    /// 2. The same input always produces the same output
    /// 3. No randomness or non-deterministic behavior exists in state transitions
    /// 4. The function is referentially transparent
    ///
    /// **Validates: Requirements 26.4, 26.9**
    #[test]
    fn prop_state_transition_determinism(
        transition in arb_transition(),
        seed in 0u64..1000
    ) {
        // Create an initial state with some data
        let config = AppConfig::default();
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config);
        
        // Add some entries to both states to make transitions more interesting
        // Use the seed to create deterministic but varied initial states
        let num_entries = (seed % 10) + 1;
        for i in 0..num_entries {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100 * i,
                is_dir: i % 3 == 0,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            };
            state1.current_tab_mut().left_pane.entries.push(entry.clone());
            state1.current_tab_mut().right_pane.entries.push(entry.clone());
            state2.current_tab_mut().left_pane.entries.push(entry.clone());
            state2.current_tab_mut().right_pane.entries.push(entry);
        }
        
        // Create additional tabs based on seed
        let num_tabs = (seed % 3) + 1;
        for _ in 1..num_tabs {
            state1.tabs.create_tab();
            state2.tabs.create_tab();
        }
        
        // Take snapshots before transition
        let snapshot1_before = StateSnapshot::from_state(&state1);
        let snapshot2_before = StateSnapshot::from_state(&state2);
        
        // Verify initial states are identical
        prop_assert_eq!(
            snapshot1_before,
            snapshot2_before,
            "Initial states should be identical"
        );
        
        // Apply the same transition to both states
        let result1 = update_state(&mut state1, transition.clone());
        let result2 = update_state(&mut state2, transition.clone());
        
        // Take snapshots after transition
        let snapshot1_after = StateSnapshot::from_state(&state1);
        let snapshot2_after = StateSnapshot::from_state(&state2);
        
        // Verify resulting states are identical
        prop_assert_eq!(
            snapshot1_after,
            snapshot2_after,
            "States should be identical after applying the same transition"
        );
        
        // Verify results are equivalent
        prop_assert!(
            results_equivalent(&result1, &result2),
            "StateUpdateResults should be equivalent for the same transition"
        );
    }

    /// **Property 22: State Transition Determinism (Multiple Applications)**
    ///
    /// Applying the same transition multiple times to the same initial state
    /// should always produce the same result.
    ///
    /// **Validates: Requirements 26.4, 26.9**
    #[test]
    fn prop_state_transition_determinism_repeated(
        transition in arb_transition(),
        seed in 0u64..1000
    ) {
        let config = AppConfig::default();
        
        // Create three identical initial states
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config.clone());
        let mut state3 = AppState::new(config);
        
        // Add entries based on seed
        let num_entries = (seed % 10) + 1;
        for i in 0..num_entries {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100 * i,
                is_dir: i % 3 == 0,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            };
            state1.current_tab_mut().left_pane.entries.push(entry.clone());
            state1.current_tab_mut().right_pane.entries.push(entry.clone());
            state2.current_tab_mut().left_pane.entries.push(entry.clone());
            state2.current_tab_mut().right_pane.entries.push(entry.clone());
            state3.current_tab_mut().left_pane.entries.push(entry.clone());
            state3.current_tab_mut().right_pane.entries.push(entry);
        }
        
        // Apply the same transition to all three states
        let result1 = update_state(&mut state1, transition.clone());
        let result2 = update_state(&mut state2, transition.clone());
        let result3 = update_state(&mut state3, transition.clone());
        
        // Take snapshots
        let snapshot1 = StateSnapshot::from_state(&state1);
        let snapshot2 = StateSnapshot::from_state(&state2);
        let snapshot3 = StateSnapshot::from_state(&state3);
        
        // All three should be identical
        prop_assert_eq!(&snapshot1, &snapshot2, "First and second applications should produce identical states");
        prop_assert_eq!(&snapshot2, &snapshot3, "Second and third applications should produce identical states");
        
        // All three results should be equivalent
        prop_assert!(results_equivalent(&result1, &result2), "First and second results should be equivalent");
        prop_assert!(results_equivalent(&result2, &result3), "Second and third results should be equivalent");
    }

    /// **Property 22: State Transition Determinism (Sequence)**
    ///
    /// Applying a sequence of transitions should be deterministic.
    ///
    /// **Validates: Requirements 26.4, 26.9**
    #[test]
    fn prop_state_transition_sequence_determinism(
        transitions in prop::collection::vec(arb_transition(), 1..10),
        seed in 0u64..1000
    ) {
        let config = AppConfig::default();
        
        // Create two identical initial states
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config);
        
        // Add entries based on seed
        let num_entries = (seed % 10) + 1;
        for i in 0..num_entries {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100 * i,
                is_dir: i % 3 == 0,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            };
            state1.current_tab_mut().left_pane.entries.push(entry.clone());
            state1.current_tab_mut().right_pane.entries.push(entry.clone());
            state2.current_tab_mut().left_pane.entries.push(entry.clone());
            state2.current_tab_mut().right_pane.entries.push(entry);
        }
        
        // Apply the same sequence of transitions to both states
        for transition in &transitions {
            update_state(&mut state1, transition.clone());
            update_state(&mut state2, transition.clone());
        }
        
        // Take snapshots
        let snapshot1 = StateSnapshot::from_state(&state1);
        let snapshot2 = StateSnapshot::from_state(&state2);
        
        // Both should be identical
        prop_assert_eq!(
            snapshot1,
            snapshot2,
            "States should be identical after applying the same sequence of transitions"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_determinism_switch_pane() {
        let config = AppConfig::default();
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config);
        
        // Both start with Left pane active
        assert_eq!(state1.ui.active_pane, ActivePane::Left);
        assert_eq!(state2.ui.active_pane, ActivePane::Left);
        
        // Apply SwitchPane to both
        update_state(&mut state1, Transition::SwitchPane);
        update_state(&mut state2, Transition::SwitchPane);
        
        // Both should now have Right pane active
        assert_eq!(state1.ui.active_pane, ActivePane::Right);
        assert_eq!(state2.ui.active_pane, ActivePane::Right);
    }

    #[test]
    fn test_determinism_cursor_move() {
        let config = AppConfig::default();
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config);
        
        // Add entries to both states
        for i in 0..10 {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            };
            state1.current_tab_mut().left_pane.entries.push(entry.clone());
            state2.current_tab_mut().left_pane.entries.push(entry);
        }
        
        // Apply cursor move to both
        update_state(&mut state1, Transition::CursorMove { pane: ActivePane::Left, delta: 3 });
        update_state(&mut state2, Transition::CursorMove { pane: ActivePane::Left, delta: 3 });
        
        // Both should have cursor at position 3
        assert_eq!(state1.current_tab().left_pane.cursor, 3);
        assert_eq!(state2.current_tab().left_pane.cursor, 3);
    }

    #[test]
    fn test_determinism_create_tab() {
        let config = AppConfig::default();
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config);
        
        // Both start with 1 tab
        assert_eq!(state1.tabs.tabs.len(), 1);
        assert_eq!(state2.tabs.tabs.len(), 1);
        
        // Apply CreateTab to both
        update_state(&mut state1, Transition::CreateTab);
        update_state(&mut state2, Transition::CreateTab);
        
        // Both should now have 2 tabs
        assert_eq!(state1.tabs.tabs.len(), 2);
        assert_eq!(state2.tabs.tabs.len(), 2);
        
        // Both should have the new tab active
        assert_eq!(state1.tabs.active_index, 1);
        assert_eq!(state2.tabs.active_index, 1);
    }

    #[test]
    fn test_determinism_mark_all() {
        let config = AppConfig::default();
        let mut state1 = AppState::new(config.clone());
        let mut state2 = AppState::new(config);
        
        // Add entries to both states
        for i in 0..5 {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            };
            state1.current_tab_mut().left_pane.entries.push(entry.clone());
            state2.current_tab_mut().left_pane.entries.push(entry);
        }
        
        // Apply MarkAll to both
        update_state(&mut state1, Transition::MarkAll);
        update_state(&mut state2, Transition::MarkAll);
        
        // Both should have 5 marked files
        assert_eq!(state1.marking.marked_locations.len(), 5);
        assert_eq!(state2.marking.marked_locations.len(), 5);
    }
}
