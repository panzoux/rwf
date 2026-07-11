//! Macro expansion for custom functions
//!
//! This module implements macro expansion for custom function commands,
//! supporting various macros like $P (active pane path), $F (cursor file), etc.

use crate::model::CustomFunction;
use crate::state::AppState;
use regex::Regex;

/// Macro expander for custom functions
pub struct MacroExpander;

impl MacroExpander {
    /// Create a new macro expander
    pub fn new() -> Self {
        Self
    }

    /// Expand all macros in a custom function command
    pub fn expand(&self, state: &AppState, function: &CustomFunction) -> Result<String, String> {
        let mut command = function
            .get_command()
            .ok_or_else(|| "Cannot expand a menu entry — no command".to_string())?
            .to_string();

        // Check for $I macro first - this requires user input
        if command.contains("$I") {
            return Err("Command contains $I macro - user input required".to_string());
        }

        // $V"VARNAME" — cross-platform env var expansion (TWF-compatible)
        // e.g. $V"APPDATA" → C:\Users\user\AppData\Roaming on Windows
        command = Self::expand_v_macro(&command);

        // Expand pane path macros
        command = self.expand_macro(&command, "$P", || {
            state.active_pane().current_location.display_path()
        });

        command = self.expand_macro(&command, "$O", || {
            state.opposite_pane().current_location.display_path()
        });

        let tab = state.current_tab();
        command = self.expand_macro(&command, "$L", || {
            tab.left_pane.current_location.display_path()
        });

        command = self.expand_macro(&command, "$R", || {
            tab.right_pane.current_location.display_path()
        });

        // Expand cursor file macros
        if let Some(entry) = state.active_pane().current_entry() {
            command = self.expand_macro(&command, "$F", || entry.name.clone());
            command = self.expand_macro(&command, "$W", || {
                entry.name_without_extension().to_string()
            });
            if let Some(ext) = entry.extension() {
                command = self.expand_macro(&command, "$E", || ext.to_string());
            } else {
                command = command.replace("$E", "");
            }
        } else {
            // No cursor entry - replace with empty
            command = command.replace("$F", "");
            command = command.replace("$W", "");
            command = command.replace("$E", "");
        }

        // Expand marked files macro
        let marked = state.active_pane().marked_entries();
        if !marked.is_empty() {
            let marked_list = marked
                .iter()
                .map(|e| shell_quote(&e.name))
                .collect::<Vec<_>>()
                .join(" ");
            command = self.expand_macro(&command, "$M", || marked_list.clone());
        } else {
            command = command.replace("$M", "");
        }

        // Expand all files macro
        let all_files = state
            .active_pane()
            .entries
            .iter()
            .map(|e| shell_quote(&e.name))
            .collect::<Vec<_>>()
            .join(" ");
        command = self.expand_macro(&command, "$*", || all_files.clone());

        // Expand home directory macro
        if let Some(home) = dirs::home_dir() {
            command = self.expand_macro(&command, "$~", || home.display().to_string());
        }

        // Expand file count macro
        command = self.expand_macro(&command, "$#", || {
            state.active_pane().entries.len().to_string()
        });

        // Expand environment variables
        command = self.expand_env_vars(&command);

        Ok(command)
    }

    /// Expand `$V"VARNAME"` patterns — cross-platform env var expansion (TWF-compatible).
    /// `$V"APPDATA"` → value of the APPDATA env var; empty string if not set.
    fn expand_v_macro(command: &str) -> String {
        // Pattern: $V"<var_name>" where var_name has no embedded quotes
        let re = Regex::new(r#"\$V"([^"]+)""#).expect("regex is a compile-time constant");
        re.replace_all(command, |caps: &regex::Captures| {
            std::env::var(&caps[1]).unwrap_or_default()
        })
        .into_owned()
    }

    /// Expand a single macro in the command
    fn expand_macro<F>(&self, command: &str, macro_name: &str, value_fn: F) -> String
    where
        F: Fn() -> String,
    {
        if command.contains(macro_name) {
            command.replace(macro_name, &value_fn())
        } else {
            command.to_string()
        }
    }

    /// Expand environment variables in the command.
    /// Supports four formats on all platforms:
    ///   $env:VAR  (PowerShell)   — expanded first to avoid $env matching as bare $VAR
    ///   ${VAR}    (curly brace)  — expanded before bare $VAR; unambiguous, preferred
    ///   %VAR%     (Windows batch)
    ///   $VAR      (Unix-style bare dollar) — NOTE: conflicts with single-letter RWF macros
    ///             ($P, $O, $L, $R, $F, $W, $E, $M) which are expanded in an earlier pass.
    ///             Env vars whose names start with those letters are unreachable via bare $VAR.
    fn expand_env_vars(&self, command: &str) -> String {
        let mut result = command.to_string();
        result = Self::replace_env_pattern(
            &result,
            Regex::new(r"\$env:([A-Za-z_][A-Za-z0-9_]*)")
                .expect("regex is a compile-time constant"),
        );
        result = Self::replace_env_pattern(
            &result,
            Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}")
                .expect("regex is a compile-time constant"),
        );
        result = Self::replace_env_pattern(
            &result,
            Regex::new(r"%([^%]+)%").expect("regex is a compile-time constant"),
        );
        result = Self::replace_env_pattern(
            &result,
            Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("regex is a compile-time constant"),
        );
        result
    }

    fn replace_env_pattern(command: &str, re: Regex) -> String {
        let matches: Vec<_> = re
            .captures_iter(command)
            .filter_map(|cap| {
                cap.get(1).and_then(|var_name| {
                    std::env::var(var_name.as_str())
                        .ok()
                        .map(|value| (cap[0].to_string(), value))
                })
            })
            .collect();
        let mut result = command.to_string();
        for (full_match, value) in matches {
            result = result.replace(&full_match, &value);
        }
        result
    }

    /// Check if a command contains the $I (user input) macro
    pub fn requires_user_input(&self, function: &CustomFunction) -> bool {
        function.get_command().is_some_and(|c| c.contains("$I"))
    }

    /// Extract the prompt text from a `$I"prompt"` or `$I5"prompt"` pattern.
    /// Returns None if the command has bare `$I` with no quoted prompt.
    pub fn extract_i_prompt(command: &str) -> Option<String> {
        let re = Regex::new(r#"\$I\d?"([^"]*)""#).expect("regex is a compile-time constant");
        re.captures(command).map(|cap| cap[1].to_string())
    }

    /// Expand the $I macro with user-provided input.
    /// Replaces the entire `$I"prompt"` / `$I5"prompt"` / bare `$I` pattern with user_input.
    pub fn expand_with_user_input(
        &self,
        state: &AppState,
        function: &CustomFunction,
        user_input: &str,
    ) -> Result<String, String> {
        let mut command = function
            .get_command()
            .ok_or_else(|| "Cannot expand a menu entry — no command".to_string())?
            .to_string();

        // Replace $I"prompt", $I5"prompt", or bare $I — the whole token — with user_input.
        let re = Regex::new(r#"\$I(?:\d?"[^"]*")?"#).expect("regex is a compile-time constant");
        command = re.replace_all(&command, user_input).into_owned();

        // Now expand all other macros
        self.expand_impl(state, &command)
    }

    /// Internal implementation of expand that works on a command string
    fn expand_impl(&self, state: &AppState, command: &str) -> Result<String, String> {
        let mut result = command.to_string();

        // $V"VARNAME" — cross-platform env var expansion
        result = Self::expand_v_macro(&result);

        // Expand pane path macros
        result = self.expand_macro(&result, "$P", || {
            state.active_pane().current_location.display_path()
        });

        result = self.expand_macro(&result, "$O", || {
            state.opposite_pane().current_location.display_path()
        });

        let tab = state.current_tab();
        result = self.expand_macro(&result, "$L", || {
            tab.left_pane.current_location.display_path()
        });

        result = self.expand_macro(&result, "$R", || {
            tab.right_pane.current_location.display_path()
        });

        // Expand cursor file macros
        if let Some(entry) = state.active_pane().current_entry() {
            result = self.expand_macro(&result, "$F", || entry.name.clone());
            result =
                self.expand_macro(&result, "$W", || entry.name_without_extension().to_string());
            if let Some(ext) = entry.extension() {
                result = self.expand_macro(&result, "$E", || ext.to_string());
            } else {
                result = result.replace("$E", "");
            }
        } else {
            result = result.replace("$F", "");
            result = result.replace("$W", "");
            result = result.replace("$E", "");
        }

        // Expand marked files macro
        let marked = state.active_pane().marked_entries();
        if !marked.is_empty() {
            let marked_list = marked
                .iter()
                .map(|e| shell_quote(&e.name))
                .collect::<Vec<_>>()
                .join(" ");
            result = self.expand_macro(&result, "$M", || marked_list.clone());
        } else {
            result = result.replace("$M", "");
        }

        // Expand all files macro
        let all_files = state
            .active_pane()
            .entries
            .iter()
            .map(|e| shell_quote(&e.name))
            .collect::<Vec<_>>()
            .join(" ");
        result = self.expand_macro(&result, "$*", || all_files.clone());

        // Expand home directory macro
        if let Some(home) = dirs::home_dir() {
            result = self.expand_macro(&result, "$~", || home.display().to_string());
        }

        // Expand file count macro
        result = self.expand_macro(&result, "$#", || {
            state.active_pane().entries.len().to_string()
        });

        // Expand environment variables
        result = self.expand_env_vars(&result);

        Ok(result)
    }
}

impl Default for MacroExpander {
    fn default() -> Self {
        Self::new()
    }
}

/// Quote a filename for shell execution if it contains spaces or special characters
fn shell_quote(filename: &str) -> String {
    if filename.contains(' ')
        || filename.contains('&')
        || filename.contains('|')
        || filename.contains(';')
        || filename.contains('<')
        || filename.contains('>')
        || filename.contains('(')
        || filename.contains(')')
        || filename.contains('$')
        || filename.contains('`')
        || filename.contains('"')
        || filename.contains('\'')
    {
        #[cfg(target_os = "windows")]
        return format!("\"{}\"", filename.replace('"', "\"\""));

        #[cfg(not(target_os = "windows"))]
        return format!("'{}'", filename.replace('\'', "'\\''"));
    }
    filename.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileEntry;
    use crate::model::Location;
    use crate::state::AppConfig;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_state() -> AppState {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add some test files
        let test_location = Location::Local(PathBuf::from("/test"));
        state.tabs.tabs[0].left_pane.entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: test_location.join("file1.txt"),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
                is_symlink: false,
                link_target: None,
                link_kind: None,
            },
            FileEntry {
                name: "file2.rs".to_string(),
                location: test_location.join("file2.rs"),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: true,
                calculated_size: None,
                is_symlink: false,
                link_target: None,
                link_kind: None,
            },
        ];

        state
    }

    #[test]
    fn test_expand_pane_macros() {
        let state = create_test_state();
        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "cd $P");

        let result = expander.expand(&state, &function).unwrap();
        // The result should contain "cd" and the expanded path
        assert!(result.starts_with("cd "));
        // The path should be expanded (not contain $P)
        assert!(!result.contains("$P"));
    }

    #[test]
    fn test_expand_file_macros() {
        let state = create_test_state();
        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "echo $F $W $E");

        let result = expander.expand(&state, &function).unwrap();
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file1"));
        assert!(result.contains("txt"));
    }

    #[test]
    fn test_expand_marked_files() {
        let state = create_test_state();
        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "process $M");

        let result = expander.expand(&state, &function).unwrap();
        assert!(result.contains("file2.rs"));
    }

    #[test]
    fn test_expand_file_count() {
        let state = create_test_state();
        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "echo $#");

        let result = expander.expand(&state, &function).unwrap();
        assert!(result.contains("2"));
    }

    #[test]
    fn test_requires_user_input() {
        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "echo $I");

        assert!(expander.requires_user_input(&function));
    }

    #[test]
    fn test_expand_with_user_input() {
        let state = create_test_state();
        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "echo $I");

        let result = expander
            .expand_with_user_input(&state, &function, "hello")
            .unwrap();
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_extract_i_prompt() {
        assert_eq!(
            MacroExpander::extract_i_prompt(r#"cmd /c copy "$P\$F" "$I"Destination path""#),
            Some("Destination path".to_string())
        );
        assert_eq!(
            MacroExpander::extract_i_prompt(r#"cmd /c ren "$P\$F" "$I"New filename""#),
            Some("New filename".to_string())
        );
        assert_eq!(
            MacroExpander::extract_i_prompt(r#"$I5"Enter path""#),
            Some("Enter path".to_string())
        );
        assert_eq!(MacroExpander::extract_i_prompt(r#"echo $I"#), None);
        assert_eq!(MacroExpander::extract_i_prompt("echo $P"), None);
    }

    #[test]
    fn test_expand_with_user_input_removes_prompt_text() {
        let state = create_test_state();
        let expander = MacroExpander::new();

        // $I"prompt" — whole token replaced, prompt text must not appear in result
        let f = CustomFunction::new("t", r#"cmd /c copy "src" "$I"Destination path""#);
        let r = expander
            .expand_with_user_input(&state, &f, r#"C:\dest"#)
            .unwrap();
        assert!(r.contains(r#"C:\dest"#), "user input missing: {r}");
        assert!(!r.contains("Destination path"), "prompt text leaked: {r}");
        assert!(!r.contains("$I"), "$I not replaced: {r}");

        // $I5"prompt" — width variant
        let f = CustomFunction::new("t", r#"notepad $I5"Enter file""#);
        let r = expander
            .expand_with_user_input(&state, &f, "out.txt")
            .unwrap();
        assert_eq!(r, "notepad out.txt");

        // bare $I — no prompt text
        let f = CustomFunction::new("t", "echo $I");
        let r = expander
            .expand_with_user_input(&state, &f, "hello")
            .unwrap();
        assert_eq!(r, "echo hello");
    }

    #[test]
    fn test_expand_env_var_formats() {
        let state = create_test_state();
        let expander = MacroExpander::new();

        // NOTE: bare $VAR conflicts with single-letter RWF macros ($P, $O, $L, $R, $F, $W, $E, $M).
        // Those are expanded first, so env vars whose names start with those letters are unreachable
        // via bare $VAR.  Use ${VAR} or $env:VAR for full reliability.
        // This test uses a name starting with 'Z' (not an RWF macro letter) to avoid the conflict.
        std::env::set_var("ZRWF_TEST_VAR", "test_value");

        // %VAR% — Windows batch
        let f = CustomFunction::new("t", "cmd /c echo %ZRWF_TEST_VAR%");
        let r = expander.expand(&state, &f).unwrap();
        assert!(r.contains("test_value"), "%VAR% not expanded: {r}");

        // $VAR — bare dollar; safe when name doesn't start with an RWF macro letter
        let f = CustomFunction::new("t", "echo $ZRWF_TEST_VAR");
        let r = expander.expand(&state, &f).unwrap();
        assert!(r.contains("test_value"), "$VAR not expanded: {r}");

        // ${VAR} — curly brace (unambiguous, preferred)
        let f = CustomFunction::new("t", "echo ${ZRWF_TEST_VAR}");
        let r = expander.expand(&state, &f).unwrap();
        assert!(r.contains("test_value"), "${{VAR}} not expanded: {r}");

        // $env:VAR — PowerShell (unambiguous, preferred)
        let f = CustomFunction::new("t", "echo $env:ZRWF_TEST_VAR");
        let r = expander.expand(&state, &f).unwrap();
        assert!(r.contains("test_value"), "$env:VAR not expanded: {r}");

        std::env::remove_var("ZRWF_TEST_VAR");
    }

    #[test]
    fn test_env_var_expansion_order() {
        // $env:VAR must not be partially consumed as bare $VAR ("env" as a var name)
        let state = create_test_state();
        let expander = MacroExpander::new();

        std::env::set_var("RWF_ORDER_VAR", "correct");
        std::env::remove_var("env"); // ensure "env" env var doesn't exist

        let f = CustomFunction::new("t", "echo $env:RWF_ORDER_VAR");
        let r = expander.expand(&state, &f).unwrap();
        assert!(r.contains("correct"), "ordering broken: {r}");
        assert!(!r.contains("$env:"), "env: prefix left unexpanded: {r}");

        std::env::remove_var("RWF_ORDER_VAR");
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("simple"), "simple");

        #[cfg(target_os = "windows")]
        assert_eq!(shell_quote("file with spaces"), "\"file with spaces\"");

        #[cfg(not(target_os = "windows"))]
        assert_eq!(shell_quote("file with spaces"), "'file with spaces'");
    }
}

#[cfg(test)]
mod macro_expander_properties;
