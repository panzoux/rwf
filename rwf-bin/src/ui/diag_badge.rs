//! Diagnostic-session indicator (Phase 7.15).
//!
//! A recording session changes nothing visible about how RWF behaves, so
//! without a marker there is no way to tell one is running — and leaving one
//! running by accident is the failure mode the design explicitly guards
//! against (`plan/7.15.diagnostic_report.md` §6.1).
//!
//! Drawn as a top-right overlay on the whole frame rather than as part of the
//! tab bar, so it survives every layout: tab bar hidden, full-screen viewer,
//! side-by-side viewer.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::Paragraph,
    Frame,
};

/// Format an elapsed duration as `mm:ss`, saturating at `99:59`.
///
/// A session running for over an hour is itself a signal that the user forgot
/// to stop it, so the display stays two-digit rather than growing an hours
/// field and shifting the badge width.
pub fn format_elapsed(elapsed: std::time::Duration) -> String {
    let total = elapsed.as_secs().min(99 * 60 + 59);
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// Overlay `● DIAG mm:ss` in the top-right corner while a session records.
///
/// No-op when idle.
///
/// # The on-screen clock is indicative, not authoritative
///
/// It refreshes only on frames the app already draws. Forcing a periodic redraw
/// to keep it ticking is **deliberately rejected** (decided 2026-08-11): it
/// would perturb the main-loop timing this feature exists to measure, and would
/// buy nothing, because timing analysis reads `events.jsonl` and never the
/// screen. Every record's `ts` and `seq` are stamped in
/// [`rwf_lib::diagnostics::observe`], entirely independent of rendering, so log
/// accuracy does not depend on this widget refreshing at all.
///
/// The badge's job is only to answer "is a session running?" — and it does that
/// even with a stale clock, since the marker persists in the last drawn frame.
pub fn render_diag_badge(frame: &mut Frame, area: Rect) {
    let Some(elapsed) = rwf_lib::diagnostics::session_elapsed() else {
        return;
    };

    let label = format!(" ● DIAG {} ", format_elapsed(elapsed));
    let width = label.chars().count() as u16;

    if area.width < width || area.height == 0 {
        return;
    }

    let badge = Rect {
        x: area.x + area.width - width,
        y: area.y,
        width,
        height: 1,
    };

    // Hardcoded rather than themed: this must stay legible against every
    // configured palette, and "recording" reads as red regardless of theme.
    frame.render_widget(
        Paragraph::new(Span::styled(
            label,
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
        badge,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::Duration;

    #[test]
    fn format_elapsed_pads_to_two_digits() {
        assert_eq!(format_elapsed(Duration::from_secs(0)), "00:00");
        assert_eq!(format_elapsed(Duration::from_secs(9)), "00:09");
        assert_eq!(format_elapsed(Duration::from_secs(75)), "01:15");
        assert_eq!(format_elapsed(Duration::from_secs(600)), "10:00");
    }

    #[test]
    fn format_elapsed_saturates_instead_of_widening() {
        // Badge width must stay stable no matter how long a forgotten session runs.
        assert_eq!(format_elapsed(Duration::from_secs(100 * 3600)), "99:59");
    }

    /// With no session running the badge must draw nothing at all — the whole
    /// point is that diagnostics is invisible when off.
    #[test]
    fn badge_is_absent_when_no_session_runs() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_diag_badge(frame, frame.area()))
            .expect("draw");

        let rendered = format!("{}", terminal.backend());
        assert!(
            !rendered.contains("DIAG"),
            "badge drawn with no active session"
        );
    }

    /// The case that actually matters: a running session must be visible.
    ///
    /// Starts a real session in a temp dir and stops it before returning, so
    /// the process-global collector is left idle for the other tests in this
    /// binary (the suite runs with `--test-threads=1`).
    #[test]
    fn badge_appears_in_the_top_right_while_a_session_runs() {
        let temp = tempfile::tempdir().expect("tempdir");
        rwf_lib::diagnostics::start_session(temp.path().to_path_buf(), "test")
            .expect("session starts");

        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_diag_badge(frame, frame.area()))
            .expect("draw");
        let rendered = format!("{}", terminal.backend());

        rwf_lib::diagnostics::stop_session(None);

        assert!(rendered.contains("DIAG"), "badge missing: {rendered}");
        assert!(rendered.contains("00:00"), "elapsed missing: {rendered}");

        // `TestBackend`'s Display wraps each buffer row in quotes, so compare
        // against the unquoted row rather than the raw line.
        let rows: Vec<&str> = rendered
            .lines()
            .map(|l| l.trim().trim_matches('"'))
            .collect();
        let badge_rows: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.contains("DIAG"))
            .map(|(i, _)| i)
            .collect();

        assert_eq!(
            badge_rows,
            vec![0],
            "badge must occupy exactly the top row: {rendered}"
        );
        assert!(
            rows[0].ends_with("● DIAG 00:00 "),
            "badge not flush right: {:?}",
            rows[0]
        );
    }

    #[test]
    fn badge_is_skipped_when_the_terminal_is_too_narrow() {
        let backend = TestBackend::new(4, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        // Must not panic on an out-of-bounds Rect.
        terminal
            .draw(|frame| render_diag_badge(frame, frame.area()))
            .expect("draw");
    }
}
