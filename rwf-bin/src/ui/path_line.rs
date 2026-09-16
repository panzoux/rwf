//! Path line rendering
//!
//! Displays the current path for both panes side by side

use super::{parse_color, shorten_path};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{model::ActivePane, AppState};
use unicode_width::UnicodeWidthStr;

/// Render the path line.
/// `single_pane`: when `Some(pane)`, render only that pane's path at full width
/// (used in SideBySide mode where the other half is the viewer).
pub fn render_path_line(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    single_pane: Option<ActivePane>,
) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    let style = Style::default()
        .fg(parse_color(&colors.filename_label_foreground_color))
        .bg(parse_color(&colors.filename_label_background_color));

    let render_one = |frame: &mut Frame, rect: Rect, pane: ActivePane| {
        let is_active = state.ui.active_pane == pane;
        let pane_model = match pane {
            ActivePane::Left => &tab.left_pane,
            ActivePane::Right => &tab.right_pane,
        };
        let prefix = if is_active { ">" } else { " " };
        let mask = pane_model
            .file_mask
            .as_deref()
            .map(|m| format!(" [{}]", m))
            .unwrap_or_default();
        // Phase 7.5 D21: `[no poll]` / `[offline]`, reserved before the path is
        // shortened so a long path can never push it off the line.
        let indicator = state
            .poll_indicator(pane)
            .map(|text| format!(" {text}"))
            .unwrap_or_default();
        let display_path = pane_model.current_location.display_path();
        let avail =
            (rect.width as usize).saturating_sub(prefix.width() + indicator.width() + mask.width());
        let shortened = shorten_path(&display_path, avail, "…");
        let warning = style.fg(parse_color(&colors.warning_color));
        let line = Line::from(vec![
            Span::styled(format!("{prefix}{shortened}"), style),
            Span::styled(indicator, warning),
            Span::styled(mask, style),
        ]);
        frame.render_widget(Paragraph::new(line).style(style), rect);
    };

    if let Some(pane) = single_pane {
        render_one(frame, area, pane);
    } else {
        let halves = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        render_one(frame, halves[0], ActivePane::Left);
        render_one(frame, halves[1], ActivePane::Right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rwf_lib::model::polling::StopReason;
    use rwf_lib::model::Location;
    use rwf_lib::{AppConfig, AppState};
    use std::time::Duration;

    fn state_on_c_and_d() -> AppState {
        let mut state = AppState::new(AppConfig::default());
        state.config.polling_interval_ms = 1000;
        let tab = state.current_tab_mut();
        tab.left_pane.current_location = Location::Local(r"C:\work".into());
        tab.right_pane.current_location = Location::Local(r"D:\data".into());
        state
    }

    fn draw(state: &AppState, width: u16) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).expect("terminal");
        terminal
            .draw(|frame| render_path_line(frame, frame.area(), state, None))
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    fn row(buffer: &ratatui::buffer::Buffer) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect()
    }

    /// Phase 7.5 D21: a stopped drive says `[no poll]`, a failing one `[offline]`, in the
    /// warning colour, on the pane it applies to.
    #[test]
    fn polling_indicators_show_on_their_pane_in_the_warning_colour() {
        let mut state = state_on_c_and_d();
        let base = Duration::from_millis(1000);
        state.polling.drive_mut(r"C:\", base).stopped = Some(StopReason::Auto);
        state.polling.drive_mut(r"D:\", base).failing = true;

        let buffer = draw(&state, 80);
        let text = row(&buffer);

        let no_poll = text.find("[no poll]").expect("left indicator");
        let offline = text.find("[offline]").expect("right indicator");
        assert!(no_poll < 40 && offline >= 40, "{text:?}");
        let warning = parse_color(&state.config.display.colors.warning_color);
        assert_eq!(buffer[(no_poll as u16, 0)].fg, warning);
        assert_eq!(buffer[(offline as u16, 0)].fg, warning);
    }

    #[test]
    fn the_indicator_survives_a_long_path_in_a_narrow_pane() {
        let mut state = state_on_c_and_d();
        state.current_tab_mut().left_pane.current_location = Location::Local(
            r"C:\a\very\long\path\that\cannot\possibly\fit\in\half\of\forty\columns".into(),
        );
        state
            .polling
            .drive_mut(r"C:\", Duration::from_millis(1000))
            .stopped = Some(StopReason::Manual);

        let text = row(&draw(&state, 40));
        // By character, not byte: the shortened path starts with a multi-byte `…`.
        let left_half: String = text.chars().take(20).collect();

        assert!(left_half.contains("[no poll]"), "{text:?}");
    }

    #[test]
    fn no_indicator_when_polling_is_off() {
        let mut state = state_on_c_and_d();
        state
            .polling
            .drive_mut(r"C:\", Duration::from_millis(1000))
            .stopped = Some(StopReason::Auto);
        state.config.polling_interval_ms = 0;

        assert!(!row(&draw(&state, 80)).contains("[no poll]"));
    }
}
