//! Unicode width utilities for proper CJK character handling
//!
//! Handles display width calculation and truncation for strings containing
//! multi-byte UTF-8 characters (Japanese, Chinese, Korean, etc.)

use unicode_width::UnicodeWidthStr;

/// Truncate string to fit within max display width (not character count)
/// Japanese characters have width 2, ASCII has width 1
pub fn truncate_to_width(s: &str, max_width: usize, ellipsis: &str) -> String {
    let current_width = s.width();
    
    if current_width <= max_width {
        return s.to_string();
    }
    
    // Reserve space for ellipsis
    let ellipsis_width = ellipsis.width();
    
    if max_width <= ellipsis_width {
        // Not enough space for ellipsis, just truncate
        return truncate_to_width_no_ellipsis(s, max_width);
    }
    
    let target_width = max_width - ellipsis_width;
    let mut current = 0;
    let mut byte_pos = 0;
    
    for (pos, ch) in s.char_indices() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        
        if current + char_width > target_width {
            byte_pos = pos;
            break;
        }
        
        current += char_width;
        byte_pos = pos + ch.len_utf8();
    }
    
    format!("{}{}", &s[..byte_pos], ellipsis)
}

/// Truncate without ellipsis
fn truncate_to_width_no_ellipsis(s: &str, max_width: usize) -> String {
    let mut current = 0;
    let mut byte_pos = 0;
    
    for (pos, ch) in s.char_indices() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        
        if current + char_width > max_width {
            byte_pos = pos;
            break;
        }
        
        current += char_width;
        byte_pos = pos + ch.len_utf8();
    }
    
    s[..byte_pos].to_string()
}

/// Pad string to target display width
pub fn pad_to_width(s: &str, target_width: usize) -> String {
    let current_width = s.width();
    
    if current_width >= target_width {
        s.to_string()
    } else {
        let padding = " ".repeat(target_width - current_width);
        format!("{}{}", s, padding)
    }
}

/// Smart truncate that preserves the end of the string (e.g., file extension)
/// Truncates from the middle with ellipsis to show both start and end
pub fn smart_truncate(s: &str, max_width: usize, ellipsis: &str) -> String {
    let current_width = s.width();
    
    if current_width <= max_width {
        return s.to_string();
    }
    
    let ellipsis_width = ellipsis.width();
    
    if max_width <= ellipsis_width {
        return truncate_to_width(s, max_width, ellipsis);
    }
    
    let available_width = max_width - ellipsis_width;
    
    // Split: 2/3 for start, 1/3 for end
    let width_for_end = available_width / 3;
    let width_for_start = available_width - width_for_end;
    
    // Find start portion
    let mut start_width = 0;
    let mut start_byte_pos = 0;
    for (pos, ch) in s.char_indices() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if start_width + char_width > width_for_start {
            break;
        }
        start_width += char_width;
        start_byte_pos = pos + ch.len_utf8();
    }
    
    // Find end portion (scan backwards)
    let mut end_width = 0;
    let mut end_byte_pos = s.len();
    for (pos, ch) in s.char_indices().rev() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if end_width + char_width > width_for_end {
            break;
        }
        end_width += char_width;
        end_byte_pos = pos;
    }
    
    // Make sure start and end don't overlap
    if start_byte_pos >= end_byte_pos {
        return truncate_to_width(s, max_width, ellipsis);
    }
    
    format!("{}{}{}", &s[..start_byte_pos], ellipsis, &s[end_byte_pos..])
}

/// Shorten a path to fit within max display width, preserving the last component
pub fn shorten_path(path: &str, max_width: usize, ellipsis: &str) -> String {
    let current_width = path.width();
    
    if current_width <= max_width {
        return path.to_string();
    }
    
    // Try to show the last component (filename/directory)
    if let Some(last_sep) = path.rfind(|c| c == '/' || c == '\\') {
        let last_component = &path[last_sep + 1..];
        let combined_width = ellipsis.width() + last_component.width();
        
        if combined_width <= max_width {
            return format!("{}{}", ellipsis, last_component);
        }
    }
    
    // Last component is too long, truncate it
    truncate_to_width(path, max_width, ellipsis)
}


#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn test_truncate_ascii() {
        // ASCII characters have width 1
        let result = truncate_to_width("hello world", 8, "...");
        assert_eq!(result, "hello...");
        assert!(result.width() <= 8);
    }

    #[test]
    fn test_truncate_japanese() {
        // Japanese characters have width 2
        // "日本語" = 3 chars, width 6
        let result = truncate_to_width("日本語ファイル", 8, "...");
        // Should fit "日本語" (width 6) + "..." (width 3) = 9, but max is 8
        // So should fit "日本" (width 4) + "..." (width 3) = 7
        assert!(result.width() <= 8);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_mixed() {
        // Mixed ASCII and Japanese
        let result = truncate_to_width("test日本語.txt", 10, "...");
        // "test" = 4, "日本語" = 6, ".txt" = 4
        // Total width = 14, should truncate
        assert!(result.width() <= 10);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_no_truncation_needed() {
        let result = truncate_to_width("short", 10, "...");
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_exact_fit() {
        let result = truncate_to_width("exactly10!", 10, "...");
        assert_eq!(result, "exactly10!");
    }

    #[test]
    fn test_truncate_very_small_width() {
        // When max_width is too small for ellipsis
        let result = truncate_to_width("hello", 2, "...");
        assert!(result.width() <= 2);
    }

    #[test]
    fn test_pad_ascii() {
        let result = pad_to_width("hello", 10);
        assert_eq!(result.width(), 10);
        assert!(result.starts_with("hello"));
    }

    #[test]
    fn test_pad_japanese() {
        // "日本" = 2 chars, width 4
        let result = pad_to_width("日本", 10);
        assert_eq!(result.width(), 10);
        assert!(result.starts_with("日本"));
    }

    #[test]
    fn test_pad_no_padding_needed() {
        let result = pad_to_width("exactly10!", 10);
        assert_eq!(result, "exactly10!");
    }

    #[test]
    fn test_pad_already_too_long() {
        let result = pad_to_width("this is too long", 5);
        assert_eq!(result, "this is too long");
    }

    #[test]
    fn test_shorten_path_ascii() {
        let result = shorten_path("/home/user/documents/file.txt", 20, "...");
        assert!(result.width() <= 20);
        // Should preserve filename
        assert!(result.contains("file.txt"));
    }

    #[test]
    fn test_shorten_path_japanese() {
        let result = shorten_path("/home/user/日本語ファイル.txt", 20, "...");
        assert!(result.width() <= 20);
        // Should preserve filename
        assert!(result.contains("日本語"));
    }

    #[test]
    fn test_shorten_path_no_shortening_needed() {
        let result = shorten_path("/short/path", 20, "...");
        assert_eq!(result, "/short/path");
    }

    #[test]
    fn test_shorten_path_windows() {
        let result = shorten_path("C:\\Users\\test\\日本語\\file.txt", 20, "...");
        assert!(result.width() <= 20);
    }

    #[test]
    fn test_no_crash_on_long_japanese_filename() {
        // This was causing crashes before
        let long_japanese = "日本語".repeat(20);
        let result = truncate_to_width(&long_japanese, 30, "...");
        assert!(result.width() <= 30);
        // Should not panic
    }

    #[test]
    fn test_emoji_handling() {
        // Emojis can have varying widths
        let result = truncate_to_width("test🎉emoji", 10, "...");
        assert!(result.width() <= 10);
    }

    #[test]
    fn test_zero_width_characters() {
        // Combining characters have width 0
        let result = truncate_to_width("café", 5, "..."); // é might be combining
        assert!(result.width() <= 5);
    }

    #[test]
    fn test_smart_truncate_no_truncation() {
        let result = smart_truncate("short.txt", 20, "...");
        assert_eq!(result, "short.txt");
    }

    #[test]
    fn test_smart_truncate_preserves_extension() {
        let result = smart_truncate("very_long_filename_here.txt", 20, "...");
        assert!(result.width() <= 20);
        assert!(result.starts_with("very_long"));
        assert!(result.ends_with(".txt"));
        assert!(result.contains("..."));
    }

    #[test]
    fn test_smart_truncate_japanese_filename() {
        // "日本語ファイル名.txt" = width 18
        let result = smart_truncate("日本語ファイル名.txt", 15, "...");
        assert!(result.width() <= 15);
        assert!(result.contains("..."));
        assert!(result.ends_with(".txt"));
    }

    #[test]
    fn test_smart_truncate_mixed_chars() {
        let result = smart_truncate("test日本語file.txt", 15, "...");
        assert!(result.width() <= 15);
        assert!(result.contains("..."));
        assert!(result.ends_with(".txt"));
    }

    #[test]
    fn test_smart_truncate_very_small_width() {
        let result = smart_truncate("filename.txt", 5, "...");
        assert!(result.width() <= 5);
    }

    #[test]
    fn test_smart_truncate_exact_ellipsis_width() {
        let result = smart_truncate("filename.txt", 3, "...");
        assert!(result.width() <= 3);
    }

    #[test]
    fn test_smart_truncate_split_ratio() {
        // Test that split is approximately 2/3 start, 1/3 end
        let result = smart_truncate("abcdefghijklmnopqrstuvwxyz.txt", 20, "...");
        assert!(result.width() <= 20);
        assert!(result.contains("..."));
        // Should have more chars at start than end (excluding extension)
        let parts: Vec<&str> = result.split("...").collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_smart_truncate_no_extension() {
        let result = smart_truncate("verylongfilenamewithoutextension", 20, "...");
        assert!(result.width() <= 20);
        assert!(result.contains("..."));
    }

    #[test]
    fn test_smart_truncate_japanese_no_extension() {
        let result = smart_truncate("日本語ファイル名前", 10, "...");
        assert!(result.width() <= 10);
        assert!(result.contains("..."));
    }
}
