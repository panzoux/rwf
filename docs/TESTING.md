# Testing Guide

How RWF's test suites are organized, why they run the way they do, and how to
write new tests. Architecture context: [ARCHITECTURE.md](ARCHITECTURE.md).

## Suites and commands

| Suite | Where | Count | Command |
|---|---|---|---|
| rwf-lib | `rwf-lib/src/*_tests.rs`, `src/input/*_tests.rs`, inline `#[cfg(test)]` modules | 1043 | `cargo test -p rwf-lib -- --test-threads=1` (~37 min) |
| rwf-bin | inline `#[cfg(test)]` modules under `rwf-bin/src/` | 51 | `cargo test -p rwf -- --test-threads=1` (seconds) |

During development, run name-filtered subsets:

```sh
cargo test -p rwf-lib marking -- --test-threads=1
```

Full verification (fmt + clippy + both suites) is bundled in `/project:check`.

Baseline: **all green** (re-baselined 2026-07-03). Any failure is a new
regression — do not dismiss failures as "known flaky"; the last batch of
"known failures" encoded a real product bug for weeks.

## Why `--test-threads=1`

Many integration tests touch the real filesystem (TempDirs, archive creation,
config files, log files) and some manipulate process-wide state such as
environment variables (`env::set_var`) and the process working directory.
Running in parallel causes races and spurious failures, so single-threaded
execution is mandatory, both locally and in CI.

Related: prefer per-package runs (`-p rwf-lib` / `-p rwf`) — workspace-wide
parallel `cargo test` has run out of memory in the past. After refactors that
remove or rename methods, also run `cargo test -p rwf --no-run`: stale
references in rwf-bin UI tests can break the whole workspace test build.

## Shared fixtures: `rwf-lib/src/test_utils.rs`

Compiled only for tests (`#[cfg(test)] pub mod test_utils;`). **Use these
instead of writing per-file `create_test_*` helpers.**

```rust
use crate::test_utils::{
    entries, entry, numbered_entries, open_dialog, state_with_temp_dirs, test_state,
    AppStateBuilder, FileEntryBuilder,
};

// AppState with default config — the standard starting point
let mut state = test_state();

// FileEntry with defaults (location /test/<name>, size 100, file, unmarked)
let e = entry("a.txt");
let list = entries(&["a.txt", "b.txt"]);
let ten = numbered_entries(10); // file0.txt .. file9.txt

// Override only what differs from the defaults
let dir = FileEntryBuilder::new("docs")
    .dir(true)
    .size(0)
    .calculated_size(Some(4096))
    .build();

// Panes pre-pointed at two fresh TempDirs (keep the TempDirs alive!)
let (mut state, _left, _right) = state_with_temp_dirs();

// Declarative state setup
let state = AppStateBuilder::new()
    .left_entries(numbered_entries(3))
    .left_cursor(1)
    .build();

// Open a dialog and get it back (panics if none opened)
let dialog = open_dialog(&mut state, Transition::ShowJumpToPathDialog);
```

Conventions:

- If your setup matches the shared defaults, use the shared helpers.
- If your setup is *intentionally different* (custom `AppConfig`, real archive
  files on disk, special timestamps), keep a local helper — do not force-fit
  the shared API. Several files (`config_*`, `sevenz`/`tar`, `concurrent_*`)
  are deliberately unmigrated for this reason.
- New rwf-bin dialog-input tests: bundle the 13 mutable variables of
  `handle_file_conflict_input` with `ui::dialog::test_support::ConflictInputHarness`
  instead of declaring them by hand.

## Property-based tests (proptest)

Property tests live in `*_properties.rs` files (e.g. `state_properties.rs`,
`edge_case_properties.rs`). CI caps the number of cases per property with the
`PROPTEST_CASES=16` environment variable to keep runtime bounded; locally the
proptest default (256) applies unless you set it yourself:

```sh
PROPTEST_CASES=16 cargo test -p rwf-lib property_ -- --test-threads=1
```

Policy: keep properties fast and deterministic-ish — no filesystem access
inside the proptest closure; generate plain data, drive `update_state`, assert
invariants. Anything slower belongs in a regular integration test.

## What "behavior-preserving" means for test refactors (Phase M)

- The test *count* must not change (`cargo test -p rwf-lib -- --list` before
  and after).
- Every original fixture field value must be reproduced exactly — sizes,
  locations, `calculated_size`, `marked`, timestamps.
- Assertions, transitions, and test names never change in a fixture migration.
