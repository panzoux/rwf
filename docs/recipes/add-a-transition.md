# Recipe: Add a Transition (DRAFT)

> **DRAFT — do not treat as authoritative.** `state.rs` is split into handler
> modules in M5; the dispatch location will move. This recipe is finalized in
> M7. Until then it records the *current* touch points.

Checklist for a new `Transition::Foo`:

1. **Define** — add the variant to `Transition` in `rwf-lib/src/state.rs`.
   Carry data by value (the enum is `Clone`); prefer IDs/`Location`s over
   references.
2. **Handle** — add a match arm in `update_state` (or the relevant
   `handle_*_transition` helper).
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
