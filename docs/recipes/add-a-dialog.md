# Recipe: Add a Dialog (DRAFT)

> **DRAFT — do not treat as authoritative.** Reflects the structure as of M4
> (per-variant structs + per-dialog render/input files). Finalized in M7.

Checklist for a new dialog `Foo`:

1. **Data** — `rwf-lib/src/model/dialog/foo.rs`
   - Define a `FooDialog` struct holding the dialog's data + UI state
     (cursor/scroll/focus). If it's a single text-input dialog with
     cursor/scroll/focused-field, embed `DialogUiState` under a `ui` field
     instead of repeating those three fields (see `file_mask.rs`).
   - Give it a `new(...)` constructor taking only the fields that vary across
     call sites; hardcode fields that always start at the same value.
   - Add `mod foo;` and `pub use foo::FooDialog;` (both alphabetically sorted)
     in `rwf-lib/src/model/dialog/mod.rs`.
   - Add the enum variant `DialogContent::Foo(FooDialog)`.
   - **Naming**: if `FooDialog` collides with an existing unrelated helper
     struct already in `model/dialog/mod.rs` (several dialogs have long-dead
     ones from before this refactor), suffix with `Content` instead of
     `Dialog` (e.g. `TabSelectorContent`, `JobManagerContent`).
2. **Transitions** — `rwf-lib/src/state.rs`
   - `Transition::ShowFooDialog` pushes the dialog (`state.dialogs.push`),
     returns `with_ui_change()`.
   - Handle `ConfirmDialog` / dialog-specific updates for the new variant.
     If confirmation starts I/O, put the `JobSpec` in `jobs_to_start` and (for
     `ReadDirectory`) set the pane's `active_job_id`.
   - On completion, pop **all** related dialogs (no ghost dialogs).
3. **Input** — `rwf-lib/src/input/`
   - Map a key to an `Action`, and the `Action` to `ShowFooDialog` in
     `action_to_transitions`.
   - Dialog-mode key handling: add `pub(super) fn handle_input(dialog: &mut
     FooDialog, key: KeyEvent, ...) -> DialogAction` in
     `rwf-bin/src/ui/dialog/foo.rs`, then dispatch to it from
     `handle_dialog_input` in `rwf-bin/src/ui/dialog/mod.rs`:
     ```rust
     if let DialogContent::Foo(d) = &mut dialog.content {
         return foo::handle_input(d, key);
     }
     ```
     Only genuinely cross-cutting routing logic (key handling that depends on
     *other* dialog types, or must run before/after the per-dialog check)
     belongs directly in `handle_dialog_input` — see the Enter-key special
     case and Tab-navigation cycling near its top/bottom for examples.
4. **Rendering** — `rwf-bin/src/ui/dialog/foo.rs`
   - Add a `render_foo_dialog` function; use `frame::render_dialog_frame` /
     `render_dialog_buttons` / `centered_rect_abs` for the chrome and the
     style constants from `common.rs` (`DIALOG_TEXT`, `DIALOG_SELECTED`, …) —
     no inline `Style::default().fg(..).bg(..)` chains.
   - Register it in the `render_dialog` match and, if the height is dynamic,
     in the min-height match at the top of `render_dialog`.
5. **Tests**
   - rwf-lib: open/confirm/cancel flow via `test_utils::open_dialog` +
     `update_state` (see `input/jump_to_path_tests.rs` for the pattern).
   - rwf-bin: a snapshot test per representative state, in
     `rwf-bin/src/ui/dialog/snapshot_tests/foo.rs`.
6. **Docs** — update `docs/DIALOG_DESIGN_SPEC.md` if the dialog introduces new
   interaction patterns.
