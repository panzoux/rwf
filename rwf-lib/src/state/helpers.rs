use crate::state::AppConfig;
use crate::state::AppState;

impl AppState {
    fn resolve_editor(config: &AppConfig) -> String {
        config.editor_command.clone().unwrap_or_else(|| {
            #[cfg(target_os = "windows")]
            {
                "notepad".to_string()
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::env::var("EDITOR")
                    .or_else(|_| std::env::var("VISUAL"))
                    .unwrap_or_else(|_| "vi".to_string())
            }
        })
    }

    /// Build a job that opens `file_path` in the configured editor.
    /// If `TerminalEditor` is set, returns `SuspendAndRun` (app suspends, editor owns terminal).
    /// Otherwise returns `SpawnProcess` via `Editor` (GUI editor).
    /// `wait_for_exit`: when true, the job doesn't complete until the editor closes
    /// (needed so callers can react to editor-closed, e.g. the config reload prompt).
    pub(crate) fn editor_job(
        config: &AppConfig,
        file_path: String,
        wait_for_exit: bool,
    ) -> crate::job::JobKind {
        if let Some(ref cmd) = config.terminal_editor {
            // Terminal editor: suspend rwf, run synchronously, then resume.
            // On Windows use cmd /D /C so .bat/.cmd wrappers (e.g. vim.bat) work.
            // /D must precede /C: it skips the user's Command Processor AutoRun hook
            // (Clink and friends), which would otherwise run inside this transient
            // shell and print into the terminal we just handed to the editor.
            // See docs/IMPLICIT_CONTRACTS.md.
            #[cfg(target_os = "windows")]
            {
                let mut args = vec!["/D".to_string(), "/C".to_string()];
                args.extend(cmd.split_whitespace().map(str::to_string));
                args.push(file_path);
                return crate::job::JobKind::SuspendAndRun {
                    program: "cmd".to_string(),
                    args,
                };
            }
            #[cfg(not(target_os = "windows"))]
            {
                let mut parts = cmd.split_whitespace();
                let program = parts.next().unwrap_or("vi").to_string();
                let mut args: Vec<String> = parts.map(str::to_string).collect();
                args.push(file_path);
                return crate::job::JobKind::SuspendAndRun { program, args };
            }
        }
        // GUI editor: launch via cmd (Windows) / directly (Unix), no shell string parsing.
        // /D /C — see the AutoRun note above.
        let cmd = Self::resolve_editor(config);
        #[cfg(target_os = "windows")]
        {
            let mut args = vec!["/D".to_string(), "/C".to_string()];
            args.extend(cmd.split_whitespace().map(str::to_string));
            args.push(file_path);
            crate::job::JobKind::SpawnProcess {
                program: "cmd".to_string(),
                args,
                wait: wait_for_exit,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut parts = cmd.split_whitespace();
            let program = parts.next().unwrap_or("vi").to_string();
            let mut args: Vec<String> = parts.map(str::to_string).collect();
            args.push(file_path);
            crate::job::JobKind::SpawnProcess {
                program,
                args,
                wait: wait_for_exit,
            }
        }
    }

    /// Build the job that `Transition::ExecuteAssociationChecked` would start for
    /// one file: a `DetectFileType { CheckAssociationMismatch }` job when
    /// `magic_byte_detection_enabled`, else a direct `ExecuteCustomFunction` job
    /// (via `JobSpec::execute_association`). Extracted (Phase 7.3 Task 4) so the
    /// batch "Open With..." flow (one job per file in a group, run through the
    /// same mismatch gate) and the single-file `ExecuteAssociationChecked` handler
    /// share one implementation instead of drifting apart.
    pub(crate) fn checked_association_job(
        &self,
        path: std::path::PathBuf,
        command: String,
        working_dir: crate::model::Location,
        shell: Option<String>,
    ) -> crate::job::JobSpec {
        if !self.config.magic_byte_detection_enabled {
            crate::job::JobSpec::execute_association(command, working_dir, shell)
        } else {
            crate::job::JobSpec::new(crate::job::JobKind::DetectFileType {
                path,
                purpose: crate::job::DetectFileTypePurpose::CheckAssociationMismatch {
                    command,
                    working_dir,
                    shell,
                },
            })
        }
    }

    /// Build a job that opens `file_path` with the OS's default file association
    /// (Windows `start`, macOS `open`, Linux `xdg-open`). Always fire-and-forget —
    /// unlike `editor_job`, there is no "wait for exit" mode, since the whole point
    /// is handing off to whatever app the OS considers the default, without RWF
    /// blocking on it.
    pub(crate) fn system_open_job(file_path: String) -> crate::job::JobKind {
        #[cfg(target_os = "windows")]
        {
            // /D /C — see the AutoRun note on `editor_job`.
            crate::job::JobKind::SpawnProcess {
                program: "cmd".to_string(),
                args: vec![
                    "/D".to_string(),
                    "/C".to_string(),
                    "start".to_string(),
                    "".to_string(),
                    file_path,
                ],
                wait: false,
            }
        }
        #[cfg(target_os = "macos")]
        {
            crate::job::JobKind::SpawnProcess {
                program: "open".to_string(),
                args: vec![file_path],
                wait: false,
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            crate::job::JobKind::SpawnProcess {
                program: "xdg-open".to_string(),
                args: vec![file_path],
                wait: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_open_job_shape_for_current_platform() {
        let job = AppState::system_open_job("C:\\videos\\clip.mp4".to_string());
        match job {
            #[cfg(target_os = "windows")]
            crate::job::JobKind::SpawnProcess {
                program,
                args,
                wait,
            } => {
                assert_eq!(program, "cmd");
                assert_eq!(
                    args,
                    vec![
                        "/D".to_string(),
                        "/C".to_string(),
                        "start".to_string(),
                        "".to_string(),
                        "C:\\videos\\clip.mp4".to_string()
                    ]
                );
                assert!(!wait);
            }
            #[cfg(target_os = "macos")]
            crate::job::JobKind::SpawnProcess {
                program,
                args,
                wait,
            } => {
                assert_eq!(program, "open");
                assert_eq!(args, vec!["C:\\videos\\clip.mp4".to_string()]);
                assert!(!wait);
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            crate::job::JobKind::SpawnProcess {
                program,
                args,
                wait,
            } => {
                assert_eq!(program, "xdg-open");
                assert_eq!(args, vec!["C:\\videos\\clip.mp4".to_string()]);
                assert!(!wait);
            }
            #[allow(unreachable_patterns)]
            other => panic!("unexpected job kind: {:?}", other),
        }
    }

    /// Contract guard: every cmd.exe invocation rwf builds must start with `/D /C`.
    ///
    /// `/D` skips the user's `HKCU\Software\Microsoft\Command Processor\AutoRun`
    /// hook. Without it that hook (Clink, a corporate login script, anything) runs
    /// inside our transient shell and writes to the console it inherited — which is
    /// rwf's own TUI for `system_open_job`, and the editor's terminal for the
    /// `SuspendAndRun` path. Nothing else in the build notices a missing `/D`; it
    /// only shows up as garbled output on the machines that happen to have a hook.
    /// See docs/IMPLICIT_CONTRACTS.md and the file-level allowlist in
    /// rwf-bin/tests/repo_contracts.rs.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_cmd_invocations_pass_slash_d_before_slash_c() {
        fn assert_slash_d_c(label: &str, job: &crate::job::JobKind) {
            let (program, args) = match job {
                crate::job::JobKind::SpawnProcess { program, args, .. }
                | crate::job::JobKind::SuspendAndRun { program, args } => (program, args),
                other => panic!("{}: unexpected job kind: {:?}", label, other),
            };
            assert_eq!(program, "cmd", "{}: expected a cmd.exe invocation", label);
            assert_eq!(
                args.first().map(String::as_str),
                Some("/D"),
                "{}: cmd.exe must get /D first (skip AutoRun), got {:?}",
                label,
                args
            );
            assert_eq!(
                args.get(1).map(String::as_str),
                Some("/C"),
                "{}: /C must immediately follow /D, got {:?}",
                label,
                args
            );
        }

        assert_slash_d_c(
            "system_open_job",
            &AppState::system_open_job("C:\\videos\\clip.mp4".to_string()),
        );

        let gui = AppConfig {
            editor_command: Some("notepad".to_string()),
            terminal_editor: None,
            ..AppConfig::default()
        };
        assert_slash_d_c(
            "editor_job (GUI editor)",
            &AppState::editor_job(&gui, "C:\\tmp\\a.txt".to_string(), false),
        );

        let terminal = AppConfig {
            editor_command: None,
            terminal_editor: Some("vim".to_string()),
            ..AppConfig::default()
        };
        assert_slash_d_c(
            "editor_job (terminal editor)",
            &AppState::editor_job(&terminal, "C:\\tmp\\a.txt".to_string(), false),
        );
    }
}
