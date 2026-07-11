use crate::job::JobSpec;
use crate::state::{AppState, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_advanced_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::SyncPanes => {
                let active_location = self.active_pane().current_location.clone();
                let opposite_pane = self.ui.active_pane.opposite();

                let cached_entries = self.cache.get(&active_location);

                let tab = self.current_tab_mut();
                let opposite_pane_model = match opposite_pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };

                tab.history
                    .push(opposite_pane, opposite_pane_model.current_location.clone());

                opposite_pane_model.current_location = active_location.clone();
                opposite_pane_model.cursor = 0;
                opposite_pane_model.scroll_offset = 0;

                if let Some(entries) = cached_entries {
                    opposite_pane_model.entries = entries;
                    opposite_pane_model.apply_sort();
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory {
                        location: active_location,
                    });
                    Some(StateUpdateResult::with_job(job_spec))
                }
            }
            Transition::SwapPanes => {
                let tab = self.current_tab_mut();

                // Swap locations only (Requirement 41.5: cursors stay with panes)
                std::mem::swap(
                    &mut tab.left_pane.current_location,
                    &mut tab.right_pane.current_location,
                );

                tab.history.swap_panes();

                let left_location = tab.left_pane.current_location.clone();
                let right_location = tab.right_pane.current_location.clone();

                let left_cached = self.cache.get(&left_location);
                let right_cached = self.cache.get(&right_location);

                let tab = self.current_tab_mut();

                let left_needs_job = if let Some(entries) = left_cached {
                    tab.left_pane.entries = entries;
                    tab.left_pane.apply_sort();
                    false
                } else {
                    true
                };

                let right_needs_job = if let Some(entries) = right_cached {
                    tab.right_pane.entries = entries;
                    tab.right_pane.apply_sort();
                    false
                } else {
                    true
                };

                let mut result = StateUpdateResult::with_ui_change();

                if left_needs_job {
                    result
                        .jobs_to_start
                        .push(JobSpec::new(crate::job::JobKind::ReadDirectory {
                            location: left_location,
                        }));
                }

                if right_needs_job {
                    result
                        .jobs_to_start
                        .push(JobSpec::new(crate::job::JobKind::ReadDirectory {
                            location: right_location,
                        }));
                }

                Some(result)
            }
            Transition::CompareFiles { left, right } => {
                let job_spec = JobSpec::new(crate::job::JobKind::CompareFiles {
                    left: left.clone(),
                    right: right.clone(),
                });
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowComparisonView { diff } => {
                let dialog = crate::model::Dialog {
                    title: "File Comparison".to_string(),
                    content: crate::model::DialogContent::ComparisonView(
                        crate::model::ComparisonViewDialog::new(diff.clone()),
                    ),
                };
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CloseComparisonView => {
                self.dialogs.pop();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ShowSplitJoinDialog => {
                let dialog = crate::model::Dialog::split_join_dialog();
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ExecuteFileSplit {
                source,
                dest_dir,
                chunk_size,
            } => {
                let job_spec = JobSpec::new(crate::job::JobKind::SplitFile {
                    source: source.clone(),
                    dest_dir: dest_dir.clone(),
                    chunk_size: *chunk_size,
                });
                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ExecuteFileJoin { parts, dest } => {
                let job_spec = JobSpec::new(crate::job::JobKind::JoinFiles {
                    parts: parts.clone(),
                    dest: dest.clone(),
                });
                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ShowPatternRenameDialog => {
                let pane = self.active_pane();
                if pane.entries.is_empty() {
                    return Some(StateUpdateResult::none());
                }
                let filenames: Vec<&str> = if pane.marking.count() > 0 {
                    pane.entries
                        .iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.name.as_str())
                        .collect()
                } else {
                    pane.entries.iter().map(|e| e.name.as_str()).collect()
                };
                // Pre-populate preview so the dialog opens at full size with all files visible
                let initial_preview =
                    crate::pattern_rename::generate_preview(&filenames, "", "", true, false);
                let mut dialog = crate::model::Dialog::pattern_rename();
                if let crate::model::DialogContent::PatternRename(
                    crate::model::PatternRenameContent { preview, .. },
                ) = &mut dialog.content
                {
                    *preview = initial_preview;
                }
                self.dialogs.push(dialog);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdatePatternRenameFields {
                find,
                replace,
                use_regex,
                case_sensitive,
            } => {
                let pane = self.active_pane();
                let filenames: Vec<&str> = if pane.marking.count() > 0 {
                    pane.entries
                        .iter()
                        .filter(|e| pane.marking.is_marked(&e.location))
                        .map(|e| e.name.as_str())
                        .collect()
                } else {
                    pane.entries.iter().map(|e| e.name.as_str()).collect()
                };

                let preview = crate::pattern_rename::generate_preview(
                    &filenames,
                    find,
                    replace,
                    *use_regex,
                    *case_sensitive,
                );
                if let Some(dialog) = self.dialogs.current_mut() {
                    if let crate::model::DialogContent::PatternRename(
                        crate::model::PatternRenameContent {
                            find: f,
                            replace: r,
                            use_regex: ur,
                            case_sensitive: cs,
                            preview: pr,
                            error_message: em,
                            ..
                        },
                    ) = &mut dialog.content
                    {
                        *f = find.clone();
                        *r = replace.clone();
                        *ur = *use_regex;
                        *cs = *case_sensitive;
                        *pr = preview;
                        *em = None;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ExecutePatternRename {
                find,
                replace,
                use_regex,
                case_sensitive,
                targets,
            } => {
                let job_spec = JobSpec::new(crate::job::JobKind::PatternRename {
                    targets: targets.clone(),
                    find: find.clone(),
                    replace: replace.clone(),
                    use_regex: *use_regex,
                    case_sensitive: *case_sensitive,
                });
                self.dialogs.pop();
                Some(StateUpdateResult::with_job(job_spec))
            }
            _ => None,
        }
    }
}
