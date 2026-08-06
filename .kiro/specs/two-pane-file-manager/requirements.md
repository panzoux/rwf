# Requirements Document: Two-Pane File Manager

## Introduction

This document specifies the requirements for cross-platform, two-pane file manager application built in Rust. The file manager provides a terminal-based user interface with dual panes for efficient file navigation and operations. The application leverages rwf for asynchronous file operations and follows the AppState architectural pattern for state management.

The file manager enables users to browse local filesystems, perform file operations (copy, move, delete, rename), search for files, mark multiple files for batch operations, navigate through directory hierarchies with keyboard-driven controls, manage multiple tabs, execute custom functions, browse archives, and access advanced features like pane synchronization, context menus, multi-language help, and shell integration.

The architecture follows these core principles:
- All file I/O operations execute as Jobs in rwf (never on UI thread)
- Jobs follow strict FIFO ordering through the queue
- State changes occur through explicit Transition enum values
- AppState coordinates all components (FilesystemModel, JobManager, SearchModel, etc.)
- Key bindings are configurable via keybindings.json (TWF-compatible defaults)

## Glossary

- **Application**: The two-pane file manager system
- **Pane**: A vertical section of the UI displaying directory contents (left or right)
- **Active_Pane**: The pane currently receiving user input
- **File_Entry**: A representation of a file or directory with metadata
- **Location**: An abstract path that can represent local filesystem paths or virtual archive paths
- **Cursor**: The currently selected file entry within a pane
- **Marked_File**: A file selected for batch operations
- **Job**: An asynchronous file operation managed by rwf, following the lifecycle: Queued → Running → Completed/Failed/Cancelled
- **JobManager**: The component managing job queue, active jobs, and completed jobs
- **UI_Thread**: The main thread handling user input and rendering, which NEVER blocks on file I/O operations
- **Worker_Pool**: The rwf fixed-size thread pool executing file operations via FIFO queue
- **FIFO_Queue**: The first-in-first-out queue feeding jobs to the Worker_Pool
- **AppState**: The central application state coordinating all components via explicit Transition enum
- **Transition**: An explicit state change operation that transforms AppState
- **Dialog**: A modal UI element for user confirmation or input
- **Keybindings**: Configurable key mappings loaded from keybindings.json configuration file
- **Navigation_History**: Stack of previously visited locations per pane
- **Sort_Mode**: The ordering criterion for file entries (name, size, date, extension)
- **Display_Mode**: The visual presentation format for file entries (1-8 columns or detailed mode)
- **Search_Query**: A text pattern for finding files
- **File_Mask**: A filter pattern for displaying specific file types
- **Status_Bar**: The UI element displaying application state and statistics
- **Task_Panel**: The UI element showing active and queued jobs
- **Tab**: An independent workspace containing left and right pane states
- **TabManager**: The component managing multiple tabs and their states
- **Active_Tab**: The tab currently receiving user input and displaying in the UI
- **Tab_Bar**: The UI element displaying all open tabs with indicators
- **Custom_Function**: A user-defined command with macro expansion loaded from custom_functions.json
- **Macro**: A placeholder token in custom functions that expands to runtime values ($P, $O, $F, etc.)
- **PipeToAction**: A directive in custom functions that processes command output (JumpToPath, ExecuteFile, ExecuteFileWithEditor)
- **Virtual_Folder**: A browsable view of archive contents presented as a directory structure
- **Archive**: A compressed file container (.zip) that can be browsed and extracted
- **Registered_Folder**: A bookmarked directory path stored in registered_directory.json
- **Pattern**: A text expression using wildcards (* and ?) or regular expressions for matching files
- **Viewer**: A modal UI component for displaying file contents (text or hex mode)
- **Encoding**: The character encoding used to interpret text files (UTF-8, UTF-16, Shift-JIS, etc.)
- **Comparison_View**: A UI component displaying differences between two files
- **Range_Marking**: A marking mode that selects all files between two cursor positions
- **Directory_Size_Job**: A background job that recursively calculates total size of a directory
- **Session_State**: Persistent storage of application state including tabs, pane locations, and marked files
- **Log_Level**: The verbosity of application logging (None, Trace, Debug, Information, Warning, Error, Critical)
- **Job_Manager_Dialog**: A modal UI displaying detailed information about all jobs
- **Scroll_Offset**: The number of lines from the top or bottom edge that triggers automatic scrolling (configurable, default: 3)
- **Volume_Name**: The label or identifier for a storage device, drive, or network share
- **Top_Separator**: The UI element displaying volume names and marked file statistics above the file panes
- **Pane_Info_Bar**: The UI element displaying pane-specific information (UI area 5)
- **Filename_Label**: The UI element displaying the currently selected filename (UI area 6)

## Requirements

### Requirement 1: Dual Pane Display

**User Story:** As a user, I want to see two independent directory listings side by side, so that I can easily compare and transfer files between locations.

#### Acceptance Criteria

1. THE Application SHALL display two vertical panes simultaneously
2. THE Application SHALL render file entries in each pane independently
3. WHEN the Application starts, THE Application SHALL initialize both panes with the current working directory
4. THE Application SHALL visually indicate which pane is the Active_Pane
5. THE Application SHALL maintain independent cursor positions for each pane
6. THE Application SHALL maintain independent scroll positions for each pane

### Requirement 2: Pane Navigation

**User Story:** As a user, I want to navigate between panes and within directory listings, so that I can select files and directories for operations.

#### Acceptance Criteria

1. WHEN the user presses the Tab key, THE Application SHALL switch the Active_Pane to the opposite pane
2. WHEN the user presses the Up arrow or 'k' key, THE Application SHALL move the Cursor up by one entry in the Active_Pane
3. WHEN the user presses the Down arrow or 'j' key, THE Application SHALL move the Cursor down by one entry in the Active_Pane
4. WHEN the user presses the Home key, THE Application SHALL move the Cursor to the first entry in the Active_Pane
5. WHEN the user presses the End key, THE Application SHALL move the Cursor to the last entry in the Active_Pane
6. WHEN the user presses Page Up, THE Application SHALL move the Cursor up by one page in the Active_Pane
7. WHEN the user presses Page Down, THE Application SHALL move the Cursor down by one page in the Active_Pane
8. WHEN the Cursor moves beyond visible bounds, THE Application SHALL automatically scroll the Active_Pane to keep the Cursor visible

### Requirement 2A: File Pane Scrolling Behavior

**User Story:** As a user, I want smooth and predictable scrolling behavior in file panes, so that I can navigate efficiently without blank space at the bottom.

#### Acceptance Criteria

1. THE Application SHALL NOT display blank lines at the bottom of the file pane
2. WHEN the Cursor reaches 3 lines from the top of the visible area, THE Application SHALL scroll the pane upward by one line
3. WHEN the Cursor reaches 3 lines from the bottom of the visible area, THE Application SHALL scroll the pane downward by one line
4. THE Application SHALL honor the scroll_offset configuration value from config.json (default: 3)
5. WHEN scroll_offset is configured to N, THE Application SHALL trigger scrolling when the Cursor is N lines from the top or bottom
6. WHEN scrolling to the end of the file list, THE Application SHALL position the last entry at the bottom of the visible area with no blank lines below
7. WHEN scrolling to the beginning of the file list, THE Application SHALL position the first entry at the top of the visible area

### Requirement 3: Directory Navigation

**User Story:** As a user, I want to enter and exit directories, so that I can browse the filesystem hierarchy.

#### Acceptance Criteria

1. WHEN the user presses Enter on a directory entry, THE Application SHALL change the Active_Pane location to that directory
2. WHEN the user presses Backspace or the Left arrow on a non-root directory, THE Application SHALL navigate to the parent directory in the Active_Pane
3. WHEN the Active_Pane location changes, THE Application SHALL submit a directory read Job to the Worker_Pool
4. THE Worker_Pool SHALL execute the directory read operation on a worker thread (not the UI_Thread)
5. WHEN the directory read Job completes, THE Application SHALL update the pane with the loaded entries
6. WHEN the Active_Pane location changes, THE Application SHALL reset the Cursor to the first entry
7. WHEN the Active_Pane location changes, THE Application SHALL add the previous location to the Navigation_History
8. WHEN directory loading fails, THE Application SHALL display an error message and remain at the current location
9. THE UI_Thread SHALL remain responsive during directory loading

### Requirement 4: File Entry Display

**User Story:** As a user, I want to see file information clearly, so that I can identify files and make informed decisions.

#### Acceptance Criteria

1. THE Application SHALL display the file name for each File_Entry
2. THE Application SHALL display the file size for each File_Entry
3. THE Application SHALL display the modification date for each File_Entry
4. THE Application SHALL visually distinguish directories from regular files
5. THE Application SHALL visually indicate marked files
6. THE Application SHALL visually indicate the cursor position
7. WHERE the user has configured hidden file display, THE Application SHALL show hidden files
8. WHERE the user has not configured hidden file display, THE Application SHALL hide hidden files by default

### Requirement 5: File Marking

**User Story:** As a user, I want to mark multiple files, so that I can perform batch operations on them.

#### Acceptance Criteria

1. WHEN the user presses Space on a File_Entry, THE Application SHALL toggle the marked state of that entry
2. WHEN the user presses '*', THE Application SHALL mark all entries in the Active_Pane
3. WHEN the user presses Ctrl+U, THE Application SHALL unmark all entries in the Active_Pane
4. THE Application SHALL maintain marked file state when navigating between directories
5. THE Application SHALL display the count of Marked_Files in the Status_Bar
6. THE Application SHALL display the total size of Marked_Files in the Status_Bar
7. WHEN a File_Entry is marked, THE Application SHALL visually highlight that entry

### Requirement 6: File Copy Operation

**User Story:** As a user, I want to copy files from one location to another, so that I can duplicate files while preserving the originals.

#### Acceptance Criteria

1. WHEN the user presses the configured copy key (default 'C' per keybindings.json), THE Application SHALL initiate a copy operation
2. IF Marked_Files exist, THEN THE Application SHALL use Marked_Files as copy sources
3. IF no Marked_Files exist, THEN THE Application SHALL use the current Cursor entry as the copy source
4. THE Application SHALL use the opposite pane's location as the default destination
5. THE Application SHALL display a Dialog requesting copy confirmation
6. WHEN the user confirms the copy Dialog, THE Application SHALL create a Job for the copy operation
7. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
8. THE Worker_Pool SHALL execute the copy Job on a worker thread (not the UI_Thread)
9. THE Application SHALL display copy progress in the Task_Panel via JobEvent updates
10. WHEN a copy Job completes successfully, THE Application SHALL refresh the destination pane
11. IF a file already exists at the destination, THEN THE Application SHALL prompt the user for overwrite confirmation
12. THE UI_Thread SHALL remain responsive during the copy operation

### Requirement 7: File Move Operation

**User Story:** As a user, I want to move files from one location to another, so that I can reorganize my filesystem.

#### Acceptance Criteria

1. WHEN the user presses the configured move key (default 'M' per keybindings.json), THE Application SHALL initiate a move operation
2. IF Marked_Files exist, THEN THE Application SHALL use Marked_Files as move sources
3. IF no Marked_Files exist, THEN THE Application SHALL use the current Cursor entry as the move source
4. THE Application SHALL use the opposite pane's location as the default destination
5. THE Application SHALL display a Dialog requesting move confirmation
6. WHEN the user confirms the move Dialog, THE Application SHALL create a Job for the move operation
7. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
8. THE Worker_Pool SHALL execute the move Job on a worker thread (not the UI_Thread)
9. THE Application SHALL display move progress in the Task_Panel via JobEvent updates
10. WHEN a move Job completes successfully, THE Application SHALL refresh both panes
11. IF a file already exists at the destination, THEN THE Application SHALL prompt the user for overwrite confirmation
12. THE UI_Thread SHALL remain responsive during the move operation

### Requirement 8: File Delete Operation

**User Story:** As a user, I want to delete files, so that I can remove unwanted files from my filesystem.

#### Acceptance Criteria

1. WHEN the user presses the configured delete key (default 'D' per keybindings.json), THE Application SHALL initiate a delete operation
2. IF Marked_Files exist, THEN THE Application SHALL use Marked_Files as delete targets
3. IF no Marked_Files exist, THEN THE Application SHALL use the current Cursor entry as the delete target
4. THE Application SHALL display a Dialog requesting delete confirmation with the count of files to be deleted
5. WHEN the user confirms the delete Dialog, THE Application SHALL create a Job for the delete operation
6. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
7. THE Worker_Pool SHALL execute the delete Job on a worker thread (not the UI_Thread)
8. THE Application SHALL display delete progress in the Task_Panel via JobEvent updates
9. WHEN a delete Job completes successfully, THE Application SHALL refresh the Active_Pane
10. WHEN a delete Job completes successfully, THE Application SHALL unmark all deleted files
11. THE UI_Thread SHALL remain responsive during the delete operation

### Requirement 9: File Rename Operation

**User Story:** As a user, I want to rename files, so that I can give them more meaningful names.

#### Acceptance Criteria

1. WHEN the user presses the configured rename key (default 'R' per keybindings.json), THE Application SHALL initiate a rename operation on the current Cursor entry
2. THE Application SHALL display a Dialog with an input field pre-filled with the current file name
3. WHEN the user submits a new name, THE Application SHALL create a Job for the rename operation
4. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
5. THE Worker_Pool SHALL execute the rename Job on a worker thread (not the UI_Thread)
6. WHEN a rename Job completes successfully, THE Application SHALL refresh the Active_Pane
7. IF a file with the new name already exists, THEN THE Application SHALL display an error and cancel the operation
8. IF the new name contains invalid characters, THEN THE Application SHALL display an error and cancel the operation
9. THE UI_Thread SHALL remain responsive during the rename operation

### Requirement 10: Directory Creation

**User Story:** As a user, I want to create new directories, so that I can organize my files.

#### Acceptance Criteria

1. WHEN the user presses the configured create directory key (default 'Shift+K' per keybindings.json), THE Application SHALL initiate a directory creation operation
2. THE Application SHALL display a Dialog with an input field for the new directory name
3. WHEN the user submits a directory name, THE Application SHALL create a Job for the mkdir operation
4. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
5. THE Worker_Pool SHALL execute the mkdir Job on a worker thread (not the UI_Thread)
6. WHEN a mkdir Job completes successfully, THE Application SHALL refresh the Active_Pane
7. IF a directory with that name already exists, THEN THE Application SHALL display an error
8. IF the directory name contains invalid characters, THEN THE Application SHALL display an error
9. THE UI_Thread SHALL remain responsive during the mkdir operation

### Requirement 11: File Search

**User Story:** As a user, I want to search for files by name, so that I can quickly locate specific files.

#### Acceptance Criteria

1. WHEN the user presses '/' or Ctrl+F, THE Application SHALL enter search mode
2. THE Application SHALL display a search input Dialog
3. WHEN the user types in the search Dialog, THE Application SHALL filter displayed entries in real-time
4. THE Application SHALL match the Search_Query against file names case-insensitively by default
5. WHERE the user has enabled case-sensitive search, THE Application SHALL match case-sensitively
6. WHERE the user has enabled regex search, THE Application SHALL interpret the Search_Query as a regular expression
7. WHEN the user presses Enter in search mode, THE Application SHALL move the Cursor to the first matching entry
8. WHEN the user presses Escape in search mode, THE Application SHALL exit search mode and restore the full file listing

### Requirement 12: Sorting

**User Story:** As a user, I want to sort files by different criteria, so that I can find files more easily.

#### Acceptance Criteria

1. WHEN the user presses 's' followed by 'n', THE Application SHALL sort the Active_Pane by name
2. WHEN the user presses 's' followed by 's', THE Application SHALL sort the Active_Pane by size
3. WHEN the user presses 's' followed by 'd', THE Application SHALL sort the Active_Pane by modification date
4. WHEN the user presses 's' followed by 'e', THE Application SHALL sort the Active_Pane by file extension
5. THE Application SHALL maintain separate Sort_Mode settings for each pane
6. THE Application SHALL always display directories before files within each sort order
7. WHEN the Sort_Mode changes, THE Application SHALL re-sort entries immediately

### Requirement 13: File Filtering

**User Story:** As a user, I want to filter files by pattern, so that I can focus on specific file types.

#### Acceptance Criteria

1. WHEN the user presses 'f', THE Application SHALL display a filter input Dialog
2. WHEN the user submits a File_Mask pattern, THE Application SHALL display only matching files
3. THE Application SHALL support wildcard patterns (* and ?)
4. THE Application SHALL maintain separate File_Mask settings for each pane
5. WHEN the File_Mask is cleared, THE Application SHALL display all files
6. THE Application SHALL display the active File_Mask in the Status_Bar

### Requirement 14: Navigation History

**User Story:** As a user, I want to navigate backward and forward through my browsing history, so that I can quickly return to previously visited directories.

#### Acceptance Criteria

1. WHEN the user presses Alt+Left, THE Application SHALL navigate to the previous location in the Navigation_History for the Active_Pane
2. WHEN the user presses Alt+Right, THE Application SHALL navigate to the next location in the Navigation_History for the Active_Pane
3. THE Application SHALL maintain separate Navigation_History stacks for each pane
4. WHEN navigating through history, THE Application SHALL not add duplicate entries to the Navigation_History
5. WHEN the user navigates to a new location (not via history), THE Application SHALL clear forward history

### Requirement 15: Job Management

**User Story:** As a user, I want to monitor and control background file operations, so that I can track progress and cancel operations if needed.

#### Acceptance Criteria

1. THE JobManager SHALL maintain three collections: queued jobs in FIFO_Queue, active jobs, and completed jobs
2. THE Application SHALL display all active Jobs in the Task_Panel
3. THE Application SHALL display queued Jobs in the Task_Panel
4. THE Application SHALL display progress percentage for each active Job via JobEvent updates
5. WHEN the user presses the configured cancel key on a Job, THE Application SHALL request cooperative cancellation via cancellation token
6. WHEN a Job receives cancellation request, THE Job SHALL check the cancellation flag periodically and transition to Cancelled state
7. WHEN a Job is cancelled, THE Application SHALL update the Task_Panel to reflect the cancellation
8. WHEN a Job completes, THE Application SHALL remove it from the Task_Panel after 3 seconds
9. WHEN a Job fails, THE Application SHALL display the FailureReason in the Task_Panel
10. THE Application SHALL display the total count of active Jobs in the Status_Bar
11. THE Worker_Pool SHALL execute jobs from the FIFO_Queue in strict first-in-first-out order
12. THE JobManager SHALL enforce maximum parallel job limit based on Worker_Pool size

### Requirement 16: Status Bar

**User Story:** As a user, I want to see application status information, so that I can understand the current state at a glance.

#### Acceptance Criteria

1. THE Application SHALL display the current directory path for the Active_Pane in the Status_Bar
2. THE Application SHALL display the count of files in the Active_Pane in the Status_Bar
3. THE Application SHALL display the count of Marked_Files in the Status_Bar
4. THE Application SHALL display the total size of Marked_Files in the Status_Bar
5. THE Application SHALL display the count of active Jobs in the Status_Bar
6. THE Application SHALL display the active File_Mask in the Status_Bar if one is set
7. THE Application SHALL display the current Sort_Mode in the Status_Bar

### Requirement 17: Configuration Loading

**User Story:** As a user, I want the application to load my preferences from a configuration file, so that my settings persist across sessions.

#### Acceptance Criteria

1. WHEN the Application starts, THE Application SHALL attempt to load configuration from a config file
2. IF the config file exists, THEN THE Application SHALL apply the loaded settings
3. IF the config file does not exist, THEN THE Application SHALL use default settings
4. THE Application SHALL load key bindings from keybindings.json configuration file
5. THE Application SHALL support configurable key mappings for all operations (copy, move, delete, rename, create directory, etc.)
6. THE Application SHALL load display preferences from the config file
7. THE Application SHALL load color scheme from the config file
8. THE Application SHALL load Worker_Pool size configuration (default: 4 workers)
9. IF the config file is malformed, THEN THE Application SHALL display an error and use default settings

### Requirement 18: Keyboard Shortcuts

**User Story:** As a user, I want to use keyboard shortcuts for common operations, so that I can work efficiently without using a mouse.

#### Acceptance Criteria

1. THE Application SHALL support configurable key bindings for all operations
2. THE Application SHALL load key bindings from keybindings.json at startup
3. THE Application SHALL use TWF-compatible default key bindings (C for copy, M for move, D for delete, R for rename, Shift+K for create directory)
4. WHEN the user presses the configured help key (default '?' or 'F1'), THE Application SHALL display a help Dialog showing all key bindings
5. THE Application SHALL support multi-key sequences (e.g., 's' followed by 'n' for sort by name)
6. THE Application SHALL provide visual feedback when waiting for the second key in a sequence
7. THE Application SHALL allow users to customize key bindings by editing keybindings.json

### Requirement 19: Error Handling

**User Story:** As a user, I want to see clear error messages when operations fail, so that I can understand what went wrong and take corrective action.

#### Acceptance Criteria

1. WHEN a file operation fails, THE Application SHALL display an error Dialog with a descriptive message
2. WHEN a directory cannot be read, THE Application SHALL display an error message and remain at the current location
3. WHEN a Job fails, THE Application SHALL display the failure reason in the Task_Panel
4. THE Application SHALL log all errors to a log file for debugging
5. WHEN a permission error occurs, THE Application SHALL indicate that the operation requires elevated privileges

### Requirement 20: Application Lifecycle

**User Story:** As a user, I want to start and exit the application cleanly, so that my work is not lost and resources are properly released.

#### Acceptance Criteria

1. WHEN the Application starts, THE Application SHALL initialize the AppState with default values
2. WHEN the Application starts, THE Application SHALL initialize the UI_Thread and Worker_Pool with configured worker count (default: 4)
3. WHEN the Application starts, THE Application SHALL load configuration including keybindings.json
4. WHEN the Application starts, THE Application SHALL initialize both panes with the current working directory
5. WHEN the user presses the configured quit key (default 'Q' or 'Escape' per keybindings.json), THE Application SHALL initiate shutdown
6. WHEN shutting down, THE Application SHALL request cancellation of all active Jobs via cancellation tokens
7. WHEN shutting down, THE Application SHALL wait for Worker_Pool threads to complete with a timeout of 5 seconds
8. WHEN shutting down, THE Application SHALL release all resources
9. WHEN shutting down, THE Application SHALL restore the terminal to its original state
10. THE Application SHALL manage all state transitions through the Transition enum

### Requirement 21: UI Responsiveness

**User Story:** As a user, I want the interface to remain responsive during file operations, so that I can continue working without interruption.

#### Acceptance Criteria

1. THE UI_Thread SHALL never block on file I/O operations
2. ALL file I/O operations (copy, move, delete, rename, mkdir, directory reading) SHALL be submitted as Jobs to the Worker_Pool
3. THE Application SHALL process user input within 16 milliseconds
4. THE Application SHALL render UI updates at least 30 times per second
5. WHEN a Job is running, THE Application SHALL continue to accept user input
6. WHEN a Job is running, THE Application SHALL allow navigation and other operations in both panes
7. THE Worker_Pool SHALL execute all file operations asynchronously on worker threads
8. THE Application SHALL receive job progress updates via JobEvent channel without blocking the UI_Thread

### Requirement 22: Directory Caching

**User Story:** As a user, I want directory contents to load quickly when revisiting locations, so that navigation feels snappy.

#### Acceptance Criteria

1. THE Application SHALL cache directory contents for recently visited locations
2. THE Application SHALL use cached data when available instead of re-reading from disk
3. THE Application SHALL invalidate cache entries after 30 seconds
4. THE Application SHALL invalidate cache entries when file operations complete in that directory
5. WHEN using cached data, THE Application SHALL verify the directory has not changed via checksum comparison
6. IF the directory has changed, THEN THE Application SHALL submit a directory read Job to the Worker_Pool
7. THE directory reading operation SHALL be executed as a Job on the Worker_Pool (not the UI_Thread)

### Requirement 23: Visual Feedback

**User Story:** As a user, I want visual feedback for my actions, so that I know the application has received my input.

#### Acceptance Criteria

1. WHEN the user presses a key, THE Application SHALL provide immediate visual feedback within 16 milliseconds
2. WHEN a Job starts, THE Application SHALL display a notification in the Task_Panel
3. WHEN a Job completes, THE Application SHALL display a completion notification
4. WHEN an error occurs, THE Application SHALL display the error with a distinct visual style
5. WHEN waiting for user input in a Dialog, THE Application SHALL display a cursor in the input field

### Requirement 24: File Size Formatting

**User Story:** As a user, I want file sizes displayed in human-readable format, so that I can quickly understand file sizes.

#### Acceptance Criteria

1. THE Application SHALL display file sizes in bytes for files smaller than 1 KB
2. THE Application SHALL display file sizes in KB for files between 1 KB and 1 MB
3. THE Application SHALL display file sizes in MB for files between 1 MB and 1 GB
4. THE Application SHALL display file sizes in GB for files between 1 GB and 1 TB
5. THE Application SHALL display file sizes in TB for files 1 TB and larger
6. THE Application SHALL display sizes with 2 decimal places of precision

### Requirement 25: Date Formatting

**User Story:** As a user, I want modification dates displayed in a readable format, so that I can understand when files were last changed.

#### Acceptance Criteria

1. THE Application SHALL display modification dates in YYYY-MM-DD HH:MM format
2. WHERE a file was modified today, THE Application SHALL display "Today HH:MM"
3. WHERE a file was modified yesterday, THE Application SHALL display "Yesterday HH:MM"
4. THE Application SHALL use 24-hour time format by default
5. WHERE the user has configured 12-hour format, THE Application SHALL display times with AM/PM indicators

### Requirement 26: State Management Architecture

**User Story:** As a developer, I want the application to follow a predictable state management pattern, so that the codebase is maintainable and testable.

#### Acceptance Criteria

1. THE Application SHALL maintain all application state in a central AppState structure
2. THE AppState SHALL coordinate FilesystemModel, JobManager, SearchModel, MarkingModel, NavigationHistory, UIState, and DialogStack
3. ALL state changes SHALL occur through explicit Transition enum values
4. THE Application SHALL implement state transitions as pure functions that transform AppState
5. THE Transition enum SHALL include variants for navigation, job operations, view operations, search operations, UI operations, and configuration updates
6. WHEN a Transition is applied, THE Application SHALL return a StateUpdateResult indicating started jobs, completed jobs, and cancelled jobs
7. THE Application SHALL process user input by mapping KeyEvent to Transition values
8. THE JobManager SHALL maintain three collections: queued jobs (FIFO_Queue), active jobs (HashMap), and completed jobs (HashMap)
9. THE Application SHALL enforce that state transitions are deterministic and testable
10. THE Application SHALL separate pure state logic from side effects (file I/O, rendering)

### Requirement 27: Tab Management

**User Story:** As a user, I want to manage multiple tabs with independent pane states, so that I can work with multiple directory contexts simultaneously.

#### Acceptance Criteria

1. THE Application SHALL support multiple tabs, each maintaining independent left and right pane states
2. WHEN the user presses Ctrl+N or Ctrl+T, THE Application SHALL create a new tab initialized with the current working directory
3. WHEN the user presses Ctrl+W, THE Application SHALL close the current tab
4. IF only one tab remains, THEN THE Application SHALL prevent tab closure
5. WHEN the user presses Ctrl+Right or Ctrl+PageDown, THE Application SHALL switch to the next tab
6. WHEN the user presses Ctrl+Left or Ctrl+PageUp, THE Application SHALL switch to the previous tab
7. WHEN the user presses Ctrl+T or Ctrl+B, THE Application SHALL display a tab selector Dialog with filtering capability
8. THE Application SHALL display a tab bar showing all open tabs
9. THE Application SHALL indicate the active tab with bracket markers
10. WHEN more tabs exist than can fit in the tab bar, THE Application SHALL display scrolling indicators
11. WHEN a tab has active Jobs, THE Application SHALL display a busy indicator (~) on that tab
12. THE Application SHALL persist tab states to session storage
13. WHEN the Application starts, THE Application SHALL restore previously open tabs from session storage
14. THE AppState SHALL maintain a TabManager component coordinating all tab states

### Requirement 28: Custom Functions with Macros

**User Story:** As a user, I want to define custom functions with macro expansion, so that I can automate repetitive tasks and extend the file manager's capabilities.

#### Acceptance Criteria

1. WHEN the Application starts, THE Application SHALL load custom functions from custom_functions.json
2. THE Application SHALL support macro expansion in custom function commands using the following macros: $P (active pane path), $O (opposite pane path), $L (left pane path), $R (right pane path), $F (cursor file name), $W (cursor file name without extension), $E (cursor file extension), $M (marked files list), $* (all files in pane), $I (user input prompt), $V (selected text), $~ (home directory), $# (file count)
3. WHEN the user presses Shift+T or Shift+F, THE Application SHALL display a custom function selector Dialog
4. WHEN the user selects a custom function, THE Application SHALL expand all macros in the command
5. WHERE a custom function includes $I macro, THE Application SHALL prompt the user for input before execution
6. THE Application SHALL support direct key binding to custom functions via keybindings.json
7. THE Application SHALL support menu files for hierarchical function organization
8. THE Application SHALL support per-function shell configuration (bash, zsh, powershell, cmd)
9. THE Application SHALL support per-OS shell configuration in custom functions
10. THE Application SHALL support PipeToAction directives: JumpToPath, ExecuteFile, ExecuteFileWithEditor
11. THE Application SHALL expand environment variables in custom function commands
12. THE Application SHALL execute custom functions as Jobs on the Worker_Pool (not the UI_Thread)
13. WHEN a custom function with JumpToPath completes, THE Application SHALL navigate the Active_Pane to the returned path
14. WHEN a custom function with ExecuteFile completes, THE Application SHALL execute the returned file path
15. WHEN a custom function with ExecuteFileWithEditor completes, THE Application SHALL open the returned file in the configured editor

### Requirement 29: Archive Browsing

**User Story:** As a user, I want to browse archive contents as if they were directories, so that I can inspect and extract files without external tools.

#### Acceptance Criteria

1. WHEN the user presses Enter on an archive file (.zip), THE Application SHALL open a virtual folder view of the archive contents
2. THE Application SHALL display archive contents as File_Entry items with full metadata
3. WHEN browsing an archive, THE Application SHALL allow navigation through nested directories within the archive
4. WHEN the user presses Backspace in an archive virtual folder, THE Application SHALL exit the virtual folder and return to the filesystem view
5. WHEN the user presses Shift+Enter on an archive file, THE Application SHALL extract the archive to the opposite pane location
6. WHEN the user presses 'P' with Marked_Files, THE Application SHALL create a .zip archive containing the marked files
7. THE Application SHALL submit archive operations (browse, extract, compress) as Jobs to the Worker_Pool
8. THE Application SHALL display archive operation progress in the Task_Panel via JobEvent updates
9. THE Application SHALL support extensible archive format handlers beyond .zip
10. THE UI_Thread SHALL remain responsive during archive operations
11. WHEN archive extraction completes, THE Application SHALL refresh the destination pane

### Requirement 30: Advanced Search and Filtering

**User Story:** As a user, I want powerful search capabilities with wildcards and regular expressions, so that I can find files using complex patterns.

#### Acceptance Criteria

1. WHEN the user presses 'F' or '/', THE Application SHALL enter incremental search mode
2. THE Application SHALL support wildcard patterns using * (match any characters) and ? (match single character)
3. THE Application SHALL support regular expression patterns enclosed in forward slashes (/pattern/)
4. THE Application SHALL support case-insensitive regex patterns using /pattern/i syntax
5. THE Application SHALL support combined include/exclude patterns using colon separator (include:exclude)
6. WHERE migemo support is enabled, THE Application SHALL support Japanese romaji search
7. WHEN the user presses Ctrl+K, THE Application SHALL clear the active search or filter
8. THE Application SHALL filter displayed entries in real-time as the user types
9. THE Application SHALL highlight matching portions of file names in search results
10. THE Application SHALL display the active search pattern in the Status_Bar

### Requirement 31: Registered Folders

**User Story:** As a user, I want to register frequently-used directories, so that I can quickly navigate to them without typing full paths.

#### Acceptance Criteria

1. WHEN the user presses Shift+B, THE Application SHALL register the current Active_Pane location as a registered folder
2. WHEN the user presses 'I', 'G', or Shift+F, THE Application SHALL display a registered folders Dialog with filtering capability
3. WHEN the user selects a registered folder from the Dialog, THE Application SHALL navigate the Active_Pane to that location
4. WHEN the user presses Shift+M, THE Application SHALL move Marked_Files to a selected registered folder
5. THE Application SHALL support environment variables in registered folder paths
6. THE Application SHALL persist registered folders to registered_directory.json
7. WHEN the Application starts, THE Application SHALL load registered folders from registered_directory.json
8. THE Application SHALL expand environment variables when navigating to registered folders
9. THE registered folders Dialog SHALL support incremental filtering by typing

### Requirement 32: Display Modes

**User Story:** As a user, I want to customize how files are displayed, so that I can optimize the view for my workflow.

#### Acceptance Criteria

1. WHEN the user presses keys '1' through '8', THE Application SHALL set the Display_Mode to 1-8 columns respectively
2. WHEN the user presses key '0', THE Application SHALL set the Display_Mode to detailed mode showing full metadata
3. THE Application SHALL support configurable directory colors via config.json
4. THE Application SHALL support CJK character width configuration for proper alignment
5. THE Application SHALL support custom color schemes for all UI elements via config.json
6. THE Application SHALL maintain separate Display_Mode settings for each pane
7. THE Application SHALL maintain separate Display_Mode settings for each tab

### Requirement 33: File Viewing

**User Story:** As a user, I want to view file contents without opening external applications, so that I can quickly inspect files.

#### Acceptance Criteria

1. WHEN the user presses 'V', THE Application SHALL open the text viewer for the current Cursor file
2. WHEN the user presses 'F8' or 'B', THE Application SHALL open the hex viewer for the current Cursor file
3. THE text viewer SHALL support multiple text encodings (UTF-8, UTF-16, Shift-JIS, etc.)
4. WHEN the user presses Shift+E in the viewer, THE Application SHALL cycle through available encodings
5. WHEN the user presses 'F4' in the viewer, THE Application SHALL enter search mode
6. WHEN the user presses 'F3' in the viewer, THE Application SHALL find the next search match
7. WHEN the user presses Shift+F3 in the viewer, THE Application SHALL find the previous search match
8. WHEN the user presses Home in the viewer, THE Application SHALL move to the start of the current line
9. WHEN the user presses End in the viewer, THE Application SHALL move to the end of the current line
10. WHEN the user presses 'F5' in the viewer, THE Application SHALL move to the top of the file
11. WHEN the user presses 'F6' in the viewer, THE Application SHALL move to the bottom of the file
12. THE Application SHALL load viewer keybindings from config.json
13. THE Application SHALL load file contents as a Job on the Worker_Pool (not the UI_Thread)
14. THE UI_Thread SHALL remain responsive during file loading

### Requirement 34: Pattern-Based Rename

**User Story:** As a user, I want to rename multiple files using patterns, so that I can batch rename files efficiently.

#### Acceptance Criteria

1. WHEN the user presses Shift+R, THE Application SHALL display a pattern-based rename Dialog
2. THE Application SHALL support pattern syntax with wildcards and replacement tokens
3. IF Marked_Files exist, THEN THE Application SHALL apply the rename pattern to all Marked_Files
4. IF no Marked_Files exist, THEN THE Application SHALL apply the rename pattern to the current Cursor file
5. THE Application SHALL preview the rename results before execution
6. WHEN the user confirms the rename, THE Application SHALL create a Job for the batch rename operation
7. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
8. THE Worker_Pool SHALL execute the rename Job on a worker thread (not the UI_Thread)
9. WHEN the rename Job completes, THE Application SHALL refresh the Active_Pane
10. THE UI_Thread SHALL remain responsive during the rename operation

### Requirement 35: File Comparison and Split/Join

**User Story:** As a user, I want to compare files and split/join large files, so that I can manage file differences and handle large file transfers.

#### Acceptance Criteria

1. WHEN the user presses 'W', THE Application SHALL compare the current Cursor file with the file at the same position in the opposite pane
2. THE Application SHALL display file differences in a comparison view
3. WHEN the user presses Shift+W, THE Application SHALL display a file split/join Dialog
4. THE Application SHALL support splitting large files into multiple parts with configurable size
5. THE Application SHALL support joining split file parts back into the original file
6. THE Application SHALL execute comparison operations as Jobs on the Worker_Pool
7. THE Application SHALL execute split/join operations as Jobs on the Worker_Pool
8. THE Application SHALL display operation progress in the Task_Panel via JobEvent updates
9. THE UI_Thread SHALL remain responsive during comparison and split/join operations

### Requirement 36: Advanced Marking Operations

**User Story:** As a user, I want advanced marking capabilities, so that I can select files using complex criteria.

#### Acceptance Criteria

1. WHEN the user presses '@', THE Application SHALL display a wildcard marking Dialog
2. THE Application SHALL support wildcard patterns (* and ?) in the marking Dialog
3. WHEN the user submits a pattern, THE Application SHALL mark all files matching the pattern
4. WHEN the user presses Ctrl+Space, THE Application SHALL enter range marking mode
5. WHEN in range marking mode, THE Application SHALL mark all files between the initial Cursor position and the current Cursor position
6. WHEN the user presses Home (with Shift or in marking mode), THE Application SHALL invert all marks in the Active_Pane
7. THE Application SHALL maintain marked file state across directory navigation
8. THE Application SHALL display marked file count and total size in the Status_Bar

### Requirement 37: Directory Size Calculation

**User Story:** As a user, I want to calculate directory sizes, so that I can understand disk space usage.

#### Acceptance Criteria

1. WHEN the user presses 'H', THE Application SHALL initiate directory size calculation for the current Cursor directory
2. THE Application SHALL create a Job for the directory size calculation
3. THE Application SHALL submit the Job to the FIFO_Queue via JobManager
4. THE Worker_Pool SHALL execute the directory size calculation on a worker thread (not the UI_Thread)
5. THE Application SHALL display calculation progress in the Task_Panel via JobEvent updates
6. WHEN the calculation completes, THE Application SHALL display the total size in the File_Entry
7. THE Application SHALL allow multiple directory size calculations to run concurrently
8. THE UI_Thread SHALL remain responsive during directory size calculation
9. THE Application SHALL support cancellation of directory size calculation Jobs

### Requirement 38: Configuration System

**User Story:** As a user, I want comprehensive configuration options, so that I can customize the application to my preferences.

#### Acceptance Criteria

1. THE Application SHALL load configuration from config.json at startup
2. WHEN the user presses Shift+Z, THE Application SHALL reload configuration from config.json without restarting
3. THE Application SHALL support configurable log levels: None, Trace, Debug, Information, Warning, Error, Critical
4. THE Application SHALL write logs to a log file with automatic rotation at 10MB
5. THE Application SHALL support per-OS shell configuration (bash, zsh, powershell, cmd)
6. THE Application SHALL support session state persistence including tab states, pane locations, and marked files
7. WHEN the Application starts, THE Application SHALL restore session state from storage
8. THE Application SHALL support multi-language help system via configuration
9. THE Application SHALL validate configuration on load and display errors for invalid settings
10. IF config.json is malformed, THEN THE Application SHALL display an error and use default settings
11. THE Application SHALL support configurable scroll_offset value (default: 3) that controls when automatic scrolling triggers
12. THE Application SHALL support all color configuration options for UI customization including FilePaneCursorForegroundColor, FilePaneCursorBackgroundColor, PaneInfoForegroundColor, PaneInfoBackgroundColor, InactiveFilePaneCursorForegroundColor, and InactiveFilePaneCursorBackgroundColor

### Requirement 39: Enhanced UI Elements

**User Story:** As a user, I want comprehensive status information and visual feedback, so that I understand the application state at all times.

#### Acceptance Criteria

1. THE Application SHALL display a top separator showing drive/share names for both panes
2. THE top separator SHALL display marked file count and total size
3. THE Status_Bar SHALL display current directory, file count, marked file stats, active job count, filter, and sort mode
4. THE Task_Panel SHALL display a scrollable log history of completed operations
5. WHEN the user presses Ctrl+J, THE Application SHALL display a job manager Dialog with detailed progress for all jobs
6. THE Application SHALL display busy spinners for active operations
7. WHEN a tab has active Jobs, THE Application SHALL display a busy indicator (slash characters) on that tab
8. THE Application SHALL support configurable colors for all UI elements via config.json
9. THE Application SHALL update all UI elements within 16 milliseconds of state changes
10. THE Application SHALL render the UI at least 30 times per second

### Requirement 39A: Volume Name Display in Top Separator

**User Story:** As a user, I want to see volume names and marked file statistics in the top separator, so that I can quickly identify which drives or shares I am working with and track marked files.

#### Acceptance Criteria

1. THE Application SHALL display volume names or network share names in the top separator for each pane
2. WHEN the path is a network path (\\server\share), THE Application SHALL display the server name in the format "\\server"
3. WHEN the path is a Linux or MacOS filesystem, THE Application SHALL display the device path and mount point with volume label if available
4. WHEN a Linux or MacOS volume has a label, THE Application SHALL display it in the format "{device} ({mount_point} - {label})"
5. WHEN a Linux or MacOS volume has no label, THE Application SHALL display it in the format "{device} ({mount_point})"
6. WHEN the mount point is root (/), THE Application SHALL display "Root" if no device path is available
7. WHEN the path is a Windows drive letter, THE Application SHALL display the volume label if available
8. WHEN a Windows drive has no volume label, THE Application SHALL display the drive letter in brackets in the format "(C:)"
9. THE Application SHALL display marked file statistics in the format "{count} {Dirs/Files} {size} marked"
10. WHEN marked files include both directories and files, THE Application SHALL display both counts (e.g., "2 Dirs 3 Files 1.5 MB marked")
11. WHEN marked files include only directories, THE Application SHALL display only directory count (e.g., "1 Dir marked")
12. WHEN marked files include only files, THE Application SHALL display only file count (e.g., "5 Files 2.3 GB marked")
13. THE Application SHALL format the top separator as "{volume_name} {marked_stats}" for each pane
14. THE Application SHALL use TopSeparatorForegroundColor and TopSeparatorBackgroundColor for the top separator display

### Requirement 40: Job Manager Dialog

**User Story:** As a user, I want detailed visibility into all background operations, so that I can monitor and control long-running tasks.

#### Acceptance Criteria

1. WHEN the user presses Ctrl+J, THE Application SHALL display the job manager Dialog
2. THE job manager Dialog SHALL display all queued jobs from the FIFO_Queue
3. THE job manager Dialog SHALL display all active jobs with progress percentages
4. THE job manager Dialog SHALL display all completed jobs with success/failure status
5. THE job manager Dialog SHALL display detailed progress information including bytes transferred and estimated time remaining
6. THE job manager Dialog SHALL support scrolling through the job list
7. THE job manager Dialog SHALL allow cancellation of queued or active jobs
8. THE job manager Dialog SHALL display job type (copy, move, delete, rename, etc.)
9. THE job manager Dialog SHALL display source and destination paths for file operations
10. THE job manager Dialog SHALL update in real-time as jobs progress

### Requirement 41: Pane Synchronization and Swapping

**User Story:** As a user, I want to synchronize and swap pane paths, so that I can quickly align both panes or exchange their locations.

#### Acceptance Criteria

1. WHEN the user presses 'O', THE Application SHALL synchronize the opposite pane to the active pane's current directory
2. WHEN synchronization occurs, THE Application SHALL navigate the opposite pane to the same location as the active pane
3. WHEN the user presses Shift+O, THE Application SHALL swap the paths of the left and right panes
4. WHEN swapping occurs, THE Application SHALL exchange the current_location of both panes
5. THE Application SHALL maintain cursor positions and marked files during swap operations
6. THE Application SHALL create Jobs to read directories for both panes after synchronization or swapping
7. THE UI_Thread SHALL remain responsive during pane synchronization and swapping

### Requirement 42: Context Menu and Drive Selection

**User Story:** As a user, I want quick access to common operations via context menu and easy drive/share selection, so that I can work more efficiently.

#### Acceptance Criteria

1. WHEN the user presses '\' or backtick key, THE Application SHALL display a context menu with common file operations
2. THE context menu SHALL include options for copy, move, delete, rename, view, and custom functions
3. WHEN the user presses Shift+L, THE Application SHALL display a drive/share selection dialog
4. THE drive selection dialog SHALL list all available drives and network shares
5. WHEN the user selects a drive or share, THE Application SHALL navigate the active pane to that location
6. THE Application SHALL display drive information including total size, free space, and drive type
7. THE Application SHALL support quick drive switching via keyboard shortcuts

### Requirement 43: File Information and Version Display

**User Story:** As a user, I want to view detailed file information and application version, so that I can access metadata and verify the application version.

#### Acceptance Criteria

1. WHEN the user presses Shift+I, THE Application SHALL display detailed information for the current cursor file
2. THE file information dialog SHALL display file name, full path, size, creation date, modification date, access date, and attributes
3. THE file information dialog SHALL display file permissions and ownership information
4. WHEN the user presses a configured version key, THE Application SHALL display the application version and build information
5. THE version dialog SHALL display version number, build date, and copyright information
6. THE Application SHALL support dismissing information dialogs with Escape or Enter keys

### Requirement 44: Log Management

**User Story:** As a user, I want to save application logs, so that I can review operations and troubleshoot issues.

#### Acceptance Criteria

1. WHEN the user presses a configured save log key, THE Application SHALL save the current session log to a file
2. THE Application SHALL save logs to the configured log path (default: logs/session.log)
3. THE Application SHALL include timestamps for all log entries
4. THE Application SHALL support configurable maximum log lines in memory (default: 2000)
5. WHEN the Application exits, THE Application SHALL optionally save the session log based on SaveLogOnExit configuration
6. THE Application SHALL support log file rotation to prevent excessive disk usage
7. THE Application SHALL log slow file operations exceeding the configured threshold (default: 5000ms)

### Requirement 45: Configuration Program Launch

**User Story:** As a user, I want to quickly edit configuration files, so that I can customize the application without manually locating config files.

#### Acceptance Criteria

1. WHEN the user presses 'Y', THE Application SHALL launch the configured editor with the main configuration file
2. THE Application SHALL support configurable editor command in config.json
3. AFTER the editor closes, THE Application SHALL prompt the user to reload configuration
4. IF the user confirms reload, THEN THE Application SHALL reload configuration without restarting
5. THE Application SHALL validate the configuration after reload and display errors if invalid
6. THE Application SHALL fall back to previous configuration if the new configuration is invalid

### Requirement 46: Exit and Change Directory

**User Story:** As a user, I want to change my shell's working directory when exiting the application, so that I can continue working in the last visited directory.

#### Acceptance Criteria

1. WHEN the user presses Shift+Q, THE Application SHALL exit and output the current active pane directory
2. THE Application SHALL support shell integration via wrapper scripts that capture the output directory
3. THE Application SHALL support -cwd command-line flag to enable directory change on exit
4. WHEN -cwd flag is provided, THE Application SHALL write the final directory to stdout before exiting
5. THE Application SHALL provide example wrapper scripts for bash, zsh, and PowerShell
6. THE wrapper scripts SHALL change the shell's working directory to the output directory after the application exits

### Requirement 47: Task Panel Management

**User Story:** As a user, I want to control the task panel visibility and size, so that I can optimize screen space for my workflow.

#### Acceptance Criteria

1. WHEN the user presses a configured toggle key, THE Application SHALL toggle the task panel visibility
2. WHEN the user presses Ctrl+Up, THE Application SHALL increase the task panel height
3. WHEN the user presses Ctrl+Down, THE Application SHALL decrease the task panel height
4. WHEN the user presses Alt+Up, THE Application SHALL scroll the task panel up
5. WHEN the user presses Alt+Down, THE Application SHALL scroll the task panel down
6. THE Application SHALL maintain task panel size and visibility settings across sessions
7. THE Application SHALL display a scrollbar when task panel content exceeds visible area

### Requirement 48: Multi-Language Help System

**User Story:** As a user, I want help documentation in my preferred language, so that I can understand the application in my native language.

#### Acceptance Criteria

1. THE Application SHALL load help content from language-specific JSON files (help.{lang}.json)
2. WHEN the user presses '?' or F1, THE Application SHALL display the help dialog in the configured language
3. WHEN the user presses 'L' in the help dialog, THE Application SHALL rotate through available languages
4. THE Application SHALL support multiple languages including English (en) and Japanese (jp)
5. THE help dialog SHALL display all key bindings with descriptions in the selected language
6. THE Application SHALL persist the selected help language in configuration
7. THE Application SHALL fall back to English if the configured language file is not found

### Requirement 49: Color Configuration Mapping

**User Story:** As a user, I want to understand which color settings apply to which UI areas, so that I can customize the appearance effectively.

#### Acceptance Criteria

1. THE Application SHALL apply ActiveTabForegroundColor, ActiveTabBackgroundColor, InactiveTabForegroundColor, InactiveTabBackgroundColor, and TabbarBackgroundColor to the tab bar (UI area 1)
2. THE Application SHALL apply ForegroundColor and BackgroundColor to the path display (UI area 2)
3. THE Application SHALL apply TopSeparatorForegroundColor and TopSeparatorBackgroundColor to the top separator showing volume names (UI area 3)
4. THE Application SHALL apply ForegroundColor, BackgroundColor, FilePaneCursorForegroundColor, FilePaneCursorBackgroundColor, MarkedFileColor, DirectoryColor, and DirectoryBackgroundColor to the active file pane (UI area 4)
5. THE Application SHALL apply InactiveForegroundColor, InactiveBackgroundColor, InactiveDirectoryColor, InactiveDirectoryBackgroundColor, InactiveFilePaneCursorForegroundColor, and InactiveFilePaneCursorBackgroundColor to the inactive file pane (UI area 4)
6. THE Application SHALL apply PaneInfoForegroundColor and PaneInfoBackgroundColor to the pane info bar (UI area 5)
7. THE Application SHALL apply FilenameLabelForegroundColor and FilenameLabelBackgroundColor to the selected filename line (UI area 6)
8. THE Application SHALL apply ForegroundColor and BackgroundColor to the task view pane (UI area 7)
9. THE Application SHALL support backward compatibility by treating HighlightForegroundColor as an alias for FilePaneCursorForegroundColor
10. THE Application SHALL support backward compatibility by treating HighlightBackgroundColor as an alias for FilePaneCursorBackgroundColor
11. THE Application SHALL support backward compatibility by treating TopSeparatorForegroundColor as an alias for PaneInfoForegroundColor when PaneInfoForegroundColor is not specified
12. THE Application SHALL support backward compatibility by treating TopSeparatorBackgroundColor as an alias for PaneInfoBackgroundColor when PaneInfoBackgroundColor is not specified
13. THE Application SHALL support backward compatibility by treating InactiveMarkedFileColor as an alias for InactiveFilePaneCursorBackgroundColor when InactiveFilePaneCursorBackgroundColor is not specified
