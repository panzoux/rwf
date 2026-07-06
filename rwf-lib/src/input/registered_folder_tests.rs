//! Tests for Phase 1.12 — Registered Folder Selector Dialog

#[cfg(test)]
mod tests {
    use crate::input::{Action, KeyBindings};
    use crate::model::dialog::{DialogContent, RegisteredFolder, RegisteredFolderSelectorContent};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::test_state;

    fn add_folder(state: &mut AppState, name: &str, path: &str) {
        state
            .registered_folders
            .add(RegisteredFolder::new(name.to_string(), path.to_string()));
    }

    fn open_dialog(state: &mut AppState) {
        update_state(state, Transition::ShowRegisteredFolderDialog);
    }

    // ---- Key bindings -------------------------------------------------------

    #[test]
    fn test_key_b_opens_registered_folder_dialog() {
        let mut bindings = KeyBindings::default();
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowRegisteredFolderDialog));
    }

    // ---- Dialog opens -------------------------------------------------------

    #[test]
    fn test_dialog_opens() {
        let mut state = test_state();
        add_folder(&mut state, "Home", "/home/user");
        open_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(matches!(
            dialog.content,
            DialogContent::RegisteredFolderSelector(_)
        ));
    }

    #[test]
    fn test_dialog_opens_with_empty_folders() {
        let mut state = test_state();
        open_dialog(&mut state);
        assert!(
            state.dialogs.current().is_some(),
            "dialog opens even with no folders"
        );
    }

    // ---- Initial state ------------------------------------------------------

    #[test]
    fn test_initial_selected_index_is_zero() {
        let mut state = test_state();
        add_folder(&mut state, "Work", "/work");
        add_folder(&mut state, "Home", "/home");
        open_dialog(&mut state);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
            selected_index,
            ..
        }) = &dialog.content
        {
            assert_eq!(*selected_index, 0);
        } else {
            panic!("Expected RegisteredFolderSelector");
        }
    }

    #[test]
    fn test_initial_filter_is_empty() {
        let mut state = test_state();
        add_folder(&mut state, "Home", "/home");
        open_dialog(&mut state);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
            filter,
            ..
        }) = &dialog.content
        {
            assert_eq!(filter, "", "filter must start empty");
        } else {
            panic!("Expected RegisteredFolderSelector");
        }
    }

    #[test]
    fn test_folders_passed_to_dialog() {
        let mut state = test_state();
        let initial_count = state.registered_folders.folders.len();
        add_folder(&mut state, "Alpha", "/alpha");
        add_folder(&mut state, "Beta", "/beta");
        add_folder(&mut state, "Gamma", "/gamma");
        open_dialog(&mut state);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
            folders,
            ..
        }) = &dialog.content
        {
            assert_eq!(folders.len(), initial_count + 3);
        } else {
            panic!("Expected RegisteredFolderSelector");
        }
    }

    // ---- Register current folder -------------------------------------------

    #[test]
    fn test_register_folder_adds_entry() {
        let mut state = test_state();
        let initial_count = state.registered_folders.folders.len();
        let path = state.active_pane().current_location.display_path();
        update_state(
            &mut state,
            Transition::RegisterCurrentFolder {
                name: "MyFolder".to_string(),
                path,
            },
        );
        assert_eq!(state.registered_folders.folders.len(), initial_count + 1);
        let last = state.registered_folders.folders.last().unwrap();
        assert_eq!(last.name, "MyFolder");
    }
}
