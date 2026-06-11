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

## Feature Testing Protocol

### The "Real Config" Problem
Unit tests and sample files in `/sample/` do NOT represent what the user actually has.
The real installed config lives at `C:\Users\user\AppData\Roaming\rwf\` (Windows).
**Whenever a feature touches config files or custom functions, always check the real installed files too.**

Lesson learned: `edit keybindings file` custom function used `$APPDATA` (unexpanded on Windows)
instead of `%APPDATA%` or `${APPDATA}`. Unit tests all passed. Real-world test failed immediately.

### End-to-End Checklist
For any feature touching keybindings, custom functions, menus, or macros:

1. **Real config check** — Read `%APPDATA%\rwf\custom_functions.json` and `keybindings.json`.
   Look for issues in the actual user config, not the samples.
2. **Full flow** — Test the entire user-facing path (key press → dialog → action → result),
   not just the individual handler.
3. **Macro collision audit** — Scan commands for bare `$VAR` where the name starts with
   P/O/L/R/F/W/E/M. These letters are RWF single-letter macros; they expand first and silently
   corrupt the env var reference. Recommend `${VAR}` or `$env:VAR` instead.
4. **External program paths** — If the feature spawns an external program, verify the string
   passed to it is a fully expanded path, not a literal macro or env var.
5. **Dialog stack** — After any dialog action chain, confirm all dialogs are popped.
   Ghost dialogs and focus traps are a common failure mode.
6. **TWF parity** — If the feature existed in the TWF prototype (`specs/twf/`), cross-check
   the spec to find anything not yet ported to RWF.

### Known Pitfalls Log
| Symptom | Root cause | Fix |
|---|---|---|
| Notepad/editor "path not found" | `$APPDATA` not expanded on Windows | Use `${APPDATA}` or `%APPDATA%` |
| Command silently gets wrong path | Bare `$VAR` starts with RWF macro letter | Use `${VAR}` or `$env:VAR` |
| cmd.exe ignores AutoRun skip | Missing `/D` flag | `cmd.exe /D /C ...` |
| Viewer hangs on large files | mmap page-fault stalls | InMemory ≤100 MB, Mmap above |
| Feature works in tests, broken live | Only tested against `/sample/`, not `%APPDATA%\rwf\` | Always test real config |

## Key File Locations
- **State Logic**: `rwf-lib/src/state.rs`
- **UI/Rendering**: `rwf-bin/src/ui/`
- **Config Models**: `rwf-lib/src/config.rs`
- **Macros ($P, $F)**: `rwf-lib/src/macro_expander.rs`
- **Real user config**: `C:\Users\user\AppData\Roaming\rwf\`
