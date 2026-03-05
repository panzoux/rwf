# Bug Fixes: Startup Scroll and Pane Info Colors

## Issue 1: Startup Scroll - Cursor Beyond Visible Area ✅

### Problem
When restoring a session, the cursor position was restored but `scroll_offset` stayed at 0, making the cursor invisible if it was beyond the visible area.

### Solution
Modified `rwf-lib/src/session.rs` in the `restore_tabs()` function (after line 209) to adjust scroll position when restoring cursor positions:

```rust
// Ensure cursor is visible by adjusting scroll if needed
// This will be properly calculated when entries are loaded,
// but we can set a reasonable initial scroll position
if tab.left_pane.cursor > 0 {
    tab.left_pane.scroll_offset = tab.left_pane.cursor.saturating_sub(10);
}
if tab.right_pane.cursor > 0 {
    tab.right_pane.scroll_offset = tab.right_pane.cursor.saturating_sub(10);
}
```

This ensures the cursor is visible on startup by scrolling to show it near the middle of the pane (10 lines from the top when possible).

## Issue 2: Pane Info Colors Not Being Used ✅

### Problem
The pane info line was using generic `foreground_color` and `background_color` instead of the specific `pane_info_foreground_color` and `pane_info_background_color` settings.

### Verification
Confirmed that `rwf-lib/src/config.rs` already has the correct getter methods:
- `get_pane_info_foreground()` - Falls back to `top_separator_foreground_color` if not set
- `get_pane_info_background()` - Falls back to `top_separator_background_color` if not set

Default values are:
- Foreground: Black
- Background: DarkGray

### Solution
Updated `rwf-bin/src/ui/pane_info_line.rs` to use the getter methods instead of direct color fields:

**Before:**
```rust
.fg(parse_color(&colors.foreground_color))
.bg(parse_color(&colors.background_color))
```

**After:**
```rust
.fg(parse_color(colors.get_pane_info_foreground()))
.bg(parse_color(colors.get_pane_info_background()))
```

This ensures that:
1. If `config.json` has `PaneInfoForegroundColor` and `PaneInfoBackgroundColor`, they will be used
2. If those fields are missing, it falls back to the defaults (Black on DarkGray)
3. The getter methods handle the `Option<String>` types correctly

## Files Modified
- `rwf-lib/src/session.rs` - Added scroll offset adjustment on session restore
- `rwf-bin/src/ui/pane_info_line.rs` - Updated to use pane info color getter methods

## Testing
Both files compile without errors or warnings.
