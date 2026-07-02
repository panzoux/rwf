//! Shared wall-clock spinner utility used by all animated indicators.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current spinner frame, derived from wall-clock time.
/// No external tick state needed — just call on every render.
pub fn current_frame<'a>(frames: &'a [String], frame_ms: u64) -> &'a str {
    if frames.is_empty() { return ""; }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis() as u64;
    let idx = (ms / frame_ms.max(1)) as usize % frames.len();
    &frames[idx]
}
