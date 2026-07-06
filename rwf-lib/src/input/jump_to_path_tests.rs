//! Tests for Phase 2.1 — Jump to Path Dialog

#[cfg(test)]
mod tests {
    use crate::input::{Action, KeyBindings};
    use crate::model::dialog::{filter_jump_to_path_suggestions, DialogContent};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::test_state;

    fn open_dialog(state: &mut AppState) {
        update_state(state, Transition::ShowJumpToPathDialog);
    }

    // ---- Key bindings -------------------------------------------------------

    #[test]
    fn test_key_j_opens_jump_to_path_dialog() {
        let mut bindings = KeyBindings::default();
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('J'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowJumpToPathDialog));
    }

    // ---- Dialog opens -------------------------------------------------------

    #[test]
    fn test_dialog_opens() {
        let mut state = test_state();
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(matches!(dialog.content, DialogContent::JumpToPath(_)));
    }

    #[test]
    fn test_dialog_title() {
        let mut state = test_state();
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(!dialog.title.is_empty(), "dialog title must not be empty");
        assert_eq!(dialog.title, "Jump to Directory");
    }

    // ---- Initial state ------------------------------------------------------

    #[test]
    fn test_initial_query_is_empty() {
        let mut state = test_state();
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToPath(crate::model::dialog::JumpToPathDialog { query, .. }) =
            &dialog.content
        {
            assert_eq!(query, "", "initial query must be empty");
        } else {
            panic!("expected JumpToPath dialog");
        }
    }

    #[test]
    fn test_initial_selected_index_is_zero() {
        let mut state = test_state();
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToPath(crate::model::dialog::JumpToPathDialog {
            selected_index,
            ..
        }) = &dialog.content
        {
            assert_eq!(*selected_index, 0);
        } else {
            panic!("expected JumpToPath dialog");
        }
    }

    #[test]
    fn test_initial_suggestions_equal_candidates() {
        let mut state = test_state();
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToPath(crate::model::dialog::JumpToPathDialog {
            candidates,
            suggestions,
            ..
        }) = &dialog.content
        {
            assert_eq!(
                candidates.len(),
                suggestions.len(),
                "initial suggestions must equal candidates (no filter applied yet)"
            );
        } else {
            panic!("expected JumpToPath dialog");
        }
    }

    #[test]
    fn test_search_root_is_set() {
        let mut state = test_state();
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::JumpToPath(crate::model::dialog::JumpToPathDialog {
            search_root,
            ..
        }) = &dialog.content
        {
            assert!(!search_root.is_empty(), "search_root must not be empty");
        } else {
            panic!("expected JumpToPath dialog");
        }
    }

    // ---- AND-filter logic ---------------------------------------------------

    #[test]
    fn test_filter_empty_query_returns_all() {
        let candidates = vec![
            "/home/user/projects".to_string(),
            "/home/user/downloads".to_string(),
            "/var/log".to_string(),
        ];
        let result = filter_jump_to_path_suggestions(&candidates, "");
        assert_eq!(result.len(), 3, "empty query returns all candidates");
    }

    #[test]
    fn test_filter_single_token() {
        let candidates = vec![
            "/home/user/projects".to_string(),
            "/home/user/downloads".to_string(),
            "/var/log".to_string(),
        ];
        let result = filter_jump_to_path_suggestions(&candidates, "proj");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "/home/user/projects");
    }

    #[test]
    fn test_filter_two_tokens_and_semantics() {
        let candidates = vec![
            "/home/user/projects/rust".to_string(),
            "/home/user/projects/python".to_string(),
            "/tmp/rust".to_string(),
        ];
        // Both "projects" AND "rust" must match
        let result = filter_jump_to_path_suggestions(&candidates, "projects rust");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "/home/user/projects/rust");
    }

    #[test]
    fn test_filter_no_match_returns_empty() {
        let candidates = vec!["/home/user/projects".to_string(), "/var/log".to_string()];
        let result = filter_jump_to_path_suggestions(&candidates, "xyzzy");
        assert!(result.is_empty(), "non-matching query returns empty list");
    }

    #[test]
    fn test_filter_case_insensitive() {
        let candidates = vec!["/home/user/Documents".to_string(), "/tmp/notes".to_string()];
        let result = filter_jump_to_path_suggestions(&candidates, "DOCUMENTS");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "/home/user/Documents");
    }

    #[test]
    fn test_filter_whitespace_only_returns_all() {
        let candidates = vec!["/home/user".to_string(), "/tmp".to_string()];
        let result = filter_jump_to_path_suggestions(&candidates, "   ");
        assert_eq!(
            result.len(),
            2,
            "whitespace-only query returns all candidates"
        );
    }
}
