//! Background polling bookkeeping (Phase 7.5, pane update Layer 2).
//!
//! Polls never own a pane: they do not set `PaneModel::active_job_id` or `is_loading`,
//! so any user-initiated read simply supersedes them. What a poll needs to know about
//! itself lives here instead.

use super::{ActivePane, Location};
use crate::job::JobId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Workers in the dedicated poll pool (D14): one per visible pane. A tick that finds
/// this many polls still in flight skips, so a hung poll stalls polling, never user work.
pub const POLL_POOL_WORKERS: usize = 2;

/// `PollingIntervalMs` below this is raised to it (D10b).
pub const MIN_POLLING_INTERVAL_MS: u64 = 250;

/// A pane, by tab **id** and side.
pub type PaneKey = (usize, ActivePane);

/// A poll read that has been submitted and not yet completed.
#[derive(Debug, Clone)]
pub struct PollInFlight {
    pub job_id: JobId,
    /// The directory read — a pane that has since moved must not take the result.
    pub location: Location,
    /// `PaneModel::listing_generation` at submission (D12).
    pub generation: u64,
    pub submitted_at: Instant,
}

#[derive(Debug, Default)]
pub struct PollingState {
    pub in_flight: HashMap<PaneKey, PollInFlight>,
    /// When each pane is next due; a pane never polled is due at once.
    pub next_due: HashMap<PaneKey, Instant>,
    /// Panes whose polls are failing, with the location that failed. Cleared by the next
    /// successful poll, which logs the recovery once (D9).
    pub failing: HashMap<PaneKey, Location>,
}

impl PollingState {
    /// The configured interval, clamped; `None` when polling is off (`PollingIntervalMs: 0`).
    pub fn interval(polling_interval_ms: u32) -> Option<Duration> {
        match u64::from(polling_interval_ms) {
            0 => None,
            ms => Some(Duration::from_millis(ms.max(MIN_POLLING_INTERVAL_MS))),
        }
    }

    /// Whether `job_id` is the poll in flight for some pane.
    pub fn owns(&self, job_id: JobId) -> Option<PaneKey> {
        self.in_flight
            .iter()
            .find(|(_, poll)| poll.job_id == job_id)
            .map(|(key, _)| *key)
    }
}
