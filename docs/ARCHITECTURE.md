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

- `Transition` (`rwf-lib/src/state.rs`) is the *only* way state changes.
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

## AppState responsibility boundaries (M5 groundwork)

`AppState` (`rwf-lib/src/state.rs`, ~3,900 lines) is deliberately a single
struct — the Transition dispatch already provides the behavioral boundary, and
1000+ tests reference `state.field` directly, so splitting the struct buys
little safety for a lot of churn. What M5 *will* do is split `update_state`'s
handler functions into per-domain modules and document a field-ownership map
(which handler reads/writes which field). Until then, the informal grouping is:

- `tabs` (per-tab `left_pane`/`right_pane` `PaneModel`: entries, cursor,
  location, per-pane `marking`) — navigation/marking handlers
- `dialogs` (stack) + `ui` (active pane, modes, viewer layout) — dialog/UI handlers
- `jobs` (`JobManager`) + `cache` (directory cache) — job handlers
- `config`, `registered_folders`, session/logging fields — config handlers

New fields must state their owning handler in the field comment (enforced by
review, ratified in M5).
