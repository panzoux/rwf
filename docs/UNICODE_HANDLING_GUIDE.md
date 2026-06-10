# RWF: Reactive Worker Filemanager - Unicode Handling Guide

## Quick Reference

When working with strings in the UI, always use the Unicode-aware utilities from `rwf-bin/src/ui/unicode_utils.rs`.

## Common Scenarios

### Truncating Text

**❌ DON'T DO THIS:**
```rust
// WRONG - causes crashes with Japanese text!
let truncated = &filename[..max_len];

// WRONG - counts characters, not display width
let truncated = filename.chars().take(max_len).collect::<String>();
```

**✅ DO THIS:**
```rust
use super::truncate_to_width;

let truncated = truncate_to_width(&filename, max_width);
```

### Padding Text

**❌ DON'T DO THIS:**
```rust
// WRONG - doesn't account for CJK character width
format!("{:width$}", text, width = target_width)
```

**✅ DO THIS:**
```rust
use super::pad_to_width;

let padded = pad_to_width(&text, target_width);
```

### Shortening Paths

**❌ DON'T DO THIS:**
```rust
// WRONG - byte-based slicing
format!("...{}", &path[path.len() - max_len..])
```

**✅ DO THIS:**
```rust
use super::shorten_path;

let shortened = shorten_path(&path, max_width);
```

## Why This Matters

### Display Width vs Character Count

```rust
// ASCII characters
"hello".chars().count()  // = 5
"hello".width()          // = 5 ✓ Same

// Japanese characters
"日本語".chars().count()  // = 3
"日本語".width()          // = 6 ✗ Different!
```

Each Japanese/Chinese/Korean character occupies **2 terminal columns** but counts as **1 character**.

### Crash Prevention

```rust
// This WILL crash with Japanese text:
let s = "日本語";
let bad = &s[..2];  // ❌ Slices mid-character! PANIC!

// This is SAFE:
let good = truncate_to_width(s, 2);  // ✓ Respects character boundaries
```

## Available Functions

### `truncate_to_width(s: &str, max_width: usize) -> String`

Truncates string to fit within display width, adding "..." if needed.

```rust
truncate_to_width("hello world", 8)      // "hello..."
truncate_to_width("日本語ファイル", 8)    // "日本..."
truncate_to_width("short", 10)           // "short"
```

### `pad_to_width(s: &str, target_width: usize) -> String`

Pads string with spaces to reach target display width.

```rust
pad_to_width("hello", 10)    // "hello     " (width 10)
pad_to_width("日本", 10)     // "日本      " (width 10)
```

### `shorten_path(path: &str, max_width: usize) -> String`

Shortens path intelligently, preserving the filename when possible.

```rust
shorten_path("/home/user/documents/file.txt", 20)
// "...file.txt"

shorten_path("/home/user/日本語.txt", 20)
// "...日本語.txt"
```

## Testing Your Changes

Always test with Japanese/CJK filenames:

```rust
#[test]
fn test_my_ui_component() {
    // Test with ASCII
    let result = my_function("test.txt");
    assert!(result.width() <= MAX_WIDTH);
    
    // Test with Japanese
    let result = my_function("日本語ファイル.txt");
    assert!(result.width() <= MAX_WIDTH);
    
    // Test with mixed
    let result = my_function("test日本語.txt");
    assert!(result.width() <= MAX_WIDTH);
}
```

## Common Pitfalls

### 1. String Slicing by Index

```rust
// ❌ NEVER do this with user input:
&s[..n]
&s[start..end]

// ✅ Use these instead:
truncate_to_width(s, n)
s.char_indices().nth(n)  // If you need character boundaries
```

### 2. Assuming Character Count = Display Width

```rust
// ❌ WRONG:
if text.chars().count() > max_width { ... }

// ✅ CORRECT:
use unicode_width::UnicodeWidthStr;
if text.width() > max_width { ... }
```

### 3. Manual Padding

```rust
// ❌ WRONG:
format!("{}{}", text, " ".repeat(target - text.len()))

// ✅ CORRECT:
pad_to_width(&text, target)
```

## Import Statement

In any UI module:

```rust
use super::{truncate_to_width, pad_to_width, shorten_path};
```

Or from outside the UI module:

```rust
use crate::ui::{truncate_to_width, pad_to_width, shorten_path};
```

## Additional Resources

- [Unicode Width Crate Documentation](https://docs.rs/unicode-width/)
- [UTF-8 Everywhere Manifesto](http://utf8everywhere.org/)
- [Unicode Standard Annex #11 - East Asian Width](https://www.unicode.org/reports/tr11/)

## Questions?

If you're unsure whether to use these utilities, ask yourself:

1. Am I displaying text in the terminal UI? → **Use the utilities**
2. Am I truncating or padding text? → **Use the utilities**
3. Could this text contain Japanese/Chinese/Korean characters? → **Use the utilities**
4. Am I slicing a string by index? → **DON'T! Use the utilities**

When in doubt, use the utilities. They're safe for all text, including ASCII.
