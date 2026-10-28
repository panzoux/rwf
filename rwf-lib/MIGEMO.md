# Migemo Support

This document describes the migemo (Japanese romaji search) support in rwf-lib.

## Overview

Migemo is a tool that enables incremental search of Japanese text using romaji (romanized Japanese) input. For example, typing "nihon" will match files containing "日本" (Japan in Japanese).

This implementation uses the [rustmigemo](https://github.com/oguna/rustmigemo) library.

## Building with Migemo Support

Migemo support is an optional feature. To enable it, build with the `migemo` feature flag:

```bash
cargo build --features migemo
```

Or add it to your `Cargo.toml`:

```toml
[dependencies]
rwf-lib = { version = "0.1.0", features = ["migemo"] }
```

## Dictionary Requirements

Migemo requires a dictionary file to function. Two formats are supported:

- **Normal dictionary** `migemo-dict` — the C/Migemo UTF-8 text dictionary. It is converted to the compact form in memory when loaded, so loading takes noticeably longer than the compact file (about 7 s in a debug build for the 5 MB dictionary).
- **Compact dictionary** `migemo-compact-dict` — the rustmigemo binary format; loads quickly.

The **normal dictionary is tried first; the compact dictionary is only a fallback** used when no normal dictionary is found (or none of them loads). The format is detected from the file content, so a configured path may point at either.

Search order (first one that loads wins):

1. The configured path (`Search.DictPath`), then the `RWF_MIGEMO_DICT` environment variable. A file is used as is; a directory is searched for `migemo-dict` then `migemo-compact-dict` (also under its `utf-8` subfolder).
2. **Normal dictionary** `migemo-dict`, in each of these directories, in order:
   1. `dict/` and `./` (current directory), `dict/utf-8/`
   2. `%APPDATA%\rwf\dict\` and `%APPDATA%\rwf\dict\utf-8\` (Windows)
   3. `~/.migemo/`, `~/.migemo/utf-8/`, `~/.config/migemo/`, `~/.config/migemo/utf-8/`
   4. `/usr/share/migemo/utf-8/`, `/usr/local/share/migemo/utf-8/`, `/opt/homebrew/share/migemo/utf-8/`
3. **Compact dictionary** `migemo-compact-dict`, in the same directories, in the same order.

### Obtaining the Dictionary

- Normal: `migemo-dict` from [C/Migemo](https://github.com/koron/cmigemo) (use the UTF-8 one).
- Compact: [migemo-compact-dict-latest](https://github.com/oguna/rustmigemo/releases).

Place the file in one of the directories listed above.

## Usage

### Basic Usage

```rust
use rwf_lib::model::SearchModel;

let mut search = SearchModel::new();

// Load the migemo dictionary (optional, will search common paths)
if let Err(e) = search.load_migemo_dict_auto() {
    eprintln!("Failed to load migemo dictionary: {}", e);
    // Migemo search will not be available
}

// Enable migemo mode
search.use_migemo = true;

// Now search with romaji
search.query = "nihon".to_string();
search.filter_entries(&entries);

// This will match files containing "日本", "にほん", "ニホン", etc.
```

### Loading Dictionary from Custom Path

```rust
use std::path::Path;

let mut search = SearchModel::new();

// Load dictionary from a specific path
let dict_path = Path::new("/path/to/migemo-compact-dict");
if let Err(e) = search.load_migemo_dict(dict_path) {
    eprintln!("Failed to load migemo dictionary: {}", e);
}

search.use_migemo = true;
```

### Toggling Migemo Mode

```rust
// Enable migemo search
search.use_migemo = true;

// Disable migemo search (use regular wildcard/regex search)
search.use_migemo = false;
```

## How It Works

When migemo mode is enabled:

1. The user types a romaji query (e.g., "kensaku")
2. The migemo library converts it to a regex pattern that matches:
   - The original romaji: "kensaku"
   - Hiragana: "けんさく"
   - Katakana: "ケンサク"
   - Kanji variations: "検索", "建策", "憲作", etc.
3. The generated regex is applied to filter file entries

## Fallback Behavior

If migemo is enabled but:
- The dictionary is not loaded, OR
- The regex generation fails

The search will fall back to simple substring matching (case-insensitive by default).

## Performance Considerations

- Dictionary loading is done once and cached in memory
- Regex generation happens for each search query
- For large file lists, migemo search may be slightly slower than simple wildcard matching
- The dictionary file is approximately 5-10 MB in memory

## Compatibility

- Migemo support is completely optional
- Code compiles and runs normally without the `migemo` feature
- The `use_migemo` field is always present in `SearchModel` but only functional when the feature is enabled
- All existing search functionality (wildcards, regex) continues to work when migemo is enabled

## Limitations

- Requires a dictionary file to be present on the system
- Dictionary must be in the rustmigemo compact format
- Only supports Japanese language search
- Regex generation may produce complex patterns that could impact performance

## Testing

Run tests with migemo support:

```bash
cargo test --features migemo
```

The migemo-specific tests will be skipped if the dictionary file is not found.

## References

- [rustmigemo GitHub Repository](https://github.com/oguna/rustmigemo)
- [C/Migemo (original implementation)](https://github.com/koron/cmigemo)
- [Migemo Wikipedia (Japanese)](https://ja.wikipedia.org/wiki/Migemo)
