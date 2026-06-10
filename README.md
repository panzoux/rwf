# RWF: Reactive Worker Filemanager

A high-performance, cross-platform terminal file manager built in Rust, following the Reactive Worker Framework (RWF) pattern.

## Overview

RWF is a modern, two-pane terminal file manager designed for efficiency, reliability, and speed. It provides a non-blocking user interface where file operations run asynchronously in the background, ensuring the UI remains responsive even during heavy tasks like copying large directories or searching through millions of files.

Inspired by the TWF (Two-pane File manager for Windows) philosophy, RWF brings advanced file management capabilities to the terminal with full CJK (Chinese, Japanese, Korean) character support and cross-platform compatibility (Windows, Linux, macOS).

## Where it is useful

- **Keyboard-centric workflows**: Perfect for users who prefer the terminal and want to minimize mouse usage.
- **Remote Servers**: Lightweight and efficient for managing files over SSH (where terminal-based UIs shine).
- **Developers & Power Users**: Highly customizable with custom functions, macros, and powerful search capabilities.
- **Cross-Platform Environments**: Consistent experience across different operating systems.
- **Multitasking**: Handle multiple background file operations simultaneously without freezing the interface.

## Key Features

- **Dual-Pane Interface**: Side-by-side view for intuitive file comparison, copying, and moving.
- **Tab Management**: Open multiple tabs, each maintaining its own pane states and independent navigation history.
- **Non-blocking Operations**: All I/O operations (copy, move, delete, archive, etc.) run as background jobs with real-time progress tracking.
- **Advanced Search & Filtering**:
  - Incremental search with wildcard and Regex support.
  - **Migemo** support for efficient Japanese text searching.
  - File mask filtering to quickly isolate specific file types.
- **Integrated Viewers**:
  - High-performance text viewer with support for multiple encodings (UTF-8, Shift-JIS, EUC-JP, etc.).
  - Hex/Binary viewer for low-level file inspection.
  - **Side-by-Side Viewer Mode**: Compare files while navigating or viewing.
- **Archive Support**: Browse and extract archives as if they were local folders (supports `.zip`, `.7z`, `.tar`, `.tgz`, `.iso`).
- **Customization**:
  - Fully configurable keybindings (`keybindings.json`).
  - Custom functions with powerful macro expansion (`$P`, `$F`, `$M`, etc.) for shell integration.
  - Registered folders for quick jumping to frequent locations.
- **Robust State Management**: Built on pure state logic and explicit transitions, ensuring predictable behavior and easier debugging.

## Development Status

RWF is currently in its **early stages** (Alpha) but is already highly functional.
- **Current Completion**: Approximately 75% of the planned feature set is implemented.
- **Phase 6 Completed**: Achieved feature parity with the original TWF C# prototype.
- **Phase 7 Underway**: Focused on RWF-specific enhancements and user experience refinements.

## Testing & Stability

Quality and reliability are top priorities.
- **Extensive Test Suite**: Over 800 tests covering unit logic, property-based state transitions, and integration scenarios.
- **Property-Based Testing**: Utilizes `proptest` to verify state consistency across complex sequences of operations.
- **Continuous Validation**: Actively tested on Windows, Linux, and macOS.

*Note: As this is an early-stage project, we recommend cautious use with critical data.*

## Future Roadmap

RWF is under active development. Planned enhancements focus on improving the core user experience and expanding platform-native integrations:

- **Leap Navigation**: Faster "quick-filter" mode for instant file jumping.
- **Smart File Opener (Rifle)**: Advanced MIME-based file associations with conditional logic.
- **Recursive Directory Size Calculation**: Non-blocking background size analysis.
- **Syntax Highlighting**: Code highlighting for the text viewer.
- **Smart Trash Support**: Integration with OS-native trash/recycle bin.

See [plan/ROADMAP.md](plan/ROADMAP.md) for the detailed development plan.

## Quick Start

### Prerequisites

- Rust (latest stable version)
- A terminal with Unicode/TrueColor support

### Building & Running

```bash
# Clone the repository
git clone https://github.com/panzoux/rwf.git
cd rwf

# Build the project
cargo build --release

# Run the application
./target/release/rwf
```

## Configuration

Configuration files are automatically created on the first launch:
- **Windows**: `%APPDATA%\rwf\`
- **Linux/macOS**: `~/.config/rwf/`

For detailed configuration options and keybindings, see the [User Guide](docs/USER_GUIDE.md).

## Technical Architecture

RWF is built with a focus on performance and maintainability:

1. **Reactive Worker Pattern**: Separation of UI thread and worker pool.
2. **Pure State Logic**: All state changes occur through explicit transitions.
3. **Rust Ecosystem**: Powered by `tokio` (async), `ratatui` (TUI), and `serde` (serialization).

For more details on the architecture and contributing, see the [Developer Guide](docs/DEVELOPER_GUIDE.md).

---

## Documentation Links

- [User Guide](docs/USER_GUIDE.md)
- [Developer Guide](docs/DEVELOPER_GUIDE.md)
- [API Reference](docs/API_REFERENCE.md)
- [Unicode Handling Guide](docs/UNICODE_HANDLING_GUIDE.md)

---
