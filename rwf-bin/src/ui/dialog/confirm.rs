//! Dialog confirmation logic: converts a confirmed dialog into JobSpecs /
//! state changes (process_dialog_confirmation and friends).
//!
//! Split from dialog/mod.rs in M3 (move-only).

use rwf_lib::model::dialog::DialogContent;
use tracing::debug;

use super::archive_ext_for_format;

/// Remove the selected entry from a RegisteredFolderSelector dialog.
/// Updates both state.registered_folders and the dialog's own folder list.
/// Returns a log message on success.
pub fn process_dialog_delete(state: &mut rwf_lib::AppState) -> Option<String> {
    // Step 1: resolve actual folder index from the filtered selection (borrow ends at block close).
    let folder_index: Option<usize> = {
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::RegisteredFolderSelector {
                folders,
                selected_index,
                filter,
            } = &dialog.content
            {
                let lower = filter.to_lowercase();
                let filtered_indices: Vec<usize> = if filter.is_empty() {
                    (0..folders.len()).collect()
                } else {
                    folders
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            f.name.to_lowercase().contains(&lower)
                                || f.path.to_lowercase().contains(&lower)
                        })
                        .map(|(i, _)| i)
                        .collect()
                };
                filtered_indices.get(*selected_index).copied()
            } else {
                None
            }
        } else {
            None
        }
    };

    let idx = folder_index?;

    // Step 2: remove from authoritative state (different field — no borrow conflict).
    let removed = state.registered_folders.remove(idx);
    let save_path = rwf_lib::model::dialog::RegisteredFolderManager::default_path();
    let _ = state.registered_folders.save_to_file(&save_path);

    // Step 3: mirror removal in the dialog's own snapshot.
    if let Some(dialog) = state.dialogs.current_mut() {
        if let DialogContent::RegisteredFolderSelector {
            folders,
            selected_index,
            ..
        } = &mut dialog.content
        {
            if idx < folders.len() {
                folders.remove(idx);
            }
            let count = folders.len();
            if count > 0 && *selected_index >= count {
                *selected_index = count - 1;
            }
        }
    }

    removed.map(|f| format!("[Folder] Removed \"{}\" → {}", f.name, f.path))
}

/// Build a human-readable job name for a delete operation showing file names.
pub fn delete_job_name(targets: &[rwf_lib::Location]) -> String {
    let file_name = |loc: &rwf_lib::Location| -> String {
        loc.path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| loc.display_path())
    };
    match targets.len() {
        0 => "Delete".to_string(),
        1 => format!("Delete '{}'", file_name(&targets[0])),
        2 => format!(
            "Delete '{}', '{}'",
            file_name(&targets[0]),
            file_name(&targets[1])
        ),
        n => format!(
            "Delete {} files: '{}', '{}'...",
            n,
            file_name(&targets[0]),
            file_name(&targets[1])
        ),
    }
}

/// Process dialog confirmation and create transitions
/// Returns the job spec if a job was created, so it can be submitted to the worker pool
pub fn process_dialog_confirmation(state: &mut rwf_lib::AppState) -> Option<rwf_lib::job::JobSpec> {
    debug!("process_dialog_confirmation called");

    // Input dialogs: extract title and embedded input first so the borrow on state.dialogs ends
    // before we call update_state.
    let input_dialog_info: Option<(String, String)> = state
        .dialogs
        .current()
        .filter(|d| matches!(d.content, DialogContent::Input { .. }))
        .map(|d| {
            let embedded = if let DialogContent::Input { input, .. } = &d.content {
                input.clone()
            } else {
                String::new()
            };
            (d.title.clone(), embedded)
        });

    if let Some((title, input)) = input_dialog_info {
        match title.as_str() {
            "Register Folder" if !input.is_empty() => {
                let path = state.active_pane().current_location.display_path();
                rwf_lib::state::update_state(
                    state,
                    rwf_lib::state::Transition::RegisterCurrentFolder { name: input, path },
                );
            }
            "Create Directory" if !input.is_empty() => {
                let current_location = state.active_pane().current_location.clone();
                let new_dir_loc = current_location.join(&input);
                return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::Mkdir {
                    location: new_dir_loc,
                }));
            }
            "Custom Function Input" => {
                if let Some(func) = state.pending_custom_function_input.take() {
                    let expander = rwf_lib::macro_expander::MacroExpander::new();
                    if let Ok(command) = expander.expand_with_user_input(state, &func, &input) {
                        let working_dir = state.active_pane().current_location.clone();
                        let shell = func.shell.clone();
                        // Pop the CustomFunctionSelector sitting below this Input dialog;
                        // app.rs will pop the Input dialog itself after we return.
                        state.dialogs.pop_below_top();
                        return Some(rwf_lib::job::JobSpec::new(
                            rwf_lib::job::JobKind::ExecuteCustomFunction {
                                command,
                                working_dir,
                                pipe_to_action: func.pipe_to_action.clone(),
                                shell,
                            },
                        ));
                    }
                }
            }
            _ => {}
        }
        return None;
    }

    if let Some(dialog) = state.dialogs.current() {
        debug!(
            "Dialog content type: {:?}",
            std::mem::discriminant(&dialog.content)
        );
        match &dialog.content {
            DialogContent::SortDialog {
                selected_mode_index,
                selected_order_index,
                ..
            } => {
                use rwf_lib::model::{SortMode, SortOrder};
                let mode = match *selected_mode_index {
                    0 => SortMode::Name,
                    1 => SortMode::Size,
                    2 => SortMode::Date,
                    _ => SortMode::Extension,
                };
                let order = if *selected_order_index == 0 {
                    SortOrder::Ascending
                } else {
                    SortOrder::Descending
                };
                let pane = state.ui.active_pane;
                // Apply both mode and order directly (no job needed)
                rwf_lib::state::update_state(
                    state,
                    rwf_lib::state::Transition::ChangeSortMode { pane, mode },
                );
                rwf_lib::state::update_state(
                    state,
                    rwf_lib::state::Transition::ChangeSortOrder { pane, order },
                );
                return None;
            }
            DialogContent::FileMask { input, .. } => {
                let mask = if input.is_empty() {
                    None
                } else {
                    Some(input.clone())
                };
                let pane = state.ui.active_pane;
                // Do NOT pop here — app.rs pops after process_dialog_confirmation returns
                rwf_lib::state::update_state(
                    state,
                    rwf_lib::state::Transition::SetFileMask { pane, mask },
                );
                return None;
            }
            DialogContent::WildcardMark { input, .. } => {
                if !input.is_empty() {
                    let pattern = input.clone();
                    rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::MarkPattern { pattern },
                    );
                }
                return None;
            }
            DialogContent::SimpleRename { input, .. } => {
                let new_name = input.clone();
                if !new_name.is_empty() {
                    if let Some(entry) = state.active_pane().current_entry() {
                        let from = entry.location.clone();
                        let to = from
                            .parent()
                            .map(|parent| parent.join(&new_name))
                            .unwrap_or_else(|| from.clone());
                        let job_spec =
                            rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::Rename { from, to });
                        return Some(job_spec);
                    }
                }
                return None;
            }
            DialogContent::DriveSelection {
                drives,
                selected_index,
                filter,
            } => {
                let lower = filter.to_lowercase();
                let filtered: Vec<&rwf_lib::model::dialog::DriveInfo> = if filter.is_empty() {
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
                    let path = drive.path.clone();
                    let pane = state.ui.active_pane;
                    let location = rwf_lib::Location::Local(std::path::PathBuf::from(&path));
                    let result = rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::ChangeLocation { pane, location },
                    );
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::HistoryDialog {
                left_entries,
                right_entries,
                left_selected,
                right_selected,
                active_pane,
                ..
            } => {
                use rwf_lib::model::ui::ActivePane;
                let (entries, selected_index, pane) = match active_pane {
                    ActivePane::Left => (left_entries.as_slice(), *left_selected, ActivePane::Left),
                    ActivePane::Right => {
                        (right_entries.as_slice(), *right_selected, ActivePane::Right)
                    }
                };
                if entries.get(selected_index).is_some() {
                    let result = rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::NavigateToHistoryIndex {
                            pane,
                            index: selected_index,
                        },
                    );
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::JumpToPath {
                suggestions,
                selected_index,
                query,
                search_root,
                loading_job_id,
                ..
            } => {
                let path_str: Option<String> =
                    if !suggestions.is_empty() && *selected_index < suggestions.len() {
                        Some(suggestions[*selected_index].clone())
                    } else if !query.is_empty() {
                        // Fallback: interpret typed text as a direct path
                        let candidate = std::path::PathBuf::from(query.as_str());
                        if candidate.is_absolute() && candidate.is_dir() {
                            Some(query.clone())
                        } else {
                            let combined =
                                std::path::PathBuf::from(search_root.as_str()).join(query.as_str());
                            if combined.is_dir() {
                                Some(combined.to_string_lossy().into_owned())
                            } else {
                                None
                            }
                        }
                    } else {
                        None
                    };
                let pending_job = *loading_job_id;
                if let Some(path) = path_str {
                    let location = rwf_lib::Location::Local(std::path::PathBuf::from(&path));
                    let pane = state.ui.active_pane;
                    state.dialogs.pop();
                    if let Some(job_id) = pending_job {
                        state.jobs.request_cancel(job_id);
                    }
                    let result = rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::ChangeLocation { pane, location },
                    );
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::JumpToFile {
                suggestions,
                selected_index,
                query,
                search_root,
                loading_job_id,
                ..
            } => {
                let path_str: Option<String> =
                    if !suggestions.is_empty() && *selected_index < suggestions.len() {
                        Some(suggestions[*selected_index].clone())
                    } else if !query.is_empty() {
                        // Fallback: interpret typed text as a direct path
                        let candidate = std::path::PathBuf::from(query.as_str());
                        if candidate.is_absolute() && (candidate.is_file() || candidate.is_dir()) {
                            Some(query.clone())
                        } else {
                            let combined =
                                std::path::PathBuf::from(search_root.as_str()).join(query.as_str());
                            if combined.is_file() || combined.is_dir() {
                                Some(combined.to_string_lossy().into_owned())
                            } else {
                                None
                            }
                        }
                    } else {
                        None
                    };
                let pending_job = *loading_job_id;
                // For a file selection, record the filename to position cursor after navigation.
                let target_file_name: Option<String> = path_str.as_ref().and_then(|p| {
                    let pb = std::path::Path::new(p);
                    if pb.is_file() {
                        pb.file_name().map(|n| n.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                });
                if let Some(path) = path_str {
                    // For files: navigate to the parent directory. For dirs: navigate into them.
                    let nav_path = {
                        let pb = std::path::PathBuf::from(&path);
                        if pb.is_dir() {
                            path.clone()
                        } else {
                            pb.parent()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or(path.clone())
                        }
                    };
                    let location = rwf_lib::Location::Local(std::path::PathBuf::from(&nav_path));
                    let pane = state.ui.active_pane;
                    state.dialogs.pop();
                    if let Some(job_id) = pending_job {
                        state.jobs.request_cancel(job_id);
                    }
                    let result = rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::ChangeLocation { pane, location },
                    );
                    if let Some(name) = target_file_name {
                        let pane_height = state.ui.layout.pane_height;
                        let scroll_margin = state.config.ui.scroll_offset;
                        let tab = state.current_tab_mut();
                        let pane_model = match pane {
                            rwf_lib::model::ActivePane::Left => &mut tab.left_pane,
                            rwf_lib::model::ActivePane::Right => &mut tab.right_pane,
                        };
                        if pane_model.is_loading {
                            pane_model.pending_cursor_name = Some(name);
                        } else if let Some(pos) =
                            pane_model.entries.iter().position(|e| e.name == name)
                        {
                            pane_model.cursor = pos;
                            pane_model.update_scroll(pane_height, scroll_margin);
                        }
                    }
                    return result.jobs_to_start.into_iter().next();
                }
                return None;
            }
            DialogContent::Compression {
                sources,
                archive_name,
                format,
                selected_format_index,
                compression_level,
                ..
            } => {
                debug!(
                    "Compression dialog confirmed: {} sources, archive_name='{}'",
                    sources.len(),
                    archive_name
                );
                debug!(
                    "Selected format index: {}, compression level: {}",
                    selected_format_index, compression_level
                );

                // Ensure archive name has the correct extension for the selected format
                let ext = archive_ext_for_format(*format);
                let archive_name_with_ext = if archive_name
                    .to_lowercase()
                    .ends_with(&format!(".{}", ext))
                {
                    archive_name.clone()
                } else {
                    // Strip any mismatched extension before adding the correct one
                    let base = ["zip", "7z", "tar", "tgz"]
                        .iter()
                        .find_map(|old_ext| {
                            archive_name
                                .to_lowercase()
                                .ends_with(&format!(".{}", old_ext))
                                .then(|| &archive_name[..archive_name.len() - old_ext.len() - 1])
                        })
                        .unwrap_or(archive_name.as_str());
                    format!("{}.{}", base, ext)
                };
                debug!("Archive name with extension: '{}'", archive_name_with_ext);

                // Build destination path in opposite pane
                let dest_path = state
                    .opposite_pane()
                    .current_location
                    .path()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf();
                let dest = rwf_lib::Location::Local(dest_path.join(&archive_name_with_ext));
                debug!(
                    "Destination path: {:?}",
                    dest_path.join(&archive_name_with_ext)
                );

                // Calculate original size for compression ratio
                let original_size: u64 = sources
                    .iter()
                    .filter_map(|loc| {
                        state
                            .active_pane()
                            .entries
                            .iter()
                            .find(|e| &e.location == loc)
                    })
                    .filter(|e| !e.is_dir)
                    .map(|e| e.size)
                    .sum();
                debug!("Original size: {} bytes", original_size);

                let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::CreateArchive {
                    sources: sources.clone(),
                    dest,
                    original_size,
                });
                debug!("Job spec created: {:?}", job_spec.kind);

                return Some(job_spec);
            }
            DialogContent::ExtractionConfirm { archive, dest, .. } => {
                // Create extraction job - dest is already a Location
                let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExtractArchive {
                    archive: archive.clone(),
                    dest: dest.clone(),
                });

                return Some(job_spec);
            }
            DialogContent::DeleteConfirm { targets, .. } => {
                let locations: Vec<rwf_lib::Location> =
                    targets.iter().map(|(loc, _)| loc.clone()).collect();
                return Some(rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::Delete {
                    targets: locations,
                }));
            }
            DialogContent::RegisteredFolderSelector {
                folders,
                selected_index,
                filter,
            } => {
                let lower = filter.to_lowercase();
                let filtered_indices: Vec<usize> = if filter.is_empty() {
                    (0..folders.len()).collect()
                } else {
                    folders
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            f.name.to_lowercase().contains(&lower)
                                || f.path.to_lowercase().contains(&lower)
                        })
                        .map(|(i, _)| i)
                        .collect()
                };
                if let Some(&folder_index) = filtered_indices.get(*selected_index) {
                    if state.active_pane().marking.count() > 0 {
                        rwf_lib::state::update_state(
                            state,
                            rwf_lib::state::Transition::MoveToRegisteredFolder { folder_index },
                        );
                    } else {
                        rwf_lib::state::update_state(
                            state,
                            rwf_lib::state::Transition::NavigateToRegisteredFolder { folder_index },
                        );
                    }
                }
                return None;
            }
            DialogContent::PatternRename {
                find,
                replace,
                use_regex,
                case_sensitive,
                ..
            } => {
                if find.is_empty() {
                    return None;
                }
                let (find, replace, use_regex, case_sensitive) =
                    (find.clone(), replace.clone(), *use_regex, *case_sensitive);
                let pane = state.active_pane();
                let targets: Vec<rwf_lib::Location> = if pane.marking.count() > 0 {
                    pane.entries
                        .iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.location.clone())
                        .collect()
                } else {
                    pane.entries.iter().map(|e| e.location.clone()).collect()
                };
                if targets.is_empty() {
                    return None;
                }
                let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::PatternRename {
                    targets,
                    find,
                    replace,
                    use_regex,
                    case_sensitive,
                });
                return Some(job_spec);
            }
            DialogContent::CustomFunctionSelector {
                functions,
                selected_index,
                filter,
            } => {
                let lower = filter.to_lowercase();
                let filtered: Vec<&rwf_lib::model::dialog::CustomFunction> = if filter.is_empty() {
                    functions.iter().collect()
                } else {
                    functions
                        .iter()
                        .filter(|f| {
                            f.name.to_lowercase().contains(&lower)
                                || f.description
                                    .as_deref()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&lower)
                        })
                        .collect()
                };
                if let Some(&func) = filtered.get(*selected_index) {
                    let func = func.clone();
                    let expander = rwf_lib::macro_expander::MacroExpander::new();
                    match expander.expand(state, &func) {
                        Ok(command) => {
                            let working_dir = state.active_pane().current_location.clone();
                            let shell = func.shell.clone();
                            return Some(rwf_lib::job::JobSpec::new(
                                rwf_lib::job::JobKind::ExecuteCustomFunction {
                                    command,
                                    working_dir,
                                    pipe_to_action: func.pipe_to_action.clone(),
                                    shell,
                                },
                            ));
                        }
                        Err(_) => {
                            // Command requires $I user input — push an Input dialog.
                            let prompt = rwf_lib::macro_expander::MacroExpander::extract_i_prompt(
                                func.get_command().unwrap_or(""),
                            )
                            .unwrap_or_else(|| "Enter input".to_string());
                            state.dialogs.push(rwf_lib::model::Dialog::input(
                                "Custom Function Input",
                                &prompt,
                                "",
                            ));
                            state.pending_custom_function_input = Some(func);
                            state.suppress_next_dialog_pop = true;
                            return None;
                        }
                    }
                }
                return None;
            }
            DialogContent::ContextMenu {
                options,
                selected_index,
            } => {
                use rwf_lib::model::dialog::ContextMenuAction;
                if let Some(opt) = options.get(*selected_index) {
                    match opt.action.clone() {
                        ContextMenuAction::Copy => {
                            let transitions = rwf_lib::input::action_to_transitions(
                                state,
                                &rwf_lib::input::Action::Copy,
                            );
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    return Some(job);
                                }
                            }
                        }
                        ContextMenuAction::Move => {
                            let transitions = rwf_lib::input::action_to_transitions(
                                state,
                                &rwf_lib::input::Action::Move,
                            );
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    return Some(job);
                                }
                            }
                        }
                        ContextMenuAction::Delete => {
                            let transitions = rwf_lib::input::action_to_transitions(
                                state,
                                &rwf_lib::input::Action::Delete,
                            );
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    return Some(job);
                                }
                            }
                        }
                        ContextMenuAction::Rename => {
                            let transitions = rwf_lib::input::action_to_transitions(
                                state,
                                &rwf_lib::input::Action::Rename,
                            );
                            for t in transitions {
                                rwf_lib::state::update_state(state, t);
                            }
                        }
                        ContextMenuAction::View => {
                            if let Some(entry) = state.active_pane().current_entry() {
                                if !entry.is_dir {
                                    let loc = entry.location.clone();
                                    rwf_lib::state::update_state(
                                        state,
                                        rwf_lib::state::Transition::OpenTextViewer {
                                            location: loc,
                                        },
                                    );
                                }
                            }
                        }
                        ContextMenuAction::CustomFunction(name) => {
                            let func = state
                                .custom_functions
                                .iter()
                                .find(|f| f.name == name)
                                .cloned();
                            if let Some(func) = func {
                                let expander = rwf_lib::macro_expander::MacroExpander::new();
                                if let Ok(command) = expander.expand(state, &func) {
                                    let working_dir = state.active_pane().current_location.clone();
                                    let shell = func.shell.clone();
                                    return Some(rwf_lib::job::JobSpec::new(
                                        rwf_lib::job::JobKind::ExecuteCustomFunction {
                                            command,
                                            working_dir,
                                            pipe_to_action: func.pipe_to_action.clone(),
                                            shell,
                                        },
                                    ));
                                }
                            }
                        }
                        ContextMenuAction::Separator => {}
                    }
                }
                return None;
            }
            DialogContent::CustomFunctionMenu {
                items,
                selected_index,
            } => {
                let items = items.clone();
                let idx = *selected_index;
                if let Some(item) = items.get(idx) {
                    if item.is_selectable() {
                        return resolve_menu_item_action(state, &item.action);
                    }
                }
                return None;
            }
            _ => {
                debug!("Unknown dialog content type");
            }
        }
    } else {
        debug!("No dialog found");
    }

    None
}

/// Resolve a menu item's `Action` string to a job spec.
/// First tries built-in action names, then looks up a custom function by name.
fn resolve_menu_item_action(
    state: &mut rwf_lib::AppState,
    action_name: &str,
) -> Option<rwf_lib::job::JobSpec> {
    let builtin: Option<rwf_lib::input::Action> = match action_name {
        "DeleteFile" | "Delete" => Some(rwf_lib::input::Action::Delete),
        "MoveFile" | "Move" => Some(rwf_lib::input::Action::Move),
        "CopyFile" | "Copy" => Some(rwf_lib::input::Action::Copy),
        "ViewFileAsText" | "View" => Some(rwf_lib::input::Action::OpenTextViewer),
        "ViewFileAsHex" => Some(rwf_lib::input::Action::OpenHexViewer),
        "ReloadConfiguration" => Some(rwf_lib::input::Action::ReloadConfig),
        "EditConfigFile" => Some(rwf_lib::input::Action::EditConfigFile),
        _ => None,
    };

    if let Some(action) = builtin {
        let transitions = rwf_lib::input::action_to_transitions(state, &action);
        for t in transitions {
            let result = rwf_lib::state::update_state(state, t);
            // Collect logs and reload-keybindings flag into staging fields on AppState
            state
                .pending_confirmation_logs
                .extend(result.task_panel_logs);
            if result.reload_keybindings {
                state.confirmation_needs_keybinding_reload = true;
            }
            if let Some(job) = result.jobs_to_start.into_iter().next() {
                return Some(job);
            }
        }
        return None;
    }

    // Fall back: find by custom function name and execute its command
    let func = state
        .custom_functions
        .iter()
        .find(|f| f.name == action_name)
        .cloned();
    if let Some(func) = func {
        if func.is_command() {
            let expander = rwf_lib::macro_expander::MacroExpander::new();
            if let Ok(command) = expander.expand(state, &func) {
                let working_dir = state.active_pane().current_location.clone();
                let shell = func.shell.clone();
                return Some(rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::ExecuteCustomFunction {
                        command,
                        working_dir,
                        pipe_to_action: func.pipe_to_action.clone(),
                        shell,
                    },
                ));
            }
        }
    }
    None
}
