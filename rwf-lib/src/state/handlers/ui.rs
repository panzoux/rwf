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
                            } else if title == "Create File" {
                                let current_location = self.active_pane().current_location.clone();
                                let new_file_location = current_location.join(&input);
                                let job_spec =
                                    crate::job::JobSpec::new(crate::job::JobKind::CreateFile {
                                        location: new_file_location,
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

                // Kick off live content-type detection for the "Open With..." row
                // label (Phase 7.3b, Task 9) — mirrors the File Info dialog's
                // on-demand 'd'-key detection (same guard philosophy as Tasks 5/6/8:
                // magic-byte detection off, or a non-Local/directory entry, means
                // no doomed detect job and the row stays plain "Open With...").
                let entry = self.active_pane().current_entry().cloned();
                if let Some(entry) = entry {
                    let is_local = matches!(entry.location, crate::model::Location::Local(_));
                    if self.config.magic_byte_detection_enabled && is_local && !entry.is_dir {
                        let path: std::path::PathBuf = entry.location.display_path().into();
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::DetectFileType {
                                path,
                                purpose: crate::job::DetectFileTypePurpose::ContextMenuLabel,
                            });
                        let job_id = job_spec.id;
                        if let Some(dialog) = self.dialogs.current_mut() {
                            if let crate::model::dialog::DialogContent::ContextMenu(d) =
                                &mut dialog.content
                            {
                                d.detected_type_job_id = Some(job_id);
                            }
                        }
                        return Some(StateUpdateResult::with_job(job_spec));
                    }
                }
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
                // Reached via the flag-off/non-Local extension-only path in
                // `resolve_extension_association` — content-type detection never ran,
                // so there's no kind to show in the title.
                let dialog =
                    crate::model::Dialog::open_with_picker(paths.clone(), candidates.clone(), None);
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
            Transition::ResolveAssociationByType { location } => {
                // Only reached for `Location::Local` (see `resolve_extension_association`'s
                // pre-check) — no non-Local branch needed here, unlike `CheckFallbackFileType`.
                let path: std::path::PathBuf = location.display_path().into();
                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::DetectFileType {
                    path,
                    purpose: crate::job::DetectFileTypePurpose::ResolveAssociation {
                        location: location.clone(),
                    },
                });
                Some(StateUpdateResult::with_job(job_spec))
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
                    let mut dialog = crate::model::Dialog::file_info(entry);
                    // Auto-start content-type detection the moment the dialog
                    // opens (Phase 7.3b, Task 13b) — this used to require an
                    // explicit `d` keypress, but detection is a cheap async
                    // `Job` (reads only the first ~64-300 bytes), not a
                    // blocking read, so gating it behind a manual trigger had
                    // no real payoff. Only meaningful for local entries —
                    // archive-internal/remote entries skip it, same guard the
                    // old manual-trigger handler used (real filesystem I/O is
                    // meaningless there).
                    if let crate::model::Location::Local(path) = &entry.location {
                        let job_spec =
                            crate::job::JobSpec::new(crate::job::JobKind::DetectFileType {
                                path: path.clone(),
                                purpose: crate::job::DetectFileTypePurpose::FileInfoDisplay,
                            });
                        let job_id = job_spec.id;
                        if let crate::model::dialog::DialogContent::FileInfo(d) =
                            &mut dialog.content
                        {
                            d.detecting = true;
                            d.detected_type_job_id = Some(job_id);
                        }
                        self.dialogs.push(dialog);
                        return Some(StateUpdateResult::with_job(job_spec));
                    }
                    // Non-local entries (archive-internal, remote): a real
                    // detect job against the synthetic display_path() would
                    // just fail, so report "not available" immediately rather
                    // than silently never detecting anything (same message
                    // the old manual `d`-trigger handler used).
                    if let crate::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
                        d.detected_type = Some("not available for this location".to_string());
                    }
                    self.dialogs.push(dialog);
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::ToggleFileInfoHeaderView => {
                // Pure UI-state flip (Phase 7.3b, Task 10) — flip whichever
                // FileInfo dialog is currently on top of the stack. No job;
                // routed through a Transition per the project's state-purity
                // rule (never mutate dialog content from the input/render
                // layers directly).
                if let Some(dialog) = self.dialogs.current_mut() {
                    if let crate::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
                        d.header_hex_mode = !d.header_hex_mode;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowAttrTimestampDialog => {
                let pane = self.active_pane();
                let targets: Vec<crate::model::Location> = if pane.marking.count() > 0 {
                    pane.entries
                        .iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.location.clone())
                        .collect()
                } else if let Some(entry) = pane.current_entry() {
                    vec![entry.location.clone()]
                } else {
                    Vec::new()
                };

                if targets.is_empty() {
                    return Some(StateUpdateResult::none());
                }

                self.dialogs
                    .push(crate::model::Dialog::attr_timestamp(targets));
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ConfirmAttrTimestampDialog => {
                let Some(dialog) = self.dialogs.current() else {
                    return Some(StateUpdateResult::none());
                };
                let crate::model::dialog::DialogContent::AttrTimestamp(d) = &dialog.content else {
                    return Some(StateUpdateResult::none());
                };

                let attrs = d.to_attribute_change();
                let times = d.to_timestamp_change();
                let targets = d.targets.clone();

                let mut jobs = Vec::new();
                if !attrs.is_empty() {
                    jobs.push(crate::job::JobSpec::new(
                        crate::job::JobKind::ChangeAttributes {
                            targets: targets.clone(),
                            attrs,
                        },
                    ));
                }
                if !times.is_empty() {
                    jobs.push(crate::job::JobSpec::new(
                        crate::job::JobKind::ChangeTimestamps { targets, times },
                    ));
                }

                self.dialogs.pop();

                if jobs.is_empty() {
                    return Some(StateUpdateResult::with_ui_change());
                }
                let mut result = StateUpdateResult::with_ui_change();
                result.jobs_to_start = jobs;
                Some(result)
            }
            Transition::ShowCreateLinkDialog => {
                let target = {
                    let pane = self.active_pane();
                    if pane.marking.count() > 0 {
                        pane.entries
                            .iter()
                            .find(|e| pane.marking.is_marked(&e.location))
                            .map(|e| e.location.clone())
                    } else {
                        pane.current_entry().map(|e| e.location.clone())
                    }
                };
                let Some(target) = target else {
                    return Some(StateUpdateResult::none());
                };

                // Create Link only makes sense between two local directories;
                // if the opposite pane is remote/archive there's nowhere
                // sensible to place the link.
                let dest_dir = match &self.opposite_pane().current_location {
                    crate::model::Location::Local(path) => path.clone(),
                    _ => return Some(StateUpdateResult::none()),
                };

                self.dialogs
                    .push(crate::model::Dialog::create_link(target, dest_dir));
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ConfirmCreateLinkDialog => {
                let Some(dialog) = self.dialogs.current() else {
                    return Some(StateUpdateResult::none());
                };
                let crate::model::dialog::DialogContent::CreateLink(d) = &dialog.content else {
                    return Some(StateUpdateResult::none());
                };
                if d.link_name.is_empty() {
                    return Some(StateUpdateResult::none());
                }

                let job_spec = crate::job::JobSpec::new(crate::job::JobKind::CreateLink {
                    target: d.target.clone(),
                    link_path: d.link_path(),
                    kind: d.kind,
                });

                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowOperationReport => {
                if let Some(report) = self.operation_reports.back() {
                    let total = self.operation_reports.len();
                    self.dialogs
                        .push(crate::model::Dialog::operation_report_view_at(
                            report.clone(),
                            total - 1,
                            total,
                        ));
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult {
                        jobs_to_start: Vec::new(),
                        jobs_to_cancel: Vec::new(),
                        completed_jobs: Vec::new(),
                        failed_jobs: Vec::new(),
                        cancelled_jobs: Vec::new(),
                        started_jobs: Vec::new(),
                        task_panel_logs: vec!["[Info] No operations recorded yet".to_string()],
                        panes_to_refresh: Vec::new(),
                        ui_changed: true,
                        reload_keybindings: false,
                    })
                }
            }
            Transition::CycleFileInfoHeaderEncoding => {
                // Pure UI-state flip (Phase 7.3b, Task 12) — cycle whichever
                // FileInfo dialog is currently on top of the stack through
                // TextEncoding::next(). The `e`-key guard in the input layer
                // only fires once `header_encoding` is `Some` (i.e. after
                // detection has run), so the `None` branch here should be
                // unreachable via the real UI path — but we still handle it
                // defensively (no-op) rather than panicking.
                if let Some(dialog) = self.dialogs.current_mut() {
                    if let crate::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
                        if let Some(enc) = d.header_encoding {
                            d.header_encoding = Some(enc.next());
                        }
                    }
                }
                Some(StateUpdateResult::with_ui_change())
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
