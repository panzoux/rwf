# Bugfix Requirements Document

## Introduction

This document specifies the requirements for fixing a scrolling bug in the file pane where blank lines appear at the bottom when scrolling down. The bug occurs when the scrolling logic triggers too early or scrolls too far, causing the viewport to show fewer entries than the visible height allows, resulting in blank space at the bottom of the pane.

The issue manifests when users have more entries than can fit in the visible area (e.g., 70 entries with visible_height=19) and scroll down. The current implementation incorrectly calculates the bottom trigger position and applies scrolling that overshoots the optimal scroll offset, leaving blank lines visible.

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN the user scrolls down with cursor at position 66, scroll_offset=50, visible_height=19, scroll_margin=3, and 70 total entries THEN the system calculates bottom_trigger as 15 (should be 16) and sets scroll_offset to 51 (max_offset), showing entries [51..70) with blank lines at the bottom

1.2 WHEN the cursor reaches the 3rd line from the bottom (cursor_in_view=16) THEN the system triggers scrolling even though the cursor is not yet at the scroll_margin boundary (should trigger at cursor_in_view > 16, not > 15)

1.3 WHEN scrolling down increments scroll_offset by 1 THEN the system may overshoot to max_offset, causing the viewport to show fewer than visible_height entries when there are enough entries remaining

1.4 WHEN cursor=73 (last entry), scroll_offset=55, cursor_in_view=18, visible_height=19, scroll_margin=3, and 74 total entries THEN the cursor is on the last visible line (line 18) but the system does not scroll to maintain scroll_margin spacing from the bottom, violating the scroll_margin requirement

### Expected Behavior (Correct)

2.1 WHEN the user scrolls down with cursor at position 66, scroll_offset=50, visible_height=19, scroll_margin=3, and 70 total entries THEN the system SHALL calculate bottom_trigger as 16 (visible_height - scroll_margin = 19 - 3) and only trigger scrolling when cursor_in_view > 16

2.2 WHEN the cursor reaches beyond the 3rd line from the bottom (cursor_in_view > visible_height - scroll_margin) THEN the system SHALL trigger scrolling to maintain the cursor at the scroll_margin position from the bottom

2.3 WHEN scrolling down adjusts scroll_offset THEN the system SHALL ensure the viewport always shows exactly visible_height entries (or all remaining entries if fewer than visible_height remain), with no blank lines at the bottom

2.4 WHEN the cursor is at the last entry and scroll_offset is at max_offset THEN the system SHALL display the last visible_height entries with the cursor on the last line, with no blank lines

2.5 WHEN cursor=73 (last entry), scroll_offset=55, cursor_in_view=18, visible_height=19, scroll_margin=3, and 74 total entries THEN the system SHALL adjust scroll_offset to maintain scroll_margin spacing from the bottom edge, positioning the cursor at line 16 (visible_height - scroll_margin - 1) instead of line 18

### Unchanged Behavior (Regression Prevention)

3.1 WHEN the total number of entries is less than or equal to visible_height THEN the system SHALL CONTINUE TO set scroll_offset to 0 and display all entries without scrolling

3.2 WHEN scrolling up (cursor moving toward the top) THEN the system SHALL CONTINUE TO trigger scrolling when cursor_in_view < scroll_margin and adjust scroll_offset to keep the cursor at scroll_margin lines from the top

3.3 WHEN the cursor jumps to a position outside the visible viewport THEN the system SHALL CONTINUE TO adjust scroll_offset to make the cursor visible with appropriate scroll_margin spacing

3.4 WHEN entries fit exactly within visible_height THEN the system SHALL CONTINUE TO display all entries without blank lines and without scrolling
