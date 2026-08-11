# Diagnostic Bundles

A **diagnostic bundle** is one folder capturing one reproduction of one problem: the keys
pressed, the state transitions they caused, the jobs they started, the frames drawn, the logs
emitted, and pictures of the screen — all on a single ordered timeline.

It exists so that a problem can be analysed *after* it happened, by someone (or something)
that was not watching. The design goal is not log volume. It is making this chain
reconstructible:

```
user input → state change → job → render → problem
```

Design rationale lives in [`plan/7.15.diagnostic_report.md`](../plan/7.15.diagnostic_report.md).
This document is the consumer's reference: what the files contain and how to read them.

---

## Recording a bundle

| | |
|---|---|
| **Start / stop** | `F12` |
| **Snapshot the screen** | `F11`, any number of times while recording |
| **Whole-run capture** | `RWF_DIAGNOSTICS=1 rwf` |

While recording, `● DIAG mm:ss` shows in the top-right corner and the task panel carries the
bundle path. Stopping prompts for a description; cancelling it or leaving it blank still
saves the bundle.

Both keys work in normal, viewer and leap modes, and while a dialog is open.

### Configuration

```json
"Diagnostics": {
  "Enabled": true,
  "OutputDirectory": "",
  "PromptForReport": true
}
```

- `Enabled: false` — the keys do nothing and no collector thread is ever spawned.
- `OutputDirectory` — empty means the default below. Env vars (`%VAR%`, `${VAR}`,
  `$env:VAR`) are expanded.

Default location, mirroring the log directory:

| Condition | Location |
|---|---|
| `./logs` exists in the working directory | `./diagnostics/<session-id>/` |
| otherwise | `<data dir>/rwf/diagnostics/<session-id>/` (`%APPDATA%` on Windows) |

---

## Layout

```
20260811-234152/
  metadata.json            environment and session timing
  config_effective.json    resolved config, keybindings, config load results
  events.jsonl             the event timeline, one JSON object per line
  logs.jsonl               tracing output for this session, one per line
  snapshots/
    000-start.txt          rendered screen, plain text
    000-start.json         semantic state at the same instant
    001-manual.txt/.json   an F11 capture
    002-final.txt/.json    automatic, at session end
  report.txt               the user's description
```

---

## The `seq` contract

**`seq` is the ordering authority, not `ts`.**

Every record in `events.jsonl` *and* `logs.jsonl` draws from one shared counter. Timestamps
collide at millisecond resolution and cannot order events emitted from different threads;
`seq` always can.

To reconstruct the full timeline, concatenate both files and sort by `seq`. The numbers are
unique across the two and monotonically increasing. A snapshot's `.json` half carries the
`seq` of its `Snapshot` event, so screen captures drop into the same ordering.

A gap in the sequence means a record was allocated but never reached disk — the writer had
already shut down, or a write failed. Gaps are not normal; treat one as a sign the bundle is
incomplete rather than as a timing artefact.

---

## `events.jsonl`

One object per line:

```json
{"seq": 184, "ts": "2026-08-11T23:41:52.113+09:00", "type": "Key", "data": {...}}
```

| `type` | `data` | Meaning |
|---|---|---|
| `SessionStart` | `rwf_version`, `trigger` | first record; `trigger` is `key`, `env` or `test` |
| `SessionEnd` | — | last record |
| `Key` | `key`, `mode`, `dialog` | a keypress that survived repeat-debounce |
| `Transition` | `name`, `detail` | a state change |
| `JobSubmit` | `job_id`, `kind` | a job entering `JobManager` |
| `Wake` | `next_wakeup_ms`, `any_pane_loading`, `active_jobs` | the main loop is about to sleep |
| `Render` | `width`, `height`, `mode` | a frame was drawn |
| `Snapshot` | `trigger`, `rows` | a screen/state pair was written |
| `Note` | `message` | free-form marker |

### `Transition` is the widest net

Every state change in rwf passes through one function, `state::update_state`. Job events get
there too: `event_receiver::process_pending_events` maps each `JobEvent` into a `Transition`
before applying it. So job lifecycle appears as transitions — `JobStarted`,
`UpdateJobProgress`, `CompleteJob`, `AcknowledgeCancel` — not as a separate event type.

`name` is the variant name alone, for cheap filtering. `detail` is the `Debug` rendering,
truncated to 512 bytes (marked `…[truncated]` when cut).

### `Key` with no following `Transition`

This is a first-class finding, not a gap in the capture. A `Key` record followed by no
`Transition` before the next event means the keypress genuinely did nothing — which is the
literal shape of an "I pressed X and nothing happened" report.

To tell an *unbound* key from a *rebound* one, check the `keybindings` section of
`config_effective.json`. That is why it is captured.

### `Wake` is usually the interesting one

`Wake` records the adaptive-poll timeout the main loop computed before sleeping. It is only
emitted when that timeout is non-zero — while the UI has pending updates the loop spins at
zero and would otherwise flood the stream.

An oversized `next_wakeup_ms` sitting between a job completing and the UI reflecting it is the
signature of a completion that failed to shorten the poll. The loop's default safety poll is
1000 ms, so a `Wake` with `next_wakeup_ms: 1000` in the middle of active work is worth
looking at.

---

## `logs.jsonl`

Mirrored `tracing` events:

```json
{"seq": 185, "ts": "...", "level": "INFO", "target": "rwf_lib::state",
 "file": "state/mod.rs", "line": 1044, "message": "...", "fields": {}}
```

`session.log` remains the ordinary rolling human-facing log. `logs.jsonl` is the slice
belonging to this session, in a machine-readable form, ordered against the event stream.

**Log level.** The mirror sits under the same filter as the file log — `INFO` by default. To
capture debug detail, record with `RUST_LOG=debug`. This is deliberate: giving the mirror its
own wider filter would make every `debug!` call site format its arguments on every main-loop
iteration even with diagnostics switched off.

Events raised on the writer thread itself are never mirrored, or a failed write would log a
warning that fed back into the failing writer.

---

## `snapshots/`

Each capture is a **pair** sharing a filename stem and a `seq`:

- `NNN-<trigger>.txt` — the rendered screen as plain text, one line per terminal row.
- `NNN-<trigger>.json` — the semantic state behind it.

`<trigger>` is `start`, `manual` or `final`. `NNN` counts up within the session, so a
directory listing reads in capture order.

**The two halves are complementary, not redundant.** The `.txt` is what was *rendered*; the
`.json` is what the state *was*. A disagreement between them is itself a finding — a stale
render is one of the bugs this feature exists to catch.

### The state half

A hand-written projection, not a dump of `AppState`. Entry lists are deliberately excluded:
what was visible is already in the `.txt`, and the rest is recoverable from the path. Notable
fields:

| Field | Why it matters |
|---|---|
| `tabs.items[].{left,right}.entry_count` vs `raw_entry_count` | an unexplained gap means a file mask or search filter is active |
| `…is_loading` + `…active_job_id` | `is_loading` true with no `active_job_id` is a pane stuck loading forever |
| `…cursor_entry` | the filename under the cursor, usually the fastest way to orient |
| `dialogs` | dialog stack, outermost first, **titles only** — payloads hold half-typed user text |
| `viewer` | position and metadata only; never file contents |

---

## `metadata.json` and `config_effective.json`

`metadata.json` carries session id, start/end times, rwf version, OS, arch and terminal
identification.

`config_effective.json` has three sections:

- `config` — the **resolved** `AppConfig`, not a copy of `config.json`. Every config field
  has a serde default, so the file on disk says what the user *wrote* while this says what the
  program *ran with*. Only the latter reproduces a bug, or survives a default changing
  between versions.
- `keybindings` — captured separately, because `AppConfig::key_bindings` is `#[serde(skip)]`
  and so absent from the config JSON.
- `load_results` — which config files were found, parsed, or silently fell back to defaults.
  A whole class of "rwf ignores my setting" reports resolves here.

---

## Analysing a bundle

1. Read `report.txt` first — what the user thought happened.
2. Merge `events.jsonl` and `logs.jsonl`, sort by `seq`.
3. Find the `Key` record for the action the report describes.
4. Read forward: the `Transition`s it caused, any `JobSubmit`, the `Transition`s carrying job
   completion, then `Render`.
5. Compare the surrounding `Wake` records against that sequence. A long sleep between a
   completion and the next `Render` is the delay the user felt.
6. Open the `snapshots/` pair nearest the problem and check the `.txt` against the `.json` —
   if they disagree, the render is stale rather than the state wrong.

### Worked shape: a delay after opening a tab

```
seq 184  Key         {"key": "Ctrl+n", "mode": "Normal"}
seq 185  Transition  {"name": "CreateTab"}
seq 186  JobSubmit   {"kind": "ReadDirectory"}
seq 187  Render      ← tab visible, pane still loading
seq 188  Wake        {"next_wakeup_ms": 50, "any_pane_loading": true}
...
seq 203  Transition  {"name": "CompleteJob"}       ← data arrived
seq 204  Wake        {"next_wakeup_ms": 1000}      ← ⚠ slept a full second
seq 205  Render                                     ← user finally sees it
```

The gap between 203 and 205 is what the user reports as "it took a second". The `Wake` at 204
names the cause: the completion did not shorten the poll.

---

## Limits — read before concluding

- **Job progress is not throttled.** `UpdateJobProgress` and `UpdateJobProgressWithDetail`
  reach `update_state` like any other transition, so a large copy can emit thousands of
  `Transition` records. They are noise, not signal; filter by `name` when scanning. (The
  design intends throttling here; it is not implemented yet.)
- **The elapsed time on the `● DIAG` badge is indicative only.** It refreshes on frames the
  app already draws, deliberately without forcing redraws that would perturb the very timing
  being measured. Record `ts`/`seq` are unaffected — trust the files, not the badge.
- **Only `INFO` and above is mirrored by default** (see `logs.jsonl` above).
- **Nothing before `SessionStart` is captured.** There is no pre-session ring buffer. If a
  problem happens at startup, record with `RWF_DIAGNOSTICS=1` instead of the key.
- **A `Transition` `detail` over 512 bytes is truncated.**
- **Snapshots stop after 200 per session.**

---

## Privacy

**A bundle contains every filename and path that was on screen, verbatim, plus the screen
contents themselves.** That is inherent — those are the things being diagnosed. Bundles are
written in the clear, with no redaction.

Review a bundle before sharing it. In particular, `config_effective.json` includes custom
function command lines, which are the one field that can contain a credential passed as an
argument — qualitatively different from a path.

`diagnostics/` is in `.gitignore`, so bundles recorded inside the repository are not committed
by accident.
