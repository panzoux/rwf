# RWF — Claude Collaboration Guide

## Project Essence
**RWF** (Reactive Worker Filemanager) is a high-performance, non-blocking Rust TUI file manager.
- **Pattern**: Reactive State Machine. UI is a projection of `AppState`.
- **Core Rule**: No side-effects in the UI thread. All I/O occurs via `Job`s in the worker pool.
- **Mental Model**: `Input -> Transition -> State Update -> UI Projection`.

## Architectural Mandates

### 1. State Purity & Transitions
Never mutate state directly in the rendering/input layers.
- **Signals**: Transitions must signal if a UI refresh is needed (`with_ui_change()`).
- **Marking**: Marking state is **per-pane** (`PaneModel.marking`). Changes here MUST return `with_ui_change()`.

### 2. The Dialog Stack
- Dialogs are managed in a stack (`AppState.dialogs`).
- **Nesting**: When a sub-action (like a menu selection) completes, ensure you pop **all** related dialogs to avoid "ghost" dialogs or focus traps.

### 3. Serialization (PascalCase Rule)
RWF uses **PascalCase** for configuration JSON to maintain compatibility with the TWF prototype.
- **Rust Structs**: Use `#[serde(rename = "FieldName")]` or name fields in PascalCase.
- **Defaults**: Always provide `#[serde(default)]` to prevent loading failures when users have older config files.

## Technical Safeguards

### 1. Environment Abstraction: Portable vs. Native
To ensure configuration files (like `custom_functions.json`) are shareable across Windows and Linux, follow these rules:

**What to Abstract (Use RWF Macros/Env-Vars):**
- **System Paths**: Use `${VAR}` or `%VAR%` for locations like `${APPDATA}` or `${HOME}`. RWF expands these cross-platform *before* execution.
- **Standard**: Always favor **`${VAR}`** or **`$env:VAR`** for 100% reliability.
- **Macro Precedence**: RWF macros ($P, $O, $R, etc.) expand **first**. Avoid bare `$VAR` (e.g., `$RWF_LOG`) as it will be corrupted if it starts with an RWF macro letter (like `$R`).

**What to keep Native (Environment-Specific):**
- **Binary Names**: Binaries (e.g., `notepad.exe` vs `vi`) are platform-specific. Define separate custom functions if binaries differ.
- **Shell Flow**: Piping (`|`), redirection (`>`), and shell wrappers (`cmd /c`) are handled by the OS shell, not RWF. Use internal expansion for paths, but let the shell handle the "flow."

### 2. External Command Execution
Do **not** wrap external commands in shell calls (like `cmd /C`) unless explicitly required. Spawn binaries directly with arguments to avoid escaping issues with paths containing spaces.

### 3. Configuration Wrappers
- `custom_functions.json` requires a version wrapper: `{ "Version": "1.0", "Functions": [...] }`.
- Bare arrays will fail silently or result in empty lists.

### 4. CJK & Unicode Width
RWF is built for perfect CJK alignment.
- Use `unicode-width` utilities in `rwf-bin/src/ui/unicode_utils.rs`.
- Never slice strings by byte index; always use width-aware truncation.

## Development Status

### 1. Test Suite Status
The test suite is highly stable (~99.5% pass rate).
- **Baseline Failures**: Exactly **5 failures** exist in `rwf-lib` related to `config_launch_integration_tests`. These are legacy issues with external process simulation.
- **Regressions**: Any failure *outside* of these 5 is a new regression.
- **Execution**: Run tests with `--test-threads=1` to avoid filesystem race conditions.

### 2. Roadmap & Context
- `plan/ROADMAP.md` (Japanese) is the source of truth for phase progress.
- Current Status: **Phase 7** (RWF-specific enhancements).

## Key File Locations
- **State Logic**: `rwf-lib/src/state.rs`
- **UI/Rendering**: `rwf-bin/src/ui/`
- **Config Models**: `rwf-lib/src/config.rs`
- **Macros ($P, $F)**: `rwf-lib/src/macro_expander.rs`
