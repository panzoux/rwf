# RWF — Project Guide for AI Assistants

**RWF** (Reactive Worker Filemanager) is a non-blocking Rust TUI file manager.
Mental model: `Input -> Transition -> State Update -> UI Projection`.
No side-effects in the UI thread; all I/O runs as `Job`s in the worker pool.

- Workspace: `rwf-lib` (state machine, jobs, backends) + `rwf-bin` (bin `rwf`; rendering, terminal).
- Roadmap / phase status: `plan/ROADMAP.md` (Japanese) is the source of truth.
- Architecture details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/DEVELOPER_GUIDE.md](docs/DEVELOPER_GUIDE.md)
- Testing guide: [docs/TESTING.md](docs/TESTING.md)
- Recipes: [docs/recipes/](docs/recipes/) — [add-a-dialog.md](docs/recipes/add-a-dialog.md), [add-a-transition.md](docs/recipes/add-a-transition.md)

## Build / Test / Verify

```sh
cargo build                                        # build
cargo fmt --all -- --check                         # formatting (CI-enforced)
cargo clippy --all-targets -- -D warnings          # lints (CI-enforced)
cargo test -p rwf -- --test-threads=1              # rwf-bin tests (51, fast)
cargo test -p rwf-lib -- --test-threads=1          # rwf-lib tests (1043, ~37 min)
cargo test -p rwf-lib <filter> -- --test-threads=1 # filtered subset during development
```

- Always `--test-threads=1` — tests share real filesystem state (see docs/TESTING.md).
- Prefer per-package runs (`-p rwf-lib` / `-p rwf`); workspace-wide parallel `cargo test` has hit OOM.
- After refactors that remove/rename methods, also run `cargo test -p rwf --no-run` —
  stale references in rwf-bin UI tests have broken the whole workspace test build before.
- `/project:check` runs the full verification pipeline.

## Quality rules (enforced since Phase M)

1. **No `unwrap()` in non-test code.** `clippy::unwrap_used` is `deny` workspace-wide
   (tests are exempt via `clippy.toml`). Modules carrying
   `#![allow(clippy::unwrap_used)] // TODO(M6): ratchet` are legacy — do not add new ones.
2. **`unsafe_code` is `deny`** (`rwf-lib/src/volume_info.rs` is the only scoped allow; every
   unsafe block needs a `// SAFETY:` comment).
3. **Tests use shared fixtures** from `rwf-lib/src/test_utils.rs`
   (`test_state()`, `FileEntryBuilder`, `AppStateBuilder`, `state_with_temp_dirs()`, …).
   Do not re-declare per-file `create_test_*` helpers unless the setup is intentionally
   different from the shared defaults.
4. **Dialog rendering goes through `rwf-bin/src/ui/dialog/common.rs`**
   (style constants, `titled_block`) **and `frame.rs`** (`render_dialog_frame`,
   `render_dialog_buttons`, `centered_rect_abs`). No new inline
   `Style::default().fg(..).bg(..)` chains in dialog code.
   Dialog input tests bundle their mutable state in `test_support::ConflictInputHarness`.

## Architectural mandates

- **State purity**: never mutate state in rendering/input layers; go through `Transition`
  and `update_state`. Transitions that change visible state must signal `with_ui_change()`.
- **Marking is per-pane** (`PaneModel.marking`).
- **Dialog stack** (`AppState.dialogs`): when a sub-action completes, pop *all* related
  dialogs — ghost dialogs and focus traps are a known failure mode.
- **Async jobs**: a Transition that starts a `ReadDirectory` job must set `active_job_id`
  on the pane, and dialog-confirm handlers must forward `jobs_to_start` — missing either
  causes a permanent `is_loading` state.
- **Serialization**: config JSON uses **PascalCase** (TWF compatibility). Use
  `#[serde(rename = "FieldName")]` and always provide `#[serde(default)]`.
- **CJK/Unicode**: never slice strings by byte index; use the width-aware utilities in
  `rwf-bin/src/ui/unicode_utils.rs`.
- **External commands**: spawn binaries directly with args (no `cmd /C` wrappers unless
  required). In config macros prefer `${VAR}`/`$env:VAR` — bare `$VAR` collides with
  single-letter RWF macros ($P, $O, $R, …).

## Repo conventions

- Line endings are **LF** with `autocrlf=false`. On Windows, edit files with tools that
  preserve LF (PowerShell `Set-Content`/`Out-File` rewrite whole files as CRLF).
- Feature development is frozen during Phase M (quality overhaul); all changes must be
  behavior-preserving. See `plan/quality_overhaul.md`.
- Machine-specific notes (real config paths, local pitfalls) live in `.claude/CLAUDE.local.md`.
