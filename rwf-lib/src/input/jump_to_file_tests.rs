//! Tests for Phase 2.2 — Jump to File Dialog

#[cfg(test)]
mod tests {
    use crate::input::{KeyBindings, Action};
    use crate::model::dialog::{DialogContent, filter_jump_to_file_suggestions};
    use crate::state::{AppState, AppConfig, Transition, update_state};

    fn open_dialog(state: &mut AppState) {
        update_state(state, Transition::ShowJumpToFileDialog);
    }

    // ---- Key bindings -------------------------------------------------------

    #[test]
    fn test_key_n_opens_jump_to_file_dialog() {
        let mut bindings = KeyBindings::default();
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('N'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowJumpToFileDialog));
    }

    // ---- Dialog opens -------------------------------------------------------

    #[test]
    fn test_dialog_opens() {
        let mut state = AppState::new(AppConfig::default());
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(matches!(dialog.content, DialogContent::JumpToFile { .. }));
    }

    #[test]
    fn test_dialog_title() {
        let mut state = AppState::new(AppConfig::default());
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert_eq!(dialog.title, "Jump to File");
    }

    // ---- Initial state ------------------------------------------------------

    #[test]
    fn test_initial_query_is_empty() {
        let mut state = AppState::new(AppConfig::default());
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToFile { query, .. } = &dialog.content {
            assert_eq!(query, "", "initial query must be empty");
        } else {
            panic!("expected JumpToFile dialog");
        }
    }

    #[test]
    fn test_initial_selected_index_is_zero() {
        let mut state = AppState::new(AppConfig::default());
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToFile { selected_index, .. } = &dialog.content {
            assert_eq!(*selected_index, 0);
        } else {
            panic!("expected JumpToFile dialog");
        }
    }

    #[test]
    fn test_initial_suggestions_equal_candidates() {
        let mut state = AppState::new(AppConfig::default());
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToFile { candidates, suggestions, .. } = &dialog.content {
            assert_eq!(candidates.len(), suggestions.len(),
                "initial suggestions must equal candidates (no filter applied yet)");
        } else {
            panic!("expected JumpToFile dialog");
        }
    }

    #[test]
    fn test_search_root_is_set() {
        let mut state = AppState::new(AppConfig::default());
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToFile { search_root, .. } = &dialog.content {
            assert!(!search_root.is_empty(), "search_root must not be empty");
        } else {
            panic!("expected JumpToFile dialog");
        }
    }

    // ---- AND-filter logic ---------------------------------------------------

    #[test]
    fn test_filter_empty_query_returns_all() {
        let candidates = vec![
            "/home/user/main.rs".to_string(),
            "/home/user/lib.rs".to_string(),
            "/home/user/notes.txt".to_string(),
        ];
        let result = filter_jump_to_file_suggestions(&candidates, "");
        assert_eq!(result.len(), 3, "empty query returns all candidates");
    }

    #[test]
    fn test_filter_single_token_matches_filename() {
        let candidates = vec![
            "/home/user/main.rs".to_string(),
            "/home/user/lib.rs".to_string(),
            "/home/user/notes.txt".to_string(),
        ];
        let result = filter_jump_to_file_suggestions(&candidates, "main");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "/home/user/main.rs");
    }

    #[test]
    fn test_filter_single_token_matches_extension() {
        let candidates = vec![
            "/home/user/main.rs".to_string(),
            "/home/user/lib.rs".to_string(),
            "/home/user/notes.txt".to_string(),
        ];
        let result = filter_jump_to_file_suggestions(&candidates, ".rs");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_two_tokens_and_semantics() {
        let candidates = vec![
            "/home/user/projects/main.rs".to_string(),
            "/home/user/projects/lib.rs".to_string(),
            "/tmp/main.txt".to_string(),
        ];
        // Both "projects" AND "main" must match
        let result = filter_jump_to_file_suggestions(&candidates, "projects main");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "/home/user/projects/main.rs");
    }

    #[test]
    fn test_filter_no_match_returns_empty() {
        let candidates = vec![
            "/home/user/main.rs".to_string(),
            "/var/log/syslog".to_string(),
        ];
        let result = filter_jump_to_file_suggestions(&candidates, "xyzzy");
        assert!(result.is_empty(), "non-matching query returns empty list");
    }

    #[test]
    fn test_filter_case_insensitive() {
        let candidates = vec![
            "/home/user/README.md".to_string(),
            "/tmp/notes".to_string(),
        ];
        let result = filter_jump_to_file_suggestions(&candidates, "readme");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "/home/user/README.md");
    }

    #[test]
    fn test_filter_whitespace_only_returns_all() {
        let candidates = vec![
            "/home/user/a.txt".to_string(),
            "/tmp/b.rs".to_string(),
        ];
        let result = filter_jump_to_file_suggestions(&candidates, "   ");
        assert_eq!(result.len(), 2, "whitespace-only query returns all candidates");
    }
}
