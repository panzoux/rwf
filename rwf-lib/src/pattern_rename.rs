//! Pattern-based rename functionality
//!
//! This module implements pattern syntax for batch file renaming with wildcards
//! and replacement tokens.

use regex::Regex;

/// Apply a rename pattern to a filename
///
/// Pattern syntax:
/// - `*` matches any sequence of characters
/// - `?` matches a single character
/// - `[N]` in the replacement refers to the Nth wildcard capture (1-indexed)
/// - `[N:L]` extracts L characters starting at position N (0-indexed)
/// - `[N-M]` extracts characters from position N to M (0-indexed, inclusive)
///
/// Examples:
/// - Pattern: `*.txt` -> `backup_[1].txt` renames `file.txt` to `backup_file.txt`
/// - Pattern: `*_*` -> `[2]_[1]` swaps parts around underscore
/// - Pattern: `*` -> `[0:3]_[1]` takes first 3 chars of first capture and appends second
pub fn apply_pattern(filename: &str, pattern: &str) -> Option<String> {
    // Split pattern into match and replacement parts
    let parts: Vec<&str> = pattern.split("->").map(|s| s.trim()).collect();
    
    if parts.len() != 2 {
        // If no arrow, treat the whole pattern as a simple wildcard match
        // and return the filename unchanged if it matches
        return if matches_wildcard(filename, pattern) {
            Some(filename.to_string())
        } else {
            None
        };
    }
    
    let match_pattern = parts[0];
    let replacement = parts[1];
    
    // Convert wildcard pattern to regex and capture groups
    let (regex_pattern, wildcard_count) = wildcard_to_regex(match_pattern);
    
    let re = Regex::new(&regex_pattern).ok()?;
    let captures = re.captures(filename)?;
    
    // Build replacement string
    let mut result = replacement.to_string();
    
    // Replace [N] tokens with captured groups
    for i in 1..=wildcard_count {
        let token = format!("[{}]", i);
        if let Some(capture) = captures.get(i) {
            result = result.replace(&token, capture.as_str());
        }
    }
    
    // Handle [N:L] and [N-M] substring extraction from the full filename
    result = apply_substring_tokens(&result, filename);
    
    Some(result)
}

/// Check if a filename matches a wildcard pattern
pub fn matches_wildcard(filename: &str, pattern: &str) -> bool {
    let (regex_pattern, _) = wildcard_to_regex(pattern);
    if let Ok(re) = Regex::new(&regex_pattern) {
        re.is_match(filename)
    } else {
        false
    }
}

/// Convert wildcard pattern to regex with capture groups
fn wildcard_to_regex(pattern: &str) -> (String, usize) {
    let mut regex = String::from("^");
    let mut wildcard_count = 0;
    let mut chars = pattern.chars().peekable();
    
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                wildcard_count += 1;
                regex.push_str("(.*)");
            }
            '?' => {
                wildcard_count += 1;
                regex.push_str("(.)");
            }
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }
    
    regex.push('$');
    (regex, wildcard_count)
}

/// Apply substring extraction tokens [N:L] and [N-M]
fn apply_substring_tokens(text: &str, source: &str) -> String {
    let mut result = text.to_string();
    
    // Match [N:L] pattern (start position : length)
    let re_length = Regex::new(r"\[(\d+):(\d+)\]").unwrap();
    for cap in re_length.captures_iter(text) {
        let start: usize = cap[1].parse().unwrap_or(0);
        let length: usize = cap[2].parse().unwrap_or(0);
        let token = &cap[0];
        
        let substring = source.chars()
            .skip(start)
            .take(length)
            .collect::<String>();
        
        result = result.replace(token, &substring);
    }
    
    // Match [N-M] pattern (start position - end position)
    let re_range = Regex::new(r"\[(\d+)-(\d+)\]").unwrap();
    for cap in re_range.captures_iter(text) {
        let start: usize = cap[1].parse().unwrap_or(0);
        let end: usize = cap[2].parse().unwrap_or(0);
        let token = &cap[0];
        
        let substring = source.chars()
            .skip(start)
            .take(end.saturating_sub(start) + 1)
            .collect::<String>();
        
        result = result.replace(token, &substring);
    }
    
    result
}

/// Generate preview of rename operations
pub fn generate_preview(filenames: &[String], pattern: &str) -> Vec<(String, String)> {
    filenames
        .iter()
        .filter_map(|name| {
            apply_pattern(name, pattern).map(|new_name| (name.clone(), new_name))
        })
        .collect()
}

/// Validate that a pattern is syntactically correct
pub fn validate_pattern(pattern: &str) -> Result<(), String> {
    // Check for empty pattern
    if pattern.trim().is_empty() {
        return Err("Pattern cannot be empty".to_string());
    }
    
    // Check for arrow separator
    let parts: Vec<&str> = pattern.split("->").map(|s| s.trim()).collect();
    
    if parts.len() > 2 {
        return Err("Pattern can only contain one '->' separator".to_string());
    }
    
    if parts.len() == 2 {
        let match_pattern = parts[0];
        let replacement = parts[1];
        
        if match_pattern.is_empty() {
            return Err("Match pattern cannot be empty".to_string());
        }
        
        if replacement.is_empty() {
            return Err("Replacement pattern cannot be empty".to_string());
        }
        
        // Try to compile the regex
        let (regex_pattern, _) = wildcard_to_regex(match_pattern);
        Regex::new(&regex_pattern)
            .map_err(|e| format!("Invalid pattern: {}", e))?;
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_wildcard_match() {
        assert_eq!(
            apply_pattern("file.txt", "*.txt -> backup_[1].txt"),
            Some("backup_file.txt".to_string())
        );
    }
    
    #[test]
    fn test_multiple_wildcards() {
        assert_eq!(
            apply_pattern("hello_world.txt", "*_*.txt -> [2]_[1].txt"),
            Some("world_hello.txt".to_string())
        );
    }
    
    #[test]
    fn test_question_mark_wildcard() {
        assert_eq!(
            apply_pattern("file1.txt", "file?.txt -> doc[1].txt"),
            Some("doc1.txt".to_string())
        );
    }
    
    #[test]
    fn test_substring_extraction_length() {
        assert_eq!(
            apply_pattern("document.txt", "*.txt -> [0:3]_backup.txt"),
            Some("doc_backup.txt".to_string())
        );
    }
    
    #[test]
    fn test_substring_extraction_range() {
        assert_eq!(
            apply_pattern("document.txt", "*.txt -> [0-2]_file.txt"),
            Some("doc_file.txt".to_string())
        );
    }
    
    #[test]
    fn test_no_match() {
        assert_eq!(
            apply_pattern("file.pdf", "*.txt -> backup_[1].txt"),
            None
        );
    }
    
    #[test]
    fn test_matches_wildcard() {
        assert!(matches_wildcard("file.txt", "*.txt"));
        assert!(matches_wildcard("file.txt", "file.*"));
        assert!(!matches_wildcard("file.pdf", "*.txt"));
    }
    
    #[test]
    fn test_validate_pattern() {
        assert!(validate_pattern("*.txt -> backup_[1].txt").is_ok());
        assert!(validate_pattern("*_* -> [2]_[1]").is_ok());
        assert!(validate_pattern("").is_err());
        assert!(validate_pattern("*.txt -> ").is_err());
        assert!(validate_pattern(" -> backup.txt").is_err());
    }
    
    #[test]
    fn test_generate_preview() {
        let files = vec![
            "file1.txt".to_string(),
            "file2.txt".to_string(),
            "document.pdf".to_string(),
        ];
        
        let preview = generate_preview(&files, "*.txt -> backup_[1].txt");
        
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0], ("file1.txt".to_string(), "backup_file1.txt".to_string()));
        assert_eq!(preview[1], ("file2.txt".to_string(), "backup_file2.txt".to_string()));
    }
}
