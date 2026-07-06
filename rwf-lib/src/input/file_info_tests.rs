//! Tests for Phase 1.8 — File Information Dialog

#[cfg(test)]
mod tests {
    use crate::input::{Action, KeyBindings};
    use crate::model::FileEntry;
    use crate::state::{update_state, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::SystemTime;

    fn make_entry(name: &str, size: u64, is_dir: bool) -> FileEntry {
        FileEntryBuilder::new(name)
            .size(size)
            .dir(is_dir)
            .modified(SystemTime::UNIX_EPOCH)
            .build()
    }

    // ---- Key binding -------------------------------------------------------

    #[test]
    fn test_file_info_key_i() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowFileInfoForCursor));
    }

    // ---- Dialog opens with file selected -----------------------------------

    #[test]
    fn test_file_info_dialog_opens_with_entry() {
        let mut state = test_state();
        let entry = make_entry("readme.txt", 1024, false);
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        update_state(&mut state, Transition::ShowFileInfo);

        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(
            matches!(
                dialog.content,
                crate::model::dialog::DialogContent::FileInfo { .. }
            ),
            "must be FileInfo dialog"
        );
    }

    #[test]
    fn test_file_info_dialog_title() {
        let mut state = test_state();
        let entry = make_entry("notes.md", 512, false);
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        update_state(&mut state, Transition::ShowFileInfo);

        let dialog = state.dialogs.current().expect("dialog must be open");
        assert_eq!(dialog.title, "File Information");
    }

    #[test]
    fn test_file_info_dialog_contains_filename() {
        let mut state = test_state();
        let entry = make_entry("document.pdf", 99999, false);
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        update_state(&mut state, Transition::ShowFileInfo);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let crate::model::dialog::DialogContent::FileInfo(
            crate::model::dialog::FileInfoDialog {
                file_name,
                size,
                is_dir,
                ..
            },
        ) = &dialog.content
        {
            assert_eq!(file_name, "document.pdf");
            assert_eq!(*size, 99999);
            assert!(!is_dir);
        } else {
            panic!("Expected FileInfo content");
        }
    }

    #[test]
    fn test_file_info_dialog_directory_entry() {
        let mut state = test_state();
        let entry = make_entry("src", 0, true);
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        update_state(&mut state, Transition::ShowFileInfo);

        let dialog = state.dialogs.current().expect("dialog must be open");
        if let crate::model::dialog::DialogContent::FileInfo(
            crate::model::dialog::FileInfoDialog { is_dir, .. },
        ) = &dialog.content
        {
            assert!(*is_dir, "directory entry must set is_dir=true");
        }
    }

    // ---- Empty pane: no dialog ---------------------------------------------

    #[test]
    fn test_file_info_no_dialog_on_empty_pane() {
        let state = test_state();
        // No entries in pane
        assert!(state.dialogs.current().is_none());

        let mut state = state;
        update_state(&mut state, Transition::ShowFileInfo);

        assert!(
            state.dialogs.current().is_none(),
            "no dialog should open when pane is empty"
        );
    }
}
