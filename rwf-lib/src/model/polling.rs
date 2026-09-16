//! Background polling bookkeeping (Phase 7.5, pane update Layer 2).
//!
//! Polls never own a pane: they do not set `PaneModel::active_job_id` or `is_loading`,
//! so any user-initiated read simply supersedes them. What a poll needs to know about
//! itself lives here instead.

use super::{ActivePane, Location};
use crate::job::JobId;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Workers in the dedicated poll pool (D14): one per visible pane. A tick that finds
/// this many polls still in flight skips, so a hung poll stalls polling, never user work.
pub const POLL_POOL_WORKERS: usize = 2;

/// `PollingIntervalMs` below this is raised to it (D10b).
pub const MIN_POLLING_INTERVAL_MS: u64 = 250;

/// A poll read slower than this backs its drive off; one at or under it recovers (D10b).
pub const SLOW_POLL: Duration = Duration::from_secs(1);

/// The backed-off interval never exceeds `max(this, PollingIntervalMs)` (D10b).
pub const BACKOFF_CEILING: Duration = Duration::from_millis(4000);

/// A pane, by tab **id** and side.
pub type PaneKey = (usize, ActivePane);

/// A poll read that has been submitted and not yet completed.
#[derive(Debug, Clone)]
pub struct PollInFlight {
    pub job_id: JobId,
    /// The directory read — a pane that has since moved must not take the result.
    pub location: Location,
    /// The drive key of `location` (D10a).
    pub drive: String,
    /// `PaneModel::listing_generation` at submission (D12).
    pub generation: u64,
    pub submitted_at: Instant,
    /// When a worker picked it up. Timing (D10b, D10c) runs from here, so a poll
    /// waiting for a free worker is not called slow.
    pub started_at: Option<Instant>,
}

/// Why a drive is not being polled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// `StopPolling` / `TogglePolling` (Phase 7.5 T4).
    Manual,
    /// A poll listing exceeded `PollingDisableAfterMs` (D10c).
    Auto,
}

/// Per-drive polling state (D10). Session-only: never persisted.
#[derive(Debug, Clone)]
pub struct DriveState {
    /// Current interval between a pane's poll completing and its next poll.
    pub interval: Duration,
    pub stopped: Option<StopReason>,
    /// Its last poll failed (cleared by the next successful one).
    pub failing: bool,
    pub last_poll: Option<Instant>,
}

impl DriveState {
    fn new(base: Duration) -> Self {
        Self {
            interval: base,
            stopped: None,
            failing: false,
            last_poll: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct PollingState {
    pub in_flight: HashMap<PaneKey, PollInFlight>,
    /// When each pane is next due; a pane never polled is due at once.
    pub next_due: HashMap<PaneKey, Instant>,
    /// Panes whose polls are failing, with the location that failed. Cleared by the next
    /// successful poll, which logs the recovery once (D9).
    pub failing: HashMap<PaneKey, Location>,
    /// Keyed by [`drive_key`].
    pub drives: HashMap<String, DriveState>,
    /// Mount points, longest-prefix matched by [`drive_key`] on Unix. Loaded once by a
    /// worker job at startup; empty until then, which keys everything as `/`.
    pub mounts: Vec<PathBuf>,
    /// The panes that were on screen at the last tick (D17). A pane that is visible now
    /// but was not — a tab switch, a viewer closing — is polled at once; clearing this is
    /// how terminal focus gained asks for the same.
    pub last_visible: HashSet<PaneKey>,
    /// Panes that came into view while the poll pool was full: polled as soon as a worker
    /// frees, instead of waiting out their interval.
    pub immediate: HashSet<PaneKey>,
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

    /// The drive key of a location, or `None` for a location that is never polled.
    pub fn drive_of(&self, location: &Location) -> Option<String> {
        match location {
            Location::Local(path) => Some(drive_key(path, &self.mounts)),
            _ => None,
        }
    }

    /// The state of `drive`, created at `base` on first sight.
    pub fn drive_mut(&mut self, drive: &str, base: Duration) -> &mut DriveState {
        self.drives
            .entry(drive.to_string())
            .or_insert_with(|| DriveState::new(base))
    }

    pub fn is_stopped(&self, drive: &str) -> bool {
        self.drives.get(drive).is_some_and(|d| d.stopped.is_some())
    }

    /// Config reload: every drive's interval returns to the new base; stopped drives
    /// stay stopped.
    pub fn reset_intervals(&mut self, base: Option<Duration>) {
        if let Some(base) = base {
            for drive in self.drives.values_mut() {
                drive.interval = base;
            }
        }
    }

    /// Adapt `drive`'s interval to a completed poll (D10b): slower than [`SLOW_POLL`] or
    /// failed doubles it, up to `max(BACKOFF_CEILING, base)`; otherwise it halves, never
    /// below `base`.
    pub fn adapt_interval(&mut self, drive: &str, base: Duration, took: Duration, failed: bool) {
        let ceiling = BACKOFF_CEILING.max(base);
        let state = self.drive_mut(drive, base);
        state.interval = if failed || took > SLOW_POLL {
            (state.interval * 2).min(ceiling)
        } else {
            (state.interval / 2).max(base)
        };
    }
}

/// The drive a local path lives on (D10a), by path parsing alone.
///
/// - `C:\…` (also `c:/…`, `\\?\C:\…`) → `C:\`. Mapped drives are keyed by letter.
/// - `\\server\share\…` (also `\\?\UNC\server\share\…`) → `\\server\share`, lowercased
///   because Windows compares them case-insensitively.
/// - Anything else → the longest `mounts` entry containing the path, else `/`.
pub fn drive_key(path: &Path, mounts: &[PathBuf]) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    let text = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    };

    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return format!("{}:\\", char::from(bytes[0]).to_ascii_uppercase());
    }
    if let Some(rest) = text.strip_prefix(r"\\") {
        let mut parts = rest.split('\\').filter(|p| !p.is_empty());
        if let (Some(server), Some(share)) = (parts.next(), parts.next()) {
            return format!(r"\\{server}\{share}").to_lowercase();
        }
    }

    mounts
        .iter()
        .filter(|mount| path.starts_with(mount))
        .max_by_key(|mount| mount.components().count())
        .map(|mount| mount.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string())
}

/// Mount points from Linux `/proc/self/mounts` (second field, with the kernel's octal
/// escapes such as `\040` for a space decoded).
pub fn parse_proc_mounts(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|field| PathBuf::from(unescape_octal(field)))
        .collect()
}

/// Mount points from BSD/macOS `mount` output: `<device> on <path> (<options>)`.
pub fn parse_mount_output(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once(" on ")?;
            let path = rest.rsplit_once(" (").map_or(rest, |(path, _)| path);
            Some(PathBuf::from(path))
        })
        .collect()
}

fn unescape_octal(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|digits| bytes[i] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)));
        match octal {
            Some(digits) => {
                let value = digits
                    .iter()
                    .fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
                out.push(value as u8);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(path: &str) -> String {
        drive_key(Path::new(path), &[])
    }

    #[test]
    fn drive_letters_key_by_letter() {
        assert_eq!(key(r"C:\Users\me"), r"C:\");
        assert_eq!(key("c:/Users/me"), r"C:\");
        assert_eq!(key(r"\\?\D:\data"), r"D:\");
        assert_eq!(key("Z:"), r"Z:\");
    }

    #[test]
    fn unc_paths_key_by_server_and_share() {
        assert_eq!(key(r"\\srv\share\dir\sub"), r"\\srv\share");
        assert_eq!(key(r"\\SRV\Share"), r"\\srv\share");
        assert_eq!(key(r"\\?\UNC\srv\share\dir"), r"\\srv\share");
    }

    #[test]
    fn unix_paths_take_the_longest_containing_mount() {
        let mounts = [PathBuf::from("/"), PathBuf::from("/mnt/usb")];
        assert_eq!(drive_key(Path::new("/mnt/usb/photos"), &mounts), "/mnt/usb");
        assert_eq!(drive_key(Path::new("/mnt/usbx"), &mounts), "/");
        assert_eq!(drive_key(Path::new("/home/me"), &[]), "/");
    }

    #[test]
    fn proc_mounts_are_parsed_and_unescaped() {
        let text = "sysfs /sys sysfs rw 0 0\n/dev/sdb1 /mnt/my\\040disk vfat rw 0 0\n";
        assert_eq!(
            parse_proc_mounts(text),
            vec![PathBuf::from("/sys"), PathBuf::from("/mnt/my disk")]
        );
    }

    #[test]
    fn bsd_mount_output_is_parsed() {
        let text = "/dev/disk1s1 on / (apfs, local)\n//u@srv/share on /Volumes/my share (smbfs)\n";
        assert_eq!(
            parse_mount_output(text),
            vec![PathBuf::from("/"), PathBuf::from("/Volumes/my share")]
        );
    }

    #[test]
    fn slow_or_failed_polls_double_up_to_the_ceiling_and_fast_ones_halve_to_base() {
        let base = Duration::from_millis(1000);
        let slow = Duration::from_millis(1500);
        let fast = Duration::from_millis(10);
        let mut polling = PollingState::default();

        polling.adapt_interval("C:\\", base, slow, false);
        assert_eq!(polling.drives["C:\\"].interval, Duration::from_millis(2000));
        polling.adapt_interval("C:\\", base, fast, true);
        assert_eq!(polling.drives["C:\\"].interval, Duration::from_millis(4000));
        polling.adapt_interval("C:\\", base, slow, false);
        assert_eq!(
            polling.drives["C:\\"].interval,
            Duration::from_millis(4000),
            "ceiling"
        );

        polling.adapt_interval("C:\\", base, fast, false);
        assert_eq!(polling.drives["C:\\"].interval, Duration::from_millis(2000));
        polling.adapt_interval("C:\\", base, fast, false);
        polling.adapt_interval("C:\\", base, fast, false);
        assert_eq!(polling.drives["C:\\"].interval, base, "never below base");
    }

    #[test]
    fn a_base_above_the_ceiling_is_the_ceiling() {
        let base = Duration::from_millis(8000);
        let mut polling = PollingState::default();

        polling.adapt_interval("C:\\", base, Duration::from_secs(2), false);

        assert_eq!(polling.drives["C:\\"].interval, base);
    }

    #[test]
    fn a_one_second_poll_is_not_slow() {
        let base = Duration::from_millis(1000);
        let mut polling = PollingState::default();
        polling.drive_mut("C:\\", base).interval = Duration::from_millis(2000);

        polling.adapt_interval("C:\\", base, SLOW_POLL, false);

        assert_eq!(polling.drives["C:\\"].interval, base);
    }
}
