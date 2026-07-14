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
            // On Windows use cmd /c so .bat/.cmd wrappers (e.g. vim.bat) work.
            #[cfg(target_os = "windows")]
            {
                let mut args = vec!["/c".to_string()];
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
        let cmd = Self::resolve_editor(config);
        #[cfg(target_os = "windows")]
        {
            let mut args = vec!["/c".to_string()];
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

    /// Build a job that opens `file_path` with the OS's default file association
    /// (Windows `start`, macOS `open`, Linux `xdg-open`). Always fire-and-forget —
    /// unlike `editor_job`, there is no "wait for exit" mode, since the whole point
    /// is handing off to whatever app the OS considers the default, without RWF
    /// blocking on it.
    #[allow(dead_code)]
    pub(crate) fn system_open_job(file_path: String) -> crate::job::JobKind {
        #[cfg(target_os = "windows")]
        {
            crate::job::JobKind::SpawnProcess {
                program: "cmd".to_string(),
                args: vec![
                    "/c".to_string(),
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
                        "/c".to_string(),
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
}
