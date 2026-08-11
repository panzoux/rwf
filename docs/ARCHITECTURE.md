# RWF Architecture

High-level map of how RWF is put together and where the boundaries are.
For code-level walkthroughs (examples, job lifecycle details, extension points)
see [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md) — this document does not repeat them.

## The one-loop mental model

```
key press ──▶ Action ──▶ Vec<Transition> ──▶ update_state(&mut AppState, Transition)
                                                 │
                                                 ├─ StateUpdateResult { ui_changed, jobs_to_start, .. }
                                                 │
              JobEvent ◀── WorkerPool ◀── JobManager ◀── jobs_to_start
                 │
                 └─▶ map_job_event_to_transition ──▶ Transition (loop back)
```

Everything visible on screen is a projection of `AppState`. Nothing in the UI
thread touches the filesystem.

## Transition state machine (rwf-lib)

- `Transition` (`rwf-lib/src/state/mod.rs`) is the *only* way state changes.
  Input mapping (`rwf-lib/src/input/`) converts key events to `Action`s, then
  `action_to_transitions(&state, &action)` decides which transitions apply —
  it may consult state (e.g. marked files) but never mutates it.
- `update_state(&mut AppState, Transition) -> StateUpdateResult` is a pure-ish
  state function: it mutates only `AppState` and *describes* side effects in the
  result instead of performing them:
  - `ui_changed` — the caller should re-render.
  - `jobs_to_start: Vec<JobSpec>` — I/O the caller must hand to the `JobManager`.
- Dialogs are transitions too: `ShowXxxDialog` pushes onto `AppState.dialogs`
  (a stack), `ConfirmDialog`/`CloseDialog` pop. A confirm handler that spawns
  I/O must forward its `jobs_to_start`; a `ReadDirectory` job must set the
  pane's `active_job_id` — missing either leaves a pane in permanent
  `is_loading`.

Because `update_state` is synchronous and side-effect-free, the 1000+ rwf-lib
tests drive the entire application (navigation, marking, dialogs, job
bookkeeping) without a terminal or a worker pool.

## JobManager and WorkerPool (rwf-lib)

Two separate responsibilities:

- **`JobManager`** (`rwf-lib/src/job.rs`) is *bookkeeping inside AppState*:
  FIFO `queue`, `active` map, `completed` list, `max_parallel`. It is updated
  via transitions (`EnqueueJob`, `StartNextJob`, `UpdateJobProgress`,
  `CompleteJob`, `CancelJob`) and never does I/O itself.
- **`WorkerPool`** (`rwf-lib/src/worker_pool.rs`) *executes* jobs
  (`job/job_executor.rs`) on worker threads and reports back with `JobEvent`s
  (progress, success data, errors) over a channel.
- **`event_receiver.rs`** closes the loop: the binary polls pending events and
  `map_job_event_to_transition` turns each into an ordinary `Transition`, so
  job completion flows through the same state machine as key presses.

Cancellation is cooperative: executors check a token; `CancelJob` only flags it.

## lib / bin boundary

| | `rwf-lib` | `rwf-bin` (binary name `rwf`) |
|---|---|---|
| Owns | `AppState`, `Transition`, models, input mapping, jobs, backends, config | event loop (`app.rs`), terminal setup, all ratatui rendering (`ui/`) |
| May depend on | std, serde, backends — **no ratatui/crossterm rendering** (crossterm key types only for input mapping) | `rwf-lib` + ratatui |
| Tested by | 1043 tests, `src/*_tests.rs` + `src/input/*_tests.rs`, shared fixtures in `test_utils.rs` | 51 tests (dialog input, widgets); snapshot tests arrive in M3 |

Rule of thumb: if it decides *what* happens, it belongs in rwf-lib; if it
decides *how it looks*, it belongs in rwf-bin. Dialog *data* lives in
`rwf-lib/src/model/dialog.rs` (`DialogContent`); dialog *pixels* live in
`rwf-bin/src/ui/dialog/` (shared chrome: `common.rs` styles + `frame.rs`
frame/buttons).

## Diagnostics: the observer tier (7.15)

`rwf-lib/src/diagnostics/` records a session — events, logs, screen/state snapshots — into one
folder. Bundle format and analysis: [DIAGNOSTIC_BUNDLES.md](DIAGNOSTIC_BUNDLES.md).

Two properties matter when touching anything it observes:

**It is an observer, never part of the control path.** The collector is reached through a
process-global handle and holds no reference to `AppState`, so it *cannot* influence a
transition even by accident. That is structural, not conventional — do not "improve" it by
passing state in.

**It is invisible when off.** `observe()` takes a closure; with no session running the cost is
one `OnceLock` read plus one relaxed atomic load, and the payload is never built. Anything
that would make an observation point pay a cost while inactive breaks the property that lets
this ship in release builds.

Observation points, deliberately few:

| Site | Captures |
|---|---|
| `state::update_state` | every `Transition` — and so every `JobEvent`, since `process_pending_events` maps job events into transitions first |
| `job::JobManager::start_job` | every job submission; all `pool.submit_job` paths route through it |
| `App::handle_key_event` | keypresses, including ones that map to nothing |
| `App::render` | frames, and the screen buffer for snapshots (inside the `draw` closure — `Backend::buffer()` is `TestBackend`-only) |
| `App::run` adaptive sleep | `Wake`, the computed poll timeout |
| `DiagnosticLogLayer` | `tracing` events, via the subscriber — no call sites involved |

## AppState responsibility boundaries (M5)

`AppState` itself (`rwf-lib/src/state/mod.rs`) is deliberately a single
struct — the Transition dispatch already provides the behavioral boundary, and
1000+ tests reference `state.field` directly, so splitting the struct buys
little safety for a lot of churn. What M5 did instead is move the ten
`handle_*_transition` methods out of `state.rs` into one file per domain
under `rwf-lib/src/state/handlers/`, with shared helpers in
`rwf-lib/src/state/helpers.rs`:

- `handlers/navigation.rs`, `tab.rs`, `marking.rs`, `job.rs`,
  `job_management.rs`, `ui.rs`, `view.rs`, `search.rs`, `viewer.rs`,
  `advanced.rs` — one file per `handle_*_transition` function, moved verbatim
  (no logic changes). Dialog transitions (`ShowDialog`/`ConfirmDialog`/
  `ShowJumpToPathDialog`/etc.) live inside `ui.rs` rather than a separate
  `dialog.rs`: they're interleaved with non-dialog UI arms and share
  `self.dialogs` state closely enough that splitting them out would require
  new judgment calls, not just moving lines.
- `helpers.rs` holds only genuinely cross-handler logic: `editor_job`/
  `resolve_editor` (used by both `job.rs` and `ui.rs` to build the "open in
  editor" job spec). Other private helpers (`save_viewer_to_current_tab`,
  `restore_viewer_from_tab`, `start_viewer_search_background`) turned out to
  each have a single caller (`tab.rs`, `tab.rs`, `viewer.rs` respectively) once
  measured, so they stayed put rather than being moved speculatively.
- `AppState`'s own methods (`new`, `current_tab(_mut)`, `active_pane(_mut)`,
  `opposite_pane`, `unmark_all_panes`, session save/restore) stay in `mod.rs`:
  they're `pub`/`pub(crate)` already, so any handler file can call
  `self.current_tab_mut()` etc. without further visibility changes.

### Field ownership map

| Field | Owning handler(s) | Notes |
|---|---|---|
| `tabs` | `tab` (create/close/switch) | read/written by nearly every handler for pane access; cross-cutting, not exclusive |
| `jobs` (`JobManager`) | `job` | `tab`/`viewer` only call `request_cancel` for cleanup |
| `background_jobs` | `job` | `tab` cancels on `CloseTab`; `job_management` starts background jobs |
| `search` (`SearchModel`) | `search` | `ui` (`ConfirmDialog` on the Search input dialog) and dispatch-level `UpdateDialogInput` also touch it |
| `ui` (`UIState`: active pane, modes, layout) | cross-cutting — read/written by nearly every handler | no single owner; treat as shared top-level state, not handler-private |
| `dialogs` (stack + input buffer) | `ui` | `job` pushes error dialogs on `CompleteJob` failure |
| `registered_folders` | `ui` | single-owner (all dialog-driven) |
| `cache` (directory cache) | `job` (writes on `CompleteJob`/invalidate) | `navigation`/`advanced` read cached entries |
| `navigation_cache` (cursor/scroll memory) | `navigation` | single-owner |
| `viewer`, `viewer_job_id`, `viewer_search_job_id`, `viewer_search_input`, `viewer_command_input` | `viewer` | `tab`'s save/restore-to-tab helpers move these fields across tab switches |
| `log_manager` | `ui` (`SaveLog`) | otherwise read-only after construction |
| `config` | dispatch-level `ReloadConfig`/`UpdateConfig` (in `update_state`, not a `handle_*` method) | `navigation`/`job`/`viewer`/`ui` read thresholds/offsets from it |
| `last_tab_created` | `tab` | single-owner (`CreateTab` debounce) |
| `extension_associations`, `custom_functions`, `config_load_results` | dispatch-level `ReloadConfig` | `ui` reads `custom_functions` for menus/invocation |
| `pending_confirmation_logs`, `confirmation_needs_keybinding_reload`, `pending_custom_function_input`, `suppress_next_dialog_pop` | none (unused inside `state/`) | read/written by `rwf-bin`'s `app.rs` integration layer, not by any transition handler |
| `leap` (`LeapState`) | dispatch-level `Leap*` transitions (in `update_state`, not a `handle_*` method) | single-owner |

`ui`, `tabs`, and `config` are genuinely cross-cutting (read or written by most
handlers) and were considered for sub-struct extraction during M5; none of
them decomposed cleanly enough to be worth the churn, so no sub-struct split
was made. New fields should still note their owning handler (or "cross-cutting")
in the field's doc comment.
