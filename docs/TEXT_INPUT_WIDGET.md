# TextInput Widget Documentation

A reusable single-line text input widget for `ratatui` with CJK support and dual Emacs/Vi editing modes.

## Overview

The `TextInput` widget provides a powerful text entry field suitable for dialogs and command lines. It handles complex text editing features like undo/redo, kill buffers, and modal editing (Vi mode) while maintaining correct display for CJK (Chinese, Japanese, Korean) characters.

## Features

- **CJK Support**: Proper width calculation and cursor positioning for double-width characters.
- **Dual Editing Modes**:
  - **Emacs (default)**: Standard CLI-style shortcuts.
  - **Vi**: Modal editing with Normal and Insert modes.
- **Undo/Redo**: Internal history stack for undoing and redoing changes.
- **Kill Buffer**: Internal clipboard for kill/yank operations (shared across modes).
- **Horizontal Scrolling**: Automatically keeps the cursor visible in long strings.
- **Mode Persistence**: State can be exported and imported to survive widget reconstruction.

## Usage Rules

To use `TextInput` properly, especially within dynamic UI components like dialogs, follow these rules:

### 1. Initialization and Context
When creating the widget, you should decide if it starts in Emacs or Vi mode.
```rust
let mut input = TextInput::new(Some("initial text".to_string()), EditMode::Emacs);
// Set the original text for Vi 'U' command (revert to original)
input.set_original_text("initial text".to_string());
```

### 2. State Persistence (Crucial)
If your UI component (e.g., a Dialog) is reconstructed every frame or when the UI state changes, you **must** persist the following internal states of the `TextInput` if you want a seamless user experience. Without this, pending operations (like `d` waiting for a motion) will be lost between frames.

**Required persistence fields:**
- `vi_mode`: Current sub-mode (Normal/Insert).
- `pending_operator`: Current pending Vi operator (e.g., `Delete`, `Change`).
- `pending_find_backward`: If waiting for a character for `f` or `F`.
- `pending_ctrl_x`: If waiting for `U` after `Ctrl+X`.
- `history` and `history_index`: To allow Undo/Redo to persist.

### 3. Event Handling
The widget provides a `handle_input` method that returns a `TextInputAction`.
```rust
let action = input.handle_input(&key_event);
match action {
    TextInputAction::Confirm => { /* User pressed Enter */ },
    TextInputAction::Cancel => { /* User pressed Esc */ },
    TextInputAction::TextChanged => { /* Text was modified, maybe update external state */ },
    _ => { /* Other actions like CursorMoved, ModeToggled, etc. */ }
}
```

### 4. Rendering
The widget requires a `Frame`, a `Rect` (the drawing area), and a focus boolean.
```rust
input.set_width(area.width); // Important for scrolling calculation
input.render(frame, area, is_focused);
```
**Note:** In Vi mode, the widget will render a mode indicator (e.g., `-NORMAL-`) at the right edge of the provided `area`.

## Keybindings

### Emacs Mode
| Key | Action |
|-----|--------|
| `Ctrl+A` / `Home` | Beginning of line |
| `Ctrl+E` / `End` | End of line |
| `Ctrl+D` / `Delete` | Delete character at cursor |
| `Ctrl+H` / `Backspace` | Delete character before cursor |
| `Ctrl+K` | Kill to end of line |
| `Ctrl+U` | Kill to beginning of line |
| `Ctrl+Y` | Yank (paste) from kill buffer |
| `Ctrl+W` | Delete word before cursor |
| `Ctrl+T` | Transpose characters |
| `Ctrl+Z` / `Ctrl+/` / `Ctrl+_` | Undo |
| `Alt+Y` | Redo |
| `Ctrl+X` / `Alt+X` | Toggle mode (Emacs <-> Vi) |
| `Enter` | Confirm (returns `TextInputAction::Confirm`) |
| `Esc` | Cancel (returns `TextInputAction::Cancel`) |

### Vi Mode (Normal)
| Key | Action |
|-----|--------|
| `h` / `l` | Move cursor left / right |
| `0` / `^` / `Home` | Beginning of line / First non-blank |
| `$` / `End` | End of line |
| `w` / `b` / `e` | Next / Previous word beginning / end |
| `W` / `B` / `E` | Next / Previous WORD (stops at `.` for filenames) |
| `f{char}` / `F{char}` | Find character next / previous |
| `;` / `,` | Repeat last find / Repeat in opposite direction |
| `x` | Delete character at cursor |
| `i` / `a` | Enter Insert mode at cursor / after cursor |
| `I` / `A` | Enter Insert mode at BOL / EOL |
| `c{motion}` | Change (delete range and enter Insert mode) |
| `d{motion}` | Delete (delete range) |
| `u` | Undo (step back in history) |
| `U` | Revert to original text (initial state when dialog opened) |
| `Ctrl+R` | Redo |
| `Ctrl+X` | Start two-key sequence (e.g., `Ctrl+X U` for undo) |
| `Alt+X` | Toggle mode (Vi <-> Emacs) |

### Vi Mode (Insert)
| Key | Action |
|-----|--------|
| `Esc` | Return to Normal mode |
| `Backspace` | Delete character before cursor |
| `Enter` | Confirm (returns `TextInputAction::Confirm`) |
| Any Char | Insert character |

## Implementation Details

### CJK Handling
The widget uses the `unicode-width` crate to determine the visual width of each character. This is essential for:
1. **Cursor Positioning**: The cursor must jump 2 cells for double-width characters.
2. **Scrolling**: Horizontal scrolling must account for the actual visual width consumed, not just the number of characters.

### Undo/Redo Logic
The widget maintains a `Vec<String>` of history. `save_to_history()` is called automatically before any destructive operation. The history is limited to 100 entries.

### Horizontal Scrolling
The `scroll` field tracks the starting visual width offset. `update_scroll()` is called whenever the cursor moves or text changes to ensure the cursor remains visible within the widget's `width`.
