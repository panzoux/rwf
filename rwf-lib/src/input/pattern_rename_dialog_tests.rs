//! Tests for Phase 1.9 — Pattern Rename Dialog (TWF-compatible: Find + Replace fields)

#[cfg(test)]
mod tests {
    use crate::input::{Action, KeyBindings};
    use crate::model::dialog::DialogContent;
    use crate::model::{FileEntry, Location};
    use crate::state::{update_state, AppConfig, AppState, Transition};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_entry(name: &str) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{}", name))),
            size: 0,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::UNIX_EPOCH,
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    fn open_pattern_rename(state: &mut AppState) {
        update_state(state, Transition::ShowPatternRenameDialog);
    }

    fn update_fields(
        state: &mut AppState,
        find: &str,
        replace: &str,
        use_regex: bool,
        case_sensitive: bool,
    ) {
        update_state(
            state,
            Transition::UpdatePatternRenameFields {
                find: find.to_string(),
                replace: replace.to_string(),
                use_regex,
                case_sensitive,
            },
        );
    }

    // ---- Key binding -------------------------------------------------------

    #[test]
    fn test_pattern_rename_key_r_uppercase() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::PatternRename));
    }

    // ---- Dialog opens with file selected -----------------------------------

    #[test]
    fn test_pattern_rename_dialog_opens_with_entry() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![make_entry("photo.jpg")];
        state.current_tab_mut().left_pane.cursor = 0;

        open_pattern_rename(&mut state);

        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(matches!(
            dialog.content,
            DialogContent::PatternRename { .. }
        ));
    }

    #[test]
    fn test_pattern_rename_dialog_title() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![make_entry("notes.txt")];
        state.current_tab_mut().left_pane.cursor = 0;

        open_pattern_rename(&mut state);

        let dialog = state.dialogs.current().expect("dialog must be open");
        assert_eq!(dialog.title, "Pattern Rename");
    }

    #[test]
    fn test_pattern_rename_dialog_initial_values() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![make_entry("file.txt")];
        state.current_tab_mut().left_pane.cursor = 0;

        open_pattern_rename(&mut state);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename {
            find,
            replace,
            use_regex,
            case_sensitive,
            preview,
            focused_field,
            preview_scroll,
            ..
        } = &dialog.content
        {
            assert_eq!(find, "", "initial find must be empty");
            assert_eq!(replace, "", "initial replace must be empty");
            assert!(*use_regex, "regex mode on by default (like TWF)");
            assert!(!*case_sensitive, "case insensitive by default");
            // Dialog pre-populates preview on open so the correct size is shown immediately
            assert!(!preview.is_empty(), "initial preview must be pre-populated");
            assert_eq!(*focused_field, 0, "focus starts on find field");
            assert_eq!(*preview_scroll, 0);
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }

    // ---- No dialog on empty pane ------------------------------------------

    #[test]
    fn test_pattern_rename_no_dialog_on_empty_pane() {
        let mut state = AppState::new(AppConfig::default());
        open_pattern_rename(&mut state);
        assert!(
            state.dialogs.current().is_none(),
            "no dialog should open on empty pane"
        );
    }

    // ---- Preview update (regex mode) -------------------------------------

    #[test]
    fn test_pattern_rename_update_preview_regex() {
        let mut state = AppState::new(AppConfig::default());
        let e1 = make_entry("photo001.jpg");
        let e2 = make_entry("photo002.jpg");
        let e3 = make_entry("photo003.jpg");
        let loc1 = e1.location.clone();
        let loc2 = e2.location.clone();
        let loc3 = e3.location.clone();
        state.current_tab_mut().left_pane.entries = vec![e1, e2, e3];
        state.current_tab_mut().left_pane.cursor = 0;
        state.current_tab_mut().left_pane.marking.toggle(loc1);
        state.current_tab_mut().left_pane.marking.toggle(loc2);
        state.current_tab_mut().left_pane.marking.toggle(loc3);

        open_pattern_rename(&mut state);
        // Regex: replace ".jpg" extension
        update_fields(&mut state, r"\.jpg$", ".jpeg", true, true);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename {
            find,
            replace,
            preview,
            ..
        } = &dialog.content
        {
            assert_eq!(find, r"\.jpg$");
            assert_eq!(replace, ".jpeg");
            assert_eq!(preview.len(), 3, "all 3 marked files in preview");
            for (_, new_name) in preview {
                assert!(
                    new_name.ends_with(".jpeg"),
                    "extension replaced: {}",
                    new_name
                );
            }
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }

    // ---- Preview shows transformation ------------------------------------

    #[test]
    fn test_pattern_rename_preview_shows_transformation() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![make_entry("file.txt")];
        state.current_tab_mut().left_pane.cursor = 0;

        open_pattern_rename(&mut state);
        // Regex: prepend "backup_"
        update_fields(&mut state, "^(.+)$", "backup_${1}", true, true);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename { preview, .. } = &dialog.content {
            assert_eq!(preview.len(), 1);
            let (original, renamed) = &preview[0];
            assert_eq!(original, "file.txt");
            assert_eq!(renamed, "backup_file.txt");
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }

    // ---- Marked files are used when marks exist --------------------------

    #[test]
    fn test_pattern_rename_uses_marked_files() {
        let mut state = AppState::new(AppConfig::default());
        let e1 = make_entry("doc1.txt");
        let e2 = make_entry("doc2.txt");
        let e3 = make_entry("image.jpg");
        let loc1 = e1.location.clone();
        let loc2 = e2.location.clone();
        state.current_tab_mut().left_pane.entries = vec![e1, e2, e3];
        state.current_tab_mut().left_pane.cursor = 0;
        state.current_tab_mut().left_pane.marking.toggle(loc1);
        state.current_tab_mut().left_pane.marking.toggle(loc2);

        open_pattern_rename(&mut state);
        // Plain mode: append "_copy"
        update_fields(&mut state, ".txt", "_copy.txt", false, true);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename { preview, .. } = &dialog.content {
            assert_eq!(preview.len(), 2, "only 2 marked files in preview");
            let originals: Vec<&str> = preview.iter().map(|(o, _)| o.as_str()).collect();
            assert!(originals.contains(&"doc1.txt"));
            assert!(originals.contains(&"doc2.txt"));
            assert!(!originals.contains(&"image.jpg"));
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }

    // ---- s/ command syntax (expert mode) ---------------------------------

    #[test]
    fn test_pattern_rename_s_command_syntax() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![make_entry("hello_world.txt")];
        state.current_tab_mut().left_pane.cursor = 0;

        open_pattern_rename(&mut state);
        // s/ command: global replace _ with -
        update_fields(&mut state, "s/_/-/g", "", false, true);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename { preview, .. } = &dialog.content {
            assert_eq!(preview.len(), 1);
            assert_eq!(preview[0].1, "hello-world.txt");
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }

    // ---- tr/ command syntax ----------------------------------------------

    #[test]
    fn test_pattern_rename_tr_command_syntax() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![make_entry("abc.txt")];
        state.current_tab_mut().left_pane.cursor = 0;

        open_pattern_rename(&mut state);
        update_fields(&mut state, "tr/abc/xyz/", "", false, true);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename { preview, .. } = &dialog.content {
            assert_eq!(preview.len(), 1);
            assert_eq!(preview[0].1, "xyz.txt");
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }

    // ---- Unchanged files still appear in preview (TWF behaviour) ----------

    #[test]
    fn test_pattern_rename_unchanged_files_in_preview() {
        let mut state = AppState::new(AppConfig::default());
        let e1 = make_entry("file.txt");
        let e2 = make_entry("image.jpg");
        let loc1 = e1.location.clone();
        let loc2 = e2.location.clone();
        state.current_tab_mut().left_pane.entries = vec![e1, e2];
        state.current_tab_mut().left_pane.cursor = 0;
        state.current_tab_mut().left_pane.marking.toggle(loc1);
        state.current_tab_mut().left_pane.marking.toggle(loc2);

        open_pattern_rename(&mut state);
        // Only matches .txt files; .jpg stays unchanged
        update_fields(&mut state, r"\.txt$", ".bak", true, true);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::PatternRename { preview, .. } = &dialog.content {
            // Both files appear (TWF shows all)
            assert_eq!(preview.len(), 2);
            let (txt_orig, txt_new) = preview.iter().find(|(o, _)| o == "file.txt").unwrap();
            assert_eq!(txt_orig, "file.txt");
            assert_eq!(txt_new, "file.bak");
            let (jpg_orig, jpg_new) = preview.iter().find(|(o, _)| o == "image.jpg").unwrap();
            assert_eq!(jpg_orig, jpg_new, "unchanged file should have same name");
        } else {
            panic!("Expected PatternRename dialog content");
        }
    }
}
