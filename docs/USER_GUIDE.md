# RWF: Reactive Worker Filemanager - User Guide

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Key Bindings](#key-bindings)
4. [Configuration](#configuration)
5. [Custom Functions](#custom-functions)
6. [Usage Examples](#usage-examples)
7. [Advanced Features](#advanced-features)

## Introduction

The Two-Pane File Manager is a terminal-based file manager built in Rust that provides efficient dual-pane navigation, asynchronous file operations, and extensive customization options. It follows the TWF (Two-pane File manager for Windows) design philosophy while being cross-platform.

### Key Features

- **Dual-pane interface** for easy file comparison and transfer
- **Tab management** with independent pane states per tab
- **Asynchronous file operations** that never block the UI
- **Custom functions** with macro expansion
- **Archive browsing** (.zip support with extensible format handlers)
- **Advanced search** with wildcards, regex, and migemo support
- **Registered folders** for quick navigation
- **Multiple display modes** (1-8 columns or detailed view)
- **Text/hex viewer** with encoding support
- **Pattern-based batch rename**
- **File comparison** and split/join operations
- **Session persistence** with tab restoration

## Getting Started

### Installation

```bash
cargo build --release
```

The binary will be located at `target/release/rwf`.

### First Launch

On first launch, the application will:
1. Initialize both panes with your current working directory
2. Create default configuration files in `~/.config/rwf/` (Linux/macOS) or `%APPDATA%\rwf\` (Windows)
3. Load TWF-compatible default key bindings

### Basic Navigation

- **Tab**: Switch between left and right panes
- **Up/Down** or **k/j**: Move cursor up/down
- **Enter**: Enter directory or browse archive
- **Backspace** or **Left**: Go to parent directory
- **Home/End**: Jump to first/last entry
- **Page Up/Down**: Scroll by page

## Key Bindings

All key bindings are configurable via `keybindings.json`. Below are the default TWF-compatible bindings:

### Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Switch Pane | Toggle between left and right panes |
| `Up` / `k` | Cursor Up | Move cursor up one entry |
| `Down` / `j` | Cursor Down | Move cursor down one entry |
| `Left` / `Backspace` | Parent Directory | Navigate to parent directory |
| `Right` / `Enter` | Enter Directory | Enter directory or browse archive |
| `Home` | Jump to First | Move cursor to first entry |
| `End` | Jump to Last | Move cursor to last entry |
| `Page Up` | Page Up | Scroll up by one page |
| `Page Down` | Page Down | Scroll down by one page |
| `Alt+Left` | History Back | Navigate to previous location in history |
| `Alt+Right` | History Forward | Navigate to next location in history |

### File Operations

| Key | Action | Description |
|-----|--------|-------------|
| `C` | Copy | Copy selected/marked files to opposite pane |
| `M` | Move | Move selected/marked files to opposite pane |
| `D` | Delete | Delete selected/marked files |
| `R` | Rename | Rename current file |
| `Shift+K` | Create Directory | Create new directory |
| `Shift+R` | Pattern Rename | Batch rename with pattern |
| `H` | Calculate Size | Calculate directory size |

### Marking

| Key | Action | Description |
|-----|--------|-------------|
| `Space` | Toggle Mark | Mark/unmark current file |
| `*` | Mark All | Mark all files in pane |
| `Ctrl+U` | Unmark All | Clear all marks |
| `@` | Mark Pattern | Mark files matching wildcard pattern |
| `Ctrl+Space` | Range Mark | Mark range from initial to current position |
| `Home` (with Shift) | Invert Marks | Invert all marks in pane |

### Search and Filtering

| Key | Action | Description |
|-----|--------|-------------|
| `/` or `Ctrl+F` | Start Search | Enter incremental search mode |
| `F` | Filter | Apply file mask filter |
| `Ctrl+K` | Clear Filter | Clear active search/filter |
| `F3` | Next Match | Find next search result |
| `Shift+F3` | Previous Match | Find previous search result |

### Sorting and Display

| Key | Action | Description |
|-----|--------|-------------|
| `s` then `n` | Sort by Name | Sort files by name |
| `s` then `s` | Sort by Size | Sort files by size |
| `s` then `d` | Sort by Date | Sort files by modification date |
| `s` then `e` | Sort by Extension | Sort files by extension |
| `1`-`8` | Column Mode | Set display to 1-8 columns |
| `0` | Detailed Mode | Show detailed file information |

### Tab Management

| Key | Action | Description |
|-----|--------|-------------|
| `Ctrl+N` / `Ctrl+T` | New Tab | Create new tab |
| `Ctrl+W` | Close Tab | Close current tab (if not last) |
| `Ctrl+Right` / `Ctrl+PageDown` | Next Tab | Switch to next tab |
| `Ctrl+Left` / `Ctrl+PageUp` | Previous Tab | Switch to previous tab |
| `Ctrl+T` / `Ctrl+B` | Tab Selector | Show tab selector dialog |

### Custom Functions and Folders

| Key | Action | Description |
|-----|--------|-------------|
| `Shift+T` / `Shift+F` | Custom Functions | Show custom function selector |
| `I` / `G` / `Shift+F` | Registered Folders | Show registered folder selector |
| `Shift+B` | Register Folder | Register current directory |
| `Shift+M` | Move to Registered | Move marked files to registered folder |

### Archive Operations

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` (on archive) | Browse Archive | Open virtual folder view of archive |
| `Shift+Enter` (on archive) | Extract Archive | Extract archive to opposite pane |
| `P` | Create Archive | Create .zip archive from marked files |

### Viewing and Comparison

| Key | Action | Description |
|-----|--------|-------------|
| `V` | Text Viewer | Open text viewer for current file |
| `F8` / `B` | Hex Viewer | Open hex viewer for current file |
| `W` | Compare Files | Compare current file with opposite pane |
| `Shift+W` | Split/Join | Show file split/join dialog |
| `Shift+I` | File Info | Show detailed file information |

### Pane Operations

| Key | Action | Description |
|-----|--------|-------------|
| `O` | Sync Panes | Synchronize opposite pane to active pane path |
| `Shift+O` | Swap Panes | Swap left and right pane paths |

### Application Control

| Key | Action | Description |
|-----|--------|-------------|
| `Q` / `Escape` | Quit | Exit application |
| `Shift+Q` | Exit and CD | Exit and change shell directory |
| `?` / `F1` | Help | Show help dialog |
| `Shift+Z` | Reload Config | Reload configuration without restart |
| `Y` | Edit Config | Launch editor with configuration file |
| `Ctrl+J` | Job Manager | Show detailed job manager dialog |
| `\` / `` ` `` | Context Menu | Show context menu |
| `Shift+L` | Drive Selection | Show drive/share selection dialog |

### Task Panel

| Key | Action | Description |
|-----|--------|-------------|
| `Ctrl+Up` | Increase Panel | Increase task panel height |
| `Ctrl+Down` | Decrease Panel | Decrease task panel height |
| `Alt+Up` | Scroll Up | Scroll task panel up |
| `Alt+Down` | Scroll Down | Scroll task panel down |

### Viewer Mode Keys

When in text/hex viewer:

| Key | Action | Description |
|-----|--------|-------------|
| `Escape` / `Q` | Close Viewer | Exit viewer and return to file manager |
| `Up` / `Down` | Scroll | Scroll content up/down |
| `Page Up` / `Page Down` | Page Scroll | Scroll by page |
| `Home` | Line Start | Move to start of current line |
| `End` | Line End | Move to end of current line |
| `F5` | File Start | Jump to start of file |
| `F6` | File End | Jump to end of file |
| `F4` | Search | Enter search mode |
| `F3` | Next Match | Find next search result |
| `Shift+F3` | Previous Match | Find previous search result |
| `Shift+E` | Cycle Encoding | Cycle through text encodings |

## Configuration

Configuration files are stored in:
- **Linux/macOS**: `~/.config/rwf/`
- **Windows**: `%APPDATA%\rwf\`

### Main Configuration (`config.json`)

```json
{
  "display": {
    "show_hidden": false,
    "show_system": false,
    "date_format": "%Y-%m-%d %H:%M",
    "time_format": "TwentyFourHour",
    "cjk_width": 2,
    "colors": {
      "foreground_color": "White",
      "background_color": "Black",
      "highlight_foreground_color": "Black",
      "highlight_background_color": "Cyan",
      "marked_file_color": "Cyan",
      "directory_color": "BrightCyan",
      "pane_border_color": "Red",
      "ok_color": "Green",
      "warning_color": "Yellow",
      "error_color": "Red"
    }
  },
  "file_operations": {
    "confirm_delete": true,
    "confirm_overwrite": true,
    "buffer_size": 8192,
    "preserve_timestamps": true
  },
  "search": {
    "case_sensitive": false,
    "use_regex": false,
    "use_migemo": false,
    "max_results": 1000
  },
  "ui": {
    "refresh_rate": 30,
    "scroll_offset": 3,
    "tab_width": 4
  },
  "worker_pool_size": 4,
  "log_level": "Info",
  "session_persistence": true
}
```

### Key Bindings (`keybindings.json`)

```json
{
  "normal_mode": {
    "Tab": "SwitchPane",
    "Up": "CursorUp",
    "Down": "CursorDown",
    "k": "CursorUp",
    "j": "CursorDown",
    "Enter": "EnterDirectory",
    "Backspace": "ParentDirectory",
    "Space": "ToggleMark",
    "*": "MarkAll",
    "Ctrl+U": "UnmarkAll",
    "C": "Copy",
    "M": "Move",
    "D": "Delete",
    "R": "Rename",
    "Shift+K": "CreateDirectory",
    "Q": "Quit",
    "Escape": "Quit"
  },
  "viewer_mode": {
    "Escape": "CloseViewer",
    "Q": "CloseViewer",
    "F4": "StartSearch",
    "F3": "NextMatch",
    "Shift+F3": "PrevMatch",
    "Shift+E": "CycleEncoding"
  }
}
```

### Custom Functions (`custom_functions.json`)

Custom functions allow you to define shell commands with macro expansion:

```json
[
  {
    "name": "Open Terminal Here",
    "key": "Ctrl+Shift+T",
    "command": "gnome-terminal --working-directory=$P",
    "shell": "bash",
    "description": "Open terminal in current directory"
  },
  {
    "name": "Git Status",
    "key": "Ctrl+G",
    "command": "git status",
    "shell": "bash",
    "pipe_to_action": null,
    "description": "Show git status"
  },
  {
    "name": "Find Large Files",
    "key": null,
    "command": "find $P -type f -size +100M",
    "shell": "bash",
    "pipe_to_action": "JumpToPath",
    "description": "Find files larger than 100MB"
  }
]
```

#### Macro Reference

| Macro | Expands To | Description |
|-------|------------|-------------|
| `$P` | Active pane path | Current directory of active pane |
| `$O` | Opposite pane path | Current directory of opposite pane |
| `$L` | Left pane path | Current directory of left pane |
| `$R` | Right pane path | Current directory of right pane |
| `$F` | Cursor file name | Name of file under cursor |
| `$W` | File name without extension | Name without extension |
| `$E` | File extension | Extension of file under cursor |
| `$M` | Marked files list | Space-separated list of marked files |
| `$*` | All files in pane | Space-separated list of all files |
| `$I` | User input prompt | Prompts user for input |
| `$V` | Selected text | Currently selected text (if any) |
| `$~` | Home directory | User's home directory |
| `$#` | File count | Number of files in active pane |

#### PipeToAction Directives

- **JumpToPath**: Navigate to the path returned by the command
- **ExecuteFile**: Execute the file path returned by the command
- **ExecuteFileWithEditor**: Open the file path in configured editor

### Opening Files (`Enter`, `Ctrl+Enter`)

Pressing **Enter** on a file checks, in order:

1. **`extension_associations.json`** — an arbitrary external command for this extension, if you've configured one.
2. **`file_type_map.json`** — RWF's built-in map of common extensions (images, video, audio, documents) to "open via OS default application." Ships with sensible cross-platform defaults; nothing to configure for common file types.
3. Otherwise, RWF's own internal text/hex viewer (the original, always-available fallback).

**Ctrl+Enter** always opens the cursor file via the OS's default association, regardless of the above — a direct escape hatch for anything the map doesn't cover (or files you specifically want to hand off to the OS rather than preview). It never runs on directories/archives (behaves like Enter there instead). By default, no extension in `file_type_map.json` maps to an executable, so plain Enter won't auto-run one out of the box — but if you manually add an extension like `exe` with `"OsDefault"`, plain Enter will launch it exactly like Ctrl+Enter would. There is currently no extension-vs-content safety check (a magic-byte mismatch warning is tracked as a future improvement in ROADMAP Phase 8.7); avoid mapping executable extensions to `OsDefault` unless you intend Enter to run them.

#### `extension_associations.json`

For per-extension (and, since Phase 7.3b, per-detected-content-type) custom commands — for example, opening `.log` files in a specific pager instead of RWF's viewer:

```json
[
  { "Extension": "log", "Command": "less $F", "Shell": "bash" },
  { "FileType": "image", "Command": "feh $F" }
]
```

Fields: `Extension` (no leading dot, case-insensitive; optional since Phase 7.3b), `FileType` (optional — a detected-content-type key or group alias, see below), `Command` (supports the same macros as custom functions — `$P`/`$F`/`$W`/`$E`/etc., see the Macro Reference above), optional `Description`, optional `Shell`. At least one of `Extension`/`FileType` must be set — an entry with neither is skipped (with a warning) at load time. Location: `%APPDATA%\rwf\extension_associations.json`. Ships empty — there's no universally-correct default command to pre-fill, so this file exists purely for your own overrides.

**`FileType` (Phase 7.3b, requires magic-byte detection to be enabled — see `magic_byte_detection_enabled` in `config.json`):** matches the file's *detected* content, not its name. When both `FileType` and `Extension` are set on the same entry, both must match (AND) for that entry to be a candidate. Resolution order when detection is on and the target is a local file: entries whose `FileType` matches the detected content are tried first; if none match (or the content is unrecognized), RWF falls back to plain `Extension`-only entries. With detection off, or for non-local files (e.g. inside an archive), only `Extension`-only entries are ever considered.

Recognized `FileType` values (case-insensitive):
- Exact kinds: `png`, `jpeg`, `gif`, `bmp`, `webp`, `zip`, `gzip`, `7z`, `pdf`, `pe`, `elf`, `macho`
- Group aliases: `image` (png/jpeg/gif/bmp/webp), `archive` (zip/gzip/7z), `executable` (pe/elf/macho)

#### `file_type_map.json`

RWF's built-in extension classification for Enter's auto-routing. Location: `%APPDATA%\rwf\file_type_map.json` — if absent or invalid, RWF falls back to its embedded defaults (covering common image/video/audio/document extensions) rather than an empty list, so this feature works out of the box with zero configuration.

```json
[
  { "Extension": "mp4", "FileType": "video/mp4", "Actions": ["OsDefault"] }
]
```

Fields: `Extension`, optional `FileType` (a MIME-ish string, currently informational only), and `Actions` — an ordered list; today only `"OsDefault"` (open via OS association) does anything, but the list format is forward-compatible with future action kinds. To add your own extension to the OS-default list, or remove one from the built-in set, edit your copy of this file (a full replacement of the file's contents — not merged with the built-in defaults).

### Registered Folders (`registered_directory.json`)

```json
[
  {
    "name": "Projects",
    "path": "$HOME/projects",
    "description": "Development projects"
  },
  {
    "name": "Downloads",
    "path": "$HOME/Downloads",
    "description": "Download folder"
  },
  {
    "name": "Documents",
    "path": "%USERPROFILE%\\Documents",
    "description": "Documents folder (Windows)"
  }
]
```

Environment variables are expanded using:
- **Unix**: `$VAR` or `${VAR}`
- **Windows**: `%VAR%` or `$env:VAR`

### Color Configuration

Available colors:
- Basic: `Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`
- Bright: `BrightBlack`, `BrightRed`, `BrightGreen`, `BrightYellow`, `BrightBlue`, `BrightMagenta`, `BrightCyan`, `BrightWhite`
- Gray shades: `Gray`, `DarkGray`
- RGB: `"#RRGGBB"` (e.g., `"#FF5733"`)

### Log Levels

- **None**: No logging
- **Trace**: Very detailed debugging information
- **Debug**: Debugging information
- **Info**: General informational messages (default)
- **Warning**: Warning messages
- **Error**: Error messages only
- **Critical**: Critical errors only

## Custom Functions

### Creating Custom Functions

1. Edit `custom_functions.json` in your config directory
2. Add a new function object with required fields
3. Optionally bind to a key in `keybindings.json`
4. Reload configuration with `Shift+Z`

### Example: Git Operations

```json
{
  "name": "Git Commit",
  "key": null,
  "command": "git add . && git commit -m \"$I\"",
  "shell": "bash",
  "description": "Stage all and commit with message"
}
```

When executed, this will prompt for commit message (`$I` macro).

### Example: File Processing

```json
{
  "name": "Convert to PDF",
  "key": null,
  "command": "libreoffice --headless --convert-to pdf $F --outdir $O",
  "shell": "bash",
  "description": "Convert document to PDF in opposite pane"
}
```

### Example: Search and Navigate

```json
{
  "name": "Find File by Name",
  "key": "Ctrl+Shift+F",
  "command": "find $P -name \"$I\" -type f | head -1",
  "shell": "bash",
  "pipe_to_action": "JumpToPath",
  "description": "Find file and navigate to it"
}
```

## Usage Examples

### Example 1: Copying Files Between Directories

1. Navigate left pane to source directory
2. Navigate right pane to destination directory
3. Mark files in left pane with `Space` or `*` (mark all)
4. Press `C` to copy marked files to right pane
5. Confirm the operation in the dialog
6. Monitor progress in the task panel

### Example 2: Batch Renaming Files

1. Mark files you want to rename
2. Press `Shift+R` for pattern rename
3. Enter pattern, e.g., `photo_*.jpg` → `vacation_*.jpg`
4. Preview the changes
5. Confirm to execute

### Example 3: Working with Archives

1. Navigate to a .zip file
2. Press `Enter` to browse archive contents
3. Navigate through the virtual folder structure
4. Press `Backspace` to exit archive view
5. Or press `Shift+Enter` to extract to opposite pane

### Example 4: Using Multiple Tabs

1. Press `Ctrl+N` to create a new tab
2. Navigate to different directories in each tab
3. Use `Ctrl+Right`/`Ctrl+Left` to switch between tabs
4. Each tab maintains independent pane states
5. Tabs with active operations show a busy indicator (~)

### Example 5: Quick Navigation with Registered Folders

1. Navigate to a frequently-used directory
2. Press `Shift+B` to register it
3. Edit `registered_directory.json` to add a name
4. Press `I` or `G` to open registered folder selector
5. Type to filter, select, and navigate instantly

### Example 6: Searching for Files

1. Press `/` to enter search mode
2. Type search pattern (supports wildcards: `*.txt`, `photo??.jpg`)
3. For regex: `/pattern/` or `/pattern/i` (case-insensitive)
4. Press `Enter` to jump to first match
5. Use `F3`/`Shift+F3` to navigate results

### Example 7: Monitoring Background Jobs

1. Start a long-running operation (e.g., copying large files)
2. The task panel shows progress automatically
3. Press `Ctrl+J` to open detailed job manager
4. View queued, active, and completed jobs
5. Cancel jobs if needed
6. Jobs execute in FIFO order

### Example 8: Synchronizing Panes

1. Navigate left pane to desired directory
2. Press `O` to sync right pane to same location
3. Or press `Shift+O` to swap left and right pane paths
4. Useful for comparing directory contents

## Advanced Features

### Session Persistence

The application automatically saves:
- All open tabs and their pane states
- Marked files across all tabs
- Current cursor positions
- Display modes and sort settings

On next launch, your session is restored automatically.

### Directory Size Calculation

Press `H` on a directory to calculate its total size recursively. The operation runs as a background job and updates the display when complete. Multiple directories can be calculated simultaneously.

### File Comparison

Press `W` to compare the file under cursor with the file at the same position in the opposite pane. The comparison view shows differences side-by-side.

### Pattern-Based Filtering

Press `F` to apply a file mask:
- Wildcards: `*.txt`, `photo*.jpg`, `file?.dat`
- Multiple patterns: `*.txt:*.md` (include .txt and .md files)
- Exclude patterns: `*:*.tmp` (all files except .tmp)

### Text Encoding Support

The text viewer supports multiple encodings:
- UTF-8, UTF-16 (LE/BE)
- ASCII, ISO-8859-1
- Shift-JIS, EUC-JP (Japanese)
- GB2312, GBK (Chinese)
- And more...

Press `Shift+E` in viewer to cycle through encodings.

### Shell Integration (Exit and CD)

To change your shell's directory when exiting:

**Bash/Zsh** - Add to `~/.bashrc` or `~/.zshrc`:
```bash
function fm() {
    local output=$(rwf -cwd)
    if [ -d "$output" ]; then
        cd "$output"
    fi
}
```

**PowerShell** - Add to profile:
```powershell
function fm {
    $output = rwf -cwd
    if (Test-Path $output -PathType Container) {
        Set-Location $output
    }
}
```

Then use `fm` command and exit with `Shift+Q` to change directory.

### Multi-Language Help

Press `L` in the help dialog to cycle through available languages. The application supports:
- English (en)
- Japanese (jp)
- Additional languages via `help.{lang}.json` files

### Logging and Debugging

- Logs are written to `logs/session.log`
- Configure log level in `config.json`
- Press configured key to save current session log
- Logs automatically rotate at 10MB
- Slow operations (>5s) are logged automatically

### Configuration Reload

Press `Shift+Z` to reload configuration without restarting. This reloads:
- Main configuration (`config.json`)
- Key bindings (`keybindings.json`)
- File-type extension associations (`extension_associations.json`)
- Built-in file-type map (`file_type_map.json`)
- Custom functions (`custom_functions.json`)
- Registered folders (`registered_directory.json`)
- Color schemes

Press `Y` to launch your configured editor with the main config file for quick editing.

## Troubleshooting

### UI Not Responsive

The UI should never block. If it does:
1. Check log file for errors
2. Verify worker pool size in config (default: 4)
3. Cancel long-running jobs with `Ctrl+J` and select job to cancel

### Files Not Showing

1. Check if hidden files are enabled in config
2. Verify file mask filter is not active (shown in status bar)
3. Press `Ctrl+K` to clear any active filters

### Configuration Not Loading

1. Check config file syntax with a JSON validator
2. Review log file for parsing errors
3. Delete config file to regenerate defaults
4. Verify file permissions

### Custom Functions Not Working

1. Verify shell is correctly specified
2. Check macro syntax (case-sensitive)
3. Test command in terminal first
4. Review logs for execution errors

### Archive Browsing Issues

1. Verify archive is not corrupted
2. Check if format is supported (.zip currently)
3. Ensure sufficient permissions
4. Review logs for detailed error messages

### Recording a Problem for Diagnosis

When something misbehaves in a way that is hard to describe — a brief freeze, a pane that
shows the wrong thing, a key that seems to do nothing — record a **diagnostic session**
instead of trying to write it up from memory.

1. Press `F12`. `● DIAG mm:ss` appears in the top-right corner and the task panel shows where
   the bundle is being written.
2. Reproduce the problem.
3. Press `F11` at any moment worth a screenshot. As many times as you like.
4. Press `F12` again. Describe what happened when prompted.

The result is one folder containing the keys you pressed, the state changes they caused, the
background jobs involved, the logs, and pictures of the screen — enough for someone (or an AI)
to reconstruct the sequence afterwards.

Both keys also work inside the viewer, in leap mode, and while a dialog is open.

If the problem happens during startup, before you can press a key:

```bash
RWF_DIAGNOSTICS=1 rwf
```

**Before sharing a bundle, look at it.** It contains the file paths and screen contents from
your session verbatim, and `config_effective.json` includes your custom function command
lines.

To turn the feature off entirely:

```json
"Diagnostics": { "Enabled": false }
```

Format details and analysis guidance: [DIAGNOSTIC_BUNDLES.md](DIAGNOSTIC_BUNDLES.md).

## Performance Tips

1. **Adjust worker pool size**: Increase for faster parallel operations on systems with many cores
2. **Disable session persistence**: If you don't need tab restoration
3. **Reduce refresh rate**: Lower `refresh_rate` in config for slower systems
4. **Use file masks**: Filter large directories to improve rendering
5. **Disable directory caching**: If working with rapidly changing directories

## Keyboard Shortcuts Summary

Quick reference card:

```
Navigation:        Tab, ↑↓←→, Home/End, PgUp/PgDn
File Ops:          C(opy), M(ove), D(elete), R(ename)
Marking:           Space, *, Ctrl+U, @
Search:            /, F, Ctrl+K
Tabs:              Ctrl+N, Ctrl+W, Ctrl+←→
View:              V(iew), F8(hex), W(compare)
Panes:             O(sync), Shift+O(swap)
App:               Q(uit), ?(help), Shift+Z(reload)
Jobs:              Ctrl+J (job manager)
Diagnostics:       F12 (record session), F11 (snapshot)
```

---

For developer documentation, see [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md).
For API documentation, see [API_REFERENCE.md](API_REFERENCE.md).
For diagnostic bundle format, see [DIAGNOSTIC_BUNDLES.md](DIAGNOSTIC_BUNDLES.md).
