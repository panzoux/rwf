use crate::job::JobSpec;
use crate::state::{AppState, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_viewer_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::OpenTextViewer { location } => {
                self.ui.mode = crate::model::UIMode::Viewer;
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = crate::model::ViewerMode::Text;
                self.viewer = Some(viewer);
                let threshold = (self.config.viewer_large_file_threshold_mb as usize) * 1024 * 1024;
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(),
                    index_lines: true,
                    large_file_threshold: threshold,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenHexViewer { location } => {
                self.ui.mode = crate::model::UIMode::Viewer;
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = crate::model::ViewerMode::Hex;
                self.viewer = Some(viewer);
                let threshold = (self.config.viewer_large_file_threshold_mb as usize) * 1024 * 1024;
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(),
                    index_lines: false,
                    large_file_threshold: threshold,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::ReloadViewer { location, mode } => {
                // Cancel the previous loading job.
                if let Some(job_id) = self.viewer_job_id.take() {
                    self.jobs.request_cancel(job_id);
                }
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = *mode;
                self.viewer = Some(viewer);
                self.viewer_search_input.clear();
                let index_lines = *mode == crate::model::ViewerMode::Text;
                let threshold = (self.config.viewer_large_file_threshold_mb as usize) * 1024 * 1024;
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(),
                    index_lines,
                    large_file_threshold: threshold,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::OpenSideBySideViewer { location, mode } => {
                // File pane keeps focus; viewer appears alongside it.
                self.ui.mode = crate::model::UIMode::Normal;
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::SideBySide;
                self.ui.layout.viewer_preferred_layout = crate::model::ViewerLayout::SideBySide;
                // Pin the viewer to the opposite side of the current active pane.
                // This stays fixed for the duration of the SideBySide session.
                self.ui.layout.viewer_anchor_pane = self.ui.active_pane;
                let mut viewer = crate::model::ViewerState::new(location.clone());
                viewer.mode = *mode;
                self.viewer = Some(viewer);
                let threshold = (self.config.viewer_large_file_threshold_mb as usize) * 1024 * 1024;
                let job_spec = JobSpec::new(crate::job::JobKind::LoadFileForViewer {
                    location: location.clone(),
                    index_lines: *mode == crate::model::ViewerMode::Text,
                    large_file_threshold: threshold,
                });
                self.viewer_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            Transition::CloseViewer => {
                if let Some(job_id) = self.viewer_job_id.take() {
                    self.jobs.request_cancel(job_id);
                }
                if let Some(job_id) = self.viewer_search_job_id.take() {
                    self.jobs.request_cancel(job_id);
                }
                self.ui.mode = crate::model::UIMode::Normal;
                self.viewer = None;
                self.viewer_search_input.clear();
                self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerSwitchLayout { layout } => {
                match layout {
                    crate::model::ViewerLayout::FullScreen => {
                        // Viewer takes full focus; remember that user came from SideBySide.
                        self.ui.mode = crate::model::UIMode::Viewer;
                        self.ui.layout.viewer_preferred_layout =
                            crate::model::ViewerLayout::SideBySide;
                    }
                    crate::model::ViewerLayout::SideBySide => {
                        // File pane gets focus.
                        self.ui.mode = crate::model::UIMode::Normal;
                    }
                }
                self.ui.layout.viewer_layout = *layout;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerReady { buffer, encoding } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.buffer = Some(buffer.clone());
                    // Only apply detected encoding on first arrival (encoding may have
                    // been manually changed by the user before the job completes).
                    if viewer.encoding == crate::model::viewer::TextEncoding::Utf8 {
                        viewer.encoding = *encoding;
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerLoadComplete { contents } => {
                // Legacy path: used by tests and the ViewerLoadComplete transition.
                if let Some(ref mut viewer) = self.viewer {
                    viewer.set_contents(contents.clone());
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerSearchComplete { job_id, matches } => {
                // Discard stale results from a superseded search job.
                if self.viewer_search_job_id != Some(*job_id) {
                    return Some(StateUpdateResult::none());
                }
                self.viewer_search_job_id = None;
                if let Some(ref mut viewer) = self.viewer {
                    viewer.is_searching = false;
                    viewer.search_matches = matches.clone();
                    if !matches.is_empty() {
                        let start_idx = if viewer.search_forward {
                            matches
                                .iter()
                                .position(|&(l, _, _)| l >= viewer.line_offset)
                                .unwrap_or(0)
                        } else {
                            matches
                                .iter()
                                .rposition(|&(l, _, _)| l <= viewer.line_offset)
                                .unwrap_or(matches.len() - 1)
                        };
                        viewer.search_match_index = Some(start_idx);
                        if viewer.address_query.is_none() {
                            viewer.jump_to_match(start_idx);
                        }
                    }
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerCycleEncoding => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.cycle_encoding();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerToggleMode => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.mode = match viewer.mode {
                        crate::model::ViewerMode::Text => crate::model::ViewerMode::Hex,
                        crate::model::ViewerMode::Hex => crate::model::ViewerMode::Text,
                    };
                    viewer.clear_search();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollDown { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.scroll_down(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollUp => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.scroll_up();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerPageDown { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.page_down(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerPageUp { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.page_up(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerJumpToTop => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.jump_to_top();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerJumpToBottom { viewport_height } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.jump_to_bottom(*viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerJumpToLine {
                line_idx,
                viewport_height,
            } => {
                if let Some(ref mut viewer) = self.viewer {
                    let max = if viewer.mode == crate::model::ViewerMode::Hex {
                        viewer.hex_line_count()
                    } else {
                        viewer.line_count()
                    };
                    viewer.line_offset = (*line_idx).min(max.saturating_sub(*viewport_height));
                    viewer.column_offset = 0;
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerMoveToLineStart => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.move_to_line_start();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerMoveToLineEnd { viewport_width } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.move_to_line_end(*viewport_width);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerStartSearch { query } => {
                // Cancel any in-progress search.
                if let Some(old) = self.viewer_search_job_id.take() {
                    self.jobs.request_cancel(old);
                }
                if self.viewer.is_none() {
                    return Some(StateUpdateResult::with_ui_change());
                }
                let result = self.start_viewer_search_background(query);
                Some(result)
            }
            Transition::ViewerFindNext => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.find_next_in_dir();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFindPrev => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.find_prev_in_dir();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerClearSearch => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.clear_search();
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerToggleCaseSensitive => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.case_sensitive = !viewer.case_sensitive;
                }
                if let Some(query) = self.viewer.as_ref().and_then(|v| v.search_query.clone()) {
                    if let Some(old) = self.viewer_search_job_id.take() {
                        self.jobs.request_cancel(old);
                    }
                    let result = self.start_viewer_search_background(&query);
                    return Some(result);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollLeft { cols } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.scroll_left(*cols);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerScrollRight { cols } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.scroll_right(*cols);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFastScrollUp { lines } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.fast_scroll_up(*lines);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ViewerFastScrollDown {
                lines,
                viewport_height,
            } => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.fast_scroll_down(*lines, *viewport_height);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }
}
