//! Shared wall-clock spinner utility used by all animated indicators.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current spinner frame, derived from wall-clock time.
/// No external tick state needed — just call on every render.
pub fn current_frame(frames: &[String], frame_ms: u64) -> &str {
    if frames.is_empty() { return ""; }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis() as u64;
    &frames[frame_index(ms, frame_ms, frames.len())]
}

/// Pure function used by [`current_frame`], separated out so the index math
/// can be unit-tested without depending on wall-clock time.
fn frame_index(subsec_ms: u64, frame_ms: u64, frame_count: usize) -> usize {
    (subsec_ms / frame_ms.max(1)) as usize % frame_count.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_frame_empty_frames_returns_empty_str() {
        let frames: Vec<String> = vec![];
        assert_eq!(current_frame(&frames, 150), "");
    }

    #[test]
    fn test_current_frame_returns_one_of_the_configured_frames() {
        let frames: Vec<String> = vec!["|".into(), "/".into(), "-".into(), "\\".into()];
        let frame = current_frame(&frames, 150);
        assert!(frames.iter().any(|f| f == frame));
    }

    #[test]
    fn test_current_frame_zero_frame_ms_does_not_panic() {
        // frame_ms=0 must not divide-by-zero; current_frame clamps via .max(1).
        let frames: Vec<String> = vec!["|".into(), "/".into()];
        let frame = current_frame(&frames, 0);
        assert!(frames.iter().any(|f| f == frame));
    }

    #[test]
    fn test_frame_index_wraps_around_frame_count() {
        // Index math cycles through all frames as wall-clock ms advances,
        // mirroring the old TaskPanel::tick() wrap-around behavior.
        let frame_ms = 150;
        let frame_count = 4;
        assert_eq!(frame_index(0, frame_ms, frame_count), 0);
        assert_eq!(frame_index(150, frame_ms, frame_count), 1);
        assert_eq!(frame_index(300, frame_ms, frame_count), 2);
        assert_eq!(frame_index(450, frame_ms, frame_count), 3);
        assert_eq!(frame_index(600, frame_ms, frame_count), 0); // wraps
    }

    #[test]
    fn test_frame_index_single_frame_always_zero() {
        assert_eq!(frame_index(999, 150, 1), 0);
    }
}
