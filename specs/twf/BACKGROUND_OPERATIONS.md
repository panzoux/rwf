# Background Operations System

TWF supports non-blocking background file operations. This allows you to perform long-running tasks like copying thousands of files while continuing to browse and manage files in other tabs or panes.

## Key Features

- **Non-blocking Execution**: Copy, Move, Delete, and Directory Size operations run in background threads.
- **Task Panel**: A scrollable log area at the bottom of the screen.
- **Job Manager**: A dedicated dialog (`Ctrl+J`) to monitor, manage, and cancel active jobs.
- **Indeterminate Tasks**: Operations with unknown totals (like directory scanning) display real-time counts and sizes without a misleading 0% progress bar.
- **Safe Collision Handling**: File collisions (Overwrite/Skip) pause only the specific background job, requesting user input without freezing the rest of the application.
- **Accurate Recursive Progress**: Tracks the total number of files and bytes recursively for directory operations, ensuring "1/N" progress counters are accurate even for deep folder trees.
- **Detailed Statistics**: Progress reporting separates global job stats from individual file progress (`Count: 5/100 - Overall: 20% - File: 50%`).
- **Performance Optimized**: Progress updates are throttled (respecting `TaskPanelUpdateIntervalMs` default 300ms) to ensure low CPU usage.

## User Interface

### 1. Task Panel (Log View)
Located at the bottom of the main window. 
- **Collapsed Mode**: Displays only the single most recent log entry.
- **Expanded Mode**: Shows a scrollable history of all operation logs.
- **Directory Info**: Calculating folder size (via **H** key) reports final results here: `[Size] MyFolder: 1.2 GB (10,500 files, 450 folders)`.
- **Log Persistence**:
    - **Memory Buffer**: Keeps the most recent 1000 lines in memory for display.
    - **Disk Logging**: Automatically flushes older logs to `%APPDATA%\TWF\twf_tasks.log` to prevent memory bloat.
    - **On Exit**: Forces a flush of all remaining logs to the file on application exit.
    - **Manual Save**: `SaveTaskLog` command allows manual saving of the current memory buffer.

### 2. Job Manager Dialog
### 2. Job Manager Dialog
Accessed via `Ctrl+J`, this dialog provides a detailed list of all background jobs and allows for manual cancellation.
- **Dimensions**: 64x24 fixed size.
- **Smart Truncation**: Long filenames are intelligently truncated in the middle (`very_long...name.txt`) to preserve extensions.
- **Detail View**: Selecting a job displays full details in a word-wrapped pane at the bottom, separating the overall progress statistics from the current filename for better readability.

### 3. Tab Busy Indicators
Active jobs are indicated directly in the tab bar using animated spinners (slashes). The number of slashes represents the number of active jobs in that tab.
- **Example**: `[1:Docs *|Backup ]//` indicates Tab 1 has 2 background operations running.

## Key Bindings

| Key | Action | Description |
| :--- | :--- | :--- |
| **Ctrl+J** | `ShowJobManager` | Open the Job Manager dialog. |
| **Ctrl+L** | `ToggleTaskPanel` | Toggle between collapsed and expanded log panel. |
| **Ctrl+Up** | `ResizeTaskPanelUp` | Increase the height of the expanded log panel. |
| **Ctrl+Down** | `ResizeTaskPanelDown` | Decrease the height of the expanded log panel. |
| **Alt+Up** | `ScrollTaskPanelUp` | Scroll up in the log history. |
| **Alt+Down** | `ScrollTaskPanelDown` | Scroll down in the log history. |

## Configuration Settings

These settings in `config.json` control the system behavior (require restart to take effect):

- `MaxSimultaneousJobs`: Maximum number of simultaneous background tasks (default: 4).
- `TaskPanelUpdateIntervalMs`: Refresh rate for spinners and progress (default: 300ms, min: 100ms).
- `TaskPanelHeight`: Default height for the expanded log panel.