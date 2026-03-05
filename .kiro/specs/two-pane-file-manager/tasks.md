# Implementation Plan: Two-Pane File Manager

## Overview

This implementation plan breaks down the two-pane file manager into 8 phases following the design roadmap. The application is built in Rust using the Reactive Worker Framework (rwf) for asynchronous file operations and the AppState pattern for state management.

**Key Architecture Principles:**
- All file I/O operations execute as Jobs in the rwf Worker Pool (never block UI thread)
- State changes occur through explicit Transition enum values
- Pure state functions return StateUpdateResult with side effects
- FIFO job ordering with cooperative cancellation
- Property-based testing for 31 correctness properties

**Implementation Language:** Rust

**Estimated Timeline:** 16 weeks (14 weeks for core features + 2 weeks for additional TWF features)

## Tasks

### Phase 1: Core Infrastructure (Weeks 1-2)

- [x] 1. Set up project structure and dependencies
  - Create Cargo workspace with main binary and library crates
  - Add dependencies: tokio, ratatui, crossterm, serde, serde_json, thiserror, anyhow, regex, tracing
  - Add dev dependencies: proptest, tempfile, assert_fs, predicates
  - Configure project for async runtime with tokio
  - _Requirements: 20.1, 20.2_

- [x] 2. Implement core data structures
  - [x] 2.1 Implement Location enum with variants (Local, Ssh, Cloud, Archive)
    - Implement display_path(), parent(), and join() methods
    - Support path manipulation for all location types
    - _Requirements: 3.1, 3.2, 29.1, 29.4_
  
  - [x] 2.2 Write property test for Location
    - **Property 7: Parent Navigation**
    - **Validates: Requirements 3.2**
  
  - [x] 2.3 Implement FileEntry struct
    - Include name, location, size, is_dir, is_hidden, modified, marked, calculated_size fields
    - Implement helper methods: extension(), name_without_extension(), formatted_size(), formatted_date()
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 24.1-24.6, 25.1-25.5_

  - [x] 2.4 Implement PaneModel struct
    - Include current_location, entries, cursor, scroll_offset, sort_mode, display_mode, file_mask fields
    - Implement current_entry(), marked_entries(), apply_sort(), apply_filter() methods
    - _Requirements: 1.5, 1.6, 2.2-2.7, 12.1-12.7, 13.1-13.6_
  
  - [x] 2.5 Write property tests for PaneModel
    - **Property 4: Cursor Bounds Invariant**
    - **Validates: Requirements 2.2, 2.3, 2.4, 2.5, 2.6, 2.7**
  
  - [x] 2.6 Implement TabState and TabManager structs
    - TabState: id, left_pane, right_pane, history fields
    - TabManager: tabs, active_index fields with create_tab(), close_tab(), switch_to_next(), switch_to_prev() methods
    - _Requirements: 27.1-27.14_
  
  - [x] 2.7 Write property test for TabManager
    - **Property 23: Tab Independence**
    - **Validates: Requirements 27.1**

- [x] 3. Implement state management core
  - [x] 3.1 Implement Transition enum
    - Define variants for navigation (ChangeLocation, NavigateUp, NavigateBack, NavigateForward)
    - Define variants for pane operations (SwitchPane, MoveCursor, ScrollPane)
    - Define variants for file operations (Copy, Move, Delete, Rename, Mkdir)
    - Define variants for marking (ToggleMark, MarkAll, UnmarkAll, MarkPattern)
    - Define variants for search (EnterSearch, UpdateSearch, ExitSearch)
    - Define variants for jobs (JobStarted, JobProgress, JobCompleted, JobFailed, JobCancelled)
    - Define variants for UI (OpenDialog, CloseDialog, UpdateInput)
    - Define variants for tabs (CreateTab, CloseTab, SwitchTab)
    - _Requirements: 26.3, 26.5, 26.10_
  
  - [x] 3.2 Implement StateUpdateResult struct
    - Include new_state, jobs_to_start, jobs_to_cancel, panes_to_refresh fields
    - _Requirements: 26.6_
  
  - [x] 3.3 Implement AppState struct
    - Include tabs, jobs, search, marking, ui, dialogs, backends, config fields
    - Implement helper methods: current_tab(), active_pane(), opposite_pane()
    - _Requirements: 26.1, 26.2_
  
  - [x] 3.4 Implement update_state pure function
    - Take AppState and Transition, return StateUpdateResult
    - Handle all Transition variants with pure logic
    - Never perform I/O operations directly
    - _Requirements: 26.4, 26.7, 26.9_
  
  - [x] 3.5 Write property test for state transitions
    - **Property 22: State Transition Determinism**
    - **Validates: Requirements 26.4, 26.9**

- [x] 4. Implement supporting models
  - [x] 4.1 Implement MarkingModel struct
    - Include marked_locations HashSet
    - Implement toggle(), mark(), unmark(), mark_all(), unmark_all(), is_marked(), count(), total_size() methods
    - _Requirements: 5.1-5.7_
  
  - [x] 4.2 Write property tests for MarkingModel
    - **Property 10: Mark Toggle Idempotence**
    - **Property 11: Mark All Completeness**
    - **Property 12: Unmark All Completeness**
    - **Property 13: Marking Persistence Across Navigation**
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4**
  
  - [x] 4.3 Implement SearchModel struct
    - Include query, results, history, current_index, case_sensitive, use_regex, use_migemo fields
    - Implement add_to_history(), matches() methods
    - Implement wildcard_to_regex() helper function
    - _Requirements: 11.1-11.8, 30.1-30.10_
  
  - [x] 4.4 Write property test for SearchModel
    - **Property 28: Search Pattern Matching**
    - **Validates: Requirements 30.2, 30.3, 30.4**
  
  - [x] 4.5 Implement NavigationHistory struct
    - Include left_stack, right_stack, left_pos, right_pos fields
    - Implement push(), go_back(), go_forward() methods
    - _Requirements: 14.1-14.5_
  
  - [x] 4.6 Write property test for NavigationHistory
    - **Property 9: Navigation History Preservation**
    - **Validates: Requirements 3.7**
  
  - [x] 4.7 Implement UIState and DialogStack structs
    - UIState: active_pane, mode, layout fields
    - DialogStack: stack, input_buffer fields with push(), pop(), current() methods
    - Define Dialog and DialogContent enums
    - _Requirements: 2.1, 11.1, 11.8_

- [x] 5. Set up property-based testing framework
  - [x] 5.1 Configure proptest in dev dependencies
    - Set up proptest strategies for core types (Location, FileEntry, PaneModel)
    - Create test utilities module
    - _Requirements: Design Section - Correctness Properties_
  
  - [x] 5.2 Implement Arbitrary instances for core types
    - Implement Arbitrary for Location with all variants
    - Implement Arbitrary for FileEntry with valid metadata
    - Implement Arbitrary for Transition enum
    - _Requirements: Design Section - Correctness Properties_

- [x] 6. Checkpoint - Core infrastructure complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 2: Job System Integration (Weeks 3-4)

- [x] 7. Implement JobManager and job types
  - [x] 7.1 Implement JobId, JobSpec, Job, JobResult structs
    - JobSpec: id, kind, created_at, cancel_token fields
    - Job: spec, state, progress, started_at fields
    - JobResult: id, kind, completed_at, result fields
    - _Requirements: 15.1, 15.2_

  - [x] 7.2 Implement JobKind enum
    - Define variants: ReadDirectory, Copy, Move, Delete, Mkdir, Rename, CalculateSize
    - Define variants: ExtractArchive, CreateArchive, ExecuteCustomFunction, Search
    - Include necessary data for each variant (sources, destinations, patterns, etc.)
    - _Requirements: 3.3, 6.6, 7.6, 8.5, 9.3, 10.3, 29.7, 37.2_
  
  - [x] 7.3 Implement JobManager struct
    - Include queue (VecDeque), active (HashMap), completed (VecDeque), max_parallel, next_id fields
    - Implement enqueue(), can_start_job(), pop_next_job(), start_job() methods
    - Implement update_progress(), complete_job(), request_cancel(), acknowledge_cancel() methods
    - Maintain FIFO queue ordering
    - _Requirements: 15.1, 15.11, 15.12, 26.8_
  
  - [x] 7.4 Write property test for JobManager
    - **Property 20: FIFO Job Ordering**
    - **Validates: Requirements 15.11**

- [x] 8. Integrate rwf Worker Pool
  - [x] 8.1 Set up rwf Worker Pool with configurable size
    - Initialize worker pool with configured thread count (default: 4)
    - Set up event channel for JobEvent communication
    - Implement job submission to worker pool
    - _Requirements: 17.8, 20.2, 21.1, 21.7_
  
  - [x] 8.2 Implement JobEvent enum
    - Define variants: Started, Progress, Completed, Failed, Cancelled
    - Include job_id and relevant data for each variant
    - _Requirements: 15.4, 15.8, 21.8_
  
  - [x] 8.3 Implement event receiver on UI thread
    - Set up non-blocking event receiver
    - Map JobEvents to Transition enum values
    - Feed transitions to update_state function
    - _Requirements: 21.1, 21.5, 21.8, 26.7_

- [x] 9. Implement FilesystemBackend trait and LocalFilesystemBackend
  - [x] 9.1 Define FilesystemBackend trait
    - Define async methods: read_directory(), copy_file(), move_file(), delete_file(), rename_file(), create_directory()
    - Define async methods: calculate_directory_size(), read_file_content()
    - All methods accept CancellationToken for cooperative cancellation
    - _Requirements: 3.4, 6.8, 7.8, 8.7, 9.5, 10.5, 15.6, 21.7, 26.10_
  
  - [x] 9.2 Implement LocalFilesystemBackend
    - Implement read_directory() with metadata extraction
    - Implement copy_file() with progress reporting
    - Implement move_file() with progress reporting
    - Implement delete_file() with error handling
    - Implement rename_file() with validation
    - Implement create_directory() with error handling
    - All operations check cancellation token periodically
    - _Requirements: 3.4, 4.1-4.3, 6.8-6.11, 7.8-7.11, 8.7-8.9, 9.5-9.8, 10.5-10.8_
  
  - [x] 9.3 Implement calculate_directory_size() with cancellation support
    - Recursively traverse directory tree
    - Check cancellation token periodically
    - Report progress via JobEvent
    - _Requirements: 37.1-37.9_
  
  - [x] 9.4 Write property test for directory size calculation
    - **Property 31: Size Calculation Updates Entry**
    - **Validates: Requirements 37.6**

- [ ] 10. Implement job execution logic
  - [x] 10.1 Implement JobExecutor
    - Create async executor that processes JobSpec
    - Dispatch to appropriate backend method based on JobKind
    - Send JobEvent updates via channel
    - Handle errors and cancellation
    - _Requirements: 3.4, 6.8, 7.8, 8.7, 9.5, 10.5, 15.4, 15.6, 15.7_
  
  - [x] 10.2 Implement progress reporting for file operations
    - Report progress as percentage for copy/move operations
    - Report bytes transferred and estimated time remaining
    - Send Progress JobEvents periodically
    - _Requirements: 6.9, 7.9, 8.8, 15.4, 40.5_
  
  - [x] 10.3 Implement cooperative cancellation in all operations
    - Check cancellation token before each file operation
    - Check cancellation token periodically during long operations
    - Send Cancelled JobEvent when cancellation acknowledged
    - _Requirements: 15.5, 15.6, 15.7, 37.9_
  
  - [x] 10.4 Write integration tests for job lifecycle
    - Test job transitions: Queued → Running → Completed
    - Test job cancellation: Running → Cancelling → Cancelled
    - Test job failure handling
    - _Requirements: 15.1-15.12_

- [x] 11. Checkpoint - Job system complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 3: UI and Input (Weeks 5-6)

- [x] 12. Implement terminal UI with ratatui
  - [x] 12.1 Set up terminal initialization and cleanup
    - Initialize crossterm terminal with raw mode
    - Set up alternate screen
    - Implement cleanup to restore terminal state
    - _Requirements: 20.9_
  
  - [x] 12.2 Implement main render loop
    - Render at 30+ FPS
    - Render within 16ms of state changes
    - Use ratatui for terminal rendering
    - _Requirements: 21.4, 39.9, 39.10_
  
  - [x] 12.3 Implement pane rendering
    - Render two vertical panes side by side
    - Display file entries with name, size, date
    - Visually distinguish directories from files
    - Highlight cursor position
    - Highlight marked files
    - Show active pane indicator
    - _Requirements: 1.1-1.6, 4.1-4.7_
  
  - [x] 12.4 Implement display modes
    - Implement 1-8 column modes
    - Implement detailed mode with full metadata
    - Support configurable colors
    - Support CJK character width
    - _Requirements: 32.1-32.7_

- [x] 13. Implement status bar and task panel
  - [x] 13.1 Implement status bar rendering
    - Display current directory path
    - Display file count
    - Display marked file count and total size
    - Display active job count
    - Display active file mask
    - Display current sort mode
    - _Requirements: 16.1-16.7_
  
  - [x] 13.2 Implement task panel rendering
    - Display queued jobs from FIFO queue
    - Display active jobs with progress bars
    - Display completed jobs with status
    - Auto-remove completed jobs after 3 seconds
    - Display failure reasons for failed jobs
    - _Requirements: 15.2, 15.3, 15.4, 15.8, 15.9, 15.10_
  
  - [x] 13.3 Implement tab bar rendering
    - Display all open tabs
    - Indicate active tab with bracket markers
    - Display busy indicator (~) for tabs with active jobs
    - Support scrolling for many tabs
    - _Requirements: 27.8, 27.9, 27.10, 27.11_
  
  - [x] 13.4 Implement top separator
    - Display drive/share names for both panes
    - Display marked file count and total size
    - _Requirements: 39.1, 39.2_

- [x] 14. Implement dialog system
  - [x] 14.1 Implement dialog rendering
    - Render modal dialogs over main UI
    - Support confirmation dialogs
    - Support input dialogs with pre-filled values
    - Support progress dialogs
    - _Requirements: 6.5, 7.5, 8.4, 9.2, 10.2_
  
  - [x] 14.2 Implement job manager dialog
    - Display all queued, active, and completed jobs
    - Show detailed progress information
    - Show job type, source, and destination
    - Support scrolling through job list
    - Support job cancellation from dialog
    - _Requirements: 40.1-40.10_
  
  - [x] 14.3 Implement custom function selector dialog
    - Display list of custom functions
    - Support incremental filtering
    - Show function descriptions
    - _Requirements: 28.3_
  
  - [x] 14.4 Implement registered folder selector dialog
    - Display list of registered folders
    - Support incremental filtering
    - _Requirements: 31.2, 31.9_
  
  - [x] 14.5 Implement tab selector dialog
    - Display list of all tabs
    - Support filtering
    - _Requirements: 27.7_
  
  - [x] 14.6 Implement pattern rename dialog
    - Show pattern input field
    - Display preview of rename results
    - _Requirements: 34.1, 34.5_

- [x] 15. Implement input handling and key bindings
  - [x] 15.1 Implement KeyBindings struct
    - Load key bindings from keybindings.json
    - Support TWF-compatible defaults
    - Map key events to Transition enum values
    - Support multi-key sequences
    - _Requirements: 17.4, 17.5, 18.1-18.7_
  
  - [x] 15.2 Implement input event loop
    - Process keyboard input within 16ms
    - Map KeyEvent to Transition
    - Feed Transition to update_state
    - Provide visual feedback for multi-key sequences
    - _Requirements: 21.3, 21.5, 23.1, 26.7_
  
  - [x] 15.3 Implement navigation key handlers
    - Tab: switch pane
    - Up/Down/j/k: move cursor
    - Home/End: jump to first/last entry
    - PageUp/PageDown: page navigation
    - Enter: enter directory
    - Backspace/Left: navigate to parent
    - Alt+Left/Right: history navigation
    - _Requirements: 2.1-2.8, 3.1, 3.2, 14.1, 14.2_
  
  - [x] 15.4 Write property tests for navigation
    - **Property 1: Pane Independence**
    - **Property 2: Scroll Independence**
    - **Property 3: Pane Switching Toggles**
    - **Property 5: Cursor Visibility Invariant**
    - **Property 6: Directory Navigation Creates Job**
    - **Property 8: Location Change Resets Cursor**
    - **Validates: Requirements 1.5, 1.6, 2.1, 2.8, 3.1, 3.3, 3.6**
  
  - [x] 15.5 Implement file operation key handlers
    - C: copy operation
    - M: move operation
    - D: delete operation
    - R: rename operation
    - Shift+K: create directory
    - _Requirements: 6.1, 7.1, 8.1, 9.1, 10.1_
  
  - [x] 15.6 Write property tests for file operations
    - **Property 14: Copy Operation Job Creation**
    - **Property 15: Move Operation Job Creation**
    - **Property 16: Delete Operation Job Creation**
    - **Property 17: Delete Completion Unmarks Files**
    - **Validates: Requirements 6.1-6.3, 7.1-7.3, 8.1-8.3, 8.10**
  
  - [x] 15.6 Implement marking key handlers
    - Space: toggle mark
    - *: mark all
    - Ctrl+U: unmark all
    - @: wildcard marking dialog
    - Ctrl+Space: range marking mode
    - Home (with Shift): invert marks
    - _Requirements: 5.1-5.3, 36.1-36.8_
  
  - [x] 15.7 Write property test for wildcard marking
    - **Property 30: Wildcard Marking Completeness**
    - **Validates: Requirements 36.3**
  
  - [x] 15.8 Implement sorting key handlers
    - s+n: sort by name
    - s+s: sort by size
    - s+d: sort by date
    - s+e: sort by extension
    - _Requirements: 12.1-12.7_
  
  - [x] 15.9 Write property test for sorting
    - **Property 18: Directory-First Sorting**
    - **Property 19: Sort Stability**
    - **Validates: Requirements 12.1-12.6**

  - [x] 15.10 Implement search and filter key handlers
  
    - /: enter search mode
    - Ctrl+F: enter search mode
    - f: file mask filter dialog
    - Ctrl+K: clear search/filter
    - Escape: exit search mode
    - _Requirements: 11.1-11.8, 13.1-13.6, 30.7, 30.8_
  
  - [x] 15.11 Implement tab management key handlers
    - Ctrl+N/Ctrl+T: create new tab
    - Ctrl+W: close tab
    - Ctrl+Right/Ctrl+PageDown: next tab
    - Ctrl+Left/Ctrl+PageUp: previous tab
    - Ctrl+T/Ctrl+B: tab selector dialog
    - _Requirements: 27.2-27.7_
  
  - [x] 15.12 Implement miscellaneous key handlers
    - Q/Escape: quit application
    - ?/F1: help dialog
    - Ctrl+J: job manager dialog
    - _Requirements: 18.4, 20.5, 40.1_

- [x] 16. Checkpoint - UI and input complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 4: File Operations (Weeks 7-8)

- [x] 17. Implement copy operation
  - [x] 17.1 Implement copy confirmation dialog
    - Show source files and destination
    - Display total size to be copied
    - Request user confirmation
    - _Requirements: 6.5_
  
  - [x] 17.2 Implement copy job creation
    - Use marked files if any, otherwise cursor file
    - Use opposite pane location as destination
    - Create JobSpec with JobKind::Copy
    - Enqueue to JobManager
    - _Requirements: 6.1, 6.2, 6.3, 6.6, 6.7_
  
  - [x] 17.3 Implement copy execution in LocalFilesystemBackend
    - Copy files with progress reporting
    - Check cancellation token periodically
    - Handle overwrite confirmation
    - Refresh destination pane on completion
    - _Requirements: 6.8, 6.9, 6.10, 6.11, 6.12_
  
  - [x] 17.4 Write integration tests for copy operation
    - Test single file copy
    - Test multiple file copy
    - Test copy with overwrite
    - Test copy cancellation
    - _Requirements: 6.1-6.12_

- [x] 18. Implement move operation
  - [x] 18.1 Implement move confirmation dialog
    - Show source files and destination
    - Request user confirmation
    - _Requirements: 7.5_
  
  - [x] 18.2 Implement move job creation
    - Use marked files if any, otherwise cursor file
    - Use opposite pane location as destination
    - Create JobSpec with JobKind::Move
    - Enqueue to JobManager
    - _Requirements: 7.1, 7.2, 7.3, 7.6, 7.7_

  - [x] 18.3 Implement move execution in LocalFilesystemBackend
    - Move files with progress reporting
    - Check cancellation token periodically
    - Handle overwrite confirmation
    - Refresh both panes on completion
    - _Requirements: 7.8, 7.9, 7.10, 7.11, 7.12_
  
  - [x] 18.4 Write integration tests for move operation
    - Test single file move
    - Test multiple file move
    - Test move with overwrite
    - Test move cancellation
    - _Requirements: 7.1-7.12_

- [x] 19. Implement delete operation
  - [x] 19.1 Implement delete confirmation dialog
    - Show count of files to be deleted
    - Request user confirmation
    - _Requirements: 8.4_
  
  - [x] 19.2 Implement delete job creation
    - Use marked files if any, otherwise cursor file
    - Create JobSpec with JobKind::Delete
    - Enqueue to JobManager
    - _Requirements: 8.1, 8.2, 8.3, 8.5, 8.6_
  
  - [x] 19.3 Implement delete execution in LocalFilesystemBackend
    - Delete files with progress reporting
    - Check cancellation token periodically
    - Refresh active pane on completion
    - Unmark deleted files
    - _Requirements: 8.7, 8.8, 8.9, 8.10, 8.11_
  
  - [x] 19.4 Write integration tests for delete operation
    - Test single file delete
    - Test multiple file delete
    - Test delete cancellation
    - Test unmarking after delete
    - _Requirements: 8.1-8.11_

- [x] 20. Implement rename and mkdir operations
  - [x] 20.1 Implement rename dialog
    - Show input field pre-filled with current name
    - Validate new name
    - _Requirements: 9.1, 9.2_
  
  - [x] 20.2 Implement rename job creation and execution
    - Create JobSpec with JobKind::Rename
    - Execute rename in LocalFilesystemBackend
    - Handle name conflicts and invalid characters
    - Refresh active pane on completion
    - _Requirements: 9.3, 9.4, 9.5, 9.6, 9.7, 9.8, 9.9_
  
  - [x] 20.3 Implement mkdir dialog
    - Show input field for new directory name
    - Validate directory name
    - _Requirements: 10.1, 10.2_
  
  - [x] 20.4 Implement mkdir job creation and execution
    - Create JobSpec with JobKind::Mkdir
    - Execute mkdir in LocalFilesystemBackend
    - Handle name conflicts and invalid characters
    - Refresh active pane on completion
    - _Requirements: 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 10.9_
  
  - [x] 20.5 Write integration tests for rename and mkdir
    - Test rename with valid name
    - Test rename with conflict
    - Test mkdir with valid name
    - Test mkdir with conflict
    - _Requirements: 9.1-9.9, 10.1-10.9_

- [x] 21. Implement error handling
  - [x] 21.1 Implement error dialog rendering
    - Display descriptive error messages
    - Distinguish permission errors
    - Show error details from JobResult
    - _Requirements: 19.1, 19.2, 19.3, 19.5_
  
  - [x] 21.2 Implement error logging
    - Set up tracing subscriber
    - Log all errors to file
    - Support configurable log levels
    - Implement log rotation at 10MB
    - _Requirements: 19.4, 38.3, 38.4_
  
  - [x] 21.3 Write integration tests for error handling
    - Test permission errors
    - Test file not found errors
    - Test invalid path errors
    - _Requirements: 19.1-19.5_

- [x] 22. Checkpoint - File operations complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 5: Advanced Features (Weeks 9-10)

- [x] 23. Implement tab management
  - [x] 23.1 Implement tab creation and closure
    - Create new tab with current working directory
    - Prevent closing last tab
    - Adjust active index on closure
    - _Requirements: 27.2, 27.3, 27.4_
  
  - [x] 23.2 Implement tab switching
    - Switch to next/previous tab
    - Wrap around at boundaries
    - _Requirements: 27.5, 27.6_
  
  - [x] 23.3 Implement tab selector dialog
    - Display all tabs with filtering
    - Show tab names and locations
    - _Requirements: 27.7_
  
  - [x] 23.4 Implement tab state persistence
    - Save tab states to session storage
    - Restore tabs on application start
    - _Requirements: 27.12, 27.13_
  
  - [x] 23.5 Write integration tests for tab management
    - Test tab creation and closure
    - Test tab switching
    - Test tab persistence
    - _Requirements: 27.1-27.14_

- [x] 24. Implement custom functions with macro expansion
  - [x] 24.1 Implement CustomFunction struct
    - Load from custom_functions.json
    - Include command, macros, shell, pipe_to_action fields
    - _Requirements: 28.1_
  
  - [x] 24.2 Implement macro expansion
    - Support $P, $O, $L, $R, $F, $W, $E, $M, $*, $I, $V, $~, $# macros
    - Expand environment variables
    - Handle user input prompts for $I
    - _Requirements: 28.2, 28.5, 28.11_
  
  - [x] 24.3 Write property test for macro expansion
    - **Property 24: Macro Expansion Consistency**
    - **Validates: Requirements 28.2**

  - [x] 24.4 Implement custom function execution
    - Create JobSpec with JobKind::ExecuteCustomFunction
    - Execute on worker pool
    - Support per-function shell configuration
    - Support per-OS shell configuration
    - _Requirements: 28.8, 28.9, 28.12_
  
  - [x] 24.5 Write property test for custom function job creation
    - **Property 25: Custom Function Job Creation**
    - **Validates: Requirements 28.12**
  
  - [x] 24.6 Implement PipeToAction directives
    - JumpToPath: navigate to returned path
    - ExecuteFile: execute returned file
    - ExecuteFileWithEditor: open file in editor
    - _Requirements: 28.10, 28.13, 28.14, 28.15_
  
  - [x] 24.7 Implement custom function selector dialog
    - Display functions with filtering
    - Support menu files for hierarchical organization
    - Support direct key binding
    - _Requirements: 28.3, 28.6, 28.7_
  
  - [x] 24.8 Write integration tests for custom functions
    - Test macro expansion
    - Test function execution
    - Test PipeToAction directives
    - _Requirements: 28.1-28.15_

- [x] 25. Implement archive browsing
  - [x] 25.1 Implement ArchiveHandler trait
    - Define methods: list_entries(), extract_file(), extract_all(), create_archive()
    - Support .zip format initially
    - _Requirements: 29.9_
  
  - [x] 25.2 Implement ZipArchiveHandler
    - List archive contents with metadata
    - Extract files with progress reporting
    - Create archives from file list
    - _Requirements: 29.1, 29.2, 29.5, 29.6_
  
  - [x] 25.3 Implement archive navigation
    - Enter archive on Enter key
    - Create Location::Archive
    - Navigate through nested directories
    - Exit archive on Backspace
    - _Requirements: 29.1, 29.2, 29.3, 29.4_
  
  - [x] 25.4 Write property tests for archive navigation
    - **Property 26: Archive Entry Creates Archive Location**
    - **Property 27: Archive Exit Returns to Filesystem**
    - **Validates: Requirements 29.1, 29.4**
  
  - [x] 25.5 Implement archive operations as jobs
    - Submit archive operations to worker pool
    - Display progress in task panel
    - Refresh panes on completion
    - _Requirements: 29.7, 29.8, 29.10, 29.11_
  
  - [x] 25.6 Write integration tests for archive operations
    - Test archive browsing
    - Test archive extraction
    - Test archive creation
    - _Requirements: 29.1-29.11_

- [x] 26. Implement registered folders
  - [x] 26.1 Implement RegisteredFolder struct
    - Load from registered_directory.json
    - Include name, path, environment variables
    - _Requirements: 31.6_
  
  - [x] 26.2 Implement environment variable expansion
    - Expand variables in folder paths
    - Support standard environment variables
    - _Requirements: 31.5, 31.8_
  
  - [x] 26.3 Write property test for environment variable expansion
    - **Property 29: Environment Variable Expansion Consistency**
    - **Validates: Requirements 31.8**
  
  - [x] 26.4 Implement registered folder operations
    - Register current location (Shift+B)
    - Navigate to registered folder (I/G/Shift+F)
    - Move marked files to registered folder (Shift+M)
    - _Requirements: 31.1, 31.2, 31.4_
  
  - [x] 26.5 Implement registered folder selector dialog
    - Display folders with filtering
    - Support incremental filtering
    - _Requirements: 31.2, 31.3, 31.9_
  
  - [x] 26.6 Implement persistence
    - Save registered folders to JSON
    - Load on application start
    - _Requirements: 31.6, 31.7_
  
  - [x] 26.7 Write integration tests for registered folders
    - Test folder registration
    - Test navigation to registered folder
    - Test environment variable expansion
    - _Requirements: 31.1-31.9_

- [x] 27. Checkpoint - Advanced features complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 6: Search and Filtering (Week 11)

- [x] 28. Implement search functionality
  - [x] 28.1 Implement incremental search
    - Enter search mode on / or Ctrl+F
    - Filter entries in real-time as user types
    - Display search pattern in status bar
    - _Requirements: 11.1, 11.2, 11.3, 30.1, 30.8, 30.10_
  
  - [x] 28.2 Implement search pattern matching
    - Support case-insensitive matching by default
    - Support case-sensitive mode
    - Support wildcard patterns (* and ?)
    - Support regex patterns (/pattern/ and /pattern/i)
    - _Requirements: 11.4, 11.5, 11.6, 30.2, 30.3, 30.4_
  
  - [x] 28.3 Implement combined include/exclude patterns
    - Support colon separator (include:exclude)
    - Apply both patterns to filter results
    - _Requirements: 30.5_
  
  - [x] 28.4 Implement search navigation
    - Move cursor to first match on Enter
    - Exit search mode on Escape
    - Clear search on Ctrl+K
    - _Requirements: 11.7, 11.8, 30.7_
  
  - [x] 28.5 Implement search history
    - Store recent searches
    - Limit to 50 entries
    - _Requirements: SearchModel design_

  - [x] 28.6 Implement search result highlighting
    - Highlight matching portions of file names
    - _Requirements: 30.9_
  
  - [x] 28.7 Write integration tests for search
    - Test wildcard search
    - Test regex search
    - Test case-sensitive search
    - Test combined patterns
    - _Requirements: 11.1-11.8, 30.1-30.10_

- [x] 29. Implement file filtering
  - [x] 29.1 Implement file mask dialog
    - Show input field for filter pattern
    - Display active mask in status bar
    - _Requirements: 13.1, 13.6_
  
  - [x] 29.2 Implement filter application
    - Apply wildcard patterns to file list
    - Maintain separate masks per pane
    - Clear filter when mask is empty
    - _Requirements: 13.2, 13.3, 13.4, 13.5_
  
  - [x] 29.3 Write integration tests for filtering
    - Test wildcard filtering
    - Test filter clearing
    - Test per-pane filters
    - _Requirements: 13.1-13.6_

- [x] 30. Implement migemo support (optional)
  - [x] 30.1 Add migemo dependency (optional)
    - Add migemo-rs or equivalent crate
    - _Requirements: 30.6_
  
  - [x] 30.2 Implement migemo search mode
    - Support Japanese romaji search
    - Toggle migemo mode in search
    - _Requirements: 30.6_

- [x] 31. Checkpoint - Search and filtering complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 7: Configuration and Persistence (Week 12)

- [x] 32. Implement configuration system
  - [x] 32.1 Implement AppConfig struct
    - Define display, key_bindings, file_operations, search, ui, worker_pool_size, log_level, session_persistence fields
    - Implement Default trait
    - _Requirements: 17.1, 17.6, 17.7, 17.8, 38.3, 38.5_
  
  - [x] 32.2 Implement configuration loading
    - Load from config.json at startup
    - Use default settings if file doesn't exist
    - Validate configuration
    - Display errors for invalid settings
    - _Requirements: 17.1, 17.2, 17.3, 17.9, 38.1, 38.9, 38.10_
  
  - [x] 32.3 Implement key bindings configuration
    - Load from keybindings.json
    - Support TWF-compatible defaults
    - Support custom key mappings
    - _Requirements: 17.4, 17.5, 18.1, 18.2, 18.3, 18.7_
  
  - [x] 32.4 Implement display configuration
    - Support configurable colors
    - Support CJK character width
    - Support custom color schemes
    - _Requirements: 17.6, 17.7, 32.3, 32.4, 32.5, 39.8_

  - [x] 32.5 Implement configuration reload
    - Reload config on Shift+Z
    - Apply new settings without restart
    - _Requirements: 38.2_
  
  - [x] 32.6 Write integration tests for configuration
    - Test config loading
    - Test default settings fallback
    - Test config reload
    - Test invalid config handling
    - _Requirements: 17.1-17.9, 38.1-38.10_

- [x] 33. Implement session state persistence
  - [x] 33.1 Implement SessionState struct
    - Include tab states, pane locations, marked files
    - Serialize to JSON
    - _Requirements: 38.6_
  
  - [x] 33.2 Implement session save
    - Save session state on shutdown
    - Save to session storage file
    - _Requirements: 38.6_
  
  - [x] 33.3 Implement session restore
    - Load session state on startup
    - Restore tabs, pane locations, marked files
    - _Requirements: 27.13, 38.7_
  
  - [x] 33.4 Write integration tests for session persistence
    - Test session save
    - Test session restore
    - Test marked file persistence
    - _Requirements: 38.6, 38.7_

- [x] 34. Implement directory caching
  - [x] 34.1 Implement DirectoryCache struct
    - Store cached directory contents
    - Include checksum for validation
    - Implement 30-second TTL
    - _Requirements: 22.1, 22.2, 22.3_
  
  - [x] 34.2 Implement cache validation
    - Verify directory hasn't changed via checksum
    - Invalidate on file operations
    - Re-read if changed
    - _Requirements: 22.4, 22.5, 22.6, 22.7_
  
  - [x] 34.3 Write property test for cache validation
    - **Property 21: Cache Checksum Validation**
    - **Validates: Requirements 22.5**
  
  - [x] 34.4 Write integration tests for caching
    - Test cache hit
    - Test cache miss
    - Test cache invalidation
    - _Requirements: 22.1-22.7_

- [x] 35. Checkpoint - Configuration and persistence complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 8: Polish and Testing (Weeks 13-14)

- [x] 36. Implement file viewer
  - [x] 36.1 Implement text viewer
    - Display file contents in modal view
    - Support multiple encodings (UTF-8, UTF-16, Shift-JIS, etc.)
    - Cycle through encodings on Shift+E
    - _Requirements: 33.1, 33.3, 33.4_

  - [x] 36.2 Implement hex viewer
    - Display file contents in hexadecimal
    - Show ASCII representation alongside hex
    - _Requirements: 33.2_
  
  - [x] 36.3 Implement viewer navigation
    - Home/End: line navigation
    - F5/F6: jump to top/bottom
    - F4: enter search mode
    - F3/Shift+F3: find next/previous
    - _Requirements: 33.5, 33.6, 33.7, 33.8, 33.9, 33.10, 33.11_
  
  - [x] 36.4 Implement viewer file loading as job
    - Load file contents on worker pool
    - Keep UI responsive during loading
    - _Requirements: 33.13, 33.14_
  
  - [x] 36.5 Write integration tests for viewer
    - Test text viewer
    - Test hex viewer
    - Test encoding switching
    - Test viewer navigation
    - _Requirements: 33.1-33.14_

- [x] 37. Implement pattern-based rename
  - [x] 37.1 Implement pattern rename dialog
    - Show pattern input field
    - Display preview of rename results
    - _Requirements: 34.1, 34.5_
  
  - [x] 37.2 Implement pattern syntax
    - Support wildcards and replacement tokens
    - Apply to marked files or cursor file
    - _Requirements: 34.2, 34.3, 34.4_
  
  - [x] 37.3 Implement pattern rename execution
    - Create job for batch rename
    - Execute on worker pool
    - Refresh pane on completion
    - _Requirements: 34.6, 34.7, 34.8, 34.9, 34.10_
  
  - [x] 37.4 Write integration tests for pattern rename
    - Test pattern application
    - Test preview generation
    - Test batch rename execution
    - _Requirements: 34.1-34.10_

- [x] 38. Implement file comparison and split/join
  - [x] 38.1 Implement file comparison
    - Compare cursor file with opposite pane file
    - Display differences in comparison view
    - Execute as job on worker pool
    - _Requirements: 35.1, 35.2, 35.6_
  
  - [x] 38.2 Implement file split/join dialog
    - Show split/join options
    - Configure split size
    - _Requirements: 35.3_
  
  - [x] 38.3 Implement split/join operations
    - Split large files into parts
    - Join split parts back together
    - Execute as jobs on worker pool
    - Display progress in task panel
    - _Requirements: 35.4, 35.5, 35.7, 35.8, 35.9_
  
  - [x] 38.4 Write integration tests for comparison and split/join
    - Test file comparison
    - Test file split
    - Test file join
    - _Requirements: 35.1-35.9_

- [x] 39. Implement advanced marking operations
  - [x] 39.1 Implement wildcard marking dialog
    - Show pattern input field
    - Support * and ? wildcards
    - _Requirements: 36.1, 36.2_
  
  - [x] 39.2 Implement wildcard marking execution
    - Mark all files matching pattern
    - _Requirements: 36.3_
  
  - [x] 39.3 Implement range marking mode
    - Enter mode on Ctrl+Space
    - Mark all files between initial and current cursor
    - _Requirements: 36.4, 36.5_
  
  - [x] 39.4 Implement mark inversion
    - Invert all marks on Home with Shift
    - _Requirements: 36.6_
  
  - [x] 39.5 Ensure marking persistence
    - Maintain marks across navigation
    - Display count and size in status bar
    - _Requirements: 36.7, 36.8_
  
  - [x] 39.6 Write integration tests for advanced marking
    - Test wildcard marking
    - Test range marking
    - Test mark inversion
    - _Requirements: 36.1-36.8_

- [x] 40. Implement directory size calculation
  - [x] 40.1 Implement size calculation job
    - Create job on H key press
    - Recursively traverse directory
    - Check cancellation token periodically
    - _Requirements: 37.1, 37.2, 37.4_
  
  - [x] 40.2 Implement progress reporting for size calculation
    - Report progress via JobEvent
    - Display in task panel
    - _Requirements: 37.5_
  
  - [x] 40.3 Update FileEntry with calculated size
    - Set calculated_size field on completion
    - Display in file entry
    - _Requirements: 37.6_
  
  - [x] 40.4 Support concurrent size calculations
    - Allow multiple calculations to run
    - Support cancellation
    - _Requirements: 37.7, 37.8, 37.9_
  
  - [x] 40.5 Write integration tests for size calculation
    - Test single directory calculation
    - Test concurrent calculations
    - Test cancellation
    - _Requirements: 37.1-37.9_

- [x] 41. Comprehensive property-based testing
  - [x] 41.1 Run all property tests
    - Execute all 31 property tests
    - Verify all properties hold across random inputs
    - Fix any failures discovered
    - _Requirements: All design properties_
  
  - [x] 41.2 Add additional property tests for edge cases
    - Test boundary conditions
    - Test error conditions
    - Test concurrent operations
    - _Requirements: All requirements_

- [x] 42. Integration testing
  - [x] 42.1 Write end-to-end workflow tests
    - Test complete file copy workflow
    - Test complete file move workflow
    - Test complete delete workflow
    - Test tab management workflow
    - Test custom function workflow
    - _Requirements: All requirements_
  
  - [x] 42.2 Write concurrent operation tests
    - Test multiple jobs running simultaneously
    - Test job cancellation during concurrent operations
    - Test UI responsiveness during heavy load
    - _Requirements: 15.11, 15.12, 21.1-21.8_
  
  - [x] 42.3 Write error recovery tests
    - Test recovery from file operation failures
    - Test recovery from invalid configuration
    - Test recovery from corrupted session state
    - _Requirements: 19.1-19.5_

- [x] 43. Performance optimization
  - [x] 43.1 Profile UI rendering performance
    - Ensure 30+ FPS rendering
    - Ensure <16ms input processing
    - Optimize hot paths
    - _Requirements: 21.3, 21.4, 39.9, 39.10_
  
  - [x] 43.2 Optimize directory reading
    - Minimize filesystem calls
    - Optimize cache usage
    - Batch metadata reads
    - _Requirements: 22.1-22.7_
  
  - [x] 43.3 Optimize job scheduling
    - Minimize queue overhead
    - Optimize FIFO queue operations
    - Reduce lock contention
    - _Requirements: 15.11, 15.12_

- [x] 44. Documentation
  - [x] 44.1 Write user documentation
    - Document all key bindings
    - Document configuration options
    - Document custom function syntax
    - Provide usage examples
    - _Requirements: 18.4_
  
  - [x] 44.2 Write developer documentation
    - Document architecture and design patterns
    - Document state management flow
    - Document job system integration
    - Document extension points
    - _Requirements: 26.1-26.10_
  
  - [x] 44.3 Write API documentation
    - Document all public APIs
    - Document trait implementations
    - Document configuration schema
    - _Requirements: All requirements_

- [x] 45. Bug fixes and refinements
  - [x] 45.1 Fix any remaining bugs
    - Address issues found during testing
    - Fix edge cases
    - Improve error messages
    - _Requirements: All requirements_
  
  - [x] 45.2 Polish UI and UX
    - Improve visual feedback
    - Refine dialog layouts
    - Optimize color schemes
    - _Requirements: 23.1-23.5, 39.1-39.10_
  
  - [x] 45.3 Final testing pass
    - Manual testing of all features
    - Verify all requirements met
    - Test on multiple platforms
    - _Requirements: All requirements_

- [ ] 46. Final checkpoint - Implementation complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 8 (Continued): Additional Features

- [ ] 47. Implement pane synchronization and swapping
  - [ ] 47.1 Implement SyncPanes transition
    - Navigate opposite pane to active pane's location
    - Create job to read directory
    - _Requirements: 41.1, 41.2, 41.6_
  
  - [ ] 47.2 Implement SwapPanes transition
    - Exchange current_location of both panes
    - Maintain cursor positions and marked files
    - Create jobs to refresh both panes
    - _Requirements: 41.3, 41.4, 41.5, 41.6_
  
  - [ ] 47.3 Add key bindings for pane operations
    - O: SyncPanes
    - Shift+O: SwapPanes
    - _Requirements: 41.1, 41.3_
  
  - [ ] 47.4 Write integration tests for pane operations
    - Test pane synchronization
    - Test pane swapping
    - Test marked file preservation during swap
    - _Requirements: 41.1-41.7_

- [ ] 48. Implement context menu and drive selection
  - [ ] 48.1 Implement context menu dialog
    - Display common file operations
    - Include copy, move, delete, rename, view, custom functions
    - _Requirements: 42.1, 42.2_
  
  - [ ] 48.2 Implement drive selection dialog
    - List all available drives and network shares
    - Display drive information (size, free space, type)
    - _Requirements: 42.3, 42.4, 42.6_
  
  - [ ] 48.3 Implement drive navigation
    - Navigate to selected drive or share
    - Support quick drive switching
    - _Requirements: 42.5, 42.7_
  
  - [ ] 48.4 Add key bindings for dialogs
    - \ or backtick: ShowContextMenu
    - Shift+L: ShowDriveChangeDialog
    - _Requirements: 42.1, 42.3_
  
  - [ ] 48.5 Write integration tests for context menu and drive selection
    - Test context menu display
    - Test drive selection
    - Test drive navigation
    - _Requirements: 42.1-42.7_

- [ ] 49. Implement file information and version display
  - [ ] 49.1 Implement file information dialog
    - Display file name, path, size, dates, attributes
    - Display permissions and ownership
    - _Requirements: 43.1, 43.2, 43.3_
  
  - [ ] 49.2 Implement version dialog
    - Display version number, build date, copyright
    - _Requirements: 43.4, 43.5_
  
  - [ ] 49.3 Add key bindings for information dialogs
    - Shift+I: ShowFileInfo
    - Configured key: ShowVersion
    - _Requirements: 43.1, 43.4_
  
  - [ ] 49.4 Implement dialog dismissal
    - Support Escape and Enter to close
    - _Requirements: 43.6_
  
  - [ ] 49.5 Write integration tests for information dialogs
    - Test file info display
    - Test version display
    - _Requirements: 43.1-43.6_

- [ ] 50. Implement log management
  - [ ] 50.1 Implement log saving
    - Save current session log to file
    - Include timestamps for all entries
    - _Requirements: 44.1, 44.2, 44.3_
  
  - [ ] 50.2 Implement log memory management
    - Support configurable max lines in memory
    - Flush to file when limit reached
    - _Requirements: 44.4_
  
  - [ ] 50.3 Implement log on exit
    - Optionally save log based on SaveLogOnExit config
    - Support log file rotation
    - _Requirements: 44.5, 44.6_
  
  - [ ] 50.4 Implement slow operation logging
    - Log file operations exceeding threshold
    - Default threshold: 5000ms
    - _Requirements: 44.7_
  
  - [ ] 50.5 Add key binding for save log
    - Configured key: SaveLog
    - _Requirements: 44.1_
  
  - [ ] 50.6 Write integration tests for log management
    - Test log saving
    - Test log rotation
    - Test slow operation logging
    - _Requirements: 44.1-44.7_

- [ ] 51. Implement configuration program launch
  - [ ] 51.1 Implement editor launch
    - Launch configured editor with config file
    - Support configurable editor command
    - _Requirements: 45.1, 45.2_
  
  - [ ] 51.2 Implement reload prompt
    - Prompt user to reload after editor closes
    - Reload configuration if confirmed
    - _Requirements: 45.3, 45.4_
  
  - [ ] 51.3 Implement configuration validation
    - Validate config after reload
    - Display errors if invalid
    - Fall back to previous config if invalid
    - _Requirements: 45.5, 45.6_
  
  - [ ] 51.4 Add key binding for config launch
    - Y: LaunchConfigurationProgram
    - _Requirements: 45.1_
  
  - [ ] 51.5 Write integration tests for config launch
    - Test editor launch
    - Test reload prompt
    - Test validation and fallback
    - _Requirements: 45.1-45.6_

- [ ] 52. Implement exit and change directory
  - [ ] 52.1 Implement exit with directory output
    - Output current active pane directory on exit
    - Support -cwd command-line flag
    - _Requirements: 46.1, 46.3, 46.4_
  
  - [ ] 52.2 Create wrapper scripts
    - Provide bash wrapper script
    - Provide zsh wrapper script
    - Provide PowerShell wrapper script
    - _Requirements: 46.2, 46.5_
  
  - [ ] 52.3 Implement directory capture in wrappers
    - Capture stdout directory from application
    - Change shell working directory after exit
    - _Requirements: 46.6_
  
  - [ ] 52.4 Add key binding for exit with cd
    - Shift+Q: ExitAndChangeDirectory
    - _Requirements: 46.1_
  
  - [ ] 52.5 Write integration tests for exit and cd
    - Test directory output
    - Test -cwd flag
    - Test wrapper script functionality
    - _Requirements: 46.1-46.6_

- [ ] 53. Implement task panel management
  - [ ] 53.1 Implement task panel toggle
    - Toggle visibility with configured key
    - Persist visibility setting
    - _Requirements: 47.1, 47.6_
  
  - [ ] 53.2 Implement task panel resizing
    - Ctrl+Up: increase height
    - Ctrl+Down: decrease height
    - Persist size setting
    - _Requirements: 47.2, 47.3, 47.6_
  
  - [ ] 53.3 Implement task panel scrolling
    - Alt+Up: scroll up
    - Alt+Down: scroll down
    - Display scrollbar when needed
    - _Requirements: 47.4, 47.5, 47.7_
  
  - [ ] 53.4 Write integration tests for task panel management
    - Test toggle visibility
    - Test resizing
    - Test scrolling
    - _Requirements: 47.1-47.7_

- [ ] 54. Implement multi-language help system
  - [ ] 54.1 Implement help content loading
    - Load from language-specific JSON files
    - Support help.{lang}.json format
    - _Requirements: 48.1_
  
  - [ ] 54.2 Implement help dialog
    - Display help in configured language
    - Show all key bindings with descriptions
    - _Requirements: 48.2, 48.5_
  
  - [ ] 54.3 Implement language rotation
    - L key: rotate through available languages
    - Persist selected language
    - _Requirements: 48.3, 48.6_
  
  - [ ] 54.4 Implement language fallback
    - Fall back to English if language file not found
    - Support multiple languages (en, jp)
    - _Requirements: 48.4, 48.7_
  
  - [ ] 54.5 Add key bindings for help
    - ? or F1: show help dialog
    - _Requirements: 48.2_
  
  - [ ] 54.6 Write integration tests for multi-language help
    - Test help loading
    - Test language rotation
    - Test fallback to English
    - _Requirements: 48.1-48.7_

- [ ] 55. Final comprehensive testing
  - [ ] 55.1 Test all new features
    - Test pane sync and swap
    - Test context menu and drive selection
    - Test file info and version display
    - Test log management
    - Test config launch
    - Test exit and cd
    - Test task panel management
    - Test multi-language help
    - _Requirements: 41.1-48.7_
  
  - [ ] 55.2 Integration testing for new features
    - Test feature interactions
    - Test error handling
    - Test performance impact
    - _Requirements: All new requirements_

- [ ] 56. Final checkpoint - All features complete
  - Ensure all tests pass, verify all 48 requirements are met, ask the user if questions arise.

- [ ] 57. Implement Requirement 2A: File Pane Scrolling Behavior
  - [x] 57.1 Add scroll_offset to UIConfig struct
    - Add scroll_offset field with default value of 3
    - Update UIConfig::default() implementation
    - _Requirements: 2A.4, 38.11_
  
  - [ ] 57.2 Implement calculate_scroll_position function
    - Create ScrollContext struct with visible_height, total_entries, cursor_position, scroll_offset, config_offset
    - Implement scrolling algorithm that prevents blank lines at bottom
    - Trigger scrolling when cursor is within scroll_offset lines from top/bottom
    - _Requirements: 2A.1, 2A.2, 2A.3, 2A.5, 2A.6, 2A.7_
  
  - [x] 57.3 Update CursorMove transition in state.rs
    - Use calculate_scroll_position to update scroll_offset
    - Pass visible_height from layout state
    - Pass config_offset from UIConfig
    - _Requirements: 2A.2, 2A.3, 2A.5_
  
  - [ ] 57.4 Update ChangeLocation transition to reset scroll_offset
    - Set scroll_offset to 0 when location changes
    - _Requirements: 2A.7_
  
  - [ ] 57.5 Write integration tests for scrolling behavior
    - Test scrolling triggers at correct offset
    - Test no blank lines at bottom
    - Test cursor visibility maintained
    - _Requirements: 2A.1-2A.7_

- [ ] 58. Implement Requirement 39A: Volume Name Display in Top Separator
  - [x] 58.1 Create volume info data structures
    - Create VolumeInfo struct with display_name and volume_type
    - Create VolumeType enum (Local, Network, Removable, Unknown)
    - Create MarkedFileStats struct with dir_count, file_count, total_size
    - _Requirements: 39A.1_
  
  - [x] 58.2 Implement get_drive_or_share_name function
    - Implement platform-specific logic for Windows, Linux, MacOS
    - Windows: Extract drive letter and volume label, handle network paths (\\server\share)
    - Linux/MacOS: Read mount points, extract device and volume label
    - _Requirements: 39A.2, 39A.3, 39A.4, 39A.5, 39A.6, 39A.7, 39A.8_
  
  - [x] 58.3 Implement calculate_marked_stats function
    - Count marked directories and files separately
    - Calculate total size of marked files
    - _Requirements: 39A.9, 39A.10, 39A.11, 39A.12_
  
  - [x] 58.4 Implement format_top_separator_info function
    - Format marked stats as "{count} {Dirs/Files} {size} marked"
    - Handle cases with only dirs, only files, or both
    - Combine volume name with marked stats
    - _Requirements: 39A.9, 39A.10, 39A.11, 39A.12, 39A.13_
  
  - [x] 58.5 Update render_top_separator in top_separator.rs
    - Call get_drive_or_share_name for both panes
    - Call calculate_marked_stats for both panes
    - Call format_top_separator_info for both panes
    - Use TopSeparatorForegroundColor and TopSeparatorBackgroundColor
    - _Requirements: 39A.1, 39A.13, 39A.14_
  
  - [ ] 58.6 Write integration tests for volume name display
    - Test Windows drive letter display
    - Test network path display
    - Test Linux/MacOS mount point display
    - Test marked file statistics formatting
    - _Requirements: 39A.1-39A.14_

- [ ] 59. Implement Requirement 49: Color Configuration Mapping
  - [x] 59.1 Update ColorScheme struct with new color properties
    - Add file_pane_cursor_foreground_color and file_pane_cursor_background_color (UI area 4)
    - Add inactive_file_pane_cursor_foreground_color and inactive_file_pane_cursor_background_color (UI area 4)
    - Add pane_info_foreground_color and pane_info_background_color (UI area 5)
    - Add filename_label_foreground_color and filename_label_background_color (UI area 6)
    - Keep old properties for backward compatibility (highlight_foreground_color, highlight_background_color)
    - _Requirements: 49.4, 49.5, 49.6, 49.7_
  
  - [x] 59.2 Implement backward compatibility layer in ColorScheme
    - Add get_file_pane_cursor_foreground() method that falls back to highlight_foreground_color
    - Add get_file_pane_cursor_background() method that falls back to highlight_background_color
    - Add get_inactive_file_pane_cursor_foreground() method with fallback
    - Add get_inactive_file_pane_cursor_background() method with fallback
    - Add get_pane_info_foreground() method with fallback to top_separator_foreground_color
    - Add get_pane_info_background() method with fallback to top_separator_background_color
    - _Requirements: 49.9, 49.10_
  
  - [ ] 59.3 Update render_panes to use correct colors for active pane
    - Use file_pane_cursor_foreground_color and file_pane_cursor_background_color for cursor
    - Use foreground_color and background_color for regular files
    - Use marked_file_color for marked files
    - Use directory_color and directory_background_color for directories
    - _Requirements: 49.4_
  
  - [ ] 59.4 Update render_panes to use correct colors for inactive pane
    - Use inactive_file_pane_cursor_foreground_color and inactive_file_pane_cursor_background_color for cursor
    - Use inactive_foreground_color and inactive_background_color for regular files
    - Use inactive_directory_color and inactive_directory_background_color for directories
    - _Requirements: 49.5_
  
  - [ ] 59.5 Create pane_info_line.rs component (if not exists)
    - Render pane info bar with pane_info_foreground_color and pane_info_background_color
    - Display directory/file counts and total size
    - _Requirements: 49.6_
  
  - [ ] 59.6 Create filename_label.rs component (if not exists)
    - Render selected filename with filename_label_foreground_color and filename_label_background_color
    - _Requirements: 49.7_
  
  - [ ] 59.7 Update tab_bar.rs to use correct colors
    - Use active_tab_foreground_color and active_tab_background_color for active tab
    - Use inactive_tab_foreground_color and inactive_tab_background_color for inactive tabs
    - Use tabbar_background_color for tab bar background
    - _Requirements: 49.1_
  
  - [ ] 59.8 Update path display rendering to use correct colors
    - Use foreground_color and background_color for path display
    - _Requirements: 49.2_
  
  - [ ] 59.9 Update top_separator.rs to use correct colors
    - Use top_separator_foreground_color and top_separator_background_color
    - _Requirements: 49.3_
  
  - [ ] 59.10 Update task_panel.rs to use correct colors
    - Use foreground_color and background_color for task view
    - _Requirements: 49.8_
  
  - [ ] 59.11 Write integration tests for color configuration
    - Test all UI areas use correct colors
    - Test backward compatibility with old color names
    - Test color fallback behavior
    - _Requirements: 49.1-49.10_

- [ ] 60. Major UI restructuring to match TWF layout
  - [ ] 60.1 Fix scrolling logic in state.rs CursorMove transition
    - Only scroll if entries.len() > visible_height
    - When at bottom, scroll_offset should be max(0, entries.len() - visible_height)
    - No blank lines below last entry
    - _Requirements: 2.8, 21.4_
  
  - [ ] 60.2 Restructure main UI layout in ui.rs
    - New layout: Tabs (1 line) → Path line (1 line) → Volume name line (1 line) → File panes (Min 10) → Pane info line (1 line) → Selected filename line (1 line) → Task view (5 lines, no border)
    - Remove borders from file panes
    - Remove borders from task panel
    - _Requirements: 1.1, 1.2, 16.1-16.7_
  
  - [ ] 60.3 Create path_line.rs component
    - Display left and right pane paths side by side
    - Show ">" indicator for active pane
    - White text on blue background
    - _Requirements: 16.1_
  
  - [ ] 60.4 Create volume_line.rs component
    - Display volume names for both panes
    - Show drive letters on Windows
    - Cyan text on dark gray background
    - _Requirements: 39.1, 39.2_
  
  - [ ] 60.5 Create pane_info_line.rs component
    - Display directory/file counts for both panes
    - Show total size of files
    - White text on dark gray background
    - _Requirements: 16.2, 16.3, 16.4_
  
  - [ ] 60.6 Update panes.rs to remove borders
    - Remove Block wrapper
    - Add "*" selection indicator instead of highlighting
    - Render list items directly without borders
    - _Requirements: 1.1, 1.4, 4.6_
  
  - [ ] 60.7 Update task_panel.rs to remove border
    - Render task list directly without border
    - _Requirements: 15.2, 15.3_
  
  - [ ] 60.8 Delete obsolete UI components
    - Delete top_separator.rs (replaced by path_line and volume_line)
    - Delete status_bar.rs (replaced by pane_info_line)
    - _Requirements: N/A_
  
  - [ ] 60.9 Update ui.rs module exports
    - Export new components (path_line, volume_line, pane_info_line)
    - Remove obsolete exports (top_separator, status_bar)
    - _Requirements: N/A_
  
  - [ ] 60.10 Test UI restructuring
    - Verify scrolling works correctly without blank lines
    - Verify all UI components render properly
    - Verify layout matches TWF exactly
    - _Requirements: 1.1-1.6, 2.8, 16.1-16.7_


## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at phase boundaries
- Property tests validate universal correctness properties from design document
- Unit tests and integration tests validate specific examples and workflows
- All file I/O operations must execute as Jobs on Worker Pool (never block UI thread)
- State transitions must flow through Transition enum and update_state function
- Jobs must follow FIFO ordering and support cooperative cancellation

## Property Test Summary

The following 31 correctness properties are tested throughout implementation:

1. Property 1: Pane Independence (Task 15.4)
2. Property 2: Scroll Independence (Task 15.4)
3. Property 3: Pane Switching Toggles (Task 15.4)
4. Property 4: Cursor Bounds Invariant (Task 2.5)
5. Property 5: Cursor Visibility Invariant (Task 15.4)
6. Property 6: Directory Navigation Creates Job (Task 15.4)
7. Property 7: Parent Navigation (Task 2.2)
8. Property 8: Location Change Resets Cursor (Task 15.4)
9. Property 9: Navigation History Preservation (Task 4.6)
10. Property 10: Mark Toggle Idempotence (Task 4.2)
11. Property 11: Mark All Completeness (Task 4.2)
12. Property 12: Unmark All Completeness (Task 4.2)
13. Property 13: Marking Persistence Across Navigation (Task 4.2)
14. Property 14: Copy Operation Job Creation (Task 15.6)
15. Property 15: Move Operation Job Creation (Task 15.6)
16. Property 16: Delete Operation Job Creation (Task 15.6)
17. Property 17: Delete Completion Unmarks Files (Task 15.6)
18. Property 18: Directory-First Sorting (Task 15.9)
19. Property 19: Sort Stability (Task 15.9)
20. Property 20: FIFO Job Ordering (Task 7.4)
21. Property 21: Cache Checksum Validation (Task 34.3)
22. Property 22: State Transition Determinism (Task 3.5)
23. Property 23: Tab Independence (Task 2.7)
24. Property 24: Macro Expansion Consistency (Task 24.3)
25. Property 25: Custom Function Job Creation (Task 24.5)
26. Property 26: Archive Entry Creates Archive Location (Task 25.4)
27. Property 27: Archive Exit Returns to Filesystem (Task 25.4)
28. Property 28: Search Pattern Matching (Task 4.4)
29. Property 29: Environment Variable Expansion Consistency (Task 26.3)
30. Property 30: Wildcard Marking Completeness (Task 15.7)
31. Property 31: Size Calculation Updates Entry (Task 9.4)
