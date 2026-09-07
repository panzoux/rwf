//! PipeToAction directive handling
//!
//! This module provides utilities for handling PipeToAction directives
//! after custom function execution completes.

use crate::job::PipeToAction;
use crate::model::Location;
use std::path::{Path, PathBuf};

/// Process a `PipeToAction` directive against a command's stdout.
///
/// `working_dir` is the directory the command ran in. Output that is a *relative*
/// path is resolved against it — fzf and friends emit paths relative to their cwd,
/// so this removes the need for wrapper commands like TWF's
/// `powershell -command join-path (pwd) (fzf.exe)`.
pub fn process_pipe_to_action(
    action: &PipeToAction,
    output: &str,
    working_dir: &Path,
) -> Result<PipeToActionResult, String> {
    // ClipText is the one directive that takes the output as text, not as a path.
    if let PipeToAction::ClipText = action {
        return Ok(PipeToActionResult::ClipText(output.to_string()));
    }

    let raw = output.trim();
    if raw.is_empty() {
        return Err(match action {
            PipeToAction::JumpToPath => "Empty path returned from command".to_string(),
            _ => "Empty file path returned from command".to_string(),
        });
    }
    let path = resolve_against(working_dir, raw);

    match action {
        PipeToAction::JumpToPath => {
            if !path.exists() {
                return Err(format!("Path does not exist: {}", path.display()));
            }
            // A file target navigates to its parent and parks the cursor on the file
            // (the `pending_cursor_name` mechanism the Jump-to-File dialog uses), so
            // pickers that return files — fzf's default walker — land somewhere useful
            // instead of trying to list a file as a directory.
            if path.is_dir() {
                Ok(PipeToActionResult::JumpToPath {
                    location: Location::Local(path),
                    cursor_name: None,
                })
            } else {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .ok_or_else(|| format!("Cannot determine file name: {}", path.display()))?;
                let parent = path
                    .parent()
                    .ok_or_else(|| format!("Path has no parent directory: {}", path.display()))?
                    .to_path_buf();
                Ok(PipeToActionResult::JumpToPath {
                    location: Location::Local(parent),
                    cursor_name: Some(name),
                })
            }
        }
        PipeToAction::ExecuteFile => {
            if !path.exists() {
                return Err(format!("File does not exist: {}", path.display()));
            }
            Ok(PipeToActionResult::ExecuteFile(path))
        }
        PipeToAction::ExecuteFileWithEditor => {
            // File doesn't need to exist for editor (can create new file)
            Ok(PipeToActionResult::ExecuteFileWithEditor(path))
        }
        PipeToAction::ClipText => unreachable!("handled above"),
    }
}

/// Join `candidate` onto `base` only when it has no root at all.
///
/// `has_root()`, deliberately not `is_absolute()`: on Windows `/tmp/x` is rooted but
/// *not* absolute (no drive prefix), so `is_absolute()` would send it through `join`
/// and silently rewrite it to `C:/tmp/x` against whatever drive the pane is on. Only
/// genuinely relative output — which is what a picker like fzf emits — gets joined.
fn resolve_against(base: &Path, candidate: &str) -> PathBuf {
    let path = PathBuf::from(candidate);
    if path.has_root() {
        path
    } else {
        base.join(path)
    }
}

/// Result of processing a PipeToAction directive
#[derive(Debug, Clone)]
pub enum PipeToActionResult {
    /// Navigate to `location`; when `cursor_name` is set, park the cursor on that
    /// entry once the directory finishes loading.
    JumpToPath {
        location: Location,
        cursor_name: Option<String>,
    },
    /// Execute the specified file
    ExecuteFile(PathBuf),
    /// Open the specified file in the configured editor
    ExecuteFileWithEditor(PathBuf),
    /// Copy the command's output to the clipboard
    ClipText(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cwd() -> PathBuf {
        std::env::current_dir().expect("cwd is readable")
    }

    #[test]
    fn jump_to_path_accepts_a_directory() {
        let dir = cwd();
        let result =
            process_pipe_to_action(&PipeToAction::JumpToPath, &dir.to_string_lossy(), &dir)
                .expect("existing directory");

        match result {
            PipeToActionResult::JumpToPath {
                location: Location::Local(path),
                cursor_name,
            } => {
                assert_eq!(path, dir);
                assert_eq!(cursor_name, None, "a directory needs no cursor target");
            }
            other => panic!("expected JumpToPath, got {other:?}"),
        }
    }

    #[test]
    fn jump_to_path_on_a_file_targets_the_parent_and_names_the_cursor() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("picked.txt");
        std::fs::write(&file, b"x").expect("write");

        let result = process_pipe_to_action(
            &PipeToAction::JumpToPath,
            &file.to_string_lossy(),
            tmp.path(),
        )
        .expect("existing file");

        match result {
            PipeToActionResult::JumpToPath {
                location: Location::Local(path),
                cursor_name,
            } => {
                assert_eq!(path, tmp.path());
                assert_eq!(cursor_name.as_deref(), Some("picked.txt"));
            }
            other => panic!("expected JumpToPath, got {other:?}"),
        }
    }

    #[test]
    fn relative_output_resolves_against_the_working_directory() {
        // fzf's walker emits paths relative to its cwd; this is what removes the
        // need for a `join-path (pwd)` wrapper around the picker.
        let tmp = tempfile::tempdir().expect("tempdir");
        let sub = tmp.path().join("nested");
        std::fs::create_dir(&sub).expect("mkdir");

        let result = process_pipe_to_action(&PipeToAction::JumpToPath, "nested", tmp.path())
            .expect("relative dir resolves");

        match result {
            PipeToActionResult::JumpToPath {
                location: Location::Local(path),
                ..
            } => assert_eq!(path, sub),
            other => panic!("expected JumpToPath, got {other:?}"),
        }
    }

    #[test]
    fn jump_to_path_rejects_a_nonexistent_target() {
        let result = process_pipe_to_action(
            &PipeToAction::JumpToPath,
            "definitely-not-here-9f3a",
            &cwd(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn execute_file_with_editor_allows_a_missing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = process_pipe_to_action(
            &PipeToAction::ExecuteFileWithEditor,
            "newfile.txt",
            tmp.path(),
        )
        .expect("editor targets need not exist");

        match result {
            PipeToActionResult::ExecuteFileWithEditor(path) => {
                assert_eq!(path, tmp.path().join("newfile.txt"));
            }
            other => panic!("expected ExecuteFileWithEditor, got {other:?}"),
        }
    }

    #[test]
    fn clip_text_takes_output_as_text_not_as_a_path() {
        // Must not be trimmed, path-resolved, or existence-checked.
        let result = process_pipe_to_action(
            &PipeToAction::ClipText,
            "  hello world
",
            &cwd(),
        )
        .expect("clip text always succeeds");

        match result {
            PipeToActionResult::ClipText(text) => assert_eq!(
                text,
                "  hello world
"
            ),
            other => panic!("expected ClipText, got {other:?}"),
        }
    }

    #[test]
    fn a_rooted_path_is_never_rewritten_against_the_working_directory() {
        // On Windows "/tmp/x" is rooted but not absolute. Resolving with is_absolute()
        // would turn it into "C:/tmp/x"; callers passing rooted paths must be left alone.
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = process_pipe_to_action(
            &PipeToAction::ExecuteFileWithEditor,
            "/tmp/x.txt",
            tmp.path(),
        )
        .expect("editor targets need not exist");

        match result {
            PipeToActionResult::ExecuteFileWithEditor(path) => {
                assert_eq!(path, PathBuf::from("/tmp/x.txt"));
            }
            other => panic!("expected ExecuteFileWithEditor, got {other:?}"),
        }
    }

    #[test]
    fn empty_output_is_an_error_for_path_directives() {
        let result = process_pipe_to_action(&PipeToAction::JumpToPath, "", &cwd());
        assert!(result.unwrap_err().contains("Empty path"));
    }
}
