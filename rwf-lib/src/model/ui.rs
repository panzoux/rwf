//! UI state

use serde::{Deserialize, Serialize};

/// UI state
#[derive(Debug)]
pub struct UIState {
    pub active_pane: ActivePane,
    pub mode: UIMode,
    pub layout: LayoutState,
    /// Range marking mode state: stores the initial cursor position when entering range marking mode
    pub range_marking_start: Option<usize>,
    /// Whether to show hidden files
    pub show_hidden: bool,
}

impl Default for UIState {
    fn default() -> Self {
        Self::new()
    }
}

impl UIState {
    /// Create a new default UI state
    pub fn new() -> Self {
        Self {
            active_pane: ActivePane::Left,
            mode: UIMode::Normal,
            layout: LayoutState::default(),
            range_marking_start: None,
            show_hidden: false,
        }
    }
}

/// How the file viewer is displayed when open
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ViewerLayout {
    /// Viewer occupies the full screen (current behaviour for "v")
    #[default]
    FullScreen,
    /// Viewer shares the screen with the file panes ("V")
    SideBySide,
}

/// Layout configuration
#[derive(Debug)]
pub struct LayoutState {
    pub show_status_bar: bool,
    pub show_task_panel: bool,
    pub show_tab_bar: bool,
    pub pane_height: usize,
    pub pane_width: usize,
    /// Task panel height in lines (default: 5)
    pub task_panel_height: usize,
    /// Task panel scroll offset (for scrolling through task history)
    pub task_panel_scroll_offset: usize,
    /// Current viewer layout (only meaningful when a viewer is open)
    pub viewer_layout: ViewerLayout,
    /// Remembered preference: the layout used when "v" opens a new viewer
    pub viewer_preferred_layout: ViewerLayout,
    /// Which file pane was active when the SideBySide viewer was opened.
    /// This is fixed for the lifetime of the SideBySide session so the viewer
    /// never jumps sides even if ui.active_pane changes.
    pub viewer_anchor_pane: ActivePane,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            show_status_bar: true,
            show_task_panel: true,
            show_tab_bar: true,
            pane_height: 20,
            pane_width: 80,
            task_panel_height: 5,
            task_panel_scroll_offset: 0,
            viewer_layout: ViewerLayout::FullScreen,
            viewer_preferred_layout: ViewerLayout::FullScreen,
            viewer_anchor_pane: ActivePane::Left,
        }
    }
}

/// Narrowest either side of the left/right split may become, in columns.
pub const MIN_PANE_COLUMNS: u16 = 16;
/// Columns the split moves per `WidenLeftPane` / `WidenRightPane`.
pub const PANE_SPLIT_STEP: u16 = 2;

/// A left pane width together with the terminal width it was chosen at.
///
/// Stored as the original pair rather than re-rounded on every resize: dragging a
/// terminal edge emits dozens of resize events, and re-saving the rounded value each
/// time lets the rounding error pile up until the ratio drifts. Scaling from the
/// original pair means returning to the original terminal width gives back exactly
/// the original column count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneSplit {
    pub left: u16,
    pub at_total: u16,
}

/// Columns given to the left and right side of a `total`-column left/right split.
///
/// `None` is the even split (the extra column of an odd width goes left, matching
/// the ratatui `Percentage(50)` split this replaced — measured in rwf-bin's
/// `even_split_matches_the_old_percentage_layout_at_every_width`). A stored split is scaled to `total`
/// (rounded), then clamped so each side keeps [`MIN_PANE_COLUMNS`]. Below
/// `2 * MIN_PANE_COLUMNS` there is no room for that, and the split is even.
pub fn split_columns(total: u16, split: Option<PaneSplit>) -> (u16, u16) {
    let even = total - total / 2;
    let left = match split {
        Some(PaneSplit { left, at_total }) if at_total > 0 && total >= 2 * MIN_PANE_COLUMNS => {
            // Rounded `left * total / at_total`; u64 because 65535 · 65535 · 2 overflows u32.
            let scaled = (u64::from(left) * u64::from(total) * 2 + u64::from(at_total))
                / (2 * u64::from(at_total));
            let max = total - MIN_PANE_COLUMNS;
            u16::try_from(scaled).map_or(max, |s| s.clamp(MIN_PANE_COLUMNS, max))
        }
        _ => even,
    };
    (left, total - left)
}

/// Active pane identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActivePane {
    #[default]
    Left,
    Right,
}

impl ActivePane {
    /// Get the opposite pane (Left ↔ Right)
    pub fn opposite(&self) -> Self {
        match self {
            ActivePane::Left => ActivePane::Right,
            ActivePane::Right => ActivePane::Left,
        }
    }
}

/// UI mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIMode {
    Normal,
    Search,
    Command,
    Dialog,
    Viewer,
    /// Viewer with the search input bar active (user is typing a query).
    ViewerSearch,
    /// Viewer with the command line active (line-jump, e.g. "100g").
    ViewerCommand,
    /// Leap Navigation mode (F3): buffer-as-path-trace filtering.
    Leap,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn at(left: u16, at_total: u16) -> Option<PaneSplit> {
        Some(PaneSplit { left, at_total })
    }

    /// Every row of the worked example in plan/7.25.pane_width_resize.md §3.2.
    #[test]
    fn split_columns_follows_the_plan_table() {
        assert_eq!(split_columns(120, None), (60, 60));
        assert_eq!(split_columns(120, at(64, 120)), (64, 56));
        assert_eq!(split_columns(160, at(64, 120)), (85, 75)); // 85.3
        assert_eq!(split_columns(120, at(64, 120)), (64, 56)); // back where it was
        assert_eq!(split_columns(34, at(64, 120)), (18, 16)); // 18.1
        assert_eq!(split_columns(33, at(64, 120)), (17, 16)); // 17.6 -> 18, right keeps 16
        assert_eq!(split_columns(160, at(87, 160)), (87, 73));
        assert_eq!(split_columns(160, at(144, 160)), (144, 16));
    }

    #[test]
    fn split_columns_even_split_gives_the_odd_column_to_the_left() {
        assert_eq!(split_columns(121, None), (61, 60));
        assert_eq!(split_columns(1, None), (1, 0));
        assert_eq!(split_columns(0, None), (0, 0));
    }

    #[test]
    fn split_columns_is_even_when_both_sides_cannot_keep_their_minimum() {
        assert_eq!(split_columns(31, at(100, 120)), (16, 15));
        assert_eq!(split_columns(10, at(2, 120)), (5, 5));
        assert_eq!(split_columns(32, at(100, 120)), (16, 16));
    }

    #[test]
    fn split_columns_clamps_both_sides_to_the_minimum() {
        assert_eq!(split_columns(120, at(0, 120)), (16, 104));
        assert_eq!(split_columns(120, at(200, 120)), (104, 16));
    }

    #[test]
    fn split_columns_zero_at_total_does_not_divide_by_zero() {
        assert_eq!(split_columns(120, at(64, 0)), (60, 60));
    }

    #[test]
    fn split_columns_survives_the_largest_values() {
        let (l, r) = split_columns(u16::MAX, at(u16::MAX, u16::MAX));
        assert_eq!(u32::from(l) + u32::from(r), u32::from(u16::MAX));
        assert_eq!(r, MIN_PANE_COLUMNS);
    }

    proptest! {
        /// Resizing away and back never moves the divider: the stored pair is
        /// scaled afresh each time instead of re-rounded.
        #[test]
        fn split_columns_round_trip_through_any_width(
            left in 16u16..=104,
            other in 0u16..=1000,
        ) {
            let split = at(left, 120);
            let _ = split_columns(other, split);
            prop_assert_eq!(split_columns(120, split), (left, 120 - left));
        }

        /// Both sides always add up to the terminal width, and keep the minimum
        /// whenever the terminal is wide enough for that.
        #[test]
        fn split_columns_partitions_the_width(
            total in 0u16..=2000,
            left in 0u16..=3000,
            at_total in 0u16..=3000,
        ) {
            let (l, r) = split_columns(total, at(left, at_total));
            prop_assert_eq!(l + r, total);
            if total >= 2 * MIN_PANE_COLUMNS {
                prop_assert!(l >= MIN_PANE_COLUMNS && r >= MIN_PANE_COLUMNS);
            }
        }
    }
}
