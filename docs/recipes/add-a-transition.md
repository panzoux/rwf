# Recipe: Add a Transition

Finalized in M7 against the post-M5 structure (`state.rs` split into
`state/mod.rs` + `state/handlers/*.rs`).

Checklist for a new `Transition::Foo`:

1. **Define** — add the variant to the `Transition` enum in
   `rwf-lib/src/state/mod.rs`. Carry data by value (the enum is `Clone`);
   prefer IDs/`Location`s over references.
2. **Handle** — add a match arm in the relevant
   `rwf-lib/src/state/handlers/{navigation,tab,marking,job,job_management,ui,
   view,search,viewer,advanced}.rs` file (dialog-related transitions go in
   `ui.rs`; `update_state` in `state/mod.rs` just dispatches to these in
   sequence — see the `handle_*_transition` calls near its top). If the new
   transition doesn't fit an existing handler's theme, add the match arm
   directly in `update_state`'s own `match transition { ... }` block instead
   of creating a new handler file for one variant.
   - Mutate `AppState` only; **no I/O**.
   - Return `StateUpdateResult`: call `with_ui_change()` whenever anything
     visible changed; put any I/O into `jobs_to_start` as `JobSpec`s.
   - For `ReadDirectory` jobs, set the target pane's `active_job_id` or the
     pane will stay `is_loading` forever.
3. **Trigger** — decide where the transition comes from:
   - a key press → map `Action` → `Transition` in
     `rwf-lib/src/input/` (`action_to_transitions`),
   - a job event → extend `map_job_event_to_transition` in
     `rwf-lib/src/event_receiver.rs`,
   - a dialog confirm → the `ConfirmDialog` handling for that dialog.
4. **Test** — rwf-lib test using `test_utils::test_state()` (+ builders):
   apply the transition with `update_state`, assert state, `ui_changed`, and
   `jobs_to_start`. Do not add `unwrap()` to non-test code paths
   (`clippy::unwrap_used` is deny).
5. **Job execution** (only if you added a new `JobKind`): implement it in
   `rwf-lib/src/job/job_executor.rs` with cooperative cancellation checks and
   progress events; completion flows back as a `JobEvent`.
