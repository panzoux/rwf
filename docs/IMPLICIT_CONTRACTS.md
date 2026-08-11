# Implicit Contracts

Rules that are **repo-wide but owned by no module**. Code can be perfectly correct
inside its own file and still break one of these, and — before this document — the
build would not have noticed.

Every contract below is either enforced by something the build runs, or listed with
an explicit reason why it is not. If you add a cross-cutting rule, add it here and
add its guard; a rule that lives only in prose is a rule that will be broken.

## Summary

| # | Contract | Status | Was it being violated? |
|---|---|---|---|
| 1 | [stdout is a data channel](#1-stdout-is-a-data-channel-not-a-message-channel) | Guarded — test | Yes, in Phase 7.15 (fixed in `d538295`) |
| 2 | [`.gitignore` must not shadow source](#2-gitignore-patterns-must-never-shadow-a-source-path) | Guarded — 2 tests + anchored patterns | Yes — `logs/`, `sample/` and four more were unanchored |
| 3 | [Rust/manifest files are LF](#3-rust-sources-and-cargo-manifests-are-lf) | Guarded — `.gitattributes` + test | Yes — 4 files had drifted to CRLF |
| 4 | [Terminal mode confined to 2 files](#4-terminal-mode-transitions-live-in-two-files) | Guarded — test | No |
| 5 | [cmd.exe needs `/D /C`](#5-cmdexe-is-invoked-as-cmd-d-c) | Guarded — file allowlist + unit test | **Yes — 3 sites in `state/helpers.rs`** |
| 6 | [config keys are PascalCase](#6-configjson-keys-are-pascalcase) | Guarded — 2 tests | **Yes — `ArchiveConfig`, `TextInputConfig`** |
| 7 | [every config field is optional](#7-every-configjson-field-is-optional) | Guarded — 2 tests | **Yes — 16 mandatory fields across 4 structs** |
| — | [`with_ui_change()` on visible changes](#transitions-that-change-visible-state-must-call-with_ui_change) | Not guardable cheaply | — |
| — | [`ReadDirectory` / `active_job_id`](#readdirectory-jobs-must-set-active_job_id) | Not guardable cheaply; audited clean | No |
| — | [Four more, deliberately skipped](#considered-and-deliberately-skipped) | See table | Two have live violations |
| — | [`unwrap`, `unsafe`, fmt, clippy](#already-enforced-elsewhere--no-guard-needed-here) | Already enforced | No |

All five live violations found by this audit are fixed in the same change as the
guards; there is no "known failing" state to inherit.

## Where the guards live

| File | Holds |
|---|---|
| [`rwf-bin/tests/repo_contracts.rs`](../rwf-bin/tests/repo_contracts.rs) | Static scans over the whole workspace source tree |
| [`rwf-bin/tests/config_contracts.rs`](../rwf-bin/tests/config_contracts.rs) | Serialized-shape guards for `config.json` |
| [`rwf-lib/src/state/helpers.rs`](../rwf-lib/src/state/helpers.rs) | `/D /C` assertions next to the cmd.exe builders |
| [`.gitattributes`](../.gitattributes) | LF enforcement at checkout time |

The static scans sit in **rwf-bin's** integration-test target even though most of
what they read is rwf-lib source. That is deliberate: `cargo test -p rwf` finishes in
about 75 seconds and runs first in CI, while `cargo test -p rwf-lib` takes ~37
minutes. A contract violation should be reported by the fast suite.

Each scan keeps an **allowlist** in the test file. Widening one means editing the
allowlist *and* the matching section here — the friction is the point. Every
allowlist also fails when it lists something that no longer matches, so it cannot rot
into a permanent hole.

---

## 1. stdout is a data channel, not a message channel

**The rule.** On the interactive path, `rwf` writes exactly one thing to stdout: the
exit directory, and only when `--cwd` or `Shift+Q` asked for it. Everything else the
user reads goes to `eprintln!` or `tracing`.

**Why.** `rwf` is designed to be wrapped by a shell function that captures stdout and
`cd`s to it (`docs/USER_GUIDE.md`, `scripts/rwf-cd.{sh,zsh,ps1}`):

```bash
function fm() {
    local output=$(rwf -cwd)
    if [ -d "$output" ]; then
        cd "$output"
    fi
}
```

**What breaks.** Any extra line makes `$output` multi-line, so `[ -d "$output" ]` is
false and the `cd` silently does not happen. The user sees a file manager that
"stopped remembering the directory", with no error anywhere. This happened during
Phase 7.15: two `println!` lines reporting a diagnostic session path, fixed in
`d538295` by switching to `eprintln!`.

**Enforced by.** `stdout_writes_in_rwf_bin_are_allowlisted`. It scans every `.rs`
under `rwf-bin/src` for `println!`, `print!` and `stdout()`, and fails on anything
not in `ALLOWED_STDOUT_WRITES`. The current allowlist is four `main.rs` entries
(`--export-function-list` and `--export-config-files`, both of which `return` before
the TUI starts; and the exit directory itself) plus two handle acquisitions that feed
crossterm/ratatui rather than printing text.

**Limitation.** This is a static scan, not a behavioural test. Running the real
interactive path requires a TTY that CI does not have, and `enable_raw_mode` fails
without one — so a spawn-the-binary test would either be skipped in CI (a guard that
passes unconditionally) or flaky. The scan catches the failure mode that actually
occurred: someone adds a print statement.

## 2. `.gitignore` patterns must never shadow a source path

**The rule.** Directory patterns in `.gitignore` are anchored with a leading slash.

**Why.** An unanchored pattern like `diagnostics/` matches at *every* depth, not just
the repo root.

**What breaks.** Phase 7.15 added `diagnostics/` to `.gitignore` for a bundle output
directory at the repo root. It also matched the brand-new `rwf-lib/src/diagnostics/`
source module, which disappeared from `git status` entirely — no warning, no error,
the files simply would never have been committed. Fixed by anchoring to
`/diagnostics/`. `logs/` and `sample/` carried the same latent hazard and are now
anchored too.

**Enforced by.** Two tests.

- `gitignore_does_not_shadow_existing_source_paths` runs every file and directory
  under `rwf-lib/src`, `rwf-lib/tests`, `rwf-bin/src` and `rwf-bin/tests` through a
  real gitignore matcher (the `ignore` crate — the one ripgrep uses; a hand-rolled
  matcher would be the one thing a guard must not be, approximately right).
- `gitignore_patterns_cannot_shadow_a_future_source_subdirectory` takes each
  glob-free pattern and asks whether a hypothetical `rwf-lib/src/<pattern>/` would be
  ignored. This is the one that catches the hazard *before* someone creates the
  module — the case the first test cannot see.

`.gitignore` also carries a comment at the top explaining the anchoring rule.

## 3. Rust sources and Cargo manifests are LF

**The rule.** `*.rs` and `*.toml` are LF in the index and in the working tree, on
every platform.

**Why.** The repo standardised on LF with `core.autocrlf=false`. That is a *local*
git setting on one machine, not a property of the repository.

**What breaks.** A tool that rewrites a whole file with CRLF turns a one-line change
into a whole-file diff, so the real edit is invisible in review and every merge
conflicts. On Windows, PowerShell's `Set-Content` and `Out-File` both do this. Four
files had already drifted before this guard existed: `rwf-bin/src/ui/colors.rs`,
`rwf-lib/src/model/ui.rs`, `rwf-lib/src/state_properties.rs` and
`rwf-bin/Cargo.toml`. All four are normalised now.

**Enforced by.** [`.gitattributes`](../.gitattributes) (`*.rs`/`*.toml` → `text eol=lf`)
plus `rust_sources_and_manifests_use_lf_line_endings`, which reads the working-tree
bytes. The `.gitattributes` entry is what makes the test deterministic: the checkout
is LF regardless of the cloner's `autocrlf`, so the test measures the repo and not
the machine.

**Deliberately out of scope.** About 100 tracked `.md` and `.json` files (most of
`docs/`, all of `specs/twf/`, the `help.*.json` at the root) are still CRLF.
Renormalising them would be a large, merge-hostile diff with no build-level benefit,
so `.gitattributes` is scoped to the two extensions the toolchain actually cares
about. If those files are ever normalised, widen both the attributes file and the
test's file list together.

## 4. Terminal mode transitions live in two files

**The rule.** Only `rwf-bin/src/terminal.rs` and `rwf-bin/src/app.rs` may call
`enable_raw_mode` / `disable_raw_mode` or enter/leave the alternate screen.

**Why.** Raw mode and the alternate screen are *process-global* state that outlives
the process's own view of itself. `terminal.rs` owns setup and teardown (including a
`Drop` guard so an early `?` still restores); `app.rs` owns exactly one other
transition, the `SuspendAndRun` handoff that gives the terminal to a TUI editor and
takes it back.

**What breaks.** A path that leaves raw mode and fails to restore it — an early
return, a `?`, an error branch — hands the user back a shell that no longer echoes
keystrokes. Nothing can test that: it happens after teardown, in a terminal the test
harness does not own. Confining the transitions to two files is what makes "is every
enter paired with a leave" a question a reviewer can answer by reading.

**Enforced by.** `terminal_mode_transitions_are_confined_to_allowlisted_files`.

## 5. cmd.exe is invoked as `cmd /D /C`

**The rule.** Every `cmd.exe` spawn passes `/D` immediately before `/C`.

**Why.** `/D` skips the user's `HKCU\Software\Microsoft\Command Processor\AutoRun`
hook. Without it, that hook — Clink, a corporate login script, anything — runs inside
our transient shell and writes to whatever console it inherited.

**What breaks.** For `system_open_job` that console is rwf's own alternate-screen
TUI, so the display is corrupted; for the `SuspendAndRun` editor path it is the
terminal we just handed to the editor. The failure only appears on machines that
happen to have an AutoRun hook, so it survives every clean-machine test run. This
rule was recorded in `.claude/CLAUDE.local.md`'s pitfalls table and followed in
`backend/local.rs` and `job/job_executor.rs`, but **three sites in
`rwf-lib/src/state/helpers.rs` had quietly drifted to a bare `/c`** — found by this
audit and fixed.

**Enforced by.** Two layers, because a source scan alone cannot tell whether the `/D`
is in the right position:

- `cmd_exe_spawn_sites_are_confined_to_allowlisted_files` fails when a file outside
  `ALLOWED_CMD_EXE_FILES` starts spawning `cmd.exe`.
- `windows_cmd_invocations_pass_slash_d_before_slash_c` in
  `rwf-lib/src/state/helpers.rs` asserts the actual argument order for all three
  builders (`system_open_job`, and `editor_job` in both its GUI and terminal forms).

A new spawn site trips the first test, whose message tells you to write the second
kind of assertion next to your builder.

## 6. `config.json` keys are PascalCase

**The rule.** Every key in the serialized config is PascalCase — `#[serde(rename_all
= "PascalCase")]` on the struct, or `#[serde(rename = "...")]` on the field.

**Why.** Config JSON is shared with the TWF prototype and hand-edited by users
following `docs/USER_GUIDE.md`.

**What breaks.** A struct that forgets `rename_all` serializes snake_case keys.
`--export-config-files` then writes a template in a style the documentation does not
describe, and a user who writes the documented PascalCase name gets silently ignored
(the field falls back to its default). Found by this audit:
**`ArchiveConfig` and `TextInputConfig` were both missing `rename_all`**, emitting
`compression_level`, `default_format`, `last_archive_name` and `edit_mode`. Both now
have it, with `#[serde(alias = "...")]` on each field so a config.json written by an
older rwf still loads.

**Enforced by.** `app_config_serializes_with_pascal_case_keys`, which serializes
`AppConfig::default()` and walks every object key in the tree.

**And its companion.** The real config at `%APPDATA%\rwf\config.json` on the
development machine contains those snake_case Archive keys *today*, so adding
`rename_all` without the aliases would have silently reset those settings on the next
launch. `legacy_snake_case_keys_still_deserialize` parses a document written in the
old style and asserts the values actually land — a dropped alias is otherwise
invisible, because the config still parses and the value just quietly reverts to its
default. If you ever rename a config key, add the alias and extend that test.

## 7. Every `config.json` field is optional

**The rule.** Every field reachable from `AppConfig` deserializes when absent —
`#[serde(default)]` on the field or on its containing struct.

**Why.** rwf reads config files written by older versions of itself. Config loading
is all-or-nothing: `ConfigManager::load_config` either parses the whole document or
falls back to `AppConfig::default()`.

**What breaks.** One mandatory field means an older config file fails to parse, and
the user loses **every** setting at once, not just the new one — the only signal is a
`tracing::warn!` in a log they are not reading. Found by this audit: `DisplayConfig`,
`FileOpConfig`, `SearchConfig` and `UIConfig` had **16 mandatory fields** between
them (`CjkWidth`, `ShowHiddenFiles`, `BufferSize`, `RefreshRate`, …). All four
structs now carry container-level `#[serde(default)]`.

**Enforced by.** Two tests. `app_config_parses_from_an_empty_object` checks the
degenerate case; `every_app_config_field_can_be_omitted` serializes the default
config, then removes each key in turn and re-parses, so a single field losing its
default is reported by name.

---

## Known contracts that are *not* guarded here

Listed so nobody assumes silence means safety.

### `Transition`s that change visible state must call `with_ui_change()`

Stated in `CLAUDE.md`. Not guarded, and not cheaply guardable: deciding whether a
transition changed anything *visible* requires knowing what the renderer reads, which
is exactly the judgement the rule exists to encode. A mechanical version — snapshot
`AppState` before and after every transition, and require `ui_changed` whenever the
snapshot differs — would need `AppState` to be comparable and would fire on
invisible bookkeeping fields (timestamps, job ids, caches), so it would be a source
of false failures rather than a guard. The real mitigation stays what it is today:
individual handler tests assert `result.ui_changed`, and the failure mode is
visible immediately when you run the app.

### `ReadDirectory` jobs must set `active_job_id`

Stated in `CLAUDE.md`; a miss leaves a pane stuck in `is_loading` forever. Worth
stating precisely, because the mechanism is not what the one-line rule suggests.

`CompleteJob` (`rwf-lib/src/state/handlers/job.rs`) has two paths for a finished
`ReadDirectory`:

- **`spec.requesting_pane` is `Some`** — the entries go to that pane, but only after
  an ownership check, `pane.active_job_id == Some(job_id)`. Fail that check and the
  result is **discarded with a `tracing::warn!`**, so `is_loading` never clears.
- **`spec.requesting_pane` is `None`** — a legacy fallback updates every pane whose
  `current_location` matches the job's location.

So the operative rule is: **a transition that sets `is_loading = true` must also put
`requesting_pane` on the spec.** `active_job_id` itself is then filled in by the main
loop in `rwf-bin/src/app.rs` (~line 441), which is conditional on `requesting_pane`
being present — that is the backstop for `ChangeLocation`/`NavigateHistory`, which set
`is_loading` in the state layer but cannot reach the pane to set the id.

Audited during this pass: every site that sets `is_loading` (`navigation.rs`,
`view.rs`, `job.rs`, `tab.rs`) sets `requesting_pane` too. `SyncPanes` and `SwapPanes`
in `handlers/advanced.rs` deliberately set neither and ride the fallback — correct
today, because they also never set `is_loading`.

Not guarded, because the only faithful guard is behavioural: drive each
directory-loading transition through `update_state` against a real temp tree and
assert `is_loading ⇒ requesting_pane.is_some()`. There is no way to enumerate
"transitions that load a directory" mechanically, so the test would be a hand-curated
list — one that a new transition silently would not join, which is exactly the failure
mode it was meant to catch. It would also have to live in rwf-lib, behind the
37-minute suite. Worth building deliberately if this bites again; not worth a
half-measure now.

The other half of the `CLAUDE.md` rule — a dialog-confirm handler that forgets to
forward `jobs_to_start`, so no job is ever submitted — needs a whole-app harness and
is left to the existing per-handler tests.

### Considered and deliberately skipped

| Rule | Why not guarded |
|---|---|
| Dialog rendering goes through `ui/dialog/common.rs` + `frame.rs`; no inline `Style::default().fg(..).bg(..)` | 9 live violations. Single-subsystem and cosmetic — a breach makes colours inconsistent, not behaviour wrong. Guarding it means a refactor first. |
| Tests use shared fixtures from `test_utils.rs`, no per-file `create_test_*` | 15 live violations. Test-hygiene, not a contract a feature can silently break at runtime. |
| Never slice strings by byte index (CJK width) | No mechanical signal distinguishes a byte-index slice of a filename from one of an ASCII key name. Covered by the width-aware helpers in `ui/unicode_utils.rs` and by CJK-specific tests. |
| Dialog stack: pop *all* related dialogs | Requires knowing which dialogs are "related" for a given flow. Per-flow tests are the only workable form. |

## Already enforced elsewhere — no guard needed here

- **No `unwrap()` in non-test code** — `clippy::unwrap_used = "deny"` workspace-wide
  (root `Cargo.toml`), with tests exempted via `clippy.toml`. The Phase M ratchet is
  complete: there are now **zero** `#![allow(clippy::unwrap_used)]` escapes left in
  the tree, so the lint is unconditional.
- **`unsafe_code = "deny"`** workspace-wide, with `rwf-lib/src/volume_info.rs` the
  only scoped allow.
- **Formatting and lints** — `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` both run in CI on a pinned toolchain.
