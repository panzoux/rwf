# Recipe: Add a Dialog (DRAFT)

> **DRAFT — do not treat as authoritative.** The dialog module is being
> restructured in M3 (`ui/dialog/mod.rs` split per dialog) and M4
> (`model/dialog.rs` per-variant structs + `new()` constructors). This recipe
> is finalized in M7. Until then it records the *current* touch points.

Checklist for a new dialog `Foo`:

1. **Data** — `rwf-lib/src/model/dialog.rs`
   - Add a `DialogContent::Foo { ... }` variant (fields: content data +
     cursor/selection UI state).
   - Add a constructor on `Dialog` (e.g. `Dialog::foo(...)`) setting `title`
     and initial content. (After M4: define a `FooDialog` struct with `new()`.)
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
   - Dialog-mode key handling: extend `handle_dialog_input` dispatch in
     `rwf-bin/src/ui/dialog/mod.rs` if the dialog needs non-default keys.
4. **Rendering** — `rwf-bin/src/ui/dialog/`
   - Add a `render_foo_dialog` function; use `frame::render_dialog_frame` /
     `render_dialog_buttons` / `centered_rect_abs` for the chrome and the
     style constants from `common.rs` (`DIALOG_TEXT`, `DIALOG_SELECTED`, …) —
     no inline `Style::default().fg(..).bg(..)` chains.
   - Register it in the `render_dialog` match and, if the height is dynamic,
     in the min-height match at the top of `render_dialog`.
5. **Tests**
   - rwf-lib: open/confirm/cancel flow via `test_utils::open_dialog` +
     `update_state` (see `input/jump_to_path_tests.rs` for the pattern).
   - rwf-bin (after M3): a snapshot test per representative state.
6. **Docs** — update `docs/DIALOG_DESIGN_SPEC.md` if the dialog introduces new
   interaction patterns.
