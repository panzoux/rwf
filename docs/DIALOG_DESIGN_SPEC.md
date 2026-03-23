# Dialog Design Specification

## Document Status
- **Created**: 2026-03-14
- **Last Updated**: 2026-03-14
- **Version**: 2.0 (Final - After Implementation)
- **Status**: Approved & Implemented

---

# Part 1: Critical Design Principles

## ⚠️ CRITICAL: Lessons Learned (DO NOT REPEAT THESE MISTAKES)

### 1. NO Magic Numbers for Dialog Height
**WRONG:**
```rust
let min_height = 23u16;  // ❌ Magic number - where did this come from?
```

**CORRECT:**
```rust
// Calculate from actual layout constraints
pub fn get_compression_dialog_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Length(9),   // Format: 1 label + 8 items
        Constraint::Length(1),   // Spacing
        Constraint::Length(7),   // Compression: 1 label + 6 items
        Constraint::Length(1),   // Spacing
        Constraint::Length(2),   // Name: 1 label + 1 input
        Constraint::Length(1),   // Spacing
        Constraint::Length(1),   // Buttons
    ]
}

pub fn calculate_compression_dialog_min_height() -> u16 {
    get_compression_dialog_constraints()
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum()
}
```

**Why:** If we add/remove archive formats or compression levels, the height automatically adjusts. No hardcoded values to update.

---

### 2. Calculate Dialog Height BEFORE Rendering
**WRONG:**
```rust
// Using 70% for all dialogs → extra blank space
let dialog_height = percent_height.max(min_height)...
```

**CORRECT:**
```rust
// Compression dialog: use EXACT minimum height
let dialog_height = match &dialog.content {
    DialogContent::Compression { .. } => {
        min_dialog_height.min(screen_height.saturating_sub(2))
    }
    _ => { /* 70% for other dialogs */ }
};
```

**Why:** Compression dialog has fixed content. Extra space creates blank lines between sections.

---

### 3. Buttons Must Be WITHIN Dialog Content Area
**WRONG:**
```rust
// Split content area, render buttons separately
let chunks = Layout::default()
    .constraints([Constraint::Min(5), Constraint::Length(3)])
    .split(content_area);
// Buttons rendered in chunks[1] which may be OUTSIDE dialog border
```

**CORRECT:**
```rust
// For compression dialog, buttons are part of the layout constraints
let constraints = vec![
    Constraint::Length(9),   // Format
    Constraint::Length(1),   // Spacing
    Constraint::Length(7),   // Compression
    Constraint::Length(1),   // Spacing
    Constraint::Length(2),   // Name
    Constraint::Length(1),   // Spacing
    Constraint::Length(1),   // Buttons ← WITHIN content area
];
let chunks = Layout::default().constraints(constraints).split(area);
// Buttons rendered in chunks[6] which is INSIDE dialog border
```

**Why:** Buttons rendered outside content area appear BELOW the dialog bottom border.

---

### 4. Focus Field Indices Must Be Consistent
**WRONG:**
```rust
// In button rendering:
let buttons = [("OK", 0, true), ("Cancel", 1, false)];
// But dialog state uses: focused_field 3=OK, 4=Cancel
// Result: Focus never matches!
```

**CORRECT:**
```rust
// OK is field 3, Cancel is field 4 in the dialog state
let buttons = [
    ("OK", 3, true),    // ← Must match focused_field values
    ("Cancel", 4, false),
];
let is_focused = dialog_focused && state.focused_field == *field_idx;
```

**Why:** Focus highlighting fails if button field indices don't match dialog state.

---

### 5. Only 1 Line Spacing Between Sections
**WRONG:**
```rust
Constraint::Length(3),  // Name: 1 label + 1 input + 1 spacing
Constraint::Length(1),  // Extra spacing ← NO!
```

**CORRECT:**
```rust
Constraint::Length(2),  // Name: 1 label + 1 input
Constraint::Length(1),  // Spacing: 1 line ONLY
Constraint::Length(1),  // Buttons
```

**Why:** Multiple spacing lines create ugly gaps between sections.

---

# Part 2: Common Dialog Features (All Dialogs)

## 2.1 General Design Principles

| Aspect | Specification | Rationale |
|--------|---------------|-----------|
| **Width** | 60% of screen | Standard dialog width |
| **Height** | Calculated from constraints (NO magic numbers) | Adapts to content automatically |
| **Position** | Centered on screen | Standard modal dialog behavior |
| **Border** | Single line, Black color | Visible against gray background |
| **Background** | Gray | Distinguishes dialog from main UI |
| **Focus Movement** | Tab (forward), Shift+Tab (backward) | Standard Windows/TUI convention |
| **Focus Wrap** | Yes (last → first, first → last) | Continuous navigation |
| **Margin** | 2 lines (1 top, 1 bottom) | Prevents dialog touching screen edges |

## 2.2 Common Color Scheme

**Note**: Currently hardcoded. Future: move to config.json

| Element | Foreground | Background | Applies To |
|---------|------------|------------|------------|
| Border | Black | - | All dialogs |
| Background | - | Gray | All dialogs |
| Title | Black | Gray (transparent) | All dialogs |
| Label | Black | Gray (transparent) | Section labels |
| Text (unfocused) | Black | Gray (transparent) | List items, buttons |
| **Text (focused item)** | **Black** | **White** | **Focused item ONLY** |
| Textbox (unfocused) | White | DarkGray | Input fields |
| **Textbox (focused)** | **Black** | **White** | **Input field when focused** |
| Button (unfocused) | Black | Gray | Unfocused buttons |
| **Button (focused)** | **Black** | **White** | **Focused button** |

## 2.3 Focus & Selection Model

### ⚠️ CRITICAL: Single Focus Rule

**Only ONE item in the ENTIRE dialog has white background at any time.** This makes it always clear which section and which item has focus.

### Focus Indicator
```
Format: White background on focused item ONLY
        (NOT on entire section, NOT on multiple items)
        
Example when Format section has focus, item 0 focused:
┌──────────────────────────────┐
│ ●ZIP ○7Z ○BZ2 ○TAR          │  ← ZIP has WHITE bg
│                              │
│ ○Store (0)                   │  ← All GRAY bg
│ ○Fastest (1)                 │
│ ●Normal (5)                  │
└──────────────────────────────┘
```

### Selection Indicator (for list-type inputs)
```
Selected:   ● (bullet character)
Unselected: ○ (white circle)

Selection is INDEPENDENT of focus.
An item can be:
- Selected but not focused: ● with gray bg
- Focused but not selected: ○ with white bg
- Both selected and focused: ● with white bg
- Neither: ○ with gray bg
```

### Default Button Indicator
```
Format: [*Label*] with asterisks for default
        [Label] for other buttons
        Focus indicated by WHITE background
Example: [*OK*] vs [Cancel]
         [*OK*] (focused, white bg) vs [Cancel] (gray bg)
```

## 2.4 Common Key Bindings

| Key | Action | Scope |
|-----|--------|-------|
| Tab | Move focus to next section | All dialogs |
| Shift+Tab | Move focus to previous section | All dialogs |
| Enter | Activate default button / Confirm | All dialogs |
| Escape | Cancel dialog | All dialogs |
| Up | Move focus up (within focused section) | List sections only |
| Down | Move focus down (within focused section) | List sections only |
| Left | Move cursor left | Text inputs only |
| Right | Move cursor right | Text inputs only |
| Space | Set selection at focused position | List sections only |
| Backspace | Delete character left | Text inputs only |
| Home | Move cursor to start | Text inputs only |
| End | Move cursor to end | Text inputs only |

## 2.5 Dialog Structure Template

```
┌──────────────────────────────────────┐  ← Border (line 1)
│ Dialog Title                         │
│                                      │
│ [Section 1 Label]                    │
│ [Item with focus has WHITE bg]       │  ← Only ONE item has white bg
│ [Other items have GRAY bg]           │
│                                      │  ← 1 line spacing ONLY
│ [Section 2 Label]                    │
│ [All items GRAY if section not focused]│
│                                      │  ← 1 line spacing ONLY
│ [Text Input Label]                   │
│ [Textbox: BLACK/WHITE if focused,    │
│  WHITE/DARKGRAY if not]              │
│                                      │  ← 1 line spacing ONLY
│              [*OK*]  [Cancel]        │  ← Buttons WITHIN content area
└──────────────────────────────────────┘  ← Border (last line)
```

## 2.6 Section Types

### Type A: Vertical List (e.g., format selection)
- Items arranged top-to-bottom
- Focus: ONE item has white background
- Selection: `●` vs `○` (independent of focus)
- Navigation: Up/Down moves focus, Space sets selection

### Type B: Text Input
- Single line input
- No border (textbox only)
- Focused: Black text on White background + cursor block (█)
- Unfocused: White text on DarkGray background
- Input: Character keys, Backspace, Left/Right, Home, End

### Type C: Button
- Single item, no internal navigation
- Format: `[Label]` or `[*Label*]` for default
- Focused: Black text on White background
- Unfocused: Black text on Gray background
- Activated by Enter when focused

---

# Part 3: Compression Dialog Specific Features

## 3.1 Dialog Title
```
"Compress Files"
```

## 3.2 Sections (In Tab Order)

| Order | Section | Type | Label | Content | Focus Field |
|-------|---------|------|-------|---------|-------------|
| 0 | Archive Format | Vertical List | "Archive Format:" | 8 format options | 0 |
| 1 | Compression Level | Vertical List | "Compression Level:" | 6 level options | 1 |
| 2 | Archive Name | Text Input | "Archive Name:" | Editable text | 2 |
| 3 | OK Button | Button | N/A | `[*OK*]` (default) | 3 |
| 4 | Cancel Button | Button | N/A | `[Cancel]` | 4 |

## 3.3 Layout Constraints (CRITICAL - NO MAGIC NUMBERS)

```rust
pub fn get_compression_dialog_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Length(9),   // Archive format: 1 label + 8 items
        Constraint::Length(1),   // Spacing: 1 line ONLY
        Constraint::Length(7),   // Compression levels: 1 label + 6 items
        Constraint::Length(1),   // Spacing: 1 line ONLY
        Constraint::Length(2),   // Archive name: 1 label + 1 input
        Constraint::Length(1),   // Spacing: 1 line ONLY
        Constraint::Length(1),   // Buttons: 1 line
    ]
}
```

**Total Content Height:** 23 lines (calculated, NOT hardcoded)
**Total Dialog Height:** 25 lines (23 content + 2 borders)

## 3.4 Archive Format Options

| Index | Format | Display | Notes |
|-------|--------|---------|-------|
| 0 | ZIP | `●ZIP ` | **Default selection** |
| 1 | 7Z | `○7Z ` | Future implementation |
| 2 | BZ2 | `○BZ2 ` | Future implementation |
| 3 | TAR | `○TAR ` | Future implementation |
| 4 | LZH | `○LZH ` | Future implementation |
| 5 | CAB | `○CAB ` | Future implementation |
| 6 | XZ | `○XZ ` | Future implementation |
| 7 | LZMA | `○LZMA ` | Future implementation |

**Layout Behavior:**
- Vertical list (one format per line)
- All 8 formats always shown
- Focus: ONE item has white background (when section 0 is focused)

## 3.5 Compression Level Options

| Index | Level | Display | Default Selection |
|-------|-------|---------|-------------------|
| 0 | 0 | `○ Store (0)` | No |
| 1 | 1 | `○ Fastest (1)` | No |
| 2 | 3 | `○ Fast (3)` | No |
| 3 | 5 | `● Normal (5)` | **Yes** |
| 4 | 7 | `○ Maximum (7)` | No |
| 5 | 9 | `○ Ultra (9)` | No |

**Layout Behavior:**
- Vertical list (one level per line)
- All 6 levels always shown
- Focus: ONE item has white background (when section 1 is focused)

## 3.6 Archive Name Input

| Property | Value |
|----------|-------|
| **Default Value** | Selected filename (without extension) or "archive" |
| **Auto-extension** | Adds `.zip` if not present on confirmation |
| **Textbox Style (unfocused)** | White text on DarkGray background |
| **Textbox Style (focused)** | **Black text on White background** + cursor |
| **Cursor** | Block character (█) when focused |
| **Max Length** | Limited by dialog width (~40 chars) |

## 3.7 Button Layout

```
Layout: [*OK*]  [Cancel]
Position: Bottom of content area, centered horizontally
Spacing: 2 spaces between buttons
```

| Button | Display | Action | Shortcut | Focus Field |
|--------|---------|--------|----------|-------------|
| OK | `[*OK*]` | Create archive with selected options | Enter (anywhere) | 3 |
| Cancel | `[Cancel]` | Close dialog without action | Escape | 4 |

**Button Focus:**
- OK focused (field 3): `[*OK*]` with **White background**
- Cancel focused (field 4): `[Cancel]` with **White background**
- Unfocused: **Gray background**

## 3.8 Complete Layout Mockup

```
┌──────────────────────────────────────────────┐  ← Line 1 (border + title)
│ Compress Files                               │
│                                              │
│ Archive Format:                              │  ← Line 2 (label)
│ ●ZIP                                         │  ← Lines 3-10 (8 formats)
│ ○7Z                                          │
│ ○BZ2                                         │
│ ○TAR                                         │
│ ○LZH                                         │
│ ○CAB                                         │
│ ○XZ                                          │
│ ○LZMA                                        │
│                                              │  ← Line 11 (1 line spacing)
│ Compression Level:                           │  ← Line 12 (label)
│   ○ Store (0)                                │  ← Lines 13-18 (6 levels)
│   ○ Fastest (1)                              │
│   ○ Fast (3)                                 │
│   ●Normal (5)                                │
│   ○ Maximum (7)                              │
│   ○ Ultra (9)                                │
│                                              │  ← Line 19 (1 line spacing)
│ Archive Name:                                │  ← Line 20 (label)
│ archive.zip                                  │  ← Line 21 (input)
│                                              │  ← Line 22 (1 line spacing)
│              [*OK*]  [Cancel]                │  ← Line 23 (buttons)
└──────────────────────────────────────────────┘  ← Lines 24-25 (bottom border)
```

**Total: 25 lines** (23 content + 2 borders) - NO extra blank lines!

## 3.9 Height Calculation (CRITICAL)

```rust
// In render_dialog(), BEFORE rendering:
let min_content_height = match &dialog.content {
    DialogContent::Compression { .. } => {
        // Calculate from actual layout constraints
        crate::ui::dialog::compression::calculate_compression_dialog_min_height()
    }
    DialogContent::ExtractionConfirm { .. } => {
        6u16  // Extraction dialog: ~6 lines content
    }
    _ => 8u16, // Default
};

// Add 2 for borders (top + bottom)
let min_dialog_height = min_content_height + 2;

let screen_height = frame.area().height;

// For compression dialog, use EXACT minimum height (no extra space)
let dialog_height = match &dialog.content {
    DialogContent::Compression { .. } => {
        min_dialog_height.min(screen_height.saturating_sub(2))
    }
    _ => {
        // Use 70% of screen or minimum, whichever is larger
        let percent_height = (screen_height * 70) / 100;
        percent_height.max(min_dialog_height).min(screen_height.saturating_sub(2))
    }
};
```

**Why:** Compression dialog has fixed content. Using 70% creates extra blank space.

## 3.10 State Transitions

### On Dialog Open
1. Load marked files or current file
2. Set default format: ZIP (index 0, selected)
3. Set default compression: Normal (index 3, selected)
4. Set default name: filename without extension
5. Set focus: Archive Format (field 0, format_focus_index = 0)

### On OK Confirm
1. Validate archive name (not empty)
2. Add `.zip` extension if missing
3. Calculate original file size
4. Create `JobKind::CreateArchive` job
5. Submit job to worker pool
6. Close dialog

### On Cancel
1. Close dialog without action
2. No job created

---

# Part 4: Implementation Checklist

## 4.1 Dialog Height Calculation
- [ ] Define layout constraints in `get_compression_dialog_constraints()`
- [ ] Implement `calculate_compression_dialog_min_height()` that sums constraints
- [ ] Call calculation function in `render_dialog()` BEFORE setting dialog height
- [ ] Use exact minimum height for compression dialog (NOT 70%)
- [ ] Add 2 for borders (top + bottom)
- [ ] Ensure dialog height ≤ screen_height - 2 (leave 1 line margin top/bottom)

## 4.2 Layout Constraints
- [ ] Format section: 9 lines (1 label + 8 items)
- [ ] Spacing: 1 line ONLY (not 2-3 lines)
- [ ] Compression section: 7 lines (1 label + 6 items)
- [ ] Spacing: 1 line ONLY
- [ ] Name section: 2 lines (1 label + 1 input)
- [ ] Spacing: 1 line ONLY
- [ ] Buttons: 1 line
- [ ] **Total: 23 lines content** (verify by summing constraints)

## 4.3 Button Rendering
- [ ] Buttons rendered WITHIN content area (chunks[6], NOT separate chunks[1])
- [ ] Button field indices match dialog state (OK=3, Cancel=4)
- [ ] Focus check: `state.focused_field == *field_idx`
- [ ] Default button shows `[*Label*]` with asterisks
- [ ] Focused button has white background
- [ ] Buttons centered horizontally

## 4.4 Focus Handling
- [ ] Only ONE item has white background at any time
- [ ] Tab cycles: 0 → 1 → 2 → 3 → 4 → 0
- [ ] Up/Down in Format (field 0): moves format_focus_index
- [ ] Up/Down in Compression (field 1): moves compression_focus_index
- [ ] Left/Right in Name (field 2): moves cursor_pos
- [ ] Space in Format/Compression: sets selection to current focus
- [ ] Enter anywhere: activates OK button
- [ ] Escape: cancels dialog

## 4.5 Visual Verification
- [ ] NO extra blank lines between sections (only 1 line spacing)
- [ ] NO extra blank lines after Archive Name (before buttons)
- [ ] Buttons WITHIN dialog border (NOT below bottom border)
- [ ] All 8 formats visible
- [ ] All 6 compression levels visible
- [ ] Focus clearly visible (white background on ONE item only)
- [ ] Selection clearly visible (● vs ○)

---

# Part 5: Testing Checklist

## 5.1 Height Calculation
- [ ] Dialog height = 25 lines (23 content + 2 borders) on tall screens
- [ ] Dialog height scales down on short screens (but never < 25 lines)
- [ ] NO blank lines between Archive Name and buttons
- [ ] NO blank lines between sections (only 1 line spacing)

## 5.2 Focus Navigation
- [ ] Tab: Format (0) → Compression (1) → Name (2) → OK (3) → Cancel (4) → Format (0) [wraps]
- [ ] Shift+Tab: Reverse order
- [ ] Up in Format (field 0): Moves format_focus_index up (max 0)
- [ ] Down in Format (field 0): Moves format_focus_index down (max 7)
- [ ] Up in Compression (field 1): Moves compression_focus_index up (max 0)
- [ ] Down in Compression (field 1): Moves compression_focus_index down (max 5)
- [ ] Left in Name (field 2): Moves cursor_pos left (max 0)
- [ ] Right in Name (field 2): Moves cursor_pos right (max name.len())
- [ ] Home in Name: Sets cursor_pos to 0
- [ ] End in Name: Sets cursor_pos to name.len()
- [ ] Up/Down/Left/Right/Space in OK/Cancel (fields 3-4): No effect

## 5.3 Selection
- [ ] Space in Format (field 0): Sets selected_format_index = format_focus_index
- [ ] Space in Compression (field 1): Sets selected_compression_index = compression_focus_index
- [ ] Only one format selected at a time (radio behavior)
- [ ] Only one compression level selected at a time (radio behavior)
- [ ] Selection persists when focus moves to different field
- [ ] Selection persists when dialog is closed and reopened (same session)

## 5.4 Visual - CRITICAL
- [ ] **ONLY ONE item in entire dialog has white background at any time**
- [ ] When Format section focused: ONE format item has white bg, ALL other items have gray bg
- [ ] When Compression section focused: ONE compression item has white bg, ALL other items have gray bg
- [ ] When Name focused: Name textbox has black text on white bg, ALL list items have gray bg
- [ ] When OK focused: OK button has white bg, ALL list items have gray bg
- [ ] When Cancel focused: Cancel button has white bg, ALL list items have gray bg
- [ ] Selection visible: `●` (selected) vs `○` (unselected), both black
- [ ] All list item text is black
- [ ] Buttons show `[*OK*]` (default) and `[Cancel]`
- [ ] Focused button: white background
- [ ] Labels ("Archive Format:", etc.) are black on gray (transparent)
- [ ] Buttons appear once (not duplicated)
- [ ] Dialog height is exactly 25 lines (no extra blank lines)
- [ ] All 8 formats visible
- [ ] All 6 compression levels visible
- [ ] Textbox has no border
- [ ] Cursor is block character (█) in textbox when focused
- [ ] Default button shows `[*OK*]` with asterisks

## 5.5 Functionality
- [ ] Enter anywhere: Activates OK button (creates archive job)
- [ ] Escape: Cancels dialog (no job created)
- [ ] Default name: Selected filename without extension (or "archive" for multiple)
- [ ] Auto .zip: Extension added on confirmation if not present
- [ ] Job created: CreateArchive with correct sources, dest, original_size
- [ ] Dialog closes: On both OK and Cancel
- [ ] State persistence: Dialog state survives if another dialog opens on top
- [ ] Multiple files: Correctly handles 1 or multiple marked files
- [ ] Empty pane: Falls back to opposite pane if active pane is empty

---

# Appendix A: Complete Focus Field Reference

| Field Value | Section | Navigation Keys | Selection Key |
|-------------|---------|-----------------|---------------|
| 0 | Archive Format | Up/Down (moves format_focus_index 0-7) | Space (sets selected_format_index) |
| 1 | Compression Level | Up/Down (moves compression_focus_index 0-5) | Space (sets selected_compression_index) |
| 2 | Archive Name | Left/Right (moves cursor_pos), Home, End | N/A (text input) |
| 3 | OK Button | N/A (single item) | Enter (activates) |
| 4 | Cancel Button | N/A (single item) | Enter (activates) |

**Tab Order:** 0 → 1 → 2 → 3 → 4 → 0 (wraps)

---

# Appendix B: State Structure

```rust
pub struct CompressionDialogState {
    // Data fields
    pub archive_name: String,
    
    // Selection state (what is chosen)
    pub selected_format_index: usize,        // 0-7
    pub selected_compression_index: usize,   // 0-5
    
    // Focus state (what has white background)
    pub focused_field: usize,                // 0-4: which section
    pub format_focus_index: usize,           // 0-7: which format item
    pub compression_focus_index: usize,      // 0-5: which compression item
    pub cursor_pos: usize,                   // Text cursor position
}
```

**Focus Field Values:**
- 0 = Archive Format section (Up/Down moves format_focus_index)
- 1 = Compression Level section (Up/Down moves compression_focus_index)
- 2 = Archive Name textbox (Left/Right moves cursor_pos)
- 3 = OK button
- 4 = Cancel button

**Tab Order:** 0 → 1 → 2 → 3 → 4 → 0 (wraps)

---

# Appendix C: Common Mistakes & Solutions

| Mistake | Symptom | Solution |
|---------|---------|----------|
| Hardcoded height (e.g., `23u16`) | Can't add/remove formats without updating magic number | Calculate from constraints using `calculate_compression_dialog_min_height()` |
| Using 70% for compression dialog | Extra blank lines between sections | Use exact minimum height for compression dialog |
| Buttons in separate layout chunk | Buttons appear BELOW dialog bottom border | Include buttons in content layout constraints (chunks[6]) |
| Wrong button field indices | Focus highlighting doesn't work on buttons | Use correct indices: OK=3, Cancel=4 |
| Multiple spacing lines | Ugly gaps between sections | Use exactly 1 line spacing between sections |
| Rendering buttons twice | Two sets of buttons visible | Render buttons ONLY in compression dialog, NOT in generic render_dialog |
| Dialog height > screen height | Dialog clipped or doesn't fit | Cap at `screen_height.saturating_sub(2)` |

---

# Appendix D: File Locations

| File | Purpose |
|------|---------|
| `rwf-bin/src/ui/dialog/mod.rs` | Main dialog rendering, height calculation |
| `rwf-bin/src/ui/dialog/compression.rs` | Compression dialog layout constraints, rendering |
| `rwf-bin/src/ui/dialog/frame.rs` | Generic button rendering (for other dialogs) |
| `rwf-lib/src/model/dialog.rs` | DialogContent enum with embedded state |
| `docs/DIALOG_DESIGN_SPEC.md` | This specification document |

---

# Appendix E: Future Enhancements (Not Yet Implemented)

- [ ] Config file color entries (currently hardcoded)
- [ ] Format-specific compression levels
- [ ] Additional archive formats (7Z, BZ2, etc.) - backend implementation
- [ ] Password protection option
- [ ] Split archive option
- [ ] Size estimation before compression
- [ ] Dynamic format availability (hide unavailable formats)
- [ ] Dialog state persistence across sessions
