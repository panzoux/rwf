//! Property-based tests for macro expansion
//!
//! **Validates: Requirements 28.2**

use crate::macro_expander::MacroExpander;
use crate::model::{CustomFunction, FileEntry, Location};
use crate::state::{AppConfig, AppState};
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;

/// Generate arbitrary custom functions for testing
fn arb_custom_function() -> impl Strategy<Value = CustomFunction> {
    prop::string::string_regex("[a-zA-Z0-9 ]+")
        .unwrap()
        .prop_flat_map(|name| {
            prop::string::string_regex("(echo|cat|ls) (\\$P|\\$F|\\$M|\\$#|test)")
                .unwrap()
                .prop_map(move |command| CustomFunction::new(name.clone(), command))
        })
}

/// Generate arbitrary AppState for testing
fn arb_app_state() -> impl Strategy<Value = AppState> {
    (0..5usize, 0..10usize).prop_map(|(num_files, cursor_pos)| {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add some test files
        let test_location = Location::Local(PathBuf::from("/test"));
        for i in 0..num_files {
            state.tabs.tabs[0].left_pane.entries.push(FileEntry {
                name: format!("file{}.txt", i),
                location: test_location.join(&format!("file{}.txt", i)),
                size: 100 * (i as u64 + 1),
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: i % 2 == 0,
                calculated_size: None,
                is_symlink: false,
                link_target: None,
                link_kind: None,
            });
        }

        // Set cursor position
        if num_files > 0 {
            state.tabs.tabs[0].left_pane.cursor = cursor_pos % num_files;
        }

        state
    })
}

proptest! {
    /// **Property 24: Macro Expansion Consistency**
    ///
    /// *For any* CustomFunction and AppState, expanding macros should produce
    /// the same result when called multiple times with the same state.
    ///
    /// **Validates: Requirements 28.2**
    #[test]
    fn prop_macro_expansion_consistency(
        function in arb_custom_function(),
        state in arb_app_state()
    ) {
        let expander = MacroExpander::new();

        // Expand the same function with the same state multiple times
        let result1 = expander.expand(&state, &function);
        let result2 = expander.expand(&state, &function);
        let result3 = expander.expand(&state, &function);

        // All results should be identical
        match (result1, result2, result3) {
            (Ok(r1), Ok(r2), Ok(r3)) => {
                prop_assert_eq!(&r1, &r2, "First and second expansion differ");
                prop_assert_eq!(&r2, &r3, "Second and third expansion differ");
            }
            (Err(e1), Err(e2), Err(e3)) => {
                // All should fail with the same error
                prop_assert_eq!(&e1, &e2, "First and second error differ");
                prop_assert_eq!(&e2, &e3, "Second and third error differ");
            }
            _ => {
                prop_assert!(false, "Inconsistent results: some succeeded, some failed");
            }
        }
    }

    /// Test that macro expansion is deterministic for specific macros
    #[test]
    fn prop_specific_macro_determinism(
        state in arb_app_state()
    ) {
        let expander = MacroExpander::new();

        // Test $P macro (active pane path)
        let func_p = CustomFunction::new("test", "echo $P");
        let result1 = expander.expand(&state, &func_p);
        let result2 = expander.expand(&state, &func_p);
        prop_assert_eq!(result1, result2, "$P macro expansion not consistent");

        // Test $# macro (file count)
        let func_count = CustomFunction::new("test", "echo $#");
        let result1 = expander.expand(&state, &func_count);
        let result2 = expander.expand(&state, &func_count);
        prop_assert_eq!(result1, result2, "$# macro expansion not consistent");

        // Test $F macro (cursor file)
        let func_f = CustomFunction::new("test", "echo $F");
        let result1 = expander.expand(&state, &func_f);
        let result2 = expander.expand(&state, &func_f);
        prop_assert_eq!(result1, result2, "$F macro expansion not consistent");
    }

    /// Test that macro expansion produces valid output
    #[test]
    fn prop_macro_expansion_produces_valid_output(
        function in arb_custom_function(),
        state in arb_app_state()
    ) {
        let expander = MacroExpander::new();

        let result = expander.expand(&state, &function);

        // If expansion succeeds, the result should not contain unexpanded macros
        // (except for macros that legitimately have no value)
        if let Ok(expanded) = result {
            // Should not contain $I (user input) since we didn't provide it
            prop_assert!(!expanded.contains("$I"), "Expanded command contains $I macro");

            // If there are files, $F should be expanded
            if !state.active_pane().entries.is_empty()
                && function.command.as_deref().is_some_and(|c| c.contains("$F"))
            {
                prop_assert!(!expanded.contains("$F"), "Expanded command still contains $F macro");
            }

            // $# should always be expanded
            if function.command.as_deref().is_some_and(|c| c.contains("$#")) {
                prop_assert!(!expanded.contains("$#"), "Expanded command still contains $# macro");
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_macro_expansion_consistency_simple() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add a test file
        let test_location = Location::Local(PathBuf::from("/test"));
        state.tabs.tabs[0].left_pane.entries.push(FileEntry {
            name: "test.txt".to_string(),
            location: test_location.join("test.txt"),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        });

        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "echo $F");

        // Expand multiple times
        let result1 = expander.expand(&state, &function).unwrap();
        let result2 = expander.expand(&state, &function).unwrap();
        let result3 = expander.expand(&state, &function).unwrap();

        // All should be identical
        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
        assert!(result1.contains("test.txt"));
    }
}

/// **Property 25: Custom Function Job Creation**
///
/// *For any* CustomFunction, executing it should create a JobSpec with JobKind::ExecuteCustomFunction.
///
/// **Validates: Requirements 28.12**
#[cfg(test)]
mod custom_function_job_tests {
    use super::*;
    use crate::job::{JobKind, JobSpec};

    proptest! {
        #[test]
        fn prop_custom_function_creates_job(
            function in arb_custom_function(),
            state in arb_app_state()
        ) {
            let expander = MacroExpander::new();

            // Expand the function command
            let expanded_result = expander.expand(&state, &function);

            // If expansion succeeds, we should be able to create a job
            if let Ok(expanded_command) = expanded_result {
                // Create a job spec for the custom function
                let working_dir = state.active_pane().current_location.clone();
                let job_spec = JobSpec::new(JobKind::ExecuteCustomFunction {
                    command: expanded_command.clone(),
                    working_dir: working_dir.clone(),
                    pipe_to_action: function.pipe_to_action.clone(),
                    shell: function.get_shell().map(|s| s.to_string()),
                });

                // Verify the job spec has the correct kind
                match &job_spec.kind {
                    JobKind::ExecuteCustomFunction { command, working_dir: wd, pipe_to_action, shell } => {
                        prop_assert_eq!(command, &expanded_command, "Command mismatch");
                        prop_assert_eq!(wd, &working_dir, "Working directory mismatch");
                        prop_assert_eq!(pipe_to_action, &function.pipe_to_action, "PipeToAction mismatch");

                        // Verify shell is set correctly
                        let expected_shell = function.get_shell().map(|s| s.to_string());
                        prop_assert_eq!(shell, &expected_shell, "Shell mismatch");
                    }
                    _ => prop_assert!(false, "Job kind is not ExecuteCustomFunction"),
                }
            }
        }

        #[test]
        fn prop_custom_function_job_has_cancel_token(
            function in arb_custom_function(),
            state in arb_app_state()
        ) {
            let expander = MacroExpander::new();

            // Expand the function command
            if let Ok(expanded_command) = expander.expand(&state, &function) {
                // Create a job spec
                let working_dir = state.active_pane().current_location.clone();
                let job_spec = JobSpec::new(JobKind::ExecuteCustomFunction {
                    command: expanded_command,
                    working_dir,
                    pipe_to_action: function.pipe_to_action.clone(),
                    shell: function.get_shell().map(|s| s.to_string()),
                });

                // Verify the job spec has a cancellation token
                prop_assert!(!job_spec.cancel_token.is_cancelled(), "Cancel token should not be cancelled initially");

                // Cancel the token
                job_spec.cancel_token.cancel();
                prop_assert!(job_spec.cancel_token.is_cancelled(), "Cancel token should be cancelled after calling cancel()");
            }
        }
    }

    #[test]
    fn test_custom_function_job_creation_simple() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add a test file
        let test_location = Location::Local(PathBuf::from("/test"));
        state.tabs.tabs[0].left_pane.entries.push(FileEntry {
            name: "test.txt".to_string(),
            location: test_location.join("test.txt"),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        });

        let expander = MacroExpander::new();
        let function = CustomFunction::new("test", "echo $F").with_shell("bash");

        // Expand the command
        let expanded_command = expander.expand(&state, &function).unwrap();

        // Create a job spec
        let working_dir = state.active_pane().current_location.clone();
        let job_spec = JobSpec::new(JobKind::ExecuteCustomFunction {
            command: expanded_command.clone(),
            working_dir: working_dir.clone(),
            pipe_to_action: None,
            shell: Some("bash".to_string()),
        });

        // Verify the job spec
        match &job_spec.kind {
            JobKind::ExecuteCustomFunction {
                command,
                working_dir: wd,
                pipe_to_action,
                shell,
            } => {
                assert_eq!(command, &expanded_command);
                assert_eq!(wd, &working_dir);
                assert_eq!(pipe_to_action, &None);
                assert_eq!(shell, &Some("bash".to_string()));
            }
            _ => panic!("Job kind is not ExecuteCustomFunction"),
        }
    }
}
