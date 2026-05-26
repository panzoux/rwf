//! Integration tests for file mask (filter) functionality

#[cfg(test)]
mod tests {
    use crate::input::{KeyBindings, Action, action_to_transitions};
    use crate::model::{ActivePane, FileEntry, Location};
    use crate::state::{AppState, AppConfig, Transition, update_state};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{}", name))),
            size: 100,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }
    }

    fn default_state_with_entries(entries: Vec<FileEntry>) -> AppState {
        let mut state = AppState::new(AppConfig::default());
        let tab = state.current_tab_mut();
        tab.left_pane.entries = entries;
        tab.left_pane.current_location = Location::Local(PathBuf::from("/test"));
        state
    }

    // ---- apply_filter -------------------------------------------------------

    #[test]
    fn test_apply_filter_txt_only() {
        let entries = vec![
            make_entry("a.txt", false),
            make_entry("b.rs", false),
            make_entry("c.txt", false),
            make_entry("dir", true),
        ];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        tab.left_pane.apply_filter("*.txt");
        let names: Vec<_> = tab.left_pane.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"), "a.txt should be visible");
        assert!(names.contains(&"c.txt"), "c.txt should be visible");
        assert!(names.contains(&"dir"),   "directories always shown");
        assert!(!names.contains(&"b.rs"), "b.rs should be filtered out");
    }

    #[test]
    fn test_apply_filter_question_mark_wildcard() {
        // '?' matches exactly one character
        let entries = vec![
            make_entry("a1.txt", false),   // one char before .txt → matches
            make_entry("ab.txt", false),   // one char before .txt → matches
            make_entry("a.txt", false),    // zero chars before .txt → no match
            make_entry("abc.txt", false),  // two chars before .txt → no match
        ];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        tab.left_pane.apply_filter("a?.txt");
        let names: Vec<_> = tab.left_pane.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a1.txt"),  "a1.txt matches a?.txt");
        assert!(names.contains(&"ab.txt"),  "ab.txt matches a?.txt");
        assert!(!names.contains(&"a.txt"),  "a.txt does NOT match a?.txt (zero chars for ?)");
        assert!(!names.contains(&"abc.txt"), "abc.txt does NOT match a?.txt (two chars for ?)");
    }

    #[test]
    fn test_apply_filter_empty_shows_all() {
        let entries = vec![make_entry("a.txt", false), make_entry("b.rs", false)];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        tab.left_pane.apply_filter("");
        assert_eq!(tab.left_pane.entries.len(), 2);
    }

    #[test]
    fn test_apply_filter_star_shows_all() {
        let entries = vec![make_entry("a.txt", false), make_entry("b.rs", false)];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        tab.left_pane.apply_filter("*");
        assert_eq!(tab.left_pane.entries.len(), 2);
    }

    #[test]
    fn test_apply_filter_dirs_always_shown() {
        let entries = vec![
            make_entry("docs", true),
            make_entry("src", true),
            make_entry("main.py", false),
        ];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        tab.left_pane.apply_filter("*.rs");
        let names: Vec<_> = tab.left_pane.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"docs"), "dirs always shown");
        assert!(names.contains(&"src"),  "dirs always shown");
        assert!(!names.contains(&"main.py"), "non-matching file filtered");
    }

    // ---- SetFileMask transition ---------------------------------------------

    #[test]
    fn test_set_file_mask_transition_stores_mask() {
        let mut state = AppState::new(AppConfig::default());
        update_state(&mut state, Transition::SetFileMask {
            pane: ActivePane::Left,
            mask: Some("*.txt".to_string()),
        });
        let mask = &state.current_tab().left_pane.file_mask;
        assert_eq!(mask.as_deref(), Some("*.txt"));
    }

    #[test]
    fn test_clear_file_mask_transition() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.file_mask = Some("*.txt".to_string());
        update_state(&mut state, Transition::SetFileMask {
            pane: ActivePane::Left,
            mask: None,
        });
        assert!(state.current_tab().left_pane.file_mask.is_none());
    }

    // ---- Action binding -----------------------------------------------------

    #[test]
    fn test_open_file_mask_action_mapped() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::FileMaskFilter));
    }

    #[test]
    fn test_file_mask_action_opens_dialog() {
        let state = AppState::new(AppConfig::default());
        let transitions = action_to_transitions(&state, &Action::FileMaskFilter);
        assert!(
            transitions.iter().any(|t| matches!(t, Transition::ShowDialog { dialog }
                if matches!(dialog.content, crate::model::dialog::DialogContent::FileMask { .. })
            )),
            "FileMaskFilter action must open a FileMask dialog"
        );
    }

    // ---- apply_current_filter -----------------------------------------------

    #[test]
    fn test_apply_current_filter_uses_stored_mask() {
        let entries = vec![
            make_entry("readme.md", false),
            make_entry("main.rs", false),
        ];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        tab.left_pane.file_mask = Some("*.rs".to_string());
        tab.left_pane.apply_current_filter();
        let names: Vec<_> = tab.left_pane.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"main.rs"));
        assert!(!names.contains(&"readme.md"));
    }

    #[test]
    fn test_apply_current_filter_noop_when_no_mask() {
        let entries = vec![
            make_entry("readme.md", false),
            make_entry("main.rs", false),
        ];
        let mut state = default_state_with_entries(entries);
        let tab = state.current_tab_mut();
        // file_mask is None by default
        tab.left_pane.apply_current_filter();
        assert_eq!(tab.left_pane.entries.len(), 2, "no mask → all entries remain");
    }
}
