//! TWF-compatible pattern-based rename
//!
//! Pattern modes (same as TWF's ApplyRenamePattern):
//!
//! 1. `find` starts with `s/` → Perl-style substitution: `s/search/replace/[gi]`
//! 2. `find` starts with `tr/` → character transliteration: `tr/from/to/`
//! 3. `use_regex = true` → `find` is a regex, `replace` is replacement (first occurrence)
//! 4. `use_regex = false` → literal string replacement (all occurrences, case-optional)
//!
//! Always returns a `String`; equals the original filename when no match/change.

#![allow(clippy::unwrap_used)] // TODO(M6): ratchet — see plan/quality_overhaul.md

use regex::Regex;

/// Apply a rename pattern to a filename using TWF-compatible logic.
pub fn apply_rename_pattern(
    filename: &str,
    find: &str,
    replace: &str,
    use_regex: bool,
    case_sensitive: bool,
) -> String {
    if find.is_empty() {
        return filename.to_string();
    }

    if find.starts_with("s/") {
        return apply_s_command(filename, find);
    }

    if find.starts_with("tr/") {
        return apply_tr_command(filename, find);
    }

    if use_regex {
        let pattern = if case_sensitive {
            find.to_string()
        } else {
            format!("(?i){}", find)
        };
        // Normalize $N → ${N} so "$1_foo" is not misread as group "1_foo"
        // (Rust regex crate's $name accepts [0-9A-Za-z_]+, unlike C#/.NET which stops at digits)
        let normalized = normalize_replacement(replace);
        match Regex::new(&pattern) {
            Ok(re) => re.replacen(filename, 1, normalized.as_str()).to_string(),
            Err(_) => filename.to_string(),
        }
    } else if case_sensitive {
        filename.replace(find, replace)
    } else {
        case_insensitive_replace_all(filename, find, replace)
    }
}

/// Generate preview for all target files (TWF shows all, unchanged ones included).
///
/// Returns `Vec<(original, new_name)>`. Pairs where both are equal are unchanged.
pub fn generate_preview(
    filenames: &[String],
    find: &str,
    replace: &str,
    use_regex: bool,
    case_sensitive: bool,
) -> Vec<(String, String)> {
    filenames
        .iter()
        .map(|name| {
            let new_name = apply_rename_pattern(name, find, replace, use_regex, case_sensitive);
            (name.clone(), new_name)
        })
        .collect()
}

/// Validate find/replace inputs. Returns `Err` with a user-visible message on failure.
pub fn validate_inputs(find: &str, use_regex: bool) -> Result<(), String> {
    if find.is_empty() {
        return Err("Find pattern cannot be empty".to_string());
    }
    if find.starts_with("s/") || find.starts_with("tr/") {
        return Ok(());
    }
    if use_regex {
        let pattern = format!("(?i){}", find); // test with case-insensitive prefix
        Regex::new(&pattern).map_err(|e| format!("Invalid regex: {}", e))?;
    }
    Ok(())
}

// ── s/ command ────────────────────────────────────────────────────────────────

fn apply_s_command(filename: &str, command: &str) -> String {
    let parts = split_slash_command(command);
    if parts.len() < 3 {
        return filename.to_string();
    }
    let search = unescape_slash(&parts[1]);
    let replacement = normalize_replacement(&unescape_slash(&parts[2]));
    let flags = if parts.len() > 3 {
        parts[3].as_str()
    } else {
        ""
    };

    let global = flags.contains('g');
    let ignore_case = flags.contains('i');

    let pattern = if ignore_case {
        format!("(?i){}", search)
    } else {
        search
    };

    match Regex::new(&pattern) {
        Ok(re) => {
            if global {
                re.replace_all(filename, replacement.as_str()).to_string()
            } else {
                re.replacen(filename, 1, replacement.as_str()).to_string()
            }
        }
        Err(_) => filename.to_string(),
    }
}

// ── tr/ command ───────────────────────────────────────────────────────────────

fn apply_tr_command(filename: &str, command: &str) -> String {
    let parts = split_slash_command(command);
    if parts.len() < 3 {
        return filename.to_string();
    }
    let from_chars: Vec<char> = unescape_slash(&parts[1]).chars().collect();
    let to_chars: Vec<char> = unescape_slash(&parts[2]).chars().collect();

    filename
        .chars()
        .map(|c| {
            if let Some(idx) = from_chars.iter().position(|&f| f == c) {
                to_chars.get(idx).copied().unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Convert bare `$N` capture references to `${N}` so Rust's regex crate stops at digit boundaries.
///
/// Rust's regex crate parses `$name` where `name` is `[0-9A-Za-z_]+`, so `$1_` is read
/// as group "1_" (empty if it doesn't exist).  C#/.NET stops at the first non-digit, so
/// `$1_` means group 1 followed by literal `_`.  Converting to `${N}` is unambiguous in
/// both engines.  Already-braced `${N}` sequences are left unchanged.
fn normalize_replacement(replace: &str) -> String {
    let re = Regex::new(r"\$(\d+)").unwrap();
    re.replace_all(replace, |caps: &regex::Captures| {
        format!("${{{}}}", &caps[1])
    })
    .to_string()
}

fn split_slash_command(command: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut escaped = false;

    for c in command.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            current.push(c);
            escaped = true;
        } else if c == '/' {
            parts.push(current.clone());
            current.clear();
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts
}

fn unescape_slash(s: &str) -> String {
    s.replace("\\/", "/")
}

fn case_insensitive_replace_all(s: &str, find: &str, replace: &str) -> String {
    if find.is_empty() {
        return s.to_string();
    }
    let escaped = regex::escape(find);
    let pattern = format!("(?i){}", escaped);
    match Regex::new(&pattern) {
        Ok(re) => re.replace_all(s, replace).to_string(),
        Err(_) => s.to_string(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_mode_simple() {
        assert_eq!(
            apply_rename_pattern("photo001.jpg", r"\d+", "NUM", true, true),
            "photoNUM.jpg"
        );
    }

    #[test]
    fn test_regex_mode_case_insensitive() {
        assert_eq!(
            apply_rename_pattern("Photo.JPG", "photo", "img", true, false),
            "img.JPG"
        );
    }

    #[test]
    fn test_regex_capture_group() {
        assert_eq!(
            apply_rename_pattern("file001.txt", r"(file)(\d+)", "${2}_${1}", true, true),
            "001_file.txt"
        );
    }

    #[test]
    fn test_plain_mode_replace_all() {
        assert_eq!(
            apply_rename_pattern("aa_bb_aa.txt", "aa", "cc", false, true),
            "cc_bb_cc.txt"
        );
    }

    #[test]
    fn test_plain_mode_case_insensitive() {
        assert_eq!(
            apply_rename_pattern("Photo.jpg", "photo", "img", false, false),
            "img.jpg"
        );
    }

    #[test]
    fn test_s_command_basic() {
        assert_eq!(
            apply_rename_pattern("file.txt", "s/file/doc/", "", false, true),
            "doc.txt"
        );
    }

    #[test]
    fn test_s_command_global() {
        assert_eq!(
            apply_rename_pattern("aa_aa.txt", "s/aa/bb/g", "", false, true),
            "bb_bb.txt"
        );
    }

    #[test]
    fn test_s_command_case_insensitive_flag() {
        assert_eq!(
            apply_rename_pattern("Photo.JPG", "s/photo/img/i", "", false, true),
            "img.JPG"
        );
    }

    #[test]
    fn test_tr_command() {
        assert_eq!(
            apply_rename_pattern("abc.txt", "tr/abc/xyz/", "", false, true),
            "xyz.txt"
        );
    }

    #[test]
    fn test_empty_find_returns_unchanged() {
        assert_eq!(
            apply_rename_pattern("file.txt", "", "anything", true, true),
            "file.txt"
        );
    }

    #[test]
    fn test_invalid_regex_returns_unchanged() {
        assert_eq!(
            apply_rename_pattern("file.txt", "[invalid", "x", true, true),
            "file.txt"
        );
    }

    #[test]
    fn test_generate_preview_all_files() {
        let files = vec!["file1.txt".to_string(), "image.jpg".to_string()];
        let preview = generate_preview(&files, r"\.txt$", ".bak", true, true);
        assert_eq!(preview.len(), 2);
        assert_eq!(
            preview[0],
            ("file1.txt".to_string(), "file1.bak".to_string())
        );
        // jpg unchanged — still in preview
        assert_eq!(
            preview[1],
            ("image.jpg".to_string(), "image.jpg".to_string())
        );
    }
}
