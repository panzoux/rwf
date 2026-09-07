//! Macro expansion for custom functions
//!
//! This module implements macro expansion for custom function commands,
//! supporting various macros like $P (active pane path), $F (cursor file), etc.

use crate::model::CustomFunction;
use crate::state::AppState;
use regex::Regex;

/// Native path separator, expanded from the `$/` macro.
#[cfg(target_os = "windows")]
const PATH_SEP: &str = "\\";
#[cfg(not(target_os = "windows"))]
const PATH_SEP: &str = "/";

/// The entries the `$M*` macros operate on: the marked entries, or — when nothing
/// is marked — the cursor entry alone. Empty only when the pane itself is empty.
fn marked_or_cursor(state: &AppState) -> Vec<&crate::model::FileEntry> {
    let pane = state.active_pane();
    let marked = pane.marked_entries();
    if marked.is_empty() {
        pane.current_entry().into_iter().collect()
    } else {
        marked
    }
}

/// True if `command` still uses the removed `$M` macro.
///
/// `$M` was replaced by the systematic `$MFS`/`$MPS`/`$MFL`/`$MPL` set. A leftover
/// `$M` would not error on its own — it would fall through to bare-`$VAR` env
/// expansion, find no variable named `M`, and survive as the literal text `$M` in
/// the command. Config loading calls this so the removal fails loudly instead.
pub fn has_stale_marked_macro(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut search_from = 0;
    while let Some(offset) = command[search_from..].find("$M") {
        let at = search_from + offset;
        // Valid forms are exactly $M + {F,P} + {S,L}
        let valid = matches!(bytes.get(at + 2), Some(b'F') | Some(b'P'))
            && matches!(bytes.get(at + 3), Some(b'S') | Some(b'L'));
        if !valid {
            return true;
        }
        search_from = at + 2;
    }
    false
}

/// Macro expander for custom functions
pub struct MacroExpander;

impl MacroExpander {
    /// Create a new macro expander
    pub fn new() -> Self {
        Self
    }

    /// Expand all macros in a custom function command
    pub fn expand(&self, state: &AppState, function: &CustomFunction) -> Result<String, String> {
        let command = function
            .get_command()
            .ok_or_else(|| "Cannot expand a menu entry — no command".to_string())?;

        // Check for $I macro first - this requires user input
        if command.contains("$I") {
            return Err("Command contains $I macro - user input required".to_string());
        }

        Ok(self.expand_all(state, command))
    }

    /// Expand a bare template string (no `$I` handling, no `CustomFunction` wrapper).
    /// Used by the `ClipText` function kind, which has no command to run.
    pub fn expand_template(&self, state: &AppState, template: &str) -> String {
        self.expand_all(state, template)
    }

    /// The single macro-expansion implementation.
    ///
    /// **Ordering invariant: longest macro first.** `$MFS`/`$MPS`/`$MFL`/`$MPL` are
    /// expanded before the single-letter macros, and env vars are expanded last so a
    /// bare `$VAR` cannot swallow a macro (and vice versa). `$/` is deliberately not
    /// a letter: an env var name can never start with `/`, so it cannot shadow one
    /// the way a hypothetical `$S` would shadow `$SYSTEMROOT`.
    fn expand_all(&self, state: &AppState, command: &str) -> String {
        // $V"VARNAME" — cross-platform env var expansion (TWF-compatible)
        // e.g. $V"APPDATA" → C:\Users\user\AppData\Roaming on Windows
        let mut command = Self::expand_v_macro(command);

        // Native path separator — lets one config work on Windows and Unix ("$P$/$F").
        command = command.replace("$/", PATH_SEP);

        // Marked-file macros (longest first). $M<what><how>:
        //   what: F = file name, P = full path
        //   how:  S = shell form (space-joined, shell-quoted)
        //         L = list form  (newline-joined, raw)
        // All four fall back to the cursor entry when nothing is marked.
        if command.contains("$M") {
            let entries = marked_or_cursor(state);
            let join = |f: &dyn Fn(&crate::model::FileEntry) -> String, sep: &str| {
                entries.iter().map(|e| f(e)).collect::<Vec<_>>().join(sep)
            };
            command = command.replace("$MFS", &join(&|e| shell_quote(&e.name), " "));
            command = command.replace(
                "$MPS",
                &join(&|e| shell_quote(&e.location.display_path()), " "),
            );
            command = command.replace("$MFL", &join(&|e| e.name.clone(), "\n"));
            command = command.replace("$MPL", &join(&|e| e.location.display_path(), "\n"));
        }

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
        self.expand_env_vars(&command)
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
    ///             ($P, $O, $L, $R, $F, $W, $E) which are expanded in an earlier pass.
    ///             Env vars whose names start with those letters are unreachable via bare $VAR.
    ///             This is exactly why the path separator is `$/` and not `$S`: `$S` would
    ///             have shadowed $SYSTEMROOT and $SESSIONNAME.
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
        Ok(self.expand_all(state, &command))
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
        // $MFS is the successor to the removed $M: names, space-joined, shell-quoted.
        let function = CustomFunction::new("test", "process $MFS");

        let result = expander.expand(&state, &function).unwrap();
        assert!(result.contains("file2.rs"));
    }

    /// The quote character `shell_quote` uses on this platform. The `S` (shell form)
    /// macros quote with `"` on Windows and `'` elsewhere, so a test must not
    /// hardcode either one.
    #[cfg(target_os = "windows")]
    const SQ: char = '"';
    #[cfg(not(target_os = "windows"))]
    const SQ: char = '\'';

    /// Builds a state with two marked entries, one of whose names contains a space,
    /// so quoting behaviour is observable.
    fn state_with_marked_spaces() -> AppState {
        let mut state = create_test_state();
        let loc = Location::Local(PathBuf::from("/test"));
        let mk = |name: &str, marked: bool| FileEntry {
            name: name.to_string(),
            location: loc.join(name),
            size: 1,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };
        state.tabs.tabs[0].left_pane.entries = vec![mk("a.txt", true), mk("b c.txt", true)];
        state
    }

    #[test]
    fn marked_macros_cover_all_four_corners() {
        let state = state_with_marked_spaces();
        let expander = MacroExpander::new();
        let sep = PATH_SEP;
        let expand = |t: &str| expander.expand_template(&state, t);

        // names / shell form — space-joined, the space-containing name quoted
        assert_eq!(expand("$MFS"), format!("a.txt {SQ}b c.txt{SQ}"));
        // names / list form — newline-joined, raw
        assert_eq!(expand("$MFL"), "a.txt\nb c.txt");
        // full paths / list form — one raw path per line.
        // "/test" is a literal base here; only `join` contributes a native separator.
        assert_eq!(
            expand("$MPL"),
            format!("/test{s}a.txt\n/test{s}b c.txt", s = sep)
        );
        // full paths / shell form — the corner the old $M could not reach
        assert_eq!(
            expand("$MPS"),
            format!("/test{s}a.txt {SQ}/test{s}b c.txt{SQ}", s = sep)
        );
    }

    #[test]
    fn marked_macros_fall_back_to_cursor_entry_when_nothing_marked() {
        let mut state = state_with_marked_spaces();
        for e in state.tabs.tabs[0].left_pane.entries.iter_mut() {
            e.marked = false;
        }
        state.tabs.tabs[0].left_pane.cursor = 1; // "b c.txt"
        let expander = MacroExpander::new();

        assert_eq!(expander.expand_template(&state, "$MFL"), "b c.txt");
        assert_eq!(
            expander.expand_template(&state, "$MFS"),
            format!("{SQ}b c.txt{SQ}")
        );
    }

    #[test]
    fn path_separator_macro_expands_natively_and_composes() {
        let state = create_test_state();
        let expander = MacroExpander::new();
        // $P$/$F is the cross-platform spelling of "full path of the cursor file".
        let result = expander.expand_template(&state, "$P$/$F");
        assert!(
            result.ends_with(&format!("{}file1.txt", PATH_SEP)),
            "{result}"
        );
        assert!(
            !result.contains("$/"),
            "separator left unexpanded: {result}"
        );
    }

    #[test]
    fn path_separator_macro_does_not_shadow_env_vars() {
        // The whole reason $/ was chosen over $S: a letter macro would have eaten
        // the leading character of env vars starting with that letter.
        let state = create_test_state();
        let expander = MacroExpander::new();
        std::env::set_var("SYSTEMROOT_RWFTEST", "C:\\Windows");

        let result = expander.expand_template(&state, "$SYSTEMROOT_RWFTEST");
        assert_eq!(result, "C:\\Windows", "env var was mangled by a macro pass");

        std::env::remove_var("SYSTEMROOT_RWFTEST");
    }

    #[test]
    fn marked_macros_expand_longest_first() {
        // $MFS must not be chewed up into "<names>S" by a shorter macro, and the
        // single-letter $F/$P passes must leave the $M* tokens alone.
        let state = state_with_marked_spaces();
        let expander = MacroExpander::new();
        let out = expander.expand_template(&state, "$MFS|$MFL");
        assert_eq!(out, format!("a.txt {SQ}b c.txt{SQ}|a.txt\nb c.txt"));
        assert!(!out.contains('$'), "a macro survived expansion: {out}");
    }

    #[test]
    fn stale_marked_macro_is_detected() {
        // The four valid forms are accepted...
        for ok in ["$MFS", "$MPS", "$MFL", "$MPL", "cmd $MPS $P", "no macros"] {
            assert!(!has_stale_marked_macro(ok), "false positive on {ok:?}");
        }
        // ...and anything else using $M is flagged, including half-migrated spellings.
        for bad in ["process $M", "$M", "$MF", "$MP", "$MX", "a $M b"] {
            assert!(has_stale_marked_macro(bad), "missed stale macro in {bad:?}");
        }
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
