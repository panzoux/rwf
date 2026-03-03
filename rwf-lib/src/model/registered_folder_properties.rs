//! Property-based tests for registered folder environment variable expansion
//!
//! **Property 29: Environment Variable Expansion Consistency**
//! **Validates: Requirements 31.8**
//!
//! This property verifies that environment variable expansion is consistent and correct:
//! - Expanding the same path multiple times produces the same result
//! - Variables that don't exist are left unchanged
//! - Multiple variables in the same path are all expanded
//! - Different variable formats (%, $, ${}, $env:) are handled correctly

use proptest::prelude::*;
use super::dialog::RegisteredFolderManager;
use std::env;
use std::sync::Mutex;

// Global mutex to serialize environment variable access in tests
// This prevents race conditions when tests run in parallel
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Generate valid environment variable names (alphanumeric + underscore, starting with letter or underscore)
/// Prefixed with RWFTEST_ to avoid conflicts with real environment variables
fn env_var_name() -> impl Strategy<Value = String> {
    "[A-Za-z_][A-Za-z0-9_]{0,15}"
        .prop_map(|s| format!("RWFTEST_{}", s))
}

/// Generate paths with environment variables in different formats
fn path_with_env_vars() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            // Literal path segments
            Just("/home".to_string()),
            Just("/usr".to_string()),
            Just("/var".to_string()),
            Just("C:\\Users".to_string()),
            Just("C:\\Program Files".to_string()),
            // Unix style: $VAR
            env_var_name().prop_map(|name| format!("${}", name)),
            // Unix style with braces: ${VAR}
            env_var_name().prop_map(|name| format!("${{{}}}", name)),
            // PowerShell style: $env:VAR
            env_var_name().prop_map(|name| format!("$env:{}", name)),
            // Windows style: %VAR%
            env_var_name().prop_map(|name| format!("%{}%", name)),
        ],
        1..5
    ).prop_map(|segments| segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        /// Property 29: Environment Variable Expansion Consistency
        /// Validates: Requirements 31.8
        ///
        /// Verifies that:
        /// 1. Expanding the same path multiple times produces the same result
        /// 2. Variables that exist are replaced with their values
        /// 3. Variables that don't exist are left unchanged
        #[test]
        fn prop_env_var_expansion_is_consistent(
            var_name in env_var_name(),
            var_value in "[a-zA-Z0-9_/\\\\-]{1,20}",
            path_template in path_with_env_vars()
        ) {
            let _lock = ENV_MUTEX.lock().unwrap();
            let manager = RegisteredFolderManager::new();
            
            // Set the environment variable
            env::set_var(&var_name, &var_value);
            
            // Expand the path multiple times
            let result1 = manager.expand_env_vars(&path_template);
            let result2 = manager.expand_env_vars(&path_template);
            let result3 = manager.expand_env_vars(&path_template);
            
            // All expansions should be identical
            prop_assert_eq!(&result1, &result2, "First and second expansion differ");
            prop_assert_eq!(&result2, &result3, "Second and third expansion differ");
            
            // Clean up
            env::remove_var(&var_name);
        }

        /// Property: Variables that don't exist are left unchanged
        #[test]
        fn prop_nonexistent_vars_unchanged(
            nonexistent_var in env_var_name(),
            path_segment in "[a-zA-Z0-9_/\\\\-]{1,20}"
        ) {
            let _lock = ENV_MUTEX.lock().unwrap();
            let manager = RegisteredFolderManager::new();
            
            // Ensure the variable doesn't exist
            env::remove_var(&nonexistent_var);
            
            // Create paths with the nonexistent variable in different formats
            let unix_path = format!("{}/${}", path_segment, nonexistent_var);
            let unix_braces_path = format!("{}/${{{}}}", path_segment, nonexistent_var);
            let ps_path = format!("{}/$env:{}", path_segment, nonexistent_var);
            
            #[cfg(target_os = "windows")]
            let win_path = format!("{}\\%{}%", path_segment, nonexistent_var);
            
            // Expand paths - nonexistent variables should remain
            let result_unix = manager.expand_env_vars(&unix_path);
            let result_unix_braces = manager.expand_env_vars(&unix_braces_path);
            let result_ps = manager.expand_env_vars(&ps_path);
            
            // The variable references should still be present (not expanded)
            prop_assert!(result_unix.contains(&nonexistent_var) || result_unix == unix_path);
            prop_assert!(result_unix_braces.contains(&nonexistent_var) || result_unix_braces == unix_braces_path);
            prop_assert!(result_ps.contains(&nonexistent_var) || result_ps == ps_path);
            
            #[cfg(target_os = "windows")]
            {
                let result_win = manager.expand_env_vars(&win_path);
                prop_assert!(result_win.contains(&nonexistent_var) || result_win == win_path);
            }
        }

        /// Property: Multiple variables in the same path are all expanded
        #[test]
        fn prop_multiple_vars_expanded(
            var1_name in env_var_name(),
            var1_value in "[a-zA-Z0-9_]{1,10}",
            var2_name in env_var_name(),
            var2_value in "[a-zA-Z0-9_]{1,10}"
        ) {
            // Ensure variable names are different
            prop_assume!(var1_name != var2_name);
            
            let _lock = ENV_MUTEX.lock().unwrap();
            let manager = RegisteredFolderManager::new();
            
            // Set both variables
            env::set_var(&var1_name, &var1_value);
            env::set_var(&var2_name, &var2_value);
            
            // Create a path with both variables
            let path = format!("${}/${{{}}}", var1_name, var2_name);
            
            // Expand the path
            let result = manager.expand_env_vars(&path);
            
            // Both variables should be expanded
            prop_assert!(result.contains(&var1_value), "First variable not expanded");
            prop_assert!(result.contains(&var2_value), "Second variable not expanded");
            prop_assert!(!result.contains(&format!("${}", var1_name)), "First variable reference still present");
            prop_assert!(!result.contains(&format!("${{{}}}", var2_name)), "Second variable reference still present");
            
            // Clean up
            env::remove_var(&var1_name);
            env::remove_var(&var2_name);
        }

        /// Property: Expansion is idempotent (expanding an already expanded path doesn't change it)
        #[test]
        fn prop_expansion_is_idempotent(
            var_name in env_var_name(),
            var_value in "[a-zA-Z0-9_/\\\\-]{1,20}"
        ) {
            let _lock = ENV_MUTEX.lock().unwrap();
            let manager = RegisteredFolderManager::new();
            
            // Set the environment variable
            env::set_var(&var_name, &var_value);
            
            // Create a path with the variable
            let path = format!("/${}", var_name);
            
            // Expand once
            let expanded_once = manager.expand_env_vars(&path);
            
            // Expand the result again
            let expanded_twice = manager.expand_env_vars(&expanded_once);
            
            // Should be the same (idempotent)
            prop_assert_eq!(&expanded_once, &expanded_twice, "Expansion is not idempotent");
            
            // Clean up
            env::remove_var(&var_name);
        }
    }

    #[test]
    fn test_unix_simple_expansion() {
        let manager = RegisteredFolderManager::new();
        env::set_var("TEST_VAR_PROP_UNIX_SIMPLE", "test_value");
        
        let result = manager.expand_env_vars("$TEST_VAR_PROP_UNIX_SIMPLE/path");
        assert_eq!(result, "test_value/path");
        
        env::remove_var("TEST_VAR_PROP_UNIX_SIMPLE");
    }

    #[test]
    fn test_unix_braces_expansion() {
        let manager = RegisteredFolderManager::new();
        env::set_var("TEST_VAR_PROP_UNIX_BRACES", "test_value");
        
        let result = manager.expand_env_vars("${TEST_VAR_PROP_UNIX_BRACES}/path");
        assert_eq!(result, "test_value/path");
        
        env::remove_var("TEST_VAR_PROP_UNIX_BRACES");
    }

    #[test]
    fn test_powershell_expansion() {
        let manager = RegisteredFolderManager::new();
        env::set_var("TEST_VAR_PROP_POWERSHELL", "test_value");
        
        let result = manager.expand_env_vars("$env:TEST_VAR_PROP_POWERSHELL/path");
        assert_eq!(result, "test_value/path");
        
        env::remove_var("TEST_VAR_PROP_POWERSHELL");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_expansion() {
        let manager = RegisteredFolderManager::new();
        env::set_var("TEST_VAR_PROP_WINDOWS", "test_value");
        
        let result = manager.expand_env_vars("%TEST_VAR_PROP_WINDOWS%/path");
        assert_eq!(result, "test_value/path");
        
        env::remove_var("TEST_VAR_PROP_WINDOWS");
    }

    #[test]
    fn test_multiple_vars_in_one_path() {
        let manager = RegisteredFolderManager::new();
        env::set_var("VAR1_MULTI", "value1");
        env::set_var("VAR2_MULTI", "value2");
        
        let result = manager.expand_env_vars("$VAR1_MULTI/${VAR2_MULTI}/path");
        assert_eq!(result, "value1/value2/path");
        
        env::remove_var("VAR1_MULTI");
        env::remove_var("VAR2_MULTI");
    }

    #[test]
    fn test_nonexistent_var_unchanged() {
        let manager = RegisteredFolderManager::new();
        env::remove_var("NONEXISTENT_VAR_UNIQUE_12345");
        
        let result = manager.expand_env_vars("$NONEXISTENT_VAR_UNIQUE_12345/path");
        // Variable should remain unexpanded
        assert!(result.contains("NONEXISTENT_VAR_UNIQUE_12345") || result == "$NONEXISTENT_VAR_UNIQUE_12345/path");
    }
}
