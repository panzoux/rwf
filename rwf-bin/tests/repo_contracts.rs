//! Guards for repo-wide contracts that no single module owns.
//!
//! Each test here encodes a rule that is **cross-cutting**: code that is perfectly
//! correct inside its own module can still break it, and nothing else in the build
//! would notice. See `docs/IMPLICIT_CONTRACTS.md` for the rationale behind each one.
//!
//! Why these live in rwf-bin's integration-test target rather than in rwf-lib:
//! `cargo test -p rwf` runs in seconds and is the first thing CI executes, while
//! `cargo test -p rwf-lib` costs a full rebuild of the larger crate's test binary.
//! A contract violation should surface in the fast suite. Several of these guards
//! are static scans over the *whole* workspace source tree, so they are not really "rwf-bin tests" at all — they just
//! need a fast home.
//!
//! Adding a legitimate exception means editing the allowlist in this file *and*
//! the corresponding section of `docs/IMPLICIT_CONTRACTS.md`. That friction is the
//! point: it forces the decision to be explicit and reviewable.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// Unwrap without `unwrap()`/`expect()`, which clippy denies workspace-wide and
/// does not reliably exempt in integration-test targets.
fn ok<T, E: std::fmt::Debug>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{}: {:?}", context, err),
    }
}

/// Workspace root (the parent of `rwf-bin/`).
fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    match manifest_dir.parent() {
        Some(parent) => parent.to_path_buf(),
        None => panic!("CARGO_MANIFEST_DIR {:?} has no parent", manifest_dir),
    }
}

/// Path relative to the workspace root, with forward slashes, for stable
/// allowlist keys and failure messages on both Windows and Unix.
fn rel(root: &Path, path: &Path) -> String {
    let stripped = path.strip_prefix(root).unwrap_or(path);
    stripped.to_string_lossy().replace('\\', "/")
}

/// Recursively collect files with the given extension, skipping build output.
fn collect_files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    let entries = ok(
        std::fs::read_dir(dir),
        &format!("cannot read directory {:?}", dir),
    );
    for entry in entries {
        let entry = ok(entry, "cannot read directory entry");
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "target" {
                continue;
            }
            collect_files(&path, extension, out);
        } else if path.extension().map(|e| e == extension).unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Recursively collect every directory under `dir`, skipping build output.
fn collect_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    out.push(dir.to_path_buf());
    let entries = ok(
        std::fs::read_dir(dir),
        &format!("cannot read directory {:?}", dir),
    );
    for entry in entries {
        let entry = ok(entry, "cannot read directory entry");
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "target" {
                continue;
            }
            collect_dirs(&path, out);
        }
    }
}

/// This file, relative to the workspace root.
///
/// Excluded from the token scans below: it necessarily spells out every token it
/// searches for, so it would always match itself. That leaves exactly one blind
/// spot — this file — which contains no product code.
const SELF_PATH: &str = "rwf-bin/tests/repo_contracts.rs";

/// Every Rust source file in the workspace (both crates, `src/` and `tests/`),
/// excluding this file.
fn workspace_rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for sub in [
        "rwf-lib/src",
        "rwf-lib/tests",
        "rwf-bin/src",
        "rwf-bin/tests",
    ] {
        collect_files(&root.join(sub), "rs", &mut files);
    }
    files.retain(|f| rel(root, f) != SELF_PATH);
    files.sort();
    files
}

/// True when `needle` occurs in `haystack` as a standalone identifier, i.e. not
/// glued to an adjacent word character. Without this, `println!` matches inside
/// `eprintln!` and every stderr call would be flagged as a stdout write.
fn contains_token(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0usize;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

// ---------------------------------------------------------------------------
// Contract 1 — stdout is a data channel, not a message channel
// ---------------------------------------------------------------------------

/// Every stdout write allowed in `rwf-bin`, as
/// `(path relative to workspace root, exact trimmed source line, why)`.
///
/// The contract: `rwf` is invoked from shell wrappers that capture stdout and
/// `cd` to it (`docs/USER_GUIDE.md`, `scripts/rwf-cd.*`). Anything printed to
/// stdout on the interactive path becomes part of that captured value and breaks
/// directory-on-exit. Diagnostics go to `eprintln!` or `tracing`.
const ALLOWED_STDOUT_WRITES: &[(&str, &str, &str)] = &[
    (
        "rwf-bin/src/main.rs",
        "print!(",
        "--export-function-list: dumps JSON to stdout and returns before the TUI starts",
    ),
    (
        "rwf-bin/src/main.rs",
        "println!(\"{}\", exit_dir);",
        "the exit directory itself — this IS the data the shell wrapper consumes",
    ),
    (
        "rwf-bin/src/main.rs",
        "println!(\"skipped (exists): {}\", path.display());",
        "--export-config-files: progress report, returns before the TUI starts",
    ),
    (
        "rwf-bin/src/main.rs",
        "println!(\"written:          {}\", path.display());",
        "--export-config-files: progress report, returns before the TUI starts",
    ),
    (
        "rwf-bin/src/terminal.rs",
        "let mut stdout = io::stdout();",
        "the ratatui backend handle — writes go through the alternate screen",
    ),
    (
        "rwf-bin/src/app.rs",
        "std::io::stdout(),",
        "SuspendAndRun: crossterm alternate-screen enter/leave target, not text output",
    ),
];

/// Tokens that put bytes on the process's stdout. `stdout()` (with the call
/// parens) rather than a bare `stdout`, so a local variable holding the handle
/// is not mistaken for a second acquisition of it.
const STDOUT_TOKENS: &[&str] = &["println!", "print!", "stdout()"];

#[test]
fn stdout_writes_in_rwf_bin_are_allowlisted() {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_files(&root.join("rwf-bin/src"), "rs", &mut files);
    files.sort();

    let mut unexpected: Vec<String> = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();

    for file in &files {
        let relative = rel(&root, file);
        let contents = ok(
            std::fs::read_to_string(file),
            &format!("cannot read {:?}", file),
        );
        for (index, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if !STDOUT_TOKENS
                .iter()
                .any(|token| contains_token(trimmed, token))
            {
                continue;
            }
            let allowed = ALLOWED_STDOUT_WRITES
                .iter()
                .any(|(f, text, _)| *f == relative && trimmed.starts_with(text));
            if allowed {
                let entry = ALLOWED_STDOUT_WRITES
                    .iter()
                    .find(|(f, text, _)| *f == relative && trimmed.starts_with(text));
                if let Some((f, text, _)) = entry {
                    seen.insert(((*f).to_string(), (*text).to_string()));
                }
            } else {
                unexpected.push(format!("{}:{}: {}", relative, index + 1, trimmed));
            }
        }
    }

    assert!(
        unexpected.is_empty(),
        "stdout is reserved for the exit directory consumed by the shell `cd` wrappers \
         (docs/USER_GUIDE.md, scripts/rwf-cd.*). These writes are not allowlisted:\n  {}\n\n\
         Use `eprintln!` or `tracing` for anything the user reads, or add an entry to \
         ALLOWED_STDOUT_WRITES in rwf-bin/tests/repo_contracts.rs *and* to the matching \
         section of docs/IMPLICIT_CONTRACTS.md if the write genuinely happens before the \
         TUI starts or after it is torn down.",
        unexpected.join("\n  ")
    );

    // A stale allowlist is a silent hole: it would keep permitting a write that no
    // longer exists, and could later match some unrelated new line.
    let stale: Vec<String> = ALLOWED_STDOUT_WRITES
        .iter()
        .filter(|(f, text, _)| !seen.contains(&((*f).to_string(), (*text).to_string())))
        .map(|(f, text, _)| format!("{}: {}", f, text))
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED_STDOUT_WRITES has entries that match nothing any more; delete them:\n  {}",
        stale.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Contract 2 — .gitignore must never shadow a source path
// ---------------------------------------------------------------------------

fn gitignore_matcher(root: &Path) -> ignore::gitignore::Gitignore {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    if let Some(err) = builder.add(root.join(".gitignore")) {
        panic!("cannot parse .gitignore: {:?}", err);
    }
    ok(builder.build(), "cannot build .gitignore matcher")
}

/// Non-comment, non-negated patterns from `.gitignore`, trimmed.
fn gitignore_patterns(root: &Path) -> Vec<String> {
    let contents = ok(
        std::fs::read_to_string(root.join(".gitignore")),
        "cannot read .gitignore",
    );
    contents
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
        .collect()
}

#[test]
fn gitignore_does_not_shadow_existing_source_paths() {
    let root = workspace_root();
    let matcher = gitignore_matcher(&root);

    let mut checked = 0usize;
    let mut shadowed: Vec<String> = Vec::new();

    // `resources/` is in the list because its files are pulled in with `include_str!`.
    // A shadowed source module fails silently; a shadowed resource fails the build —
    // but only for whoever clones fresh, long after the author has stopped looking.
    for sub in [
        "rwf-lib/src",
        "rwf-lib/tests",
        "rwf-lib/resources",
        "rwf-bin/src",
        "rwf-bin/tests",
    ] {
        let base = root.join(sub);
        if !base.is_dir() {
            continue;
        }
        let mut dirs = Vec::new();
        collect_dirs(&base, &mut dirs);
        for dir in &dirs {
            checked += 1;
            if matcher.matched_path_or_any_parents(dir, true).is_ignore() {
                shadowed.push(format!("{}/ (directory)", rel(&root, dir)));
            }
        }
        let mut files = Vec::new();
        collect_files(&base, "rs", &mut files);
        for file in &files {
            checked += 1;
            if matcher.matched_path_or_any_parents(file, false).is_ignore() {
                shadowed.push(rel(&root, file));
            }
        }
    }

    assert!(
        checked > 100,
        "expected to check the whole source tree, only saw {} paths — the walk is broken",
        checked
    );
    assert!(
        shadowed.is_empty(),
        "these source paths are matched by .gitignore and would vanish from `git status` \
         (they would never be committed, and the loss is silent):\n  {}\n\n\
         Anchor the offending pattern with a leading slash (`/name/` instead of `name/`).",
        shadowed.join("\n  ")
    );
}

#[test]
fn gitignore_patterns_cannot_shadow_a_future_source_subdirectory() {
    let root = workspace_root();
    let matcher = gitignore_matcher(&root);

    // Only directory-shaped, glob-free patterns can plausibly collide with a source
    // module name. `*.bak` / `qwen-code-export-*` name artefacts, not modules.
    let candidates: Vec<String> = gitignore_patterns(&root)
        .into_iter()
        .filter(|p| !p.contains('*') && !p.contains('?') && !p.contains('['))
        .map(|p| p.trim_end_matches('/').trim_start_matches('/').to_string())
        .filter(|p| !p.is_empty())
        .collect();

    assert!(
        !candidates.is_empty(),
        "no .gitignore patterns were parsed — the test is not actually checking anything"
    );

    let mut hazards: Vec<String> = Vec::new();
    for pattern in &candidates {
        for src in ["rwf-lib/src", "rwf-bin/src"] {
            let hypothetical = root.join(src).join(pattern);
            if matcher
                .matched_path_or_any_parents(&hypothetical, true)
                .is_ignore()
            {
                hazards.push(format!(
                    "a source module at {}/{}/ would be silently ignored",
                    src, pattern
                ));
            }
        }
    }

    assert!(
        hazards.is_empty(),
        "unanchored .gitignore patterns shadow plausible source module names:\n  {}\n\n\
         An unanchored pattern like `logs/` matches at *every* depth, including \
         `rwf-lib/src/logs/`. Anchor it to the repo root: `/logs/`.",
        hazards.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Contract 3 — Rust sources and Cargo manifests are LF
// ---------------------------------------------------------------------------

/// Every `.toml` the LF rule covers, relative to the workspace root.
///
/// Kept explicit rather than discovered by a recursive walk: `.claude/worktrees/`
/// can hold additional checkouts of this same repo, and a walk from the root would
/// scan *those* manifests too, making the result depend on which worktrees happen
/// to exist on the machine. `lf_toml_allowlist_is_complete` below keeps the list
/// honest without needing the walk.
const LF_TOML_FILES: &[&str] = &[
    ".cargo/config.toml",
    "Cargo.toml",
    "clippy.toml",
    "rustfmt.toml",
    "rwf-bin/Cargo.toml",
    "rwf-lib/Cargo.toml",
];

/// Directories that may hold a `.toml` subject to the LF rule, scanned one level
/// deep (no recursion — see the note on `LF_TOML_FILES`). `""` is the root itself.
const LF_TOML_DIRS: &[&str] = &["", ".cargo", "rwf-bin", "rwf-lib"];

#[test]
fn rust_sources_and_manifests_use_lf_line_endings() {
    let root = workspace_root();
    let mut files = workspace_rust_sources(&root);
    for manifest in LF_TOML_FILES {
        let path = root.join(manifest);
        if path.is_file() {
            files.push(path);
        }
    }

    let mut crlf: Vec<String> = Vec::new();
    for file in &files {
        let bytes = ok(std::fs::read(file), &format!("cannot read {:?}", file));
        if bytes.windows(2).any(|w| w == b"\r\n") {
            crlf.push(rel(&root, file));
        }
    }

    assert!(
        crlf.is_empty(),
        "these files contain CRLF; the repo is LF-only for Rust sources and manifests \
         (see .gitattributes and CLAUDE.md). A CRLF rewrite turns a one-line change into \
         a whole-file diff and hides the real edit in review:\n  {}\n\n\
         On Windows, PowerShell's Set-Content/Out-File rewrite whole files as CRLF — use \
         an LF-preserving editor instead. To repair: `git add --renormalize <file>`.",
        crlf.join("\n  ")
    );
}

/// The LF check above can only guard files it knows about. A new `.toml` added to
/// the workspace would be silently unguarded — which is exactly how a CRLF
/// `.cargo/config.toml` got in. This scans the directories the rule covers, one
/// level deep, and fails if any `.toml` there is missing from `LF_TOML_FILES`.
#[test]
fn lf_toml_allowlist_is_complete() {
    let root = workspace_root();
    let known: BTreeSet<&str> = LF_TOML_FILES.iter().copied().collect();
    let mut missing: Vec<String> = Vec::new();

    for dir in LF_TOML_DIRS {
        let path = if dir.is_empty() {
            root.clone()
        } else {
            root.join(dir)
        };
        if !path.is_dir() {
            continue;
        }
        let entries = ok(
            std::fs::read_dir(&path),
            &format!("cannot read directory {:?}", path),
        );
        for entry in entries {
            let entry = ok(entry, "cannot read directory entry");
            let file = entry.path();
            if !file.is_file() || file.extension().map(|e| e != "toml").unwrap_or(true) {
                continue;
            }
            let key = rel(&root, &file);
            if !known.contains(key.as_str()) {
                missing.push(key);
            }
        }
    }

    missing.sort();
    assert!(
        missing.is_empty(),
        "these .toml files are not covered by the LF guard:\n  {}\n\n\
         .gitattributes pins `*.toml text eol=lf`, so they are subject to the LF rule \
         but nothing checks them. Add each to LF_TOML_FILES in this file.",
        missing.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Contract 4 — terminal mode transitions stay in one place
// ---------------------------------------------------------------------------

/// Files permitted to change raw mode or the alternate screen.
///
/// Raw mode and the alternate screen are process-global. A code path that leaves
/// one of them and fails to restore it wrecks the user's shell after rwf exits —
/// and there is no test that can observe that, because it happens after teardown.
/// Keeping the transitions in two files makes "is every enter paired with a leave"
/// a question a reviewer can actually answer by reading.
const ALLOWED_TERMINAL_MODE_FILES: &[&str] = &[
    // Owns setup/teardown for the whole process, including the Drop-guard restore.
    "rwf-bin/src/terminal.rs",
    // SuspendAndRun: hands the terminal to a TUI editor and takes it back.
    "rwf-bin/src/app.rs",
];

const TERMINAL_MODE_TOKENS: &[&str] = &[
    "enable_raw_mode",
    "disable_raw_mode",
    "EnterAlternateScreen",
    "LeaveAlternateScreen",
];

#[test]
fn terminal_mode_transitions_are_confined_to_allowlisted_files() {
    let root = workspace_root();
    let files = workspace_rust_sources(&root);
    assert!(!files.is_empty(), "found no Rust sources to scan");

    let mut offenders: BTreeSet<String> = BTreeSet::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for file in &files {
        let relative = rel(&root, file);
        let contents = ok(
            std::fs::read_to_string(file),
            &format!("cannot read {:?}", file),
        );
        let hit = contents
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| {
                TERMINAL_MODE_TOKENS
                    .iter()
                    .any(|token| contains_token(line, token))
            });
        if !hit {
            continue;
        }
        if ALLOWED_TERMINAL_MODE_FILES.contains(&relative.as_str()) {
            seen.insert(relative);
        } else {
            offenders.insert(relative);
        }
    }

    assert!(
        offenders.is_empty(),
        "raw-mode / alternate-screen transitions appear outside the files that own them:\n  {}\n\n\
         Every enter must have a guaranteed matching leave, including on the error path — \
         a missed one leaves the user's shell in raw mode after rwf exits. Route the change \
         through rwf-bin/src/terminal.rs, or extend ALLOWED_TERMINAL_MODE_FILES here and \
         document the new owner in docs/IMPLICIT_CONTRACTS.md.",
        offenders.into_iter().collect::<Vec<_>>().join("\n  ")
    );

    let stale: Vec<&str> = ALLOWED_TERMINAL_MODE_FILES
        .iter()
        .filter(|f| !seen.contains(**f))
        .copied()
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED_TERMINAL_MODE_FILES lists files that no longer touch terminal mode; \
         remove them so the allowlist keeps meaning something:\n  {}",
        stale.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Contract 5 — cmd.exe invocations are rare and reviewed
// ---------------------------------------------------------------------------

/// Files permitted to spawn `cmd.exe`.
///
/// Every such invocation must pass `/D` before `/C` so cmd.exe skips the user's
/// `HKCU\Software\Microsoft\Command Processor\AutoRun` hook (Clink and friends).
/// Without `/D` that hook runs inside our transient shell and prints to whatever
/// console it inherited. `docs/IMPLICIT_CONTRACTS.md` has the full story; the
/// argument order itself is asserted next to each builder in rwf-lib.
const ALLOWED_CMD_EXE_FILES: &[&str] = &[
    "rwf-lib/src/state/helpers.rs",
    "rwf-lib/src/backend/local.rs",
];

/// Only literal spawn sites, not shell-name strings that happen to be `"cmd"`.
const CMD_EXE_TOKENS: &[&str] = &[
    "Command::new(\"cmd\")",
    "Command::new(\"cmd.exe\")",
    "program: \"cmd\"",
    "program: \"cmd.exe\"",
];

#[test]
fn cmd_exe_spawn_sites_are_confined_to_allowlisted_files() {
    let root = workspace_root();
    let files = workspace_rust_sources(&root);

    let mut offenders: BTreeSet<String> = BTreeSet::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for file in &files {
        let relative = rel(&root, file);
        let contents = ok(
            std::fs::read_to_string(file),
            &format!("cannot read {:?}", file),
        );
        let hit = contents
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| CMD_EXE_TOKENS.iter().any(|token| line.contains(token)));
        if !hit {
            continue;
        }
        if ALLOWED_CMD_EXE_FILES.contains(&relative.as_str()) {
            seen.insert(relative);
        } else {
            offenders.insert(relative);
        }
    }

    assert!(
        offenders.is_empty(),
        "new cmd.exe spawn sites found outside the reviewed set:\n  {}\n\n\
         cmd.exe must be invoked as `cmd /D /C ...`. Omitting /D lets the user's AutoRun \
         hook run inside our transient shell and print into the TUI. Add the file to \
         ALLOWED_CMD_EXE_FILES here, assert the /D /C prefix in a unit test next to the \
         builder, and document it in docs/IMPLICIT_CONTRACTS.md.",
        offenders.into_iter().collect::<Vec<_>>().join("\n  ")
    );

    let stale: Vec<&str> = ALLOWED_CMD_EXE_FILES
        .iter()
        .filter(|f| !seen.contains(**f))
        .copied()
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED_CMD_EXE_FILES lists files that no longer spawn cmd.exe; remove them:\n  {}",
        stale.join("\n  ")
    );
}
