# Two-Pane File Manager

A cross-platform, two-pane file manager built in Rust using the Reactive Worker Framework (rwf) pattern.

## Project Structure

This is a Cargo workspace with two main crates:

- **rwf-bin**: Main binary application
- **rwf-lib**: Core library with business logic

### Architecture

The application follows these core principles:

1. **Never Block the UI Thread**: All file I/O operations execute as Jobs in the rwf Worker Pool
2. **Explicit State Transitions**: All state changes occur through the Transition enum
3. **Pure State Logic**: State transformations are pure functions returning StateUpdateResult
4. **Event-Driven Architecture**: JobEvents flow from Worker Pool to UI thread via channels
5. **FIFO Job Ordering**: Strict first-in-first-out job execution
6. **Cooperative Cancellation**: Jobs check cancellation tokens periodically

### Key Components

- **AppState**: Central application state coordinating all components
- **Transition**: Explicit state change operations
- **JobManager**: Manages background file operations via rwf Worker Pool
- **FilesystemBackend**: Abstraction for file I/O operations
- **TabManager**: Manages multiple tabs with independent pane states
- **PaneModel**: Represents the state of a single pane

## Dependencies

### Runtime Dependencies

- **tokio**: Async runtime
- **ratatui**: Terminal UI framework
- **crossterm**: Cross-platform terminal manipulation
- **serde/serde_json**: Serialization
- **thiserror/anyhow**: Error handling
- **regex**: Pattern matching
- **tracing**: Logging and diagnostics

### Development Dependencies

- **proptest**: Property-based testing
- **tempfile**: Temporary file utilities for tests
- **assert_fs**: Filesystem assertions for tests
- **predicates**: Predicate assertions for tests

## Building

```bash
# Check the project
cargo check --workspace

# Build the project
cargo build --workspace

# Run tests
cargo test --workspace

# Run the application
cargo run --bin two-pane-fm
```

## Configuration

The application uses:
- `keybindings.json` for configurable key mappings (TWF-compatible defaults)
- `config.json` for application settings
- Default worker pool size: 4 threads

## Development Status

This project is currently in Phase 1: Core Infrastructure setup.

See `.kiro/specs/two-pane-file-manager/tasks.md` for the complete implementation plan.
