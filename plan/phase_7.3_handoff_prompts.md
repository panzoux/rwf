# Phase 7.3 Smart File Opener — Handoff Prompts (Sonnet/Haiku, copy-paste ready)

Worktree: `C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener`
Branch: `worktree-feat-7.3-smart-file-opener`
Design spec: `plan/7.3.smart_file_opener.md`

## Status as of hand-off

Done and merged into the worktree branch (commits, oldest first):
`234758f` magic.rs detection module → `7839526` config flag →
`a4117f2`+`13c883b` DetectFileType/DetectFileTypesBatch job foundation (Task 1, reviewed, fixed, approved) →
`25a74ad` TypeMismatchWarning dialog + ExecuteAssociationChecked gate (Task 2, spec-reviewed ✅, code-quality-reviewed "With fixes" — **fixes NOT yet applied, session hit usage limit before the fix-up agent could run**).

**Next action is always: paste "TASK 2 FIX-UP" below FIRST**, before Task 3. Task 3 depends on the DRY helper it adds.

Working tree is clean at `25a74ad`. `git log --oneline -5` to confirm before starting.

## How to use these prompts

Each block below is self-contained — paste the whole fenced block as-is into a fresh Sonnet or Haiku session (Claude Code, in this repo). After the implementer reports back, run the two generic review prompts at the bottom (spec review, then code quality review) with the placeholders filled from the implementer's report. Fix loop: if a reviewer finds issues, paste their findings back to the *same* implementer session and ask it to fix + recommit, then re-review. Only move to the next numbered task after code-quality review says "Ready to merge: Yes" or all "With fixes" issues are actually applied and reverified.

Do not run tasks out of order — each depends on state committed by the previous one (noted per-task below).

---

## TASK 2 FIX-UP (do this first — two Important issues from code review, unapplied)

```
You are fixing two Important issues raised by code review on commit `25a74ad` in the git worktree
at C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener (branch
worktree-feat-7.3-smart-file-opener). Do NOT work in the main repo checkout — only this worktree.

Read `git show 25a74ad` first to see what you're fixing. That commit added a `TypeMismatchWarning`
dialog: before running an `ExtensionAssociation` command, a `JobKind::DetectFileType` job sniffs the
file's magic bytes; if the content looks like an executable but the extension disagrees, a warning
dialog is shown (Confirm = run anyway, Cancel = do nothing).

## Fix 1 (Important): duplicated job-spec construction across 4 call sites

The exact same `JobKind::ExecuteCustomFunction { command, working_dir, pipe_to_action: None, shell }`
is built independently in 4 places:
1. `rwf-lib/src/state/handlers/ui.rs` — the original `Transition::ExecuteAssociation` handler
2. `rwf-lib/src/state/handlers/ui.rs` — `ExecuteAssociationChecked`'s disabled-detection branch
3. `rwf-lib/src/state/handlers/job.rs` — the no-mismatch auto-continue arm in `handle_job_transition`
4. `rwf-bin/src/ui/dialog/confirm.rs` — the `TypeMismatchWarning` Confirm handler

Find all 4 (grep for `ExecuteCustomFunction` in those three files). Extract a small shared helper —
an associated function on `JobSpec` (in `rwf-lib/src/job.rs`, near where `JobSpec` and `JobKind` are
defined) is the natural home, e.g.:

    impl JobSpec {
        pub fn execute_association(command: String, working_dir: Location, shell: Option<String>) -> Self {
            JobSpec::new(JobKind::ExecuteCustomFunction { command, working_dir, pipe_to_action: None, shell })
        }
    }

Use it at all 4 call sites so they can't drift apart on a future change. Adjust the exact shape if the
real code doesn't fit this signature cleanly — use your judgement, but the goal is: one definition,
four call sites collapse to one line each.

## Fix 2 (Important): document an intentional trade-off, no behavior change

In `rwf-lib/src/state/handlers/job.rs`, `handle_job_transition`'s `JobKind::DetectFileType` arm
(added in 25a74ad) pushes the `TypeMismatchWarning` dialog unconditionally on job success, with no
check that the triggering context is still relevant. Compare this to the `JobKind::CollectJumpCandidates`
arm in the same file, which matches completion against a `loading_job_id` still open on some dialog
before mutating anything — i.e. it guards against acting on a stale/cancelled context.

Code review assessed the `DetectFileType` gap as low-risk (the job is a ~300-byte read, completes
near-instantly, and even a "stale" confirm just re-runs the originally-captured command, which is
harmless) but asked for a code comment acknowledging this was an intentional trade-off rather than an
oversight. Add a short comment at the top of the `DetectFileType` arm explaining: this job carries
everything it needs to complete the action on its own (the command/working_dir/shell are captured in
`DetectFileTypePurpose::CheckAssociationMismatch` at job-creation time), so unlike `CollectJumpCandidates`
(which needs to find a *specific still-open dialog* to merge results into), there's nothing to
correlate against — no staleness guard needed. Do not add a `loading_job_id`-style guard; that would be
solving a problem that doesn't exist here per the reviewer's own assessment. Just document it.

## Optional (Minor, your judgement, skip if short on time)

- `DialogContent::TypeMismatchWarning`'s dialog height is hardcoded `10u16` in `rwf-bin/src/ui/dialog/mod.rs`
  (grep `DialogContent::TypeMismatchWarning(_) => { ... 10u16 }`), unlike `DeleteConfirm` which sizes
  from content. A long/deeply-nested path could wrap and clip. Consider adding a snapshot test with a
  long path to confirm current behavior is acceptable (find the existing snapshot tests at
  `rwf-bin/src/ui/dialog/snapshot_tests/type_mismatch_warning.rs` for the pattern), or size the dialog
  from path length if it's clearly broken. Your call.
- `cancel_type_mismatch_warning_pops_with_no_side_effects` (in confirm.rs's test module) manually
  pushes/pops the dialog and asserts an already-empty job queue stays empty — it never drives the real
  `app.rs` Cancel dispatch, so the name overstates what it verifies. Consider renaming it or wiring it
  through the real Cancel path if that's easy given the existing test harness.

## Verify and commit

Run in the worktree: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test -p rwf-lib file_open -- --test-threads=1`, `cargo test -p rwf -- --test-threads=1`.
All must pass clean. Commit as a new commit (not amend) with a `refactor(7.3):` prefix, message body
explaining the DRY extraction and the added comment, ending with:

    Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>

Report back: what you changed, file:line for the new helper and its 4 call sites, test output, commit SHA.
```

---

## TASK 3: Open With picker for single cursor file (Section 3a of the plan)

Depends on: Task 2 fix-up committed (uses the `JobSpec::execute_association` helper it adds).

```
You are implementing Task 3 of 7 for Phase 7.3 "Smart File Opener" in the rwf file manager (Rust TUI,
rwf-lib/rwf-bin workspace). Work from the git worktree at
C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener
(branch worktree-feat-7.3-smart-file-opener). Do not touch the main repo checkout.

Before starting, run `git log --oneline -8` and confirm you see a `refactor(7.3):` commit on top of
`25a74ad` (the Task-2 fix-up, adding a `JobSpec::execute_association` helper). If it's not there,
STOP and report NEEDS_CONTEXT — do not proceed on an unfixed Task 2.

## Task Description

Section 3 of plan/7.3.smart_file_opener.md ("新規: 「Open With...」ピッカー"), single-file half only
(marked/batch files are a separate later task — do not implement batch Open With here).

Today, `rwf-lib/src/input/mod.rs`'s `EnterDirectory` handler resolves an `ExtensionAssociation` for the
cursor entry via `.find()` — first match wins, silently. Change this so that when *multiple*
`ExtensionAssociation` entries match the same extension, the user picks which one to run via a new
picker dialog, instead of silently always running the first.

1. **Extract a resolution helper.** In `rwf-lib/src/input/mod.rs`, find the `EnterDirectory` handler's
   extension-association block (search for `state.extension_associations.iter().find`). Extract the
   "given an entry, find its matching ExtensionAssociations and produce the right Transition(s)" logic
   into a private helper function, e.g.
   `fn resolve_extension_association(state: &AppState, entry: &FileEntry) -> Option<Vec<Transition>>`
   (returns `None` if no association matches at all, so the caller falls through to the existing
   FileTypeMapping/text-viewer fallback chain unchanged). Inside this helper, change the association
   lookup from `.find()` to `.filter()` collecting *all* matches for the entry's extension:
   - **0 matches**: return `None` (existing fallthrough behavior, unchanged).
   - **1 match**: same as today — expand macros via `MacroExpander`, build
     `Transition::ExecuteAssociationChecked { path, command, working_dir, shell }` (this is what Task 2
     already made `EnterDirectory` build for the single-match case — you're just moving that code into
     the helper, not changing its behavior).
   - **2+ matches**: return a single `Transition::ShowOpenWithPicker { candidates, path }` (new
     transition — see step 3) instead of expanding any command yet (expansion happens after the user
     picks one, in the picker's Confirm handler).
   Call this helper from `EnterDirectory`'s existing non-dir/non-archive branch in place of the current
   inline `.find()` block; if it returns `None`, fall through to the FileTypeMapping check exactly as
   today.

2. **Reuse the same helper for the context menu.** This codebase's `ContextMenu` dialog (Phase 6.4)
   delegates its Copy/Move/Delete/Rename actions to `rwf_lib::input::action_to_transitions(state, &Action::X)`
   from `rwf-bin/src/ui/dialog/confirm.rs` (grep `ContextMenuAction::Copy` there to see the pattern) —
   i.e. ContextMenu doesn't duplicate action logic, it re-enters the same dispatch keyboard shortcuts use.
   Follow this idiom:
   - Add a new `Action::OpenWith` variant to the `Action` enum in `rwf-lib/src/input/mod.rs` (search
     `pub enum Action` — insert alongside similar file-open actions).
   - Add an arm for it in `action_to_transitions` that calls your `resolve_extension_association` helper
     on `state.active_pane().current_entry()`; if it returns `None` (no association at all for this
     entry), decide a sensible no-op behavior (e.g. return `vec![]` — there's nothing to "open with" via
     association; do NOT fall through to FileTypeMapping/text-viewer here, that's `EnterDirectory`-specific
     fallback behavior, not "open with" semantics).
   - Add `ContextMenuAction::OpenWith` to the `ContextMenuAction` enum in
     `rwf-lib/src/model/dialog/mod.rs` (near `Copy`/`Move`/`Delete`/`Rename`/`View`), and add an entry
     to `default_context_menu_options()` in the same file (a `vec![ContextMenuOption { label: "Open With...".into(), action: ContextMenuAction::OpenWith }]` entry — place it near `View` since both are "how to open this file" actions).
   - Add the matching dispatch arm in `rwf-bin/src/ui/dialog/confirm.rs`'s `DialogContent::ContextMenu`
     match (alongside the existing `ContextMenuAction::Copy => { ... action_to_transitions(state, &Action::Copy) ... }`
     arms) that does the same for `Action::OpenWith`.

3. **New dialog: `OpenWithPickerDialog`.** Model it structurally on `CustomFunctionMenuDialog`
   (`rwf-lib/src/model/dialog/custom_function_menu.rs`) but typed to `ExtensionAssociation` instead of
   the stringly-typed `MenuItem` (an `ExtensionAssociation` is a real struct with `command`/`description`/
   `shell` fields — don't stringify it into a `MenuItem.action` string and re-parse later, that loses
   type safety for no benefit). New file `rwf-lib/src/model/dialog/open_with_picker.rs`:
   ```rust
   #[derive(Debug, Clone)]
   pub struct OpenWithPickerDialog {
       pub path: std::path::PathBuf,
       pub candidates: Vec<crate::config::ExtensionAssociation>,
       pub selected_index: usize,
   }
   impl OpenWithPickerDialog {
       pub fn new(path: std::path::PathBuf, candidates: Vec<crate::config::ExtensionAssociation>) -> Self {
           Self { path, candidates, selected_index: 0 }
       }
   }
   ```
   Register `DialogContent::OpenWithPicker(OpenWithPickerDialog)` in the `DialogContent` enum
   (`rwf-lib/src/model/dialog/mod.rs`, alongside `TypeMismatchWarning`/`CustomFunctionMenu`), add a
   `Dialog::open_with_picker(path, candidates)` constructor (model on `Dialog::delete_confirm` or
   `Dialog::type_mismatch_warning` — grep for either, both are in `mod.rs`), title e.g. `"Open With..."`.

4. **New transition: `Transition::ShowOpenWithPicker { candidates: Vec<ExtensionAssociation>, path: PathBuf }`**
   in `rwf-lib/src/state/mod.rs`. Handler in `rwf-lib/src/state/handlers/ui.rs` (near
   `ExecuteAssociationChecked`'s handler) just pushes `Dialog::open_with_picker(path, candidates)` onto
   `self.dialogs` and returns `StateUpdateResult::with_ui_change()` (no job — this dialog only opens, it
   doesn't run anything until the user picks).

5. **Rendering.** New file `rwf-bin/src/ui/dialog/open_with_picker.rs`, modeled on
   `rwf-bin/src/ui/dialog/custom_function.rs`'s `render_custom_function_menu` (list of selectable rows,
   selected row styled with `DIALOG_SELECTED.add_modifier(Modifier::BOLD)`, using `common.rs` constants
   and `frame.rs`'s `render_dialog_frame`/`render_dialog_buttons` — zero new inline `Style::default()`
   chains). Show each candidate's `description` if present, else its `command` string, as the row label.
   Register in `rwf-bin/src/ui/dialog/mod.rs`'s three dispatch points the same way `CustomFunctionMenu`
   is registered — grep `DialogContent::CustomFunctionMenu` across that file to find all registration
   points (height calc, dialog-height-percent calc, width calc, the actual render call, and
   `handle_dialog_input`) and add a parallel arm for `OpenWithPicker` at each one you find.

6. **Input handling.** Model on `rwf-bin/src/ui/dialog/custom_function.rs`'s `handle_menu_input`
   (Up/Down navigation, Home/End) — `OpenWithPickerDialog` has no separators to skip (unlike
   `CustomFunctionMenuDialog`), so it can be simpler. Wire it into `handle_dialog_input` in
   `rwf-bin/src/ui/dialog/mod.rs` the same way `CustomFunctionMenu` is (grep
   `if let DialogContent::CustomFunctionMenu(d) = &mut dialog.content` in that file for the exact
   pattern to mirror).

7. **Confirm handling.** In `rwf-bin/src/ui/dialog/confirm.rs`'s `process_dialog_confirmation`, add an
   arm for `DialogContent::OpenWithPicker(OpenWithPickerDialog { path, candidates, selected_index })`:
   take `candidates[selected_index]`, expand its command via `MacroExpander` exactly like
   `EnterDirectory`'s existing code does (grep `MacroExpander::new()` in `input/mod.rs` for the pattern:
   build a `CustomFunction`, call `.expand(state, &func)`), then call
   `rwf_lib::state::update_state(state, Transition::ExecuteAssociationChecked { path: path.clone(), command, working_dir, shell })`
   and forward its `jobs_to_start` (same `for t in transitions { let result = update_state(...); if let Some(job) = result.jobs_to_start.into_iter().next() { return Some(job); } }`
   pattern used by the `ContextMenuAction::Copy` arm — copy that shape). This routes the picked
   candidate through Task 2's mismatch-check gate, same as the single-match path.

## Context

Task 2 (commit `25a74ad` + the fix-up you verified is present) added `Transition::ExecuteAssociationChecked`
as a generic gate: given `{path, command, working_dir, shell}`, it either runs the command directly or
detects-then-warns depending on the `magic_byte_detection_enabled` config flag and magic-byte mismatch.
It was deliberately designed to be reusable by this task — you are the intended second caller. Do not
modify `ExecuteAssociationChecked`'s handler itself; just call it with different data.

Do not touch `Action::OpenWithSystem` (Ctrl+Enter) — unrelated, uses OS-default association, intentionally
bypasses all of this per plan section 2.

Do not implement marked/batch-file Open With (multiple files selected via space-bar marking) — that's
Task 4, later, and depends on `JobKind::DetectFileTypesBatch` (already built in Task 1) plus this task's
picker dialog (which Task 4 will reuse for per-group picking).

## Before You Begin

If the exact registration points in `rwf-bin/src/ui/dialog/mod.rs` for `CustomFunctionMenu` don't match
what's described (line numbers will have drifted from Tasks 1/2's commits), read the file yourself and
adapt — but ask if the overall dispatch structure genuinely doesn't match what's described here.

## Your Job

1. Implement all 7 items above.
2. Tests: extend `rwf-lib/src/file_open_integration_tests.rs` with cases for 0/1/2+ matching associations
   producing the right `Transition` (this exercises your extracted helper directly via
   `action_to_transitions(&state, &Action::EnterDirectory)`, following the file's existing pattern — read
   its current 3(+more, from Task 2) tests first). Add a test for `Action::OpenWith` via context menu
   with 2+ candidates producing `Transition::ShowOpenWithPicker`, and with 0 candidates producing no-op.
   Add a confirm.rs test for the picker's Confirm handler (selecting index N runs the right candidate's
   command through the checked gate). Add a snapshot test for `OpenWithPickerDialog` rendering (2-3
   candidates) following the pattern in `rwf-bin/src/ui/dialog/snapshot_tests/type_mismatch_warning.rs`.
3. Verify: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test -p rwf-lib file_open -- --test-threads=1`, `cargo test -p rwf-lib enter_directory -- --test-threads=1`,
   `cargo test -p rwf -- --test-threads=1`, `cargo test -p rwf --no-run`, `cargo test -p rwf-lib --no-run`.
4. Commit `feat(7.3):` prefixed, ending with:
   ```
   Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
   ```
5. Self-review, then report using the standard format: Status (DONE/DONE_WITH_CONCERNS/BLOCKED/NEEDS_CONTEXT),
   what you implemented, test results, files changed, commit SHA, concerns.

**While you work:** if anything is genuinely unclear or the codebase doesn't match this description,
ask rather than guess.
```

---

## TASK 4: Open With picker for marked/batch files (Section 3, "複数選択")

Depends on: Task 3 committed (reuses `OpenWithPickerDialog` and `Transition::ExecuteAssociationChecked`).

```
You are implementing Task 4 of 7 for Phase 7.3 "Smart File Opener" in the rwf file manager. Work from
the git worktree at C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener
(branch worktree-feat-7.3-smart-file-opener). Confirm `git log --oneline -10` shows Task 3's
`feat(7.3):` commit (Open With picker, single-file) before starting; if not, STOP and report NEEDS_CONTEXT.

## Task Description

Section 3 of plan/7.3.smart_file_opener.md, "複数選択（マーク済みファイル）を Open With する場合"
subsection. When the user has multiple files marked (this project's per-pane marking, see
`PaneModel.marking`) and invokes Open With (via the `ContextMenuAction::OpenWith` / `Action::OpenWith`
you found in Task 3's commit), instead of acting on just the cursor entry:

1. Header-read all marked files' content types via the already-built `JobKind::DetectFileTypesBatch`
   (from Task 1 — `rwf-lib/src/job.rs`/`job_executor.rs`, already executes and cancellable, unused by
   any caller yet — you are the first consumer).
2. Group the marked files by detected `DetectedKind`.
3. Per group: resolve `ExtensionAssociation` candidates for that group's representative extension
   (reuse Task 3's `resolve_extension_association`-style logic, but note it currently takes a single
   `&FileEntry` — you'll likely need a variant that takes an extension string directly, since a group
   might span multiple *entries* that happen to share one detected content type but different literal
   extensions; use your judgement on the exact grouping key — the plan says group by `DetectedKind`,
   which is coarser than by extension, so decide how association-candidate lookup should key off that;
   document your choice in your report if the plan's intent felt ambiguous here).
4. If a group's candidate count is 1 → execute directly for all paths in that group (checked, i.e. via
   `Transition::ExecuteAssociationChecked`, but see the note below about multi-path commands). If 2+ →
   show the picker (Task 3's `OpenWithPickerDialog`) once per group.

## Important open design question — resolve it yourself, don't ask

The plan says (§3, "複数選択"): "実行時は同グループの全パスをコマンド引数として渡す（マクロ展開は既存
MacroExpander を複数パス対応で使う）" — i.e. it envisions passing *all* paths in a group to one command
invocation. **This multi-path capability does not exist today**: `rwf-lib/src/macro_expander.rs`'s
`MacroExpander::expand()` only builds a command bound to a single entry's macros (confirmed: no
multi-path variant exists in that file). Building real multi-path macro support would be a much larger,
architecturally separate change (touching `MacroExpander`'s core expansion logic, used by every other
custom-function/association call site in the app) — that is out of scope for this task and risks
destabilizing unrelated features.

**Do this instead**: for each group, run the resolved command **once per file** in the group (a simple
loop, each iteration expanding macros against that one file and going through
`Transition::ExecuteAssociationChecked` exactly like the single-file case). This is a legitimate,
documented simplification — note it explicitly in your commit message and report as a deviation from
the plan's literal text, with the reasoning above. Do not attempt to extend `MacroExpander` yourself.

## Context

Task 1 built `JobKind::DetectFileTypesBatch { paths: Vec<PathBuf> }` →
`SuccessData::FileTypesDetected(Vec<(PathBuf, DetectedKind)>)`, already cancellable
(`spec.cancel_token` checked per-iteration) — read `rwf-lib/src/job/job_executor.rs`'s
`execute_detect_file_types_batch` to see its exact signature before wiring a caller. Task 3 built
`OpenWithPickerDialog`/`Transition::ShowOpenWithPicker` for a *single* path+candidate-list; you'll need
either to generalize it to carry a list of paths (the whole group) instead of one `path: PathBuf`, or
add a parallel grouped variant — read Task 3's actual committed shape first (it may already store a
single path field you need to widen to `Vec<PathBuf>`) and pick whichever requires less rework, noting
your choice.

You'll need a new `Transition` (e.g. `Transition::StartBatchOpenWith { paths: Vec<PathBuf> }`) fired
when Open With is invoked with 2+ marked files, whose handler starts the `DetectFileTypesBatch` job.
Completion routing for it goes in `rwf-lib/src/state/handlers/job.rs`'s `handle_job_transition`
(same function Task 2 added a `DetectFileType` arm to — add a sibling `DetectFileTypesBatch` arm here,
which does the grouping-by-`DetectedKind` and then, per group, either queues the checked-execution job(s)
directly or pushes a picker dialog).

Check `PaneModel.marking` (grep it) for how marked entries are enumerated in this codebase — there should
be an existing helper for "give me the marked entries in the active pane" used by Copy/Move/Delete's
multi-file paths; follow that same enumeration pattern rather than inventing a new one.

## Your Job

1. Implement the above, using your documented judgement calls where the plan is ambiguous (grouping key,
   multi-path handling).
2. Tests: cover the grouping logic in isolation if it's a pure function (grouping N `(PathBuf, DetectedKind)`
   pairs into groups), and an integration test in `file_open_integration_tests.rs` for: 0 marked files
   (no-op or falls back to single-cursor-file behavior — decide and document), marked files all same
   detected type with 1 association candidate (auto-executes per-file, no dialog), marked files split
   across 2 detected types each with 2+ candidates (2 picker dialogs shown, or however your design
   surfaces "show picker per group" — describe the actual UX flow you built in your report since the
   plan doesn't specify whether groups are handled sequentially or need a group-selector first).
3. Verify: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test -p rwf-lib file_open -- --test-threads=1`, `cargo test -p rwf -- --test-threads=1`,
   `cargo test -p rwf --no-run`, `cargo test -p rwf-lib --no-run`.
4. Commit `feat(7.3):` prefixed, ending with:
   ```
   Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
   ```
5. Self-review, report using the standard format (Status/what/tests/files/SHA/concerns) — **be explicit
   about every judgement call you made**, since this task had more open design questions than most.

**This task has more ambiguity than the others.** If after reading Task 3's actual committed code the
"widen path to paths" refactor looks like it would meaningfully destabilize Task 3's single-file flow,
STOP and report BLOCKED with specifics rather than pushing through a risky refactor.
```

---

## TASK 5: Unregistered-extension fallback strengthening (Section 6)

Depends on: Task 1 only (the job foundation). Independent of Tasks 2/3/4 — can run any time after Task 1,
though doing it after 2–4 keeps the branch's history simpler.

```
You are implementing Task 5 of 7 for Phase 7.3 "Smart File Opener" in the rwf file manager. Work from
the git worktree at C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener
(branch worktree-feat-7.3-smart-file-opener).

## Task Description

Section 6 of plan/7.3.smart_file_opener.md ("拡張子未登録ファイルのフォールバック強化"). Today, in
`rwf-lib/src/input/mod.rs`'s `EnterDirectory` handler, when a file's extension matches neither an
`ExtensionAssociation` nor a `FileTypeMapping` entry, it unconditionally opens in the internal text
viewer (`Transition::OpenTextViewer`) — even if the file is actually, say, a PNG. Fix this: run a
content-detection job first; if it identifies a known non-text kind, open via the OS default instead of
force-feeding binary bytes into the text viewer. If detection comes back `Unknown`, fall through to the
text viewer exactly as today (unchanged).

1. Find the final fallback branch in `EnterDirectory` — search `rwf-lib/src/input/mod.rs` for
   `"EnterDirectory: opening text viewer for {}"` (a debug! log line marking this exact branch, per the
   Task 2 implementer's read of this file — verify it's still there). Currently it unconditionally
   returns `vec![Transition::OpenTextViewer { location: entry.location.clone() }]`.

2. Change this branch to instead return
   `vec![Transition::CheckFallbackFileType { path: entry.location.display_path().into(), location: entry.location.clone() }]`
   — a new transition (add to `rwf-lib/src/state/mod.rs` near `ExecuteAssociationChecked`) carrying both
   the real filesystem path (for the detection job to read) and the original `Location` (needed to build
   `OpenTextViewer` if detection comes back Unknown, since `OpenTextViewer` takes a `Location` not a path).

3. Handler for `CheckFallbackFileType` in `rwf-lib/src/state/handlers/ui.rs`: starts
   `JobKind::DetectFileType { path, purpose: DetectFileTypePurpose::FallbackOpen }` (this purpose
   variant already exists from Task 1, currently unused — you're the first consumer) via
   `StateUpdateResult::with_job(...)`. You'll need to stash the `location` somewhere for the completion
   handler to retrieve — check whether `JobKind::DetectFileType`'s existing shape (from Task 1,
   `{path, purpose}`) has room for this, or whether `DetectFileTypePurpose::FallbackOpen` (currently a
   unit variant per Task 1) needs to gain a field, e.g. `FallbackOpen { location: Location }`, so the
   completion handler doesn't need a separate correlation mechanism. Changing `FallbackOpen` from a unit
   variant to carry a field is expected and fine — nothing else constructs it yet (verify with
   `grep -rn "DetectFileTypePurpose::FallbackOpen" rwf-lib/src` — you should only find the enum
   definition and any tests Task 1 wrote using it as a placeholder purpose; update those if they exist).

4. Completion routing in `rwf-lib/src/state/handlers/job.rs`'s `handle_job_transition`: find the
   `JobKind::DetectFileType { path, purpose }` arm added by Task 2 (it currently has a no-op case for
   `DetectFileTypePurpose::FallbackOpen | DetectFileTypePurpose::FileInfoDisplay` — search for a comment
   mentioning "Tasks 5 and 6" or similar). Replace the `FallbackOpen` half of that no-op with real logic:
   - `DetectedKind::Unknown` → push `crate::job::JobSpec` is NOT needed here (no job) — instead return a
     transition-like effect: since this is inside a job-completion handler (not `input/mod.rs`), you
     can't return a `Transition` from `Action::EnterDirectory`'s dispatch anymore (that already
     happened). Instead, directly call `rwf_lib::state::update_state`-equivalent logic inline, or —
     simpler and consistent with how this same function handles other nested transitions (see the
     `CompareFiles` arm, which builds a nested `Transition::ShowComparisonView` and applies it) — apply
     `Transition::OpenTextViewer { location }` **recursively via this same `AppState`'s transition
     dispatch** (find how `CompareFiles`'s arm does this — it likely calls
     `self.handle_view_transition(&Transition::ShowComparisonView { diff })` or similar internal
     dispatch, not the public `update_state` free function, since `self` is already `&mut AppState`
     here). Mirror that exact pattern for `OpenTextViewer` (find whichever internal handler function
     already implements `Transition::OpenTextViewer` — it's called from `EnterDirectory`'s existing
     code paths today, so grep `Transition::OpenTextViewer =>` to find its handler and see if it's
     reachable as an internal method call from here).
   - Known non-`Unknown` kind → same pattern, but apply `Transition::OpenWithSystem { path: <display path> }`
     instead (its handler already exists, from Task 2's investigation: `state/handlers/ui.rs` around the
     `Transition::OpenWithSystem` arm — reuse it the same recursive-dispatch way).

## Context

This task is independent of Tasks 2/3/4's dialog/picker work — it only touches the *final* fallback
branch of `EnterDirectory` (after both `ExtensionAssociation` and `FileTypeMapping` have already been
checked and found no match), which none of the other tasks modify. It depends only on Task 1's job
foundation (`JobKind::DetectFileType`, `DetectFileTypePurpose::FallbackOpen`).

The tricky part is architectural: unlike Tasks 2/3 (which route a job's mismatch result into *opening a
new dialog*, a simple `self.dialogs.push(...)`), this task needs the job's result to trigger a *different
already-existing transition's effect* (`OpenTextViewer` or `OpenWithSystem`) from inside the job-completion
handler. Read how the existing `handle_job_transition` function invokes other transitions' logic
internally (the `CompareFiles`/`ShowComparisonView` arm is your best model — read it fully before writing
your own arm) rather than guessing at an approach. If `AppState` genuinely has no internal way to
"replay" a transition's effect from within another handler, and the only option is to duplicate
`OpenTextViewer`'s/`OpenWithSystem`'s handler logic inline, that's acceptable — but check for the
internal-dispatch pattern first, since duplicating logic here would be worse.

## Your Job

1. Implement the above.
2. Tests: extend `rwf-lib/src/file_open_integration_tests.rs` with: unregistered extension + PNG magic
   bytes on disk → ends up at `Transition::OpenWithSystem` (drive it through the real
   `EnqueueJob → StartNextJob → CompleteJob` lifecycle, following the pattern Task 2's tests already
   established in this same file — read them first); unregistered extension + plain text content →
   ends up at `Transition::OpenTextViewer`, unchanged from today's behavior. You'll need a real temp
   file with real magic bytes for the detection job to read, same as Task 1's job_executor tests — check
   `rwf-lib/src/test_utils.rs` for any temp-file fixture helpers before writing your own.
3. Verify: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test -p rwf-lib file_open -- --test-threads=1`, `cargo test -p rwf-lib enter_directory -- --test-threads=1`,
   `cargo test -p rwf -- --test-threads=1`, `cargo test -p rwf --no-run`, `cargo test -p rwf-lib --no-run`.
4. Commit `feat(7.3):` prefixed, ending with:
   ```
   Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
   ```
5. Self-review, report using the standard format.

**Escalate (BLOCKED) if**: there's no internal way to replay `OpenTextViewer`/`OpenWithSystem`'s effect
from inside `handle_job_transition` and duplicating their logic feels architecturally wrong — describe
what you found and let the controller decide rather than guessing at a workaround.
```

---

## TASK 6: File Info dialog on-demand detection (Section 7)

Depends on: Task 1 only. Independent of Tasks 2–5.

```
You are implementing Task 6 of 7 for Phase 7.3 "Smart File Opener" in the rwf file manager. Work from
the git worktree at C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener
(branch worktree-feat-7.3-smart-file-opener).

## Task Description

Section 7 of plan/7.3.smart_file_opener.md. The File Information dialog (`i` key /
`Action::ShowFileInfoForCursor`, `rwf-lib/src/model/dialog/file_info.rs` /
`rwf-bin/src/ui/dialog/file_info.rs`) currently shows static metadata only (name, path, size, dates,
permissions, symlink info) — built synchronously from `std::fs::metadata`, no job involved. Add an
on-demand action: while the dialog is open, pressing a key starts a background content-detection job,
and when it completes, the dialog (still open) gains an appended "Detected type: PNG image" line (or a
mismatch annotation if the detected type disagrees with the file's declared type).

1. **Add fields to `FileInfoDialog`** (`rwf-lib/src/model/dialog/file_info.rs`):
   ```rust
   pub detected_type: Option<String>,       // e.g. "PNG image" or "PNG image (mismatch — extension implies .txt)"
   pub detecting: bool,                      // true while the job is in flight, for a "Detecting..." indicator
   pub detected_type_job_id: Option<crate::job::JobId>,
   ```
   Update the `new()` constructor to accept/default these (it already takes many args behind
   `#[allow(clippy::too_many_arguments)]` — add the new fields defaulted to `None`/`false`/`None` inside
   the constructor body rather than as new parameters, to avoid growing the already-long parameter list
   further; only add parameters if you have a good reason not to default them internally).

2. **New key binding inside the dialog.** Find how `FileInfoDialog`'s key handling currently works —
   per earlier investigation, only Enter/Esc (close) are handled today, no other keys. Add a new key
   (pick something unused in this context — check `rwf-lib/resources/default_keybindings.json` and the
   dialog's existing input handler for what's already bound; `d` for "detect" is a reasonable choice if
   free) that, when the dialog is open and `detecting` is false, dispatches a new
   `Transition::DetectFileInfoType { path: PathBuf }` instead of closing the dialog.

3. **New transition + handler.** `Transition::DetectFileInfoType { path: PathBuf }` in
   `rwf-lib/src/state/mod.rs`. Handler in `rwf-lib/src/state/handlers/ui.rs`: build
   `JobKind::DetectFileType { path, purpose: DetectFileTypePurpose::FileInfoDisplay }` (this purpose
   variant already exists from Task 1, currently unused), get its `job_id`, then mutate the *currently
   open* `FileInfoDialog` in place — set `detecting = true` and `detected_type_job_id = Some(job_id)` —
   before returning `StateUpdateResult::with_job(job_spec)`. Model this exactly on how
   `Transition::ShowJumpToPathDialog`'s handler stores `loading_job_id` on the dialog it just pushed
   (`rwf-lib/src/state/handlers/ui.rs`, search `JumpToPathDialog { loading_job_id, .. }`) — same pattern,
   except here the dialog already exists on the stack (you're finding and mutating it, not creating a
   new one): `if let Some(DialogContent::FileInfo(d)) = self.dialogs.current_mut().map(|d| &mut d.content) { ... }`
   (check the exact accessor name — `current_mut()` or similar — on whatever the dialog-stack type is;
   grep `self.dialogs.current()`'s usages for the immutable version and find its `_mut` counterpart, or
   confirm one doesn't exist and you need `self.dialogs.stack.last_mut()` instead).

4. **Completion routing.** In `rwf-lib/src/state/handlers/job.rs`'s `handle_job_transition`, find the
   `JobKind::DetectFileType { path, purpose }` arm (added by Task 2, currently has a no-op case for
   `FallbackOpen | FileInfoDisplay`). Replace the `FileInfoDisplay` half: on success, scan
   `self.dialogs.stack` (not just the top — the user might have opened something else on top of File
   Info since starting detection, though that's an edge case) for a `DialogContent::FileInfo` whose
   `detected_type_job_id == Some(job_id)`, following exactly the pattern `JobKind::CollectJumpCandidates`'s
   arm uses to find its target dialog by `loading_job_id` (read that arm fully — it's in this same file,
   iterates `self.dialogs.stack.iter_mut().rev()`). When found: compute the label via
   `crate::magic::DetectedKind::label()`, check `crate::magic::is_mismatch(extension, kind)` for the
   mismatch annotation (extension derived from the dialog's own `file_path`/`file_name` field — check
   which one has the raw extension available), set `detected_type = Some(label or "label (mismatch — extension implies .X)")`,
   `detecting = false`, `detected_type_job_id = None`, set `result_obj.ui_changed = true`.

5. **Rendering.** In `rwf-bin/src/ui/dialog/file_info.rs`'s `render_file_info_dialog`, append a new line
   after the existing metadata rows: if `detecting` is true, show "Detecting..."; if `detected_type` is
   `Some(s)`, show `format!("Detected type: {}", s)`; if both are absent/false, show nothing extra (don't
   clutter the dialog when detection was never triggered). Update the height calculation for
   `DialogContent::FileInfo` in `rwf-bin/src/ui/dialog/mod.rs` (currently `11u16`/`12u16` depending on
   `link_target` — add +1 when either `detecting` or `detected_type.is_some()`). Add the new key hint to
   whatever hint line already shows "[Enter]/[Esc]: close".

## Context

This task is independent of Tasks 2–5's work (different dialog, different trigger). It depends only on
Task 1's job foundation. `DetectFileTypePurpose::FileInfoDisplay` is a unit variant already defined by
Task 1 — you shouldn't need to add fields to it (unlike Task 5's `FallbackOpen`), since this task
correlates via the dialog's own `detected_type_job_id`, not via data carried in the job's purpose.

Per plan §7: "既存の i（ShowFileInfoForCursor）は変更しない。ダイアログを開いた時点では検出を自動実行し
ない" — do NOT trigger detection automatically when the dialog opens; only on the new explicit keypress.

## Your Job

1. Implement the above.
2. Tests: a `rwf-lib` unit/integration test driving `Transition::DetectFileInfoType` →
   `EnqueueJob → StartNextJob → CompleteJob` and asserting the open `FileInfoDialog`'s `detected_type`
   field is populated correctly (both mismatch and non-mismatch cases), following the same
   real-lifecycle-driving pattern Task 2 established in `file_open_integration_tests.rs`. A snapshot
   test in `rwf-bin` for the dialog rendering with `detected_type = Some("PNG image")` set, following
   the pattern in `rwf-bin/src/ui/dialog/snapshot_tests/` (check if `file_info.rs` already has snapshot
   tests to extend, or if you need a new file).
3. Verify: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test -p rwf-lib file_open -- --test-threads=1`, `cargo test -p rwf-lib file_info -- --test-threads=1`,
   `cargo test -p rwf -- --test-threads=1`, `cargo test -p rwf --no-run`, `cargo test -p rwf-lib --no-run`.
4. Commit `feat(7.3):` prefixed, ending with:
   ```
   Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
   ```
5. Self-review, report using the standard format.

**Escalate if**: the dialog-stack type has no mutable accessor for "the current/top dialog" or for
iterating-with-mutation over the full stack — that would mean the `CollectJumpCandidates`/`JumpToPath`
pattern this task is modeled on doesn't actually work the way described, which would be surprising and
worth confirming with the controller before working around it.
```

---

## TASK 7: Logging sweep + full verification + final review (Section 8/9)

Depends on: Tasks 2, 3, 4, 5, 6 all committed.

```
You are doing the final pass for Phase 7.3 "Smart File Opener" in the rwf file manager — Task 7 of 7.
Work from the git worktree at C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener
(branch worktree-feat-7.3-smart-file-opener). Confirm `git log --oneline -20` shows commits for Tasks
2 through 6 (mismatch warning, single-file Open With picker, batch Open With, fallback strengthening,
File Info on-demand detection) before starting; if any are missing, STOP and report which, do not
improvise the missing task's scope yourself.

## Task Description

Sections 8 and 9 of plan/7.3.smart_file_opener.md, plus a full-suite verification pass.

### Part A — Logging sweep (Section 8)

Plan §8 says: no new persistent logging mechanism — everything rides on the existing task panel
(`task_panel.add_log`, surfaced generically via `StateUpdateResult.task_panel_logs` — grep `task_panel_logs`
in `rwf-bin/src/app.rs` to see the two drain sites that already exist and require no new plumbing, just
someone pushing strings into that vec). Expected log lines per plan §8:
- Mismatch warning shown: `[Warning] Type mismatch: notes.txt looks like PE executable (declared: text/plain)`
- Ordinary on-demand detection (e.g. File Info): `[System] Detected type: PNG image for photo.jpg`

Audit every place Tasks 2–6 push a `TypeMismatchWarning` dialog, auto-continue an association after a
non-mismatch, resolve a fallback-open decision, or complete a File Info on-demand detection — check
whether each of these already pushes a matching line into `result_obj.task_panel_logs` (or equivalent).
If any are missing, add them, following the exact log-line format shown above (adapt wording per event,
keep the `[Warning]`/`[System]` prefix convention consistent with how other features in this codebase
tag their task-panel log lines — grep existing `task_panel_logs.push(format!(` call sites for the house
style before adding new ones).

### Part B — Test coverage sweep (Section 9)

Re-read plan §9 in full. Checklist, verify each already has coverage from Tasks 2–6 (don't re-add if it
does — just confirm and note where):
- `magic.rs` unit tests (table-driven, per signature) — done in Task 1, no action needed, just confirm.
- `file_open_integration_tests.rs`: mismatch warning show/confirm/cancel branches; unregistered-extension
  fallback (PNG → OsDefault routing); multi-candidate picker construction (0/1/2+ branches).
- Dialog snapshot tests: `TypeMismatchWarning` and the Open With picker, following the existing insta
  snapshot pattern (both dialogs, both standard sizes — check `snapshot_tests/` for what "both sizes"
  means in this codebase's existing tests and confirm the new ones match).
- Network-drive-latency-equivalent case ("existing Job cancellation/timeout pattern") — confirm
  `DetectFileTypesBatch`'s cancellation (Task 1, already tested) is the closure of this requirement; no
  new test needed unless you find a gap.

For any checklist item with a real gap (not just "could be more thorough" — an actual untested branch),
add the test. Don't gold-plate items that are already adequately covered.

### Part C — Full verification

Run the complete verification pipeline, not filtered subsets:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p rwf -- --test-threads=1
cargo test -p rwf-lib -- --test-threads=1
```
The full `rwf-lib` suite takes ~35-40 minutes single-threaded — this is expected, let it run
(`run_in_background` if your environment supports it, or just wait). Do not substitute a filtered subset
for this final pass; the whole point is catching cross-feature regressions Tasks 2–6 might have
introduced in unrelated areas (e.g. did the `EnterDirectory` refactor in Task 3 break something in an
unrelated dialog test?). Paste the final pass/fail counts.

If anything fails: this is a real regression from one of Tasks 2–6's work. Investigate which commit
introduced it (`git bisect` or just reading the failing test against the task commits), fix it in a new
`fix(7.3):` commit (do not amend the original task's commit), and re-run the full suite to confirm green
before proceeding.

### Part D — Dispatch a final reviewer

Once Parts A–C are done and the full suite is green, request one more code review — this time of the
**entire Phase 7.3 diff**, not just this task's changes, since no one has looked at the whole feature
end-to-end yet. If you have access to a code-reviewing tool/skill/agent in your environment, use it;
otherwise, if you are a capable-enough model to review your own multi-task feature honestly, do a
structured self-review pass instead: `git diff 7839526..HEAD` (that's the commit right before Task 1
started — i.e. the entire Phase 7.3 diff) and check for:
- Cross-task consistency: do Tasks 2/3/5/6 all reach `JobKind::DetectFileType` via `state/handlers/job.rs`'s
  single `DetectFileType` match arm cleanly, or did later tasks awkwardly bolt onto earlier ones' code?
- Any leftover no-op/placeholder code from Task 2's original `FallbackOpen | FileInfoDisplay => {}` stub
  that Tasks 5/6 should have replaced but didn't fully clean up.
- Any TODO comments, `#[allow(...)]` suppressions, or dead code left behind across the whole feature.
- Whether `plan/ROADMAP.md`'s Phase 7 table (search for "7.3" — currently marked `[ ]` not started) should
  now be updated to `[x]` complete — if so, update it as part of this task's commit, following the exact
  formatting convention used by other completed Phase 7 rows (e.g. row 7.1 Leap Navigation) in that file.

## Your Job

1. Complete Parts A–D above.
2. Final commit(s): `feat(7.3):`/`fix(7.3):`/`docs:` prefixed as appropriate, each ending with:
   ```
   Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
   ```
3. Report using the standard format: what logging gaps you found/fixed, what test gaps you found/fixed,
   full verification pass/fail counts (paste the actual final lines of each cargo command's output, not
   a paraphrase), findings from Part D's review, whether ROADMAP.md was updated, final commit SHA(s).

This is the last task — after this, the whole worktree branch should be ready for the controller to
decide on merge/PR via the project's normal branch-completion process. Do not merge or push yourself;
just report readiness.
```

---

## Generic review prompts (reuse for every task above)

After each implementer reports DONE, run these two in order. Fill `{TASK_TEXT}` with the exact numbered
requirements from that task's prompt above, `{IMPLEMENTER_REPORT}` with what they reported, and the SHAs.

### Spec compliance review (paste into a fresh session)

```
You are reviewing whether an implementation matches its specification. Work from the git worktree at
C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener. Review commit
{HEAD_SHA} (diff against {BASE_SHA}: `git diff {BASE_SHA} {HEAD_SHA}`).

## What Was Requested
{TASK_TEXT}

## What Implementer Claims They Built
{IMPLEMENTER_REPORT}

## CRITICAL: Do Not Trust the Report
Read the actual diff. Verify every claimed item is genuinely present, with the exact types/fields/behavior
specified — not an approximation. Check for missing requirements, extra/unrequested work, and
misunderstandings of what was asked. Independently run the test commands the implementer claims passed.

Report:
- ✅ Spec compliant (if everything matches after code inspection and command verification)
- ❌ Issues found: [specifically what's missing, extra, or misimplemented, with file:line references]
```

### Code quality review (paste into a fresh session, only after spec review passes)

```
You are a Senior Code Reviewer with Rust expertise. Review completed work against requirements and code
quality standards. Work from the git worktree at
C:\Users\user\source\repos\panzoux\rwf\.claude\worktrees\feat-7.3-smart-file-opener.

## What Was Implemented
{IMPLEMENTER_REPORT}

## Requirements
{TASK_TEXT}
(Already passed spec-compliance review — don't re-litigate whether it matches spec, focus on quality.)

## Git Range
Base: {BASE_SHA}   Head: {HEAD_SHA}
`git diff --stat {BASE_SHA}..{HEAD_SHA}` then `git diff {BASE_SHA}..{HEAD_SHA}`

## What to Check
- Code quality: separation of concerns, error handling (zero new unwrap() in production code —
  clippy::unwrap_used is deny workspace-wide), DRY without premature abstraction, edge cases.
- Architecture: sound design, integrates cleanly with existing Job/Transition/Dialog patterns, no
  "ghost dialog" risk (CLAUDE.md's known failure mode — dialogs must be poppable cleanly), no permanent
  is_loading risk for any job that touches pane state.
- Testing: tests verify real behavior via the actual state machine, not shortcuts; edge cases covered;
  all tests actually passing (verify yourself, don't trust the report).
- File organization: proportionate growth, one responsibility per new file.

## Output Format
### Strengths
### Issues
#### Critical (Must Fix)
#### Important (Should Fix)
#### Minor (Nice to Have)
### Recommendations
### Assessment
**Ready to merge?** [Yes | No | With fixes]
**Reasoning:**
```

If the code-quality reviewer returns "With fixes" or "No", paste its Critical/Important findings back
into the *same* implementer session (or a fresh one with the diff + findings if starting fresh) and ask
for fixes as a new commit, then re-run the code quality review until it's clean before moving to the
next numbered task.
