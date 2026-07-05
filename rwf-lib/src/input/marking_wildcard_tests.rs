//! Tests for Phase 1.3 — Wildcard Marking Dialog

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action, KeyBindings};
    use crate::model::{FileEntry, Location};
    use crate::state::{update_state, Transition};
    use crate::test_utils::test_state;
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
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    // ---- Key binding --------------------------------------------------------

    #[test]
    fn test_wildcard_marking_key_star() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::WildcardMarking));
    }

    // ---- Dialog opening ----------------------------------------------------

    #[test]
    fn test_wildcard_marking_opens_wildcard_mark_dialog() {
        let state = test_state();
        let transitions = action_to_transitions(&state, &Action::WildcardMarking);
        assert!(
            transitions.iter().any(|t| matches!(
                t,
                Transition::ShowDialog { dialog }
                    if matches!(dialog.content, crate::model::dialog::DialogContent::WildcardMark { .. })
            )),
            "WildcardMarking action must open a WildcardMark dialog"
        );
    }

    #[test]
    fn test_wildcard_mark_dialog_title() {
        let state = test_state();
        let transitions = action_to_transitions(&state, &Action::WildcardMarking);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            assert_eq!(dialog.title, "Wildcard Marking");
        } else {
            panic!("Expected ShowDialog transition");
        }
    }

    #[test]
    fn test_wildcard_mark_dialog_starts_empty() {
        let state = test_state();
        let transitions = action_to_transitions(&state, &Action::WildcardMarking);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            if let crate::model::dialog::DialogContent::WildcardMark {
                input,
                focused_field,
                ..
            } = &dialog.content
            {
                assert_eq!(input, "");
                assert_eq!(*focused_field, 0, "textbox should be focused by default");
            } else {
                panic!("Expected WildcardMark content");
            }
        }
    }

    // ---- MarkPattern transition ---------------------------------------------

    #[test]
    fn test_mark_pattern_marks_matching_files() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![
            make_entry("readme.txt", false),
            make_entry("main.rs", false),
            make_entry("lib.rs", false),
        ];

        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "*.rs".to_string(),
            },
        );

        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/main.rs"))));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/lib.rs"))));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/readme.txt"))));
    }

    #[test]
    fn test_mark_pattern_star_marks_all() {
        let mut state = test_state();
        state.active_pane_mut().entries =
            vec![make_entry("a.txt", false), make_entry("b.rs", false)];

        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "*".to_string(),
            },
        );

        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
    }

    #[test]
    fn test_mark_pattern_question_mark() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![
            make_entry("a1.txt", false),
            make_entry("ab.txt", false),
            make_entry("a.txt", false),   // zero chars for ? — no match
            make_entry("abc.txt", false), // two chars for ? — no match
        ];

        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "a?.txt".to_string(),
            },
        );

        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/a1.txt"))));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/ab.txt"))));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/a.txt"))));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/abc.txt"))));
    }

    #[test]
    fn test_mark_pattern_accumulates_with_existing_marks() {
        let mut state = test_state();
        state.active_pane_mut().entries =
            vec![make_entry("a.txt", false), make_entry("b.rs", false)];

        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "*.txt".to_string(),
            },
        );
        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "*.rs".to_string(),
            },
        );

        assert_eq!(
            state.current_tab_mut().left_pane.marking.count(),
            2,
            "second mark_pattern must not clear first marks"
        );
    }

    #[test]
    fn test_mark_pattern_dirs_can_be_marked() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![
            make_entry("src", true),
            make_entry("docs", true),
            make_entry("main.rs", false),
        ];

        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "src".to_string(),
            },
        );

        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/src"))));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/docs"))));
    }
}
