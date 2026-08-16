//! Dialog confirmation logic: converts a confirmed dialog into JobSpecs /
//! state changes (process_dialog_confirmation and friends).
//!
//! Split from dialog/mod.rs in M3 (move-only).

use rwf_lib::model::dialog::{
    ActionConfirmDialog, CompressionDialog, ConfirmableAction, ContextMenuDialog,
    CustomFunctionMenuDialog, CustomFunctionSelectorContent, DeleteConfirmDialog, DialogContent,
    DriveSelectionDialog, ExtractionConfirmDialog, FileMaskDialog, HistoryDialogContent,
    InputDialog, OpenWithPickerDialog, PatternRenameContent, RegisteredFolderSelectorContent,
    SimpleRenameDialog, SortDialog, TrashBrowserDialog, TypeMismatchWarningDialog,
    WildcardMarkDialog,
};
use tracing::debug;

use super::archive_ext_for_format;

/// Remove the selected entry from a RegisteredFolderSelector dialog.
/// Updates both state.registered_folders and the dialog's own folder list.
/// Returns a log message on success.
pub fn process_dialog_delete(state: &mut rwf_lib::AppState) -> Option<String> {
    // Step 1: resolve actual folder index from the filtered selection (borrow ends at block close).
    let folder_index: Option<usize> = {
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                folders,
                selected_index,
                filter,
            }) = &dialog.content
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
        if let DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
            folders,
            selected_index,
            ..
        }) = &mut dialog.content
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

/// Build a human-readable job name for a move-to-trash operation showing file names.
pub fn trash_job_name(targets: &[rwf_lib::Location]) -> String {
    let file_name = |loc: &rwf_lib::Location| -> String {
        loc.path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| loc.display_path())
    };
    match targets.len() {
        0 => "Move to Trash".to_string(),
        1 => format!("Move '{}' to Trash", file_name(&targets[0])),
        2 => format!(
            "Move '{}', '{}' to Trash",
            file_name(&targets[0]),
            file_name(&targets[1])
        ),
        n => format!("Move {n} files to Trash"),
    }
}

/// Build a human-readable job name for a restore-from-trash operation. Shows the full
/// destination path (not just the filename) for a single record — this label is reused
/// verbatim for the "[OK]"/"[NG]" completion log, which is the only place a user can
/// confirm exactly where a restored file landed.
pub fn restore_job_name(records: &[rwf_lib::model::TrashRecord]) -> String {
    match records.len() {
        0 => "Restore".to_string(),
        1 => format!("Restore to '{}'", records[0].original.display_path()),
        n => format!("Restore {n} files"),
    }
}

/// Process dialog confirmation and create transitions
/// Returns the job spec if a job was created, so it can be submitted to the worker pool
pub fn process_dialog_confirmation(state: &mut rwf_lib::AppState) -> Option<rwf_lib::job::JobSpec> {
    debug!("process_dialog_confirmation called");

    // Input dialogs: extract title and embedded input first so the borrow on state.dialogs ends
    // before we call update_state. Covers both the single-line `Input` dialog
    // (Register Folder, Create Directory, ...) and the Phase 7.17
    // `MultiLineInput` dialog (currently only the diagnostic report prompt) —
    // the `match title.as_str()` below dispatches on title regardless of
    // which variant produced the text, so existing single-line callers are
    // unaffected.
    let input_dialog_info: Option<(String, String)> = state
        .dialogs
        .current()
        .filter(|d| {
            matches!(
                d.content,
                DialogContent::Input { .. } | DialogContent::MultiLineInput { .. }
            )
        })
        .map(|d| {
            let embedded = match &d.content {
                DialogContent::Input(InputDialog { input, .. }) => input.clone(),
                DialogContent::MultiLineInput(dlg) => dlg.text(),
                _ => String::new(),
            };
            (d.title.clone(), embedded)
        });

    if let Some((title, input)) = input_dialog_info {
        match title.as_str() {
            // Phase 7.15. An empty description still finalises the session:
            // the user already did the work of reproducing the problem, and
            // discarding the bundle over a blank field would be hostile.
            crate::app::DIAGNOSTIC_REPORT_DIALOG_TITLE => {
                let report = if input.trim().is_empty() {
                    None
                } else {
                    Some(format!("{input}\n"))
                };
                if let Some(paths) = rwf_lib::diagnostics::stop_session(report) {
                    state.pending_confirmation_logs.push(format!(
                        "[DIAG] Session written to {} — contains file paths and screen \
                         contents, review before sharing",
                        paths.dir.display()
                    ));
                }
            }
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
            "Create File" if !input.is_empty() => {
                let current_location = state.active_pane().current_location.clone();
                let new_file_loc = current_location.join(&input);
                return Some(rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::CreateFile {
                        location: new_file_loc,
                    },
                ));
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
            DialogContent::SortDialog(SortDialog {
                selected_mode_index,
                selected_order_index,
                ..
            }) => {
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
            DialogContent::FileMask(FileMaskDialog { input, .. }) => {
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
            DialogContent::WildcardMark(WildcardMarkDialog { input, .. }) => {
                if !input.is_empty() {
                    let pattern = input.clone();
                    rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::MarkPattern { pattern },
                    );
                }
                return None;
            }
            DialogContent::SimpleRename(SimpleRenameDialog { input, .. }) => {
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
            DialogContent::AttrTimestamp(d) => {
                let attrs = d.to_attribute_change();
                let times = d.to_timestamp_change();
                let targets = d.targets.clone();

                let mut jobs = Vec::new();
                if !attrs.is_empty() {
                    jobs.push(rwf_lib::job::JobSpec::new(
                        rwf_lib::job::JobKind::ChangeAttributes {
                            targets: targets.clone(),
                            attrs,
                        },
                    ));
                }
                if !times.is_empty() {
                    jobs.push(rwf_lib::job::JobSpec::new(
                        rwf_lib::job::JobKind::ChangeTimestamps { targets, times },
                    ));
                }

                if jobs.len() == 1 {
                    return jobs.into_iter().next();
                } else if !jobs.is_empty() {
                    state.pending_confirmation_jobs.extend(jobs);
                }
                return None;
            }
            DialogContent::CreateLink(d) => {
                if d.link_name.is_empty() {
                    return None;
                }
                return Some(rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::CreateLink {
                        target: d.target.clone(),
                        link_path: d.link_path(),
                        kind: d.kind,
                    },
                ));
            }
            DialogContent::DriveSelection(DriveSelectionDialog {
                drives,
                selected_index,
                filter,
            }) => {
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
            DialogContent::HistoryDialog(HistoryDialogContent {
                left_entries,
                right_entries,
                left_selected,
                right_selected,
                active_pane,
                ..
            }) => {
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
            DialogContent::JumpToPath(rwf_lib::model::dialog::JumpToPathDialog {
                suggestions,
                selected_index,
                query,
                search_root,
                loading_job_id,
                ..
            }) => {
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
            DialogContent::JumpToFile(rwf_lib::model::dialog::JumpToFileDialog {
                suggestions,
                selected_index,
                query,
                search_root,
                loading_job_id,
                ..
            }) => {
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
            DialogContent::Compression(CompressionDialog {
                sources,
                archive_name,
                format,
                selected_format_index,
                compression_level,
                ..
            }) => {
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
            DialogContent::ExtractionConfirm(ExtractionConfirmDialog { archive, dest, .. }) => {
                // Create extraction job - dest is already a Location
                let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExtractArchive {
                    archive: archive.clone(),
                    dest: dest.clone(),
                });

                return Some(job_spec);
            }
            DialogContent::DeleteConfirm(DeleteConfirmDialog {
                targets,
                to_trash,
                force_fallback,
                ..
            }) => {
                let locations: Vec<rwf_lib::Location> =
                    targets.iter().map(|(loc, _)| loc.clone()).collect();
                let kind = if *to_trash {
                    rwf_lib::job::JobKind::MoveToTrash {
                        targets: locations,
                        force_fallback: *force_fallback,
                    }
                } else {
                    rwf_lib::job::JobKind::Delete { targets: locations }
                };
                return Some(rwf_lib::job::JobSpec::new(kind));
            }
            DialogContent::TypeMismatchWarning(TypeMismatchWarningDialog {
                command,
                working_dir,
                shell,
                ..
            }) => {
                return Some(rwf_lib::job::JobSpec::execute_association(
                    command.clone(),
                    working_dir.clone(),
                    shell.clone(),
                ));
            }
            DialogContent::OpenWithPicker(OpenWithPickerDialog {
                paths,
                candidates,
                selected_index,
                ..
            }) => {
                let paths = paths.clone();
                let assoc = candidates.get(*selected_index).cloned();
                if let Some(assoc) = assoc {
                    if let Ok((command, working_dir, shell)) =
                        rwf_lib::expand_association_command(state, &assoc)
                    {
                        // One job per file, each routed through the same
                        // ExecuteAssociationChecked gate Task 2 built (per-file magic-byte
                        // mismatch warnings still apply). A single-file picker (the
                        // ordinary cursor-file flow) produces exactly one job, returned
                        // the same way it always was; a multi-file picker (batch
                        // "Open With..." on a marked-file group, Phase 7.3 Task 4)
                        // produces N jobs, which `process_dialog_confirmation`'s single
                        // `Option<JobSpec>` return can't carry — those go through
                        // `pending_confirmation_jobs` instead, drained by app.rs
                        // alongside the single-job path.
                        let mut jobs: Vec<rwf_lib::job::JobSpec> = Vec::with_capacity(paths.len());
                        for path in paths {
                            let result = rwf_lib::state::update_state(
                                state,
                                rwf_lib::state::Transition::ExecuteAssociationChecked {
                                    path,
                                    command: command.clone(),
                                    working_dir: working_dir.clone(),
                                    shell: shell.clone(),
                                },
                            );
                            jobs.extend(result.jobs_to_start);
                        }
                        if jobs.len() == 1 {
                            return jobs.into_iter().next();
                        } else if !jobs.is_empty() {
                            state.pending_confirmation_jobs.extend(jobs);
                        }
                    }
                }
                return None;
            }
            DialogContent::RegisteredFolderSelector(RegisteredFolderSelectorContent {
                folders,
                selected_index,
                filter,
            }) => {
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
            DialogContent::PatternRename(PatternRenameContent {
                find,
                replace,
                use_regex,
                case_sensitive,
                ..
            }) => {
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
            DialogContent::CustomFunctionSelector(CustomFunctionSelectorContent {
                functions,
                selected_index,
                filter,
            }) => {
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
            DialogContent::ContextMenu(ContextMenuDialog {
                options,
                selected_index,
                ..
            }) => {
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
                        ContextMenuAction::OpenWith => {
                            // 2+ candidates: OpenWith pushes the picker dialog and starts
                            // no job. The generic post-confirm path in app.rs pops
                            // whatever is now on top of the stack (the picker we just
                            // pushed, not this ContextMenu) unless we suppress it — same
                            // idiom the "Custom Function Input" $I case above uses.
                            // 0/1 candidates never push a dialog here, so the ContextMenu
                            // must pop normally in those cases.
                            let depth_before = state.dialogs.stack.len();
                            let transitions = rwf_lib::input::action_to_transitions(
                                state,
                                &rwf_lib::input::Action::OpenWith,
                            );
                            let mut job_to_return = None;
                            for t in transitions {
                                let result = rwf_lib::state::update_state(state, t);
                                if let Some(job) = result.jobs_to_start.into_iter().next() {
                                    job_to_return = Some(job);
                                    break;
                                }
                            }
                            if state.dialogs.stack.len() > depth_before {
                                state.suppress_next_dialog_pop = true;
                            }
                            if let Some(job) = job_to_return {
                                return Some(job);
                            }
                        }
                        ContextMenuAction::Separator => {}
                    }
                }
                return None;
            }
            DialogContent::OperationReportView(content) => {
                let actions = content.selected_reversal_actions();
                if actions.is_empty() {
                    return None;
                }
                let operation_name = content.report.operation_name.clone();
                let resulting_is_undo = !content.report.is_undo;

                let (ready, blocked) = rwf_lib::job::preflight_check(&actions);
                if blocked.is_empty() {
                    return Some(rwf_lib::job::JobSpec::new(
                        rwf_lib::job::JobKind::ExecuteReversal {
                            actions: ready,
                            operation_name,
                            resulting_is_undo,
                        },
                    ));
                }

                // Some rows are currently blocked — show the pre-flight
                // summary and let the user decide whether to proceed with
                // just the ready ones.
                let mut message = format!(
                    "{} of {} rows can be {}.\n{} blocked:\n",
                    ready.len(),
                    actions.len(),
                    if resulting_is_undo {
                        "undone"
                    } else {
                        "redone"
                    },
                    blocked.len()
                );
                for (_, reason) in &blocked {
                    message.push_str("  - ");
                    message.push_str(reason);
                    message.push('\n');
                }

                state.dialogs.push(rwf_lib::model::Dialog::action_confirm(
                    format!(
                        "{} {}",
                        if resulting_is_undo { "Undo" } else { "Redo" },
                        operation_name
                    ),
                    message,
                    None,
                    ConfirmableAction::ExecuteReversal {
                        actions: ready,
                        operation_name,
                        resulting_is_undo,
                    },
                ));
                state.suppress_next_dialog_pop = true;
                return None;
            }
            DialogContent::TrashBrowser(TrashBrowserDialog {
                records,
                selected_index,
            }) => {
                let record = records.get(*selected_index)?.clone();
                return Some(rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::RestoreFromTrash {
                        records: vec![record],
                    },
                ));
            }
            DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog {
                items,
                selected_index,
            }) => {
                let items = items.clone();
                let idx = *selected_index;
                if let Some(item) = items.get(idx) {
                    if item.is_selectable() {
                        return resolve_menu_item_action(state, &item.action);
                    }
                }
                return None;
            }
            DialogContent::Confirmation(ActionConfirmDialog { action, .. }) => match action {
                ConfirmableAction::ReloadConfig => {
                    let result = rwf_lib::state::update_state(
                        state,
                        rwf_lib::state::Transition::ReloadConfig,
                    );
                    state
                        .pending_confirmation_logs
                        .extend(result.task_panel_logs);
                    if result.reload_keybindings {
                        state.confirmation_needs_keybinding_reload = true;
                    }
                    return None;
                }
                ConfirmableAction::EmptyTrash { fallback_roots } => {
                    let job_spec = rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::EmptyTrash {
                        scope: rwf_lib::model::EmptyTrashScope::All,
                        older_than_days: None,
                        fallback_roots: fallback_roots.clone(),
                    });
                    return Some(job_spec);
                }
                ConfirmableAction::ExecuteReversal {
                    actions,
                    operation_name,
                    resulting_is_undo,
                } => {
                    if actions.is_empty() {
                        return None;
                    }
                    // Clone out of `action` (borrowed from `state.dialogs`'
                    // current entry) before mutating the stack below — the
                    // borrow checker won't allow `pop_below_top()` while
                    // `action`'s fields are still borrowed.
                    let job_spec =
                        rwf_lib::job::JobSpec::new(rwf_lib::job::JobKind::ExecuteReversal {
                            actions: actions.clone(),
                            operation_name: operation_name.clone(),
                            resulting_is_undo: *resulting_is_undo,
                        });
                    // This summary dialog was pushed ON TOP of the
                    // OperationReportView it was confirmed from (see the
                    // `blocked.is_empty()` branch above), which set
                    // `suppress_next_dialog_pop` so the report dialog stayed
                    // underneath rather than being popped there. The generic
                    // post-confirm path pops only the current (summary)
                    // dialog, so without this the stale report dialog would
                    // be left open underneath after the job starts — pop it
                    // explicitly to match the "pop all related dialogs"
                    // dialog-stack-hygiene rule the direct-submit path
                    // already satisfies for free (no summary dialog means
                    // only the report dialog itself gets popped there).
                    state.dialogs.pop_below_top();
                    return Some(job_spec);
                }
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use rwf_lib::magic::DetectedKind;
    use rwf_lib::model::dialog::{ContextMenuAction, ContextMenuOption, Dialog};
    use rwf_lib::model::Location;
    use rwf_lib::{AppConfig, AppState};
    use std::path::PathBuf;

    fn test_state() -> AppState {
        AppState::new(AppConfig::default())
    }

    #[test]
    fn confirm_create_file_dialog_starts_create_file_job() {
        let mut state = test_state();
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/test"));
        let dialog = Dialog::input("Create File", "File name:", "");
        let mut dialog = dialog;
        if let rwf_lib::model::dialog::DialogContent::Input(rwf_lib::model::InputDialog {
            input,
            ..
        }) = &mut dialog.content
        {
            *input = "newfile.txt".to_string();
        }
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::CreateFile { location } => {
                assert_eq!(
                    location,
                    Location::Local(PathBuf::from("/test/newfile.txt"))
                );
            }
            other => panic!("expected CreateFile, got {:?}", other),
        }
    }

    #[test]
    fn confirm_delete_dialog_builds_move_to_trash_job_when_to_trash() {
        let mut state = test_state();
        let target = Location::Local(PathBuf::from("/test/doomed.txt"));
        let dialog = Dialog::delete_confirm(vec![(target.clone(), false)], true, true);
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::MoveToTrash {
                targets,
                force_fallback,
            } => {
                assert_eq!(targets, vec![target]);
                assert!(force_fallback, "force_fallback must thread through");
            }
            other => panic!("expected MoveToTrash, got {:?}", other),
        }
    }

    #[test]
    fn confirm_delete_dialog_builds_permanent_delete_job_when_not_to_trash() {
        let mut state = test_state();
        let target = Location::Local(PathBuf::from("/test/doomed.txt"));
        let dialog = Dialog::delete_confirm(vec![(target.clone(), false)], false, false);
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::Delete { targets } => {
                assert_eq!(targets, vec![target]);
            }
            other => panic!("expected Delete, got {:?}", other),
        }
    }

    #[cfg(windows)]
    #[test]
    fn confirm_attr_timestamp_dialog_starts_change_attributes_job() {
        use tempfile::TempDir;

        let mut state = test_state();
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        std::fs::write(&file_a, b"x").unwrap();

        let mut dialog =
            rwf_lib::model::Dialog::attr_timestamp(vec![Location::Local(file_a.clone())]);
        if let rwf_lib::model::dialog::DialogContent::AttrTimestamp(d) = &mut dialog.content {
            d.hidden.toggle();
        } else {
            panic!("expected AttrTimestamp dialog");
        }
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::ChangeAttributes { targets, attrs } => {
                assert_eq!(targets, vec![Location::Local(file_a)]);
                assert_eq!(attrs.hidden, Some(true));
            }
            other => panic!("expected ChangeAttributes, got {:?}", other),
        }
        assert!(state.pending_confirmation_jobs.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn confirm_attr_timestamp_dialog_with_no_edits_starts_no_job() {
        use tempfile::TempDir;

        let mut state = test_state();
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        std::fs::write(&file_a, b"x").unwrap();

        let dialog = rwf_lib::model::Dialog::attr_timestamp(vec![Location::Local(file_a)]);
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state);
        assert!(job_spec.is_none());
        assert!(state.pending_confirmation_jobs.is_empty());
    }

    #[test]
    fn confirm_create_link_dialog_starts_create_link_job() {
        use tempfile::TempDir;

        let mut state = test_state();
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        std::fs::write(&target, b"x").unwrap();

        let dialog = rwf_lib::model::Dialog::create_link(
            Location::Local(target.clone()),
            temp_dir.path().to_path_buf(),
        );
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::CreateLink {
                target: job_target,
                link_path,
                kind,
            } => {
                assert_eq!(job_target, Location::Local(target));
                assert_eq!(
                    link_path,
                    Location::Local(temp_dir.path().join("target.txt"))
                );
                assert_eq!(kind, rwf_lib::model::LinkCreateKind::Symlink);
            }
            other => panic!("expected CreateLink, got {:?}", other),
        }
    }

    #[test]
    fn confirm_create_link_dialog_with_empty_name_starts_no_job() {
        use tempfile::TempDir;

        let mut state = test_state();
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        std::fs::write(&target, b"x").unwrap();

        let mut dialog = rwf_lib::model::Dialog::create_link(
            Location::Local(target),
            temp_dir.path().to_path_buf(),
        );
        if let rwf_lib::model::dialog::DialogContent::CreateLink(d) = &mut dialog.content {
            d.link_name.clear();
        }
        state.dialogs.push(dialog);

        assert!(process_dialog_confirmation(&mut state).is_none());
    }

    #[test]
    fn confirm_create_directory_dialog_starts_mkdir_job() {
        let mut state = test_state();
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/test"));
        let mut dialog = Dialog::input("Create Directory", "Directory name:", "");
        if let rwf_lib::model::dialog::DialogContent::Input(rwf_lib::model::InputDialog {
            input,
            ..
        }) = &mut dialog.content
        {
            *input = "newdir".to_string();
        }
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::Mkdir { location } => {
                assert_eq!(location, Location::Local(PathBuf::from("/test/newdir")));
            }
            other => panic!("expected Mkdir, got {:?}", other),
        }
    }

    /// Confirming a TypeMismatchWarning dialog must run the *original*
    /// association command — the whole point of the dialog is "warn, then
    /// let the user proceed unchanged" (see plan/7.3.smart_file_opener.md §5).
    #[test]
    fn confirm_type_mismatch_warning_runs_original_command() {
        let mut state = test_state();
        let dialog = Dialog::type_mismatch_warning(
            PathBuf::from("/test/notes.txt"),
            DetectedKind::Pe,
            "notepad $F".to_string(),
            Location::Local(PathBuf::from("/test")),
            Some("cmd".to_string()),
        );
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::ExecuteCustomFunction {
                command,
                working_dir,
                shell,
                pipe_to_action,
            } => {
                assert_eq!(command, "notepad $F");
                assert_eq!(working_dir, Location::Local(PathBuf::from("/test")));
                assert_eq!(shell, Some("cmd".to_string()));
                assert!(pipe_to_action.is_none());
            }
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }

    /// TypeMismatchWarning needs no per-variant Cancel handling: Esc falls
    /// through `handle_dialog_input`'s generic "Esc cancels" arm (see
    /// mod.rs's `handle_dialog_input`), and app.rs's `DialogAction::Cancel`
    /// arm does nothing but pop. This drives the real Esc -> DialogAction
    /// dispatch (the part reachable without an `App`/terminal harness) and
    /// then replicates app.rs's pop, verifying the stack ends up clean and
    /// no job is ever produced.
    #[test]
    fn esc_on_type_mismatch_warning_returns_cancel_and_pop_leaves_no_side_effects() {
        let mut state = test_state();
        let dialog = Dialog::type_mismatch_warning(
            PathBuf::from("/test/notes.txt"),
            DetectedKind::Pe,
            "notepad $F".to_string(),
            Location::Local(PathBuf::from("/test")),
            None,
        );
        state.dialogs.push(dialog);
        assert!(!state.dialogs.is_empty());

        let action = super::super::handle_dialog_input(
            state.dialogs.current_mut().expect("dialog pushed above"),
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ),
            None,
        );
        assert_eq!(action, super::super::DialogAction::Cancel);

        // app.rs's DialogAction::Cancel arm does nothing but pop for this
        // dialog — no call to process_dialog_confirmation, so no job is
        // ever produced.
        state.dialogs.pop();
        assert!(state.dialogs.is_empty());
        assert_eq!(state.jobs.queue.len(), 0);
        assert_eq!(state.jobs.active.len(), 0);
    }

    /// Confirming the OpenWithPicker dialog must expand and run the
    /// *selected* candidate's command (Phase 7.3 §3) — not the first one —
    /// routed through the same `ExecuteAssociationChecked` gate the
    /// single-match EnterDirectory path uses.
    #[test]
    fn confirm_open_with_picker_runs_selected_candidate() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false; // exercise the direct path
        let candidates = vec![
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "less".to_string(),
                description: Some("View with less".to_string()),
                shell: None,
            },
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "notepad".to_string(),
                description: Some("Edit with Notepad".to_string()),
                shell: Some("cmd".to_string()),
            },
        ];
        let mut dialog =
            Dialog::open_with_picker(vec![PathBuf::from("/test/server.log")], candidates, None);
        if let rwf_lib::model::dialog::DialogContent::OpenWithPicker(d) = &mut dialog.content {
            d.selected_index = 1; // pick the second candidate, not the first
        } else {
            panic!("expected OpenWithPicker dialog");
        }
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::ExecuteCustomFunction {
                command,
                shell,
                pipe_to_action,
                ..
            } => {
                assert_eq!(command, "notepad");
                assert_eq!(shell, Some("cmd".to_string()));
                assert!(pipe_to_action.is_none());
            }
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }

    /// selected_index=0 (the default / only-navigated-away-from case) runs
    /// the first candidate, confirming the picker isn't just always running
    /// whichever candidate happens to be last.
    #[test]
    fn confirm_open_with_picker_default_selection_runs_first_candidate() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false;
        let candidates = vec![
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "less".to_string(),
                description: None,
                shell: None,
            },
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "notepad".to_string(),
                description: None,
                shell: None,
            },
        ];
        let dialog =
            Dialog::open_with_picker(vec![PathBuf::from("/test/server.log")], candidates, None);
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state).expect("expected a job spec");
        match job_spec.kind {
            rwf_lib::job::JobKind::ExecuteCustomFunction { command, .. } => {
                assert_eq!(command, "less");
            }
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }

    /// Confirming a widened picker over a marked-file group (Phase 7.3 Task 4,
    /// batch "Open With...") must start one job per file. `process_dialog_confirmation`'s
    /// single `Option<JobSpec>` return can't carry 3 jobs, so they're routed through
    /// `state.pending_confirmation_jobs` instead (decision C) — verify all 3 land there,
    /// nothing is returned directly, and each carries the selected candidate's command.
    #[test]
    fn confirm_open_with_picker_multi_file_group_starts_job_per_file() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false;
        let candidates = vec![
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "less".to_string(),
                description: None,
                shell: None,
            },
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "notepad".to_string(),
                description: None,
                shell: None,
            },
        ];
        let mut dialog = Dialog::open_with_picker(
            vec![
                PathBuf::from("/test/a.log"),
                PathBuf::from("/test/b.log"),
                PathBuf::from("/test/c.log"),
            ],
            candidates,
            None,
        );
        if let rwf_lib::model::dialog::DialogContent::OpenWithPicker(d) = &mut dialog.content {
            d.selected_index = 1; // "notepad"
        } else {
            panic!("expected OpenWithPicker dialog");
        }
        state.dialogs.push(dialog);

        assert!(state.pending_confirmation_jobs.is_empty());
        let job_spec = process_dialog_confirmation(&mut state);
        assert!(
            job_spec.is_none(),
            "3-file group must route through pending_confirmation_jobs, not the single-job return"
        );

        assert_eq!(state.pending_confirmation_jobs.len(), 3);
        for job in &state.pending_confirmation_jobs {
            match &job.kind {
                rwf_lib::job::JobKind::ExecuteCustomFunction { command, .. } => {
                    assert_eq!(command, "notepad");
                }
                other => panic!("expected ExecuteCustomFunction, got {:?}", other),
            }
        }
    }

    /// Places `entry` as the cursor entry of the active (left) pane so
    /// `Action::OpenWith` (delegated to via `ContextMenuAction::OpenWith`)
    /// has something to resolve extension associations against.
    fn set_cursor_entry(state: &mut AppState, entry: rwf_lib::model::FileEntry) {
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;
    }

    fn log_file_entry(name: &str) -> rwf_lib::model::FileEntry {
        rwf_lib::model::FileEntry {
            name: name.to_string(),
            location: rwf_lib::model::Location::Local(PathBuf::from(format!("/test/{name}"))),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: std::time::SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    /// Regression test for the ghost-dialog bug: selecting "Open With..."
    /// from the ContextMenu with 2+ matching associations must leave the
    /// picker on top of the stack, not have it silently popped away by the
    /// generic post-confirm path (see `suppress_next_dialog_pop` in the
    /// `ContextMenuAction::OpenWith` arm above). This drives the real
    /// `process_dialog_confirmation` dispatch the way `app.rs` does, then
    /// asserts the survived stack shape.
    #[test]
    fn confirm_context_menu_open_with_two_candidates_leaves_picker_on_stack() {
        let mut state = test_state();
        set_cursor_entry(&mut state, log_file_entry("server.log"));
        state.extension_associations = vec![
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "less".to_string(),
                description: None,
                shell: None,
            },
            rwf_lib::config::ExtensionAssociation {
                extension: Some("log".to_string()),
                file_type: None,
                command: "notepad".to_string(),
                description: None,
                shell: None,
            },
        ];
        let mut dialog =
            rwf_lib::model::dialog::Dialog::context_menu_with_options(vec![ContextMenuOption {
                label: "Open With...".to_string(),
                action: ContextMenuAction::OpenWith,
            }]);
        if let DialogContent::ContextMenu(ContextMenuDialog { selected_index, .. }) =
            &mut dialog.content
        {
            *selected_index = 0;
        }
        state.dialogs.push(dialog);
        let depth_before_confirm = state.dialogs.stack.len();

        // Phase 7.3b: with magic-byte detection on (the default), resolving
        // "Open With..." candidates is no longer synchronous — it defers to the
        // detect-then-resolve pipeline (`Transition::ResolveAssociationByType`),
        // which starts a `DetectFileType` job instead of pushing the picker
        // immediately. No dialog is pushed on this tick, so nothing needs
        // suppressing; the ContextMenu pops normally, exactly like the
        // "no candidates" companion case below.
        let job_spec = process_dialog_confirmation(&mut state);
        assert!(
            job_spec.is_some(),
            "expected the DetectFileType job that the detect-then-resolve pipeline starts"
        );
        assert!(
            !state.suppress_next_dialog_pop,
            "no dialog was pushed synchronously, so the ContextMenu must pop normally"
        );

        // Replicate app.rs's post-confirm handling: check + consume the
        // suppress flag, only pop if it wasn't set.
        let mut should_pop = true;
        if state.suppress_next_dialog_pop {
            state.suppress_next_dialog_pop = false;
            should_pop = false;
        }
        if should_pop {
            state.dialogs.pop();
        }
        assert_eq!(
            state.dialogs.stack.len(),
            depth_before_confirm - 1,
            "ContextMenu should have popped normally (no picker on this tick)"
        );

        // Now drive the detect job to completion the way app.rs's real job
        // pipeline would (enqueue -> start -> complete). Plain-text content
        // (Unknown) is enough here since both candidates are pure-extension.
        let job_spec = job_spec.expect("checked above");
        rwf_lib::state::update_state(
            &mut state,
            rwf_lib::state::Transition::EnqueueJob {
                spec: job_spec.clone(),
            },
        );
        let job_id = state.jobs.queue[0].id;
        rwf_lib::state::update_state(&mut state, rwf_lib::state::Transition::StartNextJob);
        rwf_lib::state::update_state(
            &mut state,
            rwf_lib::state::Transition::CompleteJob {
                job_id,
                result: rwf_lib::job::OpResult::Success(
                    rwf_lib::job::SuccessData::FileTypeDetected {
                        kind: rwf_lib::magic::DetectedKind::Unknown,
                        header_bytes: Vec::new(),
                    },
                ),
            },
        );

        assert_eq!(
            state.dialogs.stack.len(),
            1,
            "the picker should now be on top, pushed once detection completed"
        );
        match &state.dialogs.current().expect("picker on top").content {
            DialogContent::OpenWithPicker(d) => assert_eq!(d.candidates.len(), 2),
            other => panic!(
                "expected OpenWithPicker on top of the stack, got {:?}",
                other
            ),
        }
    }

    /// Companion case: with 0 matching associations, `action_to_transitions`
    /// returns nothing, no dialog is pushed, and the ContextMenu must pop
    /// normally (not suppressed, no ghost left behind).
    #[test]
    fn confirm_context_menu_open_with_no_candidates_pops_normally() {
        let mut state = test_state();
        set_cursor_entry(&mut state, log_file_entry("notes.md"));
        state.extension_associations = Vec::new();
        let mut dialog =
            rwf_lib::model::dialog::Dialog::context_menu_with_options(vec![ContextMenuOption {
                label: "Open With...".to_string(),
                action: ContextMenuAction::OpenWith,
            }]);
        if let DialogContent::ContextMenu(ContextMenuDialog { selected_index, .. }) =
            &mut dialog.content
        {
            *selected_index = 0;
        }
        state.dialogs.push(dialog);

        let job_spec = process_dialog_confirmation(&mut state);
        assert!(job_spec.is_none());
        assert!(
            !state.suppress_next_dialog_pop,
            "no dialog was pushed, so the ContextMenu must pop normally"
        );

        let mut should_pop = true;
        if state.suppress_next_dialog_pop {
            state.suppress_next_dialog_pop = false;
            should_pop = false;
        }
        if should_pop {
            state.dialogs.pop();
        }

        assert!(
            state.dialogs.is_empty(),
            "ContextMenu should have popped normally, leaving no ghost dialog"
        );
    }
}
