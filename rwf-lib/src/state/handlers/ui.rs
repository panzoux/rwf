use crate::job::JobSpec;
use crate::state::{
    collect_jump_file_fast_candidates, collect_jump_path_fast_candidates,
    get_share_root_from_location, update_state, AppState, StateUpdateResult, Transition,
};

impl AppState {
    pub(crate) fn handle_ui_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::PaneRefreshed { tab_id, pane } => {
                if let Some(tab) = self.tabs.tabs.iter_mut().find(|t| t.id == *tab_id) {
                    let pane_model = match pane {
                        crate::model::ActivePane::Left => &mut tab.left_pane,
                        crate::model::ActivePane::Right => &mut tab.right_pane,
                    };
                    pane_model.is_loading = false;
                    pane_model.apply_sort();
                    pane_model.apply_current_filter();
                    pane_model
                        .update_scroll(self.ui.layout.pane_height, self.config.ui.scroll_offset);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::ChangeUIMode { mode } => {
                self.ui.mode = *mode;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePaneHeight { height } => {
                self.ui.layout.pane_height = *height;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePaneWidth { width } => {
                self.ui.layout.pane_width = *width;
                let content_w = width.saturating_sub(10);
                if let Some(ref mut viewer) = self.viewer {
                    viewer.content_width = content_w;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowDialog { dialog } => {
                self.dialogs.push(dialog.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CloseDialog => {
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ToggleTaskPanel => {
                self.ui.layout.show_task_panel = !self.ui.layout.show_task_panel;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::IncreaseTaskPanelHeight => {
                if self.ui.layout.task_panel_height < 20 {
                    self.ui.layout.task_panel_height += 1;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::DecreaseTaskPanelHeight => {
                if self.ui.layout.task_panel_height > 3 {
                    self.ui.layout.task_panel_height -= 1;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ScrollTaskPanelUp => {
                if self.ui.layout.task_panel_scroll_offset > 0 {
                    self.ui.layout.task_panel_scroll_offset -= 1;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ScrollTaskPanelDown => {
                let total_items =
                    self.jobs.queue.len() + self.jobs.active.len() + self.jobs.completed.len();

                if total_items > self.ui.layout.task_panel_height {
                    let max_scroll = total_items.saturating_sub(self.ui.layout.task_panel_height);
                    if self.ui.layout.task_panel_scroll_offset < max_scroll {
                        self.ui.layout.task_panel_scroll_offset += 1;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ConfirmDialog => {
                if let Some(dialog) = self.dialogs.current() {
                    match &dialog.content {
                        crate::model::DialogContent::Confirmation(_) => {
                            let title = dialog.title.as_str();
                            if title == "Copy" {
                                let sources: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries
                                            .iter()
                                            .filter(|e| pane.marking.is_marked(&e.location))
                                            .map(|e| e.location.clone())
                                            .collect()
                                    } else if let Some(entry) = pane.current_entry() {
                                        vec![entry.location.clone()]
                                    } else {
                                        vec![]
                                    }
                                };

                                if !sources.is_empty() {
                                    let dest = self.opposite_pane().current_location.clone();
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Copy {
                                            sources,
                                            dest,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Move" {
                                let sources: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries
                                            .iter()
                                            .filter(|e| pane.marking.is_marked(&e.location))
                                            .map(|e| e.location.clone())
                                            .collect()
                                    } else if let Some(entry) = pane.current_entry() {
                                        vec![entry.location.clone()]
                                    } else {
                                        vec![]
                                    }
                                };

                                if !sources.is_empty() {
                                    let dest = self.opposite_pane().current_location.clone();
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Move {
                                            sources,
                                            dest,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Delete" {
                                let targets: Vec<_> = {
                                    let pane = self.active_pane();
                                    if pane.marking.count() > 0 {
                                        pane.entries
                                            .iter()
                                            .filter(|e| pane.marking.is_marked(&e.location))
                                            .map(|e| e.location.clone())
                                            .collect()
                                    } else if let Some(entry) = pane.current_entry() {
                                        vec![entry.location.clone()]
                                    } else {
                                        vec![]
                                    }
                                };

                                if !targets.is_empty() {
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Delete {
                                            targets,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            } else if title == "Configuration Editor Closed" {
                                self.dialogs.pop();
                                return Some(update_state(self, Transition::ReloadConfig));
                            }
                        }
                        crate::model::DialogContent::Input { .. } => {
                            let title = dialog.title.as_str();
                            let input = self.dialogs.input_buffer.clone();

                            if title == "Search" {
                                if !input.is_empty() {
                                    self.search.add_to_history(input.clone());
                                    if let Some(first_result) = self.search.current_result() {
                                        if let Some(index) = self
                                            .active_pane()
                                            .entries
                                            .iter()
                                            .position(|e| e.location == first_result.location)
                                        {
                                            self.dialogs.pop();
                                            return Some(update_state(
                                                self,
                                                Transition::CursorJump {
                                                    pane: self.ui.active_pane,
                                                    position: index,
                                                },
                                            ));
                                        }
                                    }
                                }
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::ChangeUIMode {
                                        mode: crate::model::UIMode::Normal,
                                    },
                                ));
                            } else if title == "Create Directory" {
                                let current_location = self.active_pane().current_location.clone();
                                let new_dir_location = current_location.join(&input);
                                let job_spec =
                                    crate::job::JobSpec::new(crate::job::JobKind::Mkdir {
                                        location: new_dir_location,
                                    });
                                self.dialogs.pop();
                                return Some(StateUpdateResult::with_job(job_spec));
                            } else if title == "Wildcard Marking" {
                                if !input.is_empty() {
                                    self.dialogs.pop();
                                    return Some(update_state(
                                        self,
                                        Transition::MarkPattern { pattern: input },
                                    ));
                                }
                            } else if title == "Register Folder" {
                                if !input.is_empty() {
                                    self.dialogs.pop();
                                    let path = self.active_pane().current_location.display_path();
                                    return Some(update_state(
                                        self,
                                        Transition::RegisterCurrentFolder { name: input, path },
                                    ));
                                }
                            } else if title == "File Mask Filter" {
                                let mask = if input.is_empty() { None } else { Some(input) };
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::SetFileMask { pane, mask },
                                ));
                            }
                        }
                        crate::model::DialogContent::DeleteConfirm(
                            crate::model::dialog::DeleteConfirmDialog { targets, .. },
                        ) => {
                            let jobs_targets: Vec<_> =
                                targets.iter().map(|(loc, _)| loc.clone()).collect();
                            if !jobs_targets.is_empty() {
                                let job_spec =
                                    crate::job::JobSpec::new(crate::job::JobKind::Delete {
                                        targets: jobs_targets,
                                    });
                                self.dialogs.pop();
                                return Some(StateUpdateResult::with_job(job_spec));
                            }
                        }
                        crate::model::DialogContent::SimpleRename(_) => {
                            let new_name = self.dialogs.input_buffer.clone();
                            if !new_name.is_empty() {
                                if let Some(entry) = self.active_pane().current_entry() {
                                    let from = entry.location.clone();
                                    let to = from
                                        .parent()
                                        .unwrap_or_else(|| {
                                            self.active_pane().current_location.clone()
                                        })
                                        .join(&new_name);
                                    let job_spec =
                                        crate::job::JobSpec::new(crate::job::JobKind::Rename {
                                            from,
                                            to,
                                        });
                                    self.dialogs.pop();
                                    return Some(StateUpdateResult::with_job(job_spec));
                                }
                            }
                        }
                        crate::model::DialogContent::FileMask(
                            crate::model::dialog::FileMaskDialog { input, .. },
                        ) => {
                            let mask = if input.is_empty() {
                                None
                            } else {
                                Some(input.clone())
                            };
                            let pane = self.ui.active_pane;
                            self.dialogs.pop();
                            return Some(update_state(
                                self,
                                Transition::SetFileMask { pane, mask },
                            ));
                        }
                        crate::model::DialogContent::DriveSelection(
                            crate::model::dialog::DriveSelectionDialog {
                                drives,
                                selected_index,
                                filter,
                            },
                        ) => {
                            let lower = filter.to_lowercase();
                            let filtered: Vec<&crate::model::dialog::DriveInfo> =
                                if filter.is_empty() {
                                    drives.iter().collect()
                                } else {
                                    drives
                                        .iter()
                                        .filter(|d| {
                                            d.display_label().to_lowercase().contains(&lower)
                                                || d.path.to_lowercase().contains(&lower)
                                        })
                                        .collect()
                                };
                            if let Some(drive) = filtered.get(*selected_index) {
                                let location = crate::model::Location::Local(
                                    std::path::PathBuf::from(&drive.path),
                                );
                                let pane = self.ui.active_pane;
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::ChangeLocation { pane, location },
                                ));
                            }
                        }
                        crate::model::DialogContent::RegisteredFolderSelector(
                            crate::model::RegisteredFolderSelectorContent {
                                selected_index, ..
                            },
                        ) => {
                            let folder_index = *selected_index;
                            if self.active_pane().marking.count() > 0 {
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::MoveToRegisteredFolder { folder_index },
                                ));
                            } else {
                                self.dialogs.pop();
                                return Some(update_state(
                                    self,
                                    Transition::NavigateToRegisteredFolder { folder_index },
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CancelDialog => {
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowContextMenu => {
                let dialog = crate::model::Dialog::context_menu();
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowCustomFunctionsDialog => {
                let functions = self.custom_functions.clone();
                if functions.is_empty() {
                    tracing::info!(
                        "No custom functions loaded (custom_functions.json missing or empty)"
                    );
                    None
                } else {
                    let dialog = crate::model::Dialog::custom_function_selector(functions);
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                }
            }
            Transition::InvokeCustomFunctionByName { name } => {
                let func = self
                    .custom_functions
                    .iter()
                    .find(|f| f.name == *name)
                    .cloned();
                if let Some(func) = func {
                    if func.is_menu() {
                        let title = func.name.clone();
                        let items = func.menu_items().to_vec();
                        self.dialogs
                            .push(crate::model::Dialog::custom_function_menu(title, items));
                        Some(StateUpdateResult::with_ui_change())
                    } else if let Some(_cmd) = func.get_command() {
                        let expander = crate::macro_expander::MacroExpander::new();
                        let command = expander
                            .expand(self, &func)
                            .unwrap_or_else(|_| func.get_command().unwrap_or("").replace("$I", ""));
                        let working_dir = self.active_pane().current_location.clone();
                        let shell = func.get_shell().map(|s| s.to_string());
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::ExecuteCustomFunction {
                                command,
                                working_dir,
                                pipe_to_action: func.pipe_to_action.clone(),
                                shell,
                            });
                        Some(StateUpdateResult::with_job(job_spec))
                    } else {
                        None
                    }
                } else {
                    tracing::warn!("InvokeCustomFunctionByName: no function named {:?}", name);
                    None
                }
            }
            Transition::ExecuteAssociation {
                command,
                working_dir,
                shell,
            } => {
                let job_spec = crate::job::JobSpec::execute_association(
                    command.clone(),
                    working_dir.clone(),
                    shell.clone(),
                );
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ExecuteAssociationChecked {
                path,
                command,
                working_dir,
                shell,
            } => {
                let job_spec = self.checked_association_job(
                    path.clone(),
                    command.clone(),
                    working_dir.clone(),
                    shell.clone(),
                );
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowOpenWithPicker { candidates, paths } => {
                let dialog =
                    crate::model::Dialog::open_with_picker(paths.clone(), candidates.clone());
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::StartBatchOpenWith { paths } => {
                let job_spec =
                    crate::job::JobSpec::new(crate::job::JobKind::DetectFileTypesBatch {
                        paths: paths.clone(),
                    });
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::CheckFallbackFileType { location } => {
                // Magic-byte detection only makes sense against a real filesystem
                // path. `display_path()` for non-Local locations (Archive, Ssh,
                // Cloud) is a synthetic string (e.g. "archive.zip#inner/notes.txt"),
                // not something `std::fs` can open — a DetectFileType job against
                // it would just fail. Skip detection for those and go straight to
                // the text viewer, exactly like before this fallback existed (the
                // viewer's LoadFileForViewer job is location-aware and handles
                // Archive/Ssh/Cloud correctly).
                match location {
                    crate::model::Location::Local(_) => {
                        let path: std::path::PathBuf = location.display_path().into();
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::DetectFileType {
                                path,
                                purpose: crate::job::DetectFileTypePurpose::FallbackOpen {
                                    location: location.clone(),
                                },
                            });
                        Some(StateUpdateResult::with_job(job_spec))
                    }
                    _ => Some(update_state(
                        self,
                        Transition::OpenTextViewer {
                            location: location.clone(),
                        },
                    )),
                }
            }
            Transition::ShowDriveChangeDialog => {
                let mut entries = Vec::new();

                // 1. Home directory
                if let Some(home) = dirs::home_dir() {
                    entries.push(crate::model::dialog::DriveInfo {
                        path: home.to_string_lossy().into_owned(),
                        label: "~ User Directory".to_string(),
                        drive_type: crate::model::dialog::DriveType::Local,
                        total_space: None,
                        free_space: None,
                    });
                }

                // 2. Network shares discovered from both panes' history
                let (left_stack, right_stack, cur_left, cur_right) = {
                    let tab = self.current_tab();
                    let (ls, _) = tab
                        .history
                        .stack_and_pos(crate::model::ui::ActivePane::Left);
                    let (rs, _) = tab
                        .history
                        .stack_and_pos(crate::model::ui::ActivePane::Right);
                    (
                        ls.to_vec(),
                        rs.to_vec(),
                        tab.left_pane.current_location.clone(),
                        tab.right_pane.current_location.clone(),
                    )
                };
                let mut nw_roots: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();
                for loc in left_stack
                    .iter()
                    .chain(right_stack.iter())
                    .chain(std::iter::once(&cur_left))
                    .chain(std::iter::once(&cur_right))
                {
                    if let Some(root) = get_share_root_from_location(loc) {
                        nw_roots.insert(root);
                    }
                }
                for root in &nw_roots {
                    entries.push(crate::model::dialog::DriveInfo {
                        path: root.clone(),
                        label: root.clone(),
                        drive_type: crate::model::dialog::DriveType::Network,
                        total_space: None,
                        free_space: None,
                    });
                }

                // 3. System drives
                entries.extend(crate::volume_info::get_all_drives());

                let dialog = crate::model::Dialog::drive_selection(entries, self.ui.active_pane);
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowFileInfo => {
                if let Some(entry) = self.active_pane().current_entry() {
                    let dialog = crate::model::Dialog::file_info(entry);
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::DetectFileInfoType { path } => {
                // Content-type detection does real filesystem I/O, which is meaningless
                // for archive-internal or remote entries (Phase 7.3 §7, same guard
                // philosophy as Task 5's Local-only fallback detection). Report "not
                // available" instead of starting a doomed job.
                let is_local = matches!(
                    self.dialogs.current(),
                    Some(crate::model::dialog::Dialog {
                        content: crate::model::dialog::DialogContent::FileInfo(
                            crate::model::dialog::FileInfoDialog { is_local: true, .. }
                        ),
                        ..
                    })
                );
                if !is_local {
                    if let Some(dialog) = self.dialogs.current_mut() {
                        if let crate::model::dialog::DialogContent::FileInfo(d) =
                            &mut dialog.content
                        {
                            d.detected_type = Some("not available for this location".to_string());
                            d.detecting = false;
                            d.detected_type_job_id = None;
                        }
                    }
                    return Some(StateUpdateResult::with_ui_change());
                }
                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::DetectFileType {
                    path: path.clone(),
                    purpose: crate::job::DetectFileTypePurpose::FileInfoDisplay,
                });
                let job_id = job_spec.id;
                if let Some(dialog) = self.dialogs.current_mut() {
                    if let crate::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
                        d.detecting = true;
                        d.detected_type_job_id = Some(job_id);
                    }
                }
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowVersion => {
                let dialog = crate::model::Dialog::version();
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SaveLog => {
                let _ = self.log_manager.save_to_file();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::RotateHelpLanguage => {
                let current_lang = &self.config.help_language;
                let next_lang = crate::help_content::HelpContent::next_language(current_lang);
                self.config.help_language = next_lang.clone();

                if let Some(dialog) = self.dialogs.current_mut() {
                    if let crate::model::DialogContent::Help(crate::model::HelpDialog {
                        ref mut language,
                        ref mut entries,
                        ..
                    }) = dialog.content
                    {
                        *language = next_lang.clone();
                        let descriptions =
                            crate::help_content::ActionDescriptions::load(&next_lang);
                        *entries = crate::help_content::build_help_entries(
                            &self.config.key_bindings,
                            &descriptions,
                            &self.custom_functions,
                            self.config.help_show_unbound,
                            &self.config,
                        );
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowRegisteredFolderDialog => {
                let folders = self.registered_folders.folders.clone();
                let dialog = crate::model::Dialog::registered_folder_selector(folders);
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowJumpToPathDialog => {
                let root = self.active_pane().current_location.display_path();
                let fast_candidates = collect_jump_path_fast_candidates(self);
                let job_spec =
                    crate::job::JobSpec::new(crate::job::JobKind::CollectJumpCandidates {
                        root: root.clone(),
                        include_files: false,
                        max_results: self.config.jump_nav.jump_path_max_results,
                        max_depth: self.config.jump_nav.jump_path_max_depth,
                    });
                let job_id = job_spec.id;
                let mut dialog = crate::model::Dialog::jump_to_path(root, fast_candidates);
                if let crate::model::dialog::DialogContent::JumpToPath(
                    crate::model::dialog::JumpToPathDialog { loading_job_id, .. },
                ) = &mut dialog.content
                {
                    *loading_job_id = Some(job_id);
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowJumpToFileDialog => {
                let root = self.active_pane().current_location.display_path();
                let fast_candidates = collect_jump_file_fast_candidates(self);
                let job_spec =
                    crate::job::JobSpec::new(crate::job::JobKind::CollectJumpCandidates {
                        root: root.clone(),
                        include_files: true,
                        max_results: self.config.jump_nav.jump_file_max_results,
                        max_depth: self.config.jump_nav.jump_file_max_depth,
                    });
                let job_id = job_spec.id;
                let mut dialog = crate::model::Dialog::jump_to_file(root, fast_candidates);
                if let crate::model::dialog::DialogContent::JumpToFile(
                    crate::model::dialog::JumpToFileDialog { loading_job_id, .. },
                ) = &mut dialog.content
                {
                    *loading_job_id = Some(job_id);
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::RegisterCurrentFolder { name, path } => {
                let folder = crate::model::RegisteredFolder::new(name.clone(), path.clone());
                self.registered_folders.add(folder);

                let save_path = crate::model::RegisteredFolderManager::default_path();
                let _ = self.registered_folders.save_to_file(&save_path);

                let log = format!("[Folder] Registered \"{}\" → {}", name, path);
                let mut result = StateUpdateResult::with_ui_change();
                result.task_panel_logs.push(log);
                Some(result)
            }
            Transition::NavigateToRegisteredFolder { folder_index } => {
                if let Some(folder) = self.registered_folders.folders.get(*folder_index) {
                    let expanded_path = self.registered_folders.expand_path(folder);
                    let location = crate::model::Location::Local(expanded_path);

                    self.dialogs.pop();
                    return Some(update_state(
                        self,
                        Transition::ChangeLocation {
                            pane: self.ui.active_pane,
                            location,
                        },
                    ));
                }
                Some(StateUpdateResult::none())
            }
            Transition::MoveToRegisteredFolder { folder_index } => {
                if let Some(folder) = self.registered_folders.folders.get(*folder_index) {
                    let expanded_path = self.registered_folders.expand_path(folder);
                    let dest = crate::model::Location::Local(expanded_path);

                    let sources: Vec<_> = {
                        let pane = self.active_pane();
                        if pane.marking.count() > 0 {
                            pane.entries
                                .iter()
                                .filter(|e| pane.marking.is_marked(&e.location))
                                .map(|e| e.location.clone())
                                .collect()
                        } else {
                            vec![]
                        }
                    };

                    if !sources.is_empty() {
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::Move { sources, dest });
                        self.dialogs.pop();
                        return Some(StateUpdateResult::with_job(job_spec));
                    }
                }
                Some(StateUpdateResult::none())
            }
            Transition::EditConfigFile => {
                let config_manager = crate::config::ConfigManager::new();
                let config_path = config_manager.config_path().to_string_lossy().to_string();
                let job_spec = JobSpec::new(Self::editor_job(&self.config, config_path, true));
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenWithEditor { path } => {
                let job_spec = JobSpec::new(Self::editor_job(&self.config, path.clone(), false));
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenWithSystem { path } => {
                let job_spec = JobSpec::new(Self::system_open_job(path.clone()));
                Some(StateUpdateResult::with_job(job_spec))
            }
            _ => None,
        }
    }
}
