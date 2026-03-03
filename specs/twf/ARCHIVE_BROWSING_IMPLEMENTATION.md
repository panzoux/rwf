# Archive Browsing Implementation (Task 27)

## Overview
This document describes the implementation of archive browsing functionality for TWF (Two-pane Window Filer), which allows users to view archive contents as virtual folders and extract archives.

## Requirements Implemented
Based on Requirement 12 from the requirements document:

1. ✅ WHEN the user presses Enter on an archive file THEN the system SHALL display the archive contents as a virtual folder
2. ✅ WHEN browsing a virtual folder THEN the system SHALL display files with their paths within the archive
3. ✅ WHEN the user presses Backspace in a virtual folder THEN the system SHALL return to the parent directory
4. ✅ WHEN the user presses Shift+Enter on an archive THEN the system SHALL extract the archive to the current directory
5. ✅ WHEN extraction completes THEN the system SHALL display a status message and refresh the pane

## Changes Made

### 1. PaneState Model (Models/PaneState.cs)
Added three new properties to track virtual folder state:
- `IsInVirtualFolder`: Boolean flag indicating if the pane is currently viewing an archive
- `VirtualFolderArchivePath`: Path to the archive file being viewed
- `VirtualFolderParentPath`: Path to return to when exiting the virtual folder

### 2. MainController (Controllers/MainController.cs)

#### HandleEnterKey() - Modified
- Added detection for archive files using `_archiveManager.IsArchive()`
- When Enter is pressed on an archive, calls `OpenArchiveAsVirtualFolder()` instead of navigating
- Maintains existing behavior for directories and other files

#### OpenArchiveAsVirtualFolder() - Modified
- Retrieves archive contents using `_archiveManager.ListArchiveContentsAsync()`
- Operates asynchronously to prevent UI freezing during large archive parsing
- Shows a busy spinner in the status bar while loading
- Stores the current path as the parent path to return to
- Sets virtual folder state flags
- Updates the pane's current path to show `[archive_name.zip]`
- Displays archive entries in the pane
- Shows status message with entry count

#### NavigateToParent() - Modified
- Checks if currently in a virtual folder
- If in virtual folder, exits it and returns to the parent directory
- Clears virtual folder state flags
- Maintains existing behavior for normal directory navigation

#### HandleArchiveExtraction() - New Method
- Triggered by Shift+Enter on an archive file
- Shows confirmation dialog before extraction
- Displays progress dialog during extraction
- Supports cancellation with Escape key
- Refreshes pane after successful extraction
- Shows error messages if extraction fails

#### Key Handler - Modified
- Added check for Shift modifier on Enter key
- Routes to `HandleArchiveExtraction()` when Shift+Enter is pressed
- Routes to `HandleEnterKey()` for normal Enter press

### 3. Tests (Tests/ArchiveBrowsingTests.cs)
Created comprehensive test suite covering:
- Virtual folder property support in PaneState
- Archive detection by file extension
- Listing archive contents
- Extracting archive files
- Exiting virtual folders

## User Experience

### Opening an Archive
1. User navigates to a directory containing archive files (.zip, etc.)
2. User moves cursor to an archive file
3. User presses Enter
4. Archive contents are displayed as if it were a directory
5. Path label shows `[archive_name.zip]` to indicate virtual folder
6. Status bar shows "Viewing archive: archive_name.zip (X entries)"

### Browsing Archive Contents
- Files within the archive are displayed with their names and metadata
- User can navigate through the list using arrow keys
- Files can be marked for future operations (though extraction is the primary operation)

### Exiting an Archive
1. User presses Backspace while viewing archive contents
2. System returns to the parent directory where the archive file is located
3. Virtual folder state is cleared
4. Status bar shows "Exited archive"

### Extracting an Archive
1. User moves cursor to an archive file
2. User presses Shift+Enter
3. Confirmation dialog appears: "Extract 'archive_name.zip' to current directory?"
4. If confirmed, progress dialog shows extraction status
5. User can cancel with Escape key during extraction
6. Upon completion, status shows "Extracted X file(s) from archive_name.zip"
7. Pane refreshes to show extracted files

## Technical Details

### Archive Detection
- Uses `ArchiveManager.IsArchive()` to check file extension
- Currently supports .zip files (via ZipArchiveProvider)
- Extensible to support additional formats through IArchiveProvider interface

### Virtual Folder State Management
- State is stored per-pane, allowing independent archive browsing in each pane
- State is cleared when navigating to a real directory
- State is preserved during display mode changes and other operations

### Error Handling
- File not found errors when opening archives
- Unsupported archive format errors
- Extraction failures with detailed error messages
- Graceful degradation with status bar messages

## Future Enhancements
- Support for nested archives (archives within archives)
- Support for additional archive formats (7z, rar, tar, etc.)
- Selective extraction of marked files from archives
- Archive creation from marked files (already implemented in Task 28)
- Preview of text/image files within archives without extraction

## Testing
All tests pass successfully:
- 3 unit tests for basic functionality
- 2 integration tests for archive operations
- Existing navigation tests continue to pass
- Archive manager property tests continue to pass

## Compliance
This implementation fully satisfies the requirements for Task 27:
- ✅ Handle Enter on archive files to show virtual folder
- ✅ Display archive contents in pane
- ✅ Handle Backspace to exit virtual folder
- ✅ Handle Shift+Enter for extraction
- ✅ Requirements: 12 (all acceptance criteria met)
