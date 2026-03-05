# Scrolling Blank Line Bugfix Design

## Overview

This design addresses two related scrolling bugs in the file pane that cause blank lines to appear at the bottom of the viewport and violate scroll_margin requirements. The bugs occur in the `update_state` function in `rwf-lib/src/state.rs` when handling `CursorMove` and `CursorJump` transitions.

The first bug triggers scrolling too early (at cursor_in_view > 15 instead of > 16 when visible_height=19, scroll_margin=3) and increments scroll_offset by only 1, which can overshoot to max_offset and leave blank lines at the bottom. The second bug fails to maintain scroll_margin spacing when the cursor is at the last entry, allowing the cursor to sit on the last visible line instead of maintaining the required margin.

The fix involves correcting the bottom_trigger calculation to use >= instead of >, and implementing a proper scroll_offset calculation that maintains the cursor at the scroll_margin position from the bottom edge while ensuring the viewport is always filled with entries when possible.

## Glossary

- **Bug_Condition (C)**: The condition that triggers the scrolling bugs - when scrolling down causes blank lines or violates scroll_margin
- **Property (P)**: The desired behavior - viewport always shows visible_height entries (or all remaining if fewer), cursor maintains scroll_margin spacing from edges
- **Preservation**: Existing scrolling-up behavior, jump-to-position behavior, and no-scroll behavior for small lists that must remain unchanged
- **scroll_offset**: The index of the first entry displayed in the viewport (0-based)
- **cursor**: The absolute index of the selected entry in the full entry list (0-based)
- **cursor_in_view**: The relative position of the cursor within the visible viewport (0-based, range 0 to visible_height-1)
- **visible_height**: The number of entry lines that can be displayed in the viewport (e.g., 19)
- **scroll_margin**: The minimum number of lines between the cursor and the viewport edges (e.g., 3)
- **bottom_trigger**: The cursor_in_view position that triggers downward scrolling (should be visible_height - scroll_margin - 1)
- **max_offset**: The maximum valid scroll_offset value, calculated as entries.len() - visible_height

## Bug Details

### Fault Condition

The bugs manifest in two scenarios when scrolling down in a file pane with more entries than can fit in the visible area:

**Bug 1: Premature Scrolling with Blank Lines**
The scrolling logic triggers too early and overshoots, causing blank lines at the bottom. This occurs when the condition `cursor_in_view > bottom_trigger` is evaluated with `bottom_trigger = visible_height - scroll_margin` (e.g., 16), which triggers at cursor_in_view=17 instead of the correct position. The subsequent increment by 1 can push scroll_offset to max_offset, showing fewer than visible_height entries.

**Bug 2: Scroll Margin Violation at Last Entry**
When the cursor is at the last entry and scroll_offset is at max_offset, the cursor sits on the last visible line (cursor_in_view = visible_height - 1) instead of maintaining scroll_margin spacing from the bottom. The system should scroll to position the cursor at line (visible_height - scroll_margin - 1).

**Formal Specification:**
```
FUNCTION isBugCondition(input)
  INPUT: input of type ScrollState {
    cursor: usize,
    scroll_offset: usize,
    visible_height: usize,
    scroll_margin: usize,
    total_entries: usize
  }
  OUTPUT: boolean
  
  LET cursor_in_view = cursor - scroll_offset
  LET max_offset = total_entries - visible_height
  LET bottom_trigger = visible_height - scroll_margin
  
  // Bug 1: Premature scrolling trigger
  LET bug1 = (cursor_in_view > bottom_trigger) 
             AND (scroll_offset < max_offset)
             AND (total_entries > visible_height)
  
  // Bug 2: Scroll margin violation at last entry
  LET bug2 = (cursor == total_entries - 1)
             AND (scroll_offset == max_offset)
             AND (cursor_in_view > visible_height - scroll_margin - 1)
             AND (total_entries > visible_height)
  
  RETURN bug1 OR bug2
END FUNCTION
```

### Examples

- **Bug 1 Example**: cursor=66, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70
  - cursor_in_view = 66 - 50 = 16
  - bottom_trigger = 19 - 3 = 16
  - Condition: 16 > 16 is FALSE (correct), but code uses > instead of >=, so it triggers at 17
  - When cursor moves to 67: cursor_in_view=17, triggers scroll, scroll_offset becomes 51 (max_offset)
  - Result: Shows entries [51..70), only 19 entries, but last entry is at index 69, leaving blank lines
  - Expected: Should trigger at cursor_in_view >= 16, and calculate scroll_offset to keep cursor at position 16

- **Bug 2 Example**: cursor=73, scroll_offset=55, visible_height=19, scroll_margin=3, total_entries=74
  - cursor_in_view = 73 - 55 = 18 (last visible line)
  - max_offset = 74 - 19 = 55
  - Expected cursor_in_view = 19 - 3 - 1 = 15 (to maintain scroll_margin)
  - Result: Cursor violates scroll_margin requirement by sitting on line 18 instead of line 15
  - Expected: scroll_offset should be 73 - 15 = 58 to position cursor at line 15

- **Edge Case**: cursor=69, scroll_offset=51, visible_height=19, scroll_margin=3, total_entries=70
  - cursor_in_view = 69 - 51 = 18 (last entry, last visible line)
  - max_offset = 70 - 19 = 51
  - This is correct because we're at max_offset and showing all remaining entries
  - Expected: No scrolling, cursor stays at line 18 (last line is acceptable when at max_offset and it's the last entry)

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- Scrolling up when cursor_in_view < scroll_margin must continue to work exactly as before
- CursorJump handling for jumps outside the visible viewport must remain unchanged
- No scrolling when total_entries <= visible_height must remain unchanged
- All mouse click, keyboard navigation, and UI display behaviors unrelated to downward scrolling must remain unchanged

**Scope:**
All inputs that do NOT involve scrolling down (cursor moving toward higher indices with cursor_in_view approaching the bottom edge) should be completely unaffected by this fix. This includes:
- Scrolling up (cursor moving toward index 0)
- Cursor jumps to positions outside the viewport
- Small lists that fit entirely within visible_height
- Horizontal pane switching and other UI interactions

## Hypothesized Root Cause

Based on the bug description and code analysis, the root causes are:

1. **Incorrect Trigger Condition**: The code uses `cursor_in_view > bottom_trigger` which triggers scrolling one position too late. With visible_height=19 and scroll_margin=3, bottom_trigger=16, so scrolling triggers at cursor_in_view=17 instead of 16. This should use `>=` to trigger at the correct position.

2. **Naive Increment Logic**: The code increments scroll_offset by 1 (`scroll_offset + 1`) without considering the desired cursor position. This can overshoot to max_offset, causing the viewport to show fewer entries than visible_height when more entries are available.

3. **Missing Last-Entry Handling**: The code doesn't have special logic to maintain scroll_margin when the cursor is at the last entry. When cursor reaches the last entry at max_offset, it should adjust scroll_offset to position the cursor at (visible_height - scroll_margin - 1) instead of allowing it to sit at (visible_height - 1).

4. **Lack of Desired Position Calculation**: The code doesn't calculate the desired scroll_offset based on where the cursor should be positioned (at scroll_margin lines from the bottom). Instead, it blindly increments, which doesn't guarantee the cursor ends up at the correct position.

## Correctness Properties

Property 1: Fault Condition - Correct Scrolling Trigger and Offset Calculation

_For any_ scroll state where the cursor moves down and cursor_in_view >= (visible_height - scroll_margin - 1), the fixed scrolling logic SHALL trigger scrolling and calculate scroll_offset to position the cursor at exactly (visible_height - scroll_margin - 1) lines from the top, ensuring the viewport shows visible_height entries (or all remaining entries if fewer than visible_height remain), with no blank lines at the bottom.

**Validates: Requirements 2.1, 2.2, 2.3**

Property 2: Fault Condition - Scroll Margin Maintenance at Last Entry

_For any_ scroll state where the cursor is at the last entry (cursor == total_entries - 1) and total_entries > visible_height, the fixed scrolling logic SHALL adjust scroll_offset to position the cursor at (visible_height - scroll_margin - 1) lines from the top, maintaining the required scroll_margin spacing from the bottom edge.

**Validates: Requirements 2.4, 2.5**

Property 3: Preservation - Scrolling Up Behavior

_For any_ scroll state where cursor_in_view < scroll_margin and the cursor is moving upward, the fixed code SHALL produce exactly the same scroll_offset calculation as the original code (scroll_offset = cursor - scroll_margin), preserving the existing scrolling-up behavior.

**Validates: Requirements 3.2**

Property 4: Preservation - Small List Behavior

_For any_ scroll state where total_entries <= visible_height, the fixed code SHALL set scroll_offset to 0 and display all entries without scrolling, exactly as the original code does.

**Validates: Requirements 3.1, 3.4**

Property 5: Preservation - Cursor Jump Behavior

_For any_ CursorJump transition where the cursor jumps to a position outside the current visible viewport, the fixed code SHALL adjust scroll_offset using the same logic as the original code, preserving the jump-to-position behavior.

**Validates: Requirements 3.3**

## Fix Implementation

### Changes Required

**File**: `rwf-lib/src/state.rs`

**Functions**: `update_state` - specifically the `Transition::CursorMove` and `Transition::CursorJump` match arms

**Specific Changes**:

1. **Fix Bottom Trigger Calculation**: Change the trigger condition from `cursor_in_view > bottom_trigger` to `cursor_in_view >= bottom_trigger` where `bottom_trigger = visible_height - scroll_margin - 1`. This ensures scrolling triggers at the correct position.

2. **Implement Desired Position Algorithm for CursorMove**: Replace the naive `scroll_offset + 1` increment with a calculation that positions the cursor at the desired line:
   ```rust
   // Calculate desired scroll_offset to keep cursor at bottom_trigger position
   let desired_offset = pane_model.cursor.saturating_sub(bottom_trigger);
   let max_offset = pane_model.entries.len().saturating_sub(visible_height);
   pane_model.scroll_offset = desired_offset.min(max_offset);
   ```

3. **Implement Desired Position Algorithm for CursorJump**: Apply the same logic in the CursorJump smooth scrolling section (when cursor is visible and needs to scroll down):
   ```rust
   // Calculate desired scroll_offset to keep cursor at bottom_trigger position
   let desired_offset = pane_model.cursor.saturating_sub(bottom_trigger);
   let max_offset = pane_model.entries.len().saturating_sub(visible_height);
   pane_model.scroll_offset = desired_offset.min(max_offset);
   ```

4. **Update bottom_trigger Calculation**: Change from `visible_height.saturating_sub(scroll_margin)` to `visible_height.saturating_sub(scroll_margin).saturating_sub(1)` to correctly calculate the trigger line (0-indexed).

5. **Preserve Existing Logic**: Keep all other scrolling logic unchanged, including:
   - Scroll up logic: `scroll_offset = cursor.saturating_sub(scroll_margin)`
   - CursorJump above viewport: `scroll_offset = cursor.saturating_sub(scroll_margin)`
   - CursorJump below viewport: existing calculation
   - Small list handling: `scroll_offset = 0`

### Algorithm Specification

**CursorMove Downward Scrolling Algorithm:**
```
GIVEN:
  cursor: current cursor position (absolute index)
  scroll_offset: current scroll offset
  visible_height: number of visible lines
  scroll_margin: minimum margin from edges
  total_entries: total number of entries

CALCULATE:
  cursor_in_view = cursor - scroll_offset
  bottom_trigger = visible_height - scroll_margin - 1
  max_offset = total_entries - visible_height

IF cursor_in_view >= bottom_trigger THEN
  desired_offset = cursor - bottom_trigger
  scroll_offset = MIN(desired_offset, max_offset)
END IF
```

**CursorJump Downward Scrolling Algorithm (when cursor is visible):**
```
GIVEN:
  cursor: new cursor position after jump (absolute index)
  scroll_offset: current scroll offset
  visible_height: number of visible lines
  scroll_margin: minimum margin from edges
  total_entries: total number of entries

CALCULATE:
  cursor_in_view = cursor - scroll_offset
  bottom_trigger = visible_height - scroll_margin - 1
  max_offset = total_entries - visible_height

IF cursor is visible AND cursor_in_view >= bottom_trigger THEN
  desired_offset = cursor - bottom_trigger
  scroll_offset = MIN(desired_offset, max_offset)
END IF
```

**Key Insight**: Both algorithms use the same core calculation: `desired_offset = cursor - bottom_trigger`, which positions the cursor at exactly the bottom_trigger line. The `min(max_offset)` clamp ensures we never scroll past the point where the viewport would show blank lines.

## Testing Strategy

### Validation Approach

The testing strategy follows a two-phase approach: first, write exploratory tests that demonstrate the bugs on the unfixed code to confirm the root cause analysis, then verify the fix works correctly and preserves existing behavior through comprehensive unit and property-based tests.

### Exploratory Fault Condition Checking

**Goal**: Surface counterexamples that demonstrate both bugs BEFORE implementing the fix. Confirm or refute the root cause analysis. If we refute, we will need to re-hypothesize.

**Test Plan**: Write unit tests that set up specific scroll states and simulate cursor movements, then assert the expected scroll_offset values. Run these tests on the UNFIXED code to observe failures and understand the root cause.

**Test Cases**:
1. **Premature Scroll Trigger Test**: Set cursor=66, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70, then move cursor to 67. Assert that scroll_offset should remain 50 (cursor_in_view=17 should not trigger yet) - will fail on unfixed code showing it triggers too early.

2. **Blank Lines Test**: Set cursor=66, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70, then move cursor to 67. After scrolling, assert that the viewport shows exactly 19 entries with no blank lines - will fail on unfixed code showing blank lines appear.

3. **Last Entry Margin Test**: Set cursor=73, scroll_offset=55, visible_height=19, scroll_margin=3, total_entries=74. Assert that scroll_offset should be 58 to position cursor at line 15 (maintaining scroll_margin) - will fail on unfixed code showing cursor at line 18.

4. **Correct Trigger Position Test**: Set cursor=65, scroll_offset=50, visible_height=19, scroll_margin=3, total_entries=70. Move cursor to 66 (cursor_in_view=16). Assert that scrolling should trigger and scroll_offset should become 50 (cursor - 16 = 66 - 16 = 50, which equals current offset, so no change yet) - will fail on unfixed code showing it doesn't trigger.

**Expected Counterexamples**:
- Scrolling triggers at cursor_in_view=17 instead of 16 (off by one)
- scroll_offset overshoots to max_offset, leaving blank lines
- Cursor at last entry violates scroll_margin by sitting on last visible line
- Possible causes: incorrect trigger condition (> instead of >=), naive increment logic, missing last-entry handling

### Fix Checking

**Goal**: Verify that for all inputs where the bug conditions hold, the fixed function produces the expected behavior.

**Pseudocode:**
```
FOR ALL input WHERE isBugCondition(input) DO
  result := update_state_fixed(input)
  ASSERT expectedBehavior(result)
END FOR

WHERE expectedBehavior(result) IS:
  // No blank lines
  LET visible_entries = MIN(visible_height, total_entries - scroll_offset)
  ASSERT visible_entries == MIN(visible_height, total_entries)
  
  // Cursor maintains scroll_margin when not at boundaries
  IF cursor < total_entries - 1 THEN
    ASSERT cursor_in_view <= visible_height - scroll_margin - 1
  END IF
  
  // Cursor at last entry maintains scroll_margin
  IF cursor == total_entries - 1 AND total_entries > visible_height THEN
    ASSERT cursor_in_view == visible_height - scroll_margin - 1
  END IF
END FOR
```

### Preservation Checking

**Goal**: Verify that for all inputs where the bug conditions do NOT hold, the fixed function produces the same result as the original function.

**Pseudocode:**
```
FOR ALL input WHERE NOT isBugCondition(input) DO
  ASSERT update_state_original(input) = update_state_fixed(input)
END FOR
```

**Testing Approach**: Property-based testing is recommended for preservation checking because:
- It generates many test cases automatically across the input domain
- It catches edge cases that manual unit tests might miss
- It provides strong guarantees that behavior is unchanged for all non-buggy inputs

**Test Plan**: Observe behavior on UNFIXED code first for scrolling up, small lists, and cursor jumps, then write property-based tests capturing that behavior.

**Test Cases**:
1. **Scroll Up Preservation**: Set various scroll states with cursor_in_view < scroll_margin, move cursor up, verify scroll_offset = cursor - scroll_margin (same as original)

2. **Small List Preservation**: Set total_entries <= visible_height, perform cursor movements, verify scroll_offset remains 0 (same as original)

3. **Cursor Jump Above Preservation**: Set cursor to jump above visible viewport, verify scroll_offset = cursor - scroll_margin (same as original)

4. **Cursor Jump Below Preservation**: Set cursor to jump below visible viewport, verify scroll_offset calculation matches original

### Unit Tests

- Test scrolling down with cursor at position 66 (visible_height=19, scroll_margin=3, 70 entries)
- Test cursor at last entry (cursor=73, 74 entries) maintains scroll_margin
- Test edge case where cursor is at last entry and max_offset is reached
- Test that scrolling triggers at correct cursor_in_view position (>= bottom_trigger)
- Test that viewport always shows visible_height entries when possible
- Test scrolling up continues to work correctly
- Test small lists (entries <= visible_height) don't scroll

### Property-Based Tests

- Generate random scroll states and verify downward scrolling maintains scroll_margin and fills viewport
- Generate random cursor positions and verify no blank lines appear after scrolling
- Generate random small lists and verify scroll_offset stays 0
- Generate random cursor jumps and verify scroll_offset calculations are correct
- Test that all scroll states maintain invariants: cursor_in_view in [0, visible_height), scroll_offset <= max_offset

### Integration Tests

- Test full scrolling flow from top to bottom of a large file list
- Test switching between panes and scrolling in each
- Test cursor jumps (Home, End, PageUp, PageDown) maintain correct scroll_offset
- Test that visual display shows no blank lines during continuous scrolling
- Test edge cases: exactly visible_height entries, visible_height + 1 entries, etc.
