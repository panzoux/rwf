//! Background polling (Phase 7.5 T2): which panes a tick reads, and what happens to the
//! result. Pure — the App loop supplies the time and runs the jobs.

use crate::job::{JobId, JobKind, JobOrigin, JobSpec, OpResult, SuccessData};
use crate::model::polling::{PaneKey, PollInFlight, PollingState, POLL_POOL_WORKERS};
use crate::model::{ActivePane, Location, UIMode, ViewerLayout};
use crate::state::{update_state, AppState, StateUpdateResult, Transition};
use std::time::{Duration, Instant};

impl AppState {
    pub(crate) fn handle_polling_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::PollTick { now } => Some(self.poll_tick(*now)),
            _ => None,
        }
    }

    /// How long until the next visible pane is due for a poll: `Some(ZERO)` when one is
    /// due now, `None` when nothing is eligible (polling off, paused, every pane busy).
    pub fn poll_due_in(&self, now: Instant) -> Option<Duration> {
        PollingState::interval(self.config.polling_interval_ms)?;
        if self.polling.in_flight.len() >= POLL_POOL_WORKERS {
            return None;
        }
        self.pollable_panes()
            .into_iter()
            .map(|(key, _)| match self.polling.next_due.get(&key) {
                Some(due) => due.saturating_duration_since(now),
                None => Duration::ZERO,
            })
            .min()
    }

    /// Whether any poll is still running, so the loop should wake to collect it.
    pub fn poll_in_flight(&self) -> bool {
        !self.polling.in_flight.is_empty()
    }

    /// Visible panes of the active tab that may be polled right now, ignoring timing.
    ///
    /// Pauses (D4, D15): a FullScreen viewer hides both panes; a SideBySide viewer hides
    /// all but its anchor; range marking and leap mode hold row indices of the active
    /// pane. `SuspendAndRun` needs no rule — it blocks the App loop, so no tick is sent.
    fn pollable_panes(&self) -> Vec<(PaneKey, Location)> {
        let viewer_layout = self.viewer.as_ref().map(|_| self.ui.layout.viewer_layout);
        if viewer_layout == Some(ViewerLayout::FullScreen) {
            return Vec::new();
        }
        let tab = self.current_tab();
        [ActivePane::Left, ActivePane::Right]
            .into_iter()
            .filter(|side| {
                viewer_layout != Some(ViewerLayout::SideBySide)
                    || *side == self.ui.layout.viewer_anchor_pane
            })
            .filter(|side| {
                *side != self.ui.active_pane
                    || (self.ui.range_marking_start.is_none()
                        && self.leap.is_none()
                        && self.ui.mode != UIMode::Leap)
            })
            .filter_map(|side| {
                let pane = match side {
                    ActivePane::Left => &tab.left_pane,
                    ActivePane::Right => &tab.right_pane,
                };
                let key = (tab.id, side);
                let eligible = matches!(pane.current_location, Location::Local(_))
                    && pane.active_job_id.is_none()
                    && !self.polling.in_flight.contains_key(&key);
                eligible.then(|| (key, pane.current_location.clone()))
            })
            .collect()
    }

    fn poll_tick(&mut self, now: Instant) -> StateUpdateResult {
        let mut result = StateUpdateResult::none();
        if PollingState::interval(self.config.polling_interval_ms).is_none() {
            return result;
        }
        for (key, location) in self.pollable_panes() {
            if self.polling.in_flight.len() >= POLL_POOL_WORKERS {
                break;
            }
            if self
                .polling
                .next_due
                .get(&key)
                .is_some_and(|due| now < *due)
            {
                continue;
            }
            let Some(generation) = self.pane_by_key(key).map(|p| p.listing_generation) else {
                continue;
            };
            let job = JobSpec::new(JobKind::ReadDirectory {
                location: location.clone(),
            })
            .with_requesting_pane(key.0, key.1)
            .with_origin(JobOrigin::Poll);
            self.polling.in_flight.insert(
                key,
                PollInFlight {
                    job_id: job.id,
                    location,
                    generation,
                    submitted_at: now,
                },
            );
            result.jobs_to_start.push(job);
        }
        result
    }

    fn pane_by_key(&self, (tab_id, side): PaneKey) -> Option<&crate::model::PaneModel> {
        let tab = self.tabs.tabs.iter().find(|t| t.id == tab_id)?;
        Some(match side {
            ActivePane::Left => &tab.left_pane,
            ActivePane::Right => &tab.right_pane,
        })
    }

    fn pane_by_key_mut(&mut self, (tab_id, side): PaneKey) -> Option<&mut crate::model::PaneModel> {
        let tab = self.tabs.tabs.iter_mut().find(|t| t.id == tab_id)?;
        Some(match side {
            ActivePane::Left => &mut tab.left_pane,
            ActivePane::Right => &mut tab.right_pane,
        })
    }

    /// Finish the bookkeeping for a completed poll and schedule the pane's next one
    /// from now (D10b). Returns the in-flight record when the pane may still take the
    /// result: same directory, no user read started meanwhile, listing unchanged since
    /// submission (D12).
    fn finish_poll(&mut self, job_id: JobId) -> Option<(PaneKey, PollInFlight, bool)> {
        let key = self.polling.owns(job_id)?;
        let poll = self.polling.in_flight.remove(&key)?;
        if let Some(interval) = PollingState::interval(self.config.polling_interval_ms) {
            self.polling.next_due.insert(key, Instant::now() + interval);
        }
        let current = self.pane_by_key(key).is_some_and(|pane| {
            pane.current_location == poll.location
                && pane.active_job_id.is_none()
                && pane.listing_generation == poll.generation
        });
        Some((key, poll, current))
    }

    /// Completion of a `ReadDirectory` with `JobOrigin::Poll`.
    ///
    /// Success applies the listing like any refresh. Failure is split (D9): the
    /// directory may simply have been deleted, which `ResolveFallbackPath` finds out on
    /// a worker — the returned job — and anything else stays silent apart from one
    /// task-panel line when failures start and one when they stop.
    pub(crate) fn complete_poll_read(
        &mut self,
        job_id: JobId,
        result: &OpResult,
        result_obj: &mut StateUpdateResult,
    ) {
        let Some((key, poll, current)) = self.finish_poll(job_id) else {
            return;
        };
        match result {
            OpResult::Success(SuccessData::DirectoryRead(entries)) => {
                if let Some(failed_at) = self.polling.failing.remove(&key) {
                    if failed_at == poll.location {
                        result_obj.task_panel_logs.push(format!(
                            "{} [OK] Polling: {} is readable again",
                            chrono::Local::now().format("[%H:%M:%S]"),
                            poll.location.display_path()
                        ));
                        result_obj.ui_changed = true;
                    }
                }
                if !current {
                    tracing::debug!(
                        "[Poll] discarding result for {} (pane moved, busy or changed)",
                        poll.location.display_path()
                    );
                    return;
                }
                let pane_height = self.ui.layout.pane_height;
                let scroll_offset = self.config.ui.scroll_offset;
                if let Some(pane) = self.pane_by_key_mut(key) {
                    if pane.apply_directory_listing(
                        &poll.location,
                        entries.clone(),
                        pane_height,
                        scroll_offset,
                    ) {
                        result_obj.ui_changed = true;
                    }
                }
            }
            OpResult::Failed(message) => {
                if !current || self.polling.failing.contains_key(&key) {
                    return;
                }
                let fallback = JobSpec::new(JobKind::ResolveFallbackPath {
                    requested: poll.location,
                })
                .with_requesting_pane(key.0, key.1)
                .with_origin(JobOrigin::Poll);
                self.pending_read_failures
                    .insert(fallback.id, message.clone());
                result_obj.jobs_to_start.push(fallback);
            }
            _ => {}
        }
    }

    /// Completion of the `ResolveFallbackPath` a failed poll asked for.
    ///
    /// Only a readable **parent** means the directory was deleted: the pane lands there
    /// as it would after any failed read. Anything else — the path exists after all
    /// (access denied, a timeout), or only something further up survives (an unmounted
    /// drive, an unreachable share) — marks the pane as failing, silently apart from
    /// one line.
    pub(crate) fn complete_poll_fallback(
        &mut self,
        spec: &JobSpec,
        requested: &Location,
        result: &OpResult,
        result_obj: &mut StateUpdateResult,
    ) {
        let message = self.pending_read_failures.remove(&spec.id);
        let Some(key) = spec.requesting_pane else {
            return;
        };
        let found = match result {
            OpResult::Success(SuccessData::FallbackPath(found)) => found.clone(),
            _ => None,
        };
        let still_there = self.pane_by_key(key).is_some_and(|pane| {
            pane.current_location == *requested && pane.active_job_id.is_none()
        });
        if !still_there {
            return;
        }
        let parent_readable = found.is_some() && found == requested.parent();
        let on_active_tab = self.current_tab().id == key.0;
        if let (true, true, Some(parent)) = (parent_readable, on_active_tab, found) {
            result_obj.task_panel_logs.push(format!(
                "{} [Session] {} no longer exists — opened {} instead",
                chrono::Local::now().format("[%H:%M:%S]"),
                requested.display_path(),
                parent.display_path()
            ));
            let navigated = update_state(
                self,
                Transition::ChangeLocation {
                    pane: key.1,
                    location: parent,
                },
            );
            result_obj.jobs_to_start.extend(navigated.jobs_to_start);
            result_obj.ui_changed = true;
            return;
        }
        self.polling.failing.insert(key, requested.clone());
        result_obj.task_panel_logs.push(format!(
            "{} [WARN] Polling: cannot read {}{}",
            chrono::Local::now().format("[%H:%M:%S]"),
            requested.display_path(),
            message
                .as_deref()
                .map(|m| format!(" — {}", m.trim()))
                .unwrap_or_default()
        ));
        result_obj.ui_changed = true;
    }
}
