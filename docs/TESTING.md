# Testing Guide

How RWF's test suites are organized, why they run the way they do, and how to
write new tests. Architecture context: [ARCHITECTURE.md](ARCHITECTURE.md).

## Suites and commands

| Suite | Where | Count | Command |
|---|---|---|---|
| rwf-lib | `rwf-lib/src/*_tests.rs`, `src/input/*_tests.rs`, inline `#[cfg(test)]` modules | 1361 | `cargo test -p rwf-lib -- --test-threads=1` (~8.5 min; see *Where the time actually goes*) |
| rwf-bin | inline `#[cfg(test)]` modules under `rwf-bin/src/`, plus `rwf-bin/tests/` | 322 (+11) | `cargo test -p rwf -- --test-threads=1` (~58s) |

Full verification (fmt + clippy + both suites) is bundled in `/project:check`.

## When to run what

The full suite is cheap enough to run often (see *Where the time actually goes*
below). Reach for a narrower tier only to keep the edit loop tight, not because
the full run is unaffordable.

| Tier | When | What |
|---|---|---|
| Inner loop | every edit | `cargo check -p rwf-lib`, then a name-filtered subset: `cargo test -p rwf-lib marking -- --test-threads=1` |
| Pre-commit | every commit | `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p rwf -- --test-threads=1` (322 tests in ~58s, and it carries the repo-wide contract guards) |
| Pre-push | every push | both packages in full |
| Phase close | end of a 7.x item | both packages with `PROPTEST_CASES=256` (see *Property-based tests*) |

CI runs the full suite on every push and PR in about four minutes and is the
authoritative gate. **A red CI run blocks the next commit** — check
`gh run list --limit 1` after pushing. CI was red for two weeks straight in
August 2026 without anyone acting on it, which is exactly the failure mode a
fast gate is supposed to prevent.

## Where the time actually goes

Measured 2026-09-01 on the primary dev machine (2-core i5-7300U, 8 GB, Defender
exclusions applied), full `cargo test -p rwf-lib` at `PROPTEST_CASES=32`. The
default has since been lowered to 16, which takes the total to **257s** (252.93s
execution, no rebuild needed — changing `[env]` does not invalidate the build):

| Slice | Time |
|---|---|
| compile + link the test binary | 114s |
| **132 property tests** | **330s — 84% of execution** |
| the other 1228 tests | ~61s |
| total | 509s |

**Property tests dominate everything else combined by 5x.** Their cost is linear
in `PROPTEST_CASES`, measured on the same subset:

| `PROPTEST_CASES` | 132 property tests |
|---|---|
| 1 | 26s |
| 16 (CI's pin, and the local default) | 138s |
| 32 | 330s |
| 256 (proptest's own default) | ~32 min — this is what the original 37-minute run was |

So the first question about any slow local run is *what proptest depth am I
running*, not *how fast is this machine*.

For reference, CI runs the **entire** rwf-lib suite at `PROPTEST_CASES=16` in 12.65s,
against 138s here for the property subset alone. Part of that gap is hardware,
but not 11x worth — it is not yet attributed, so do not treat CI's timing as a
prediction of local timing. If you chase it, the prime suspect is antivirus
scanning of the temp trees the filesystem-touching properties create.

## Keeping a local run fast

In order of measured impact:

1. **Antivirus exclusions — 2206s to 294s (7.5x)** on this machine, the single
   biggest win and it costs nothing. In an *elevated* PowerShell:

   ```powershell
   Add-MpPreference -ExclusionPath '<repo>\target'
   Add-MpPreference -ExclusionPath $env:TEMP
   Add-MpPreference -ExclusionProcess 'rustc.exe','cargo.exe','link.exe'
   ```

   Excluding the temp directory matters as much as excluding `target/`: the
   filesystem tests churn thousands of temp files and every one gets scanned.
2. **proptest depth.** `.cargo/config.toml` pins `PROPTEST_CASES=16`, matching
   CI. Anything deeper pays real time per run to catch what CI would not; see the
   table above. Raise it deliberately, not by default.
3. **Debuginfo.** `[profile.dev] debug = "line-tables-only"` in the workspace root
   cuts MSVC PDB generation, which dominates test-binary link time.

Measure before changing anything, and re-measure after:

```sh
powershell -c "Measure-Command { cargo test -p rwf-lib -- --test-threads=1 } | Select TotalSeconds"
```

To skip the expensive part entirely during the edit loop, the property tests all
match one filter:

```sh
cargo test -p rwf-lib -- --test-threads=1 --skip propert
```

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

## Dialog snapshot tests (insta)

Every `DialogContent` variant is pinned by snapshot tests in
`rwf-bin/src/ui/dialog/snapshot_tests/` (94 tests, 188 snapshots): the dialog
is rendered through the real `render_dialog` dispatch onto a ratatui
`TestBackend` at 80x24 and 120x40, and the buffer dump (text + style runs) is
compared against `snapshot_tests/snapshots/*.snap`.

- Run: part of the normal suite (`cargo test -p rwf snapshot_tests -- --test-threads=1`).
- After an *intentional* visual change, regenerate and review:
  `cargo insta review` (or `INSTA_UPDATE=always cargo test -p rwf snapshot_tests`
  followed by inspecting the git diff). Commit updated `.snap` files with the
  change; never commit `.snap.new` pending files.
- New dialogs: add a module under `snapshot_tests/` using
  `snapshot_dialog(name, &dialog, &state)` with 2-4 representative states.
- Determinism rules: fixed ASCII data only, no `SystemTime::now()`/`Instant`,
  no TempDir/filesystem scans (build `JumpToPath`/`JumpToFile` contents
  directly, never via transitions). Wall-clock timestamps and UUIDs are
  redacted by harness filters; HashMap-ordered lists (job manager rows) must
  contain at most one element.

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
