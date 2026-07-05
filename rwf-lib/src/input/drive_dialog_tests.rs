//! Tests for Phase 1.7 — Drive Change Dialog

#[cfg(test)]
mod tests {
    use crate::input::{Action, KeyBindings};
    use crate::model::dialog::{DriveInfo, DriveType};
    use crate::model::Location;
    use crate::state::{update_state, AppConfig, AppState, Transition};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn loc(path: &str) -> Location {
        Location::Local(PathBuf::from(path))
    }

    fn make_drive(path: &str, label: &str, drive_type: DriveType) -> DriveInfo {
        DriveInfo {
            path: path.to_string(),
            label: label.to_string(),
            drive_type,
            total_space: None,
            free_space: None,
        }
    }

    // ---- Key binding -------------------------------------------------------

    #[test]
    fn test_drive_dialog_key_l() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('L'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowDriveChangeDialog));
    }

    // ---- DriveInfo::display_label() ----------------------------------------

    #[test]
    fn test_display_label_home() {
        let d = make_drive("/home/user", "~ User Directory", DriveType::Local);
        assert_eq!(d.display_label(), "~ User Directory");
    }

    #[test]
    fn test_display_label_network_share() {
        let d = make_drive("\\\\server\\share", "\\\\server\\share", DriveType::Network);
        assert_eq!(d.display_label(), "\\\\server\\share");
    }

    #[test]
    fn test_display_label_local_drive_with_label() {
        let d = make_drive("C:\\", "Windows", DriveType::Local);
        assert_eq!(d.display_label(), "C: - Windows (Local)");
    }

    #[test]
    fn test_display_label_local_drive_empty_label() {
        let d = make_drive("D:\\", "", DriveType::Local);
        assert_eq!(d.display_label(), "D: (Local)");
    }

    #[test]
    fn test_display_label_removable() {
        let d = make_drive("E:\\", "USB Drive", DriveType::Removable);
        assert_eq!(d.display_label(), "E: - USB Drive (Removable)");
    }

    fn open_drive_dialog(state: &mut AppState) {
        update_state(state, Transition::ShowDriveChangeDialog);
    }

    // ---- Dialog opens -------------------------------------------------------

    #[test]
    fn test_drive_dialog_opens() {
        let mut state = AppState::new(AppConfig::default());
        open_drive_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(
            matches!(
                dialog.content,
                crate::model::dialog::DialogContent::DriveSelection { .. }
            ),
            "must be DriveSelection dialog"
        );
    }

    #[test]
    fn test_drive_dialog_title() {
        let mut state = AppState::new(AppConfig::default());
        open_drive_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(
            dialog.title.starts_with("Select Drive"),
            "title must start with 'Select Drive', got: {}",
            dialog.title
        );
    }

    #[test]
    fn test_drive_dialog_contains_home_entry_first() {
        let mut state = AppState::new(AppConfig::default());
        open_drive_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let crate::model::dialog::DialogContent::DriveSelection { drives, .. } = &dialog.content
        {
            assert!(!drives.is_empty(), "drive list must not be empty");
            assert_eq!(
                drives[0].label, "~ User Directory",
                "first entry must be home"
            );
        } else {
            panic!("Expected DriveSelection content");
        }
    }

    // ---- Network shares from history ----------------------------------------

    #[test]
    fn test_drive_dialog_includes_network_share_from_history() {
        let mut state = AppState::new(AppConfig::default());
        // Navigate to a network path so it ends up in history
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("\\\\fileserver\\data\\project"),
            },
        );
        // Navigate away so the NW path goes into history stack
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("C:\\"),
            },
        );

        open_drive_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let crate::model::dialog::DialogContent::DriveSelection { drives, .. } = &dialog.content
        {
            let paths: Vec<&str> = drives.iter().map(|d| d.path.as_str()).collect();
            assert!(
                paths
                    .iter()
                    .any(|p| p.contains("fileserver") && p.contains("data")),
                "history NW share root must appear; got: {:?}",
                paths
            );
        } else {
            panic!("Expected DriveSelection content");
        }
    }

    // ---- Filter (DriveSelection model) --------------------------------------

    #[test]
    fn test_drive_selection_filter_initial_empty() {
        let mut state = AppState::new(AppConfig::default());
        open_drive_dialog(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let crate::model::dialog::DialogContent::DriveSelection { filter, .. } = &dialog.content
        {
            assert_eq!(filter, "", "initial filter must be empty");
        } else {
            panic!("Expected DriveSelection content");
        }
    }
}
