# TWF Manual

## Table of Contents
1. [Basic Navigation](#basic-navigation)
2. [File Operations](#file-operations)
3. [Search and Filter](#search-and-filter)
4. [Custom Functions](#custom-functions)
5. [Advanced Features](#advanced-features)

## Basic Navigation

### Key Navigation
- **Arrow Keys** - Move cursor up/down, switch panes left/right
- **Tab** - Switch between left and right panes
- **Enter** - Navigate into directory, open file, or execute
- **Backspace** - Navigate to parent directory
- **Home** - Invert marks (or navigate to root with Ctrl)
- **End** - Clear all marks
- **Ctrl+PageUp** - Move cursor to first entry
- **Ctrl+PageDown** - Move cursor to last entry

### Pane Operations
- **Shift+O** - Swap paths of left and right panes
- **O** - Sync opposite pane to active pane's directory

### Tab Management
- **Ctrl+T** - Open a new tab (clones the current paths)
- **Ctrl+W** - Close the current tab
- **Ctrl+Right** - Switch to the next tab
- **Ctrl+Left** - Switch to the previous tab
- **Ctrl+B** - Open Tab Selector Dialog (filter, jump to, or close tabs)

**Tab Bar Behavior**:
- **Brackets**: The active tab is always wrapped in square brackets (e.g., `[1:Windows *|Backup ]`).
- **Scrolling**: If you have more tabs than can fit on your screen, the tab bar will automatically scroll to keep the active tab visible. Indicators (`<` and `>`) show if there are more tabs off-screen.
- **Custom Colors**: The active and inactive tabs can be assigned distinct colors in the configuration.

**Tab Selector Dialog**:
- Provides a list of all open tabs with full path previews for the left and right panes.
- Supports incremental search (including Migemo).
- Press **Enter** to jump to the selected tab.
- Press **Delete** to close the highlighted tab directly from the list.
- Press **Ctrl+K** to clear the search filter.

### Background Operations
- **Ctrl+J** - Open Job Manager (monitor and cancel background tasks).
    - **Detail View**: Select a job in the list to see detailed progress statistics (Count: X/Y, Overall %, File %) and the full current filename.
- **Ctrl+L** - Toggle Task Pane (expand/collapse log area).
- **Ctrl+Up/Down** - Resize expanded Task Pane.
- **Alt+Up/Down** - Scroll Task Pane log history.

Operations like Copy, Move, and Delete run in the background. Tabs with active jobs show a `~` indicator.
Logs are automatically buffered and saved to `%APPDATA%\TWF\twf_tasks.log`. You can also manually save the current view with the command bound to `SaveTaskLog` (if configured).

## File Operations

### File Marking
- **Space** - Toggle mark and move cursor down
- **Shift+Space** - Toggle mark and move cursor up
- **Ctrl+Space** - Mark range from previous mark to cursor
- **End** - Clear all marks

### File Operations
- **C** - Copy marked files
- **M** - Move marked files
- **D** - Delete marked files
- **K** - Create directory

### Display Modes
- **1-8** - Switch display modes (1-8 columns)

## Search and Filter

### File Filtering
- **:** (colon) - File mask filter dialog
- **@** - Wildcard marking dialog

#### Traditional Wildcards (Case-Insensitive)
- `*.txt` - Matches all .txt files
- `?.log` - Matches single-character name with .log extension
- `test*` - Matches files starting with "test"

#### Regular Expressions
- `/pattern/` - Case-sensitive regex pattern (e.g., `/test/` matches only "test", not "Test")
- `/pattern/i` - Case-insensitive regex pattern (e.g., `/test/i` matches both "test" and "Test")
- `/(jpg|jpeg|png|gif)$/i` - Matches image files using alternation

#### Combined Patterns
- `*.json :*.txt` - Include JSON files, exclude TXT files
- `/.*\.json$/ :/.*\.txt$/` - Same functionality using regex patterns

The file mask is displayed in the paths label, showing the current filter applied to each pane.

### Search
- **F** - Enter incremental search mode
- **Ctrl+K** - Clear search or filter input (works in File Pane, Text Viewer, and List Dialogs)
- **S** - Cycle sort mode (Name, Ext, Size, Date, Unsorted)
- **Shift+S** - Open Sort Selection Dialog

## Custom Functions

### Macro Reference
- `$P` - Active pane path
- `$O` - Other pane path
- `$L` - Left pane path
- `$R` - Right pane path
- `$F` - Current filename
- `$W` - Filename without extension
- `$E` - File extension
- `$M` - Marked files (spaces separated, quoted)
- `$*` - Active pane file mask
- `$I"Prompt"` - Input dialog
- `$~` - Home directory
- `$V"VAR"` - Environment variable value
- `$V"twf"` - TWF application directory
- `$#XX` - ASCII character (hex code)

### Key Bindings
- Custom functions can be bound to keys in `keybindings.json`
- **Shift+F** - Show custom functions dialog

## Archive Operations
- **P** - Compress marked files
- **Shift+Enter** (on archive) - Extract archive

## File Viewing
- **V** - View file as text
- **F8** - View file as hex (binary view)
- **H** - Show file info (for directories, triggers background size calculation reported to Task Pane)

## Advanced Features

### Registered Folders
- **I/G** - Show registered folders dialog (with filter support)
- **Shift+M** - Move to registered folder
- **Shift+B** - Register current directory

### High-Performance Navigation
- **Directory Caching**: Frequently visited folders are cached for instant access.
- **Cursor Memory**: TWF remembers your last cursor position and scroll offset for every visited directory.
- **Async Loading**: Directory listing and network share verification are non-blocking. A spinner in the status bar indicates background loading.

### Migemo Search
- Japanese incremental search using romaji input (e.g., 'nihon' matches '日本')
- Enabled/disabled in configuration

### Status Bar Information
The status bar shows:
- Directory and file counts
- Drive usage information
- Marked file statistics

### Top Separator Information
The top separator shows:
- Volume names or network share names
- Marked file statistics

### Configuration Reload
- **Shift+Z** - Reload configuration without restart
- Updates log level, colors, and other settings dynamically

### Version Information
- **Ctrl+Shift+V** - Show version and environment information in the task log area.

### Sort Options
- **S** - Cycle through sort modes
- Name, extension, size, date, unsorted
- Case-insensitive name sorting

### Custom Functions with Shell Support
Custom functions can specify which shell to use:
- **Shell configuration** in `config.json` for default shells per OS
- **Per-function shell** specification in `custom_functions.json`
- Cross-platform shell support for Windows, Linux, and macOS