//! Leap Navigation filter: AND-segment matching with optional Migemo.

use crate::model::{FileEntry, SearchModel};

/// Split `query` on unescaped spaces into filter segments.
/// `"map tex"` → `["map", "tex"]`.
/// `"file\ name"` → `["file name"]` (backslash-space = literal space).
/// Empty segments (from trailing/doubled spaces) are discarded.
pub fn parse_segments(query: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = query.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&' ') {
            chars.next();
            current.push(' ');
        } else if c == ' ' {
            if !current.is_empty() {
                segments.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

/// Match priority for a single entry: higher value = higher cursor priority.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum MatchPriority {
    None,
    Migemo,
    Substring,
    Prefix,
}

/// Check if `name` matches a single segment (case-insensitive).
fn match_segment(name: &str, seg: &str, migemo_regex: Option<&regex::Regex>) -> MatchPriority {
    let name_lower = name.to_lowercase();
    let seg_lower = seg.to_lowercase();

    if name_lower.starts_with(&seg_lower) {
        return MatchPriority::Prefix;
    }
    if name_lower.contains(&seg_lower) {
        return MatchPriority::Substring;
    }
    if let Some(re) = migemo_regex {
        if re.is_match(name) {
            return MatchPriority::Migemo;
        }
    }
    MatchPriority::None
}

/// Apply the leap local filter to `raw_entries`.
///
/// Returns `(visible_entries, best_cursor_idx)` where:
/// - `visible_entries` is the subset of `raw_entries` that pass all segments.
/// - `best_cursor_idx` is the index within the returned vec (prefix-match preferred).
///
/// If `local_filter` is empty, returns all entries with cursor at 0.
pub fn apply_leap_filter<'a>(
    raw_entries: &'a [FileEntry],
    local_filter: &str,
    search: &SearchModel,
) -> (Vec<&'a FileEntry>, usize) {
    if local_filter.is_empty() {
        return (raw_entries.iter().collect(), 0);
    }

    let segments = parse_segments(local_filter);
    if segments.is_empty() {
        return (raw_entries.iter().collect(), 0);
    }

    // Build Migemo regexes for each segment (once per filter call).
    let migemo_regexes: Vec<Option<regex::Regex>> = segments
        .iter()
        .map(|seg| {
            search
                .get_migemo_regex(seg, false)
                .and_then(|pattern| regex::Regex::new(&pattern).ok())
        })
        .collect();

    let mut visible: Vec<(&FileEntry, MatchPriority)> = Vec::new();

    for entry in raw_entries {
        // An entry passes if ALL segments match (AND logic).
        let mut overall_priority = MatchPriority::Prefix;
        let mut all_match = true;
        for (seg, mig) in segments.iter().zip(migemo_regexes.iter()) {
            let tier = match_segment(&entry.name, seg, mig.as_ref());
            if tier == MatchPriority::None {
                all_match = false;
                break;
            }
            if tier < overall_priority {
                overall_priority = tier;
            }
        }
        if all_match {
            visible.push((entry, overall_priority));
        }
    }

    if visible.is_empty() {
        return (Vec::new(), 0);
    }

    // Cursor goes to first prefix-match; otherwise first visible entry.
    let best = visible
        .iter()
        .enumerate()
        .find(|(_, (_, p))| *p == MatchPriority::Prefix)
        .map(|(i, _)| i)
        .unwrap_or(0);

    (visible.into_iter().map(|(e, _)| e).collect(), best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Location;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(name)),
            size: 0,
            is_dir,
            is_hidden: false,
            modified: SystemTime::UNIX_EPOCH,
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    fn entries(names: &[(&str, bool)]) -> Vec<FileEntry> {
        names.iter().map(|(n, d)| make_entry(n, *d)).collect()
    }

    fn no_migemo() -> SearchModel {
        SearchModel::new()
    }

    // ---- parse_segments ----

    #[test]
    fn segments_simple() {
        assert_eq!(parse_segments("map tex"), vec!["map", "tex"]);
    }

    #[test]
    fn segments_escaped_space() {
        assert_eq!(parse_segments("file\\ name"), vec!["file name"]);
    }

    #[test]
    fn segments_trailing_space_ignored() {
        assert_eq!(parse_segments("map "), vec!["map"]);
    }

    #[test]
    fn segments_empty() {
        assert!(parse_segments("").is_empty());
    }

    // ---- apply_leap_filter ----

    #[test]
    fn filter_empty_shows_all() {
        let e = entries(&[("maps", true), ("main.rs", false), ("utils.rs", false)]);
        let (vis, cur) = apply_leap_filter(&e, "", &no_migemo());
        assert_eq!(vis.len(), 3);
        assert_eq!(cur, 0);
    }

    #[test]
    fn filter_prefix_match() {
        let e = entries(&[("maps", true), ("main.rs", false), ("utils.rs", false)]);
        let (vis, cur) = apply_leap_filter(&e, "ma", &no_migemo());
        let names: Vec<_> = vis.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["maps", "main.rs"]);
        assert_eq!(cur, 0); // first prefix match
    }

    #[test]
    fn filter_and_segments_order_independent() {
        let e = entries(&[("tex2map", false), ("map2tex", false), ("readme", false)]);
        let (vis, _) = apply_leap_filter(&e, "map tex", &no_migemo());
        assert_eq!(vis.len(), 2);
        assert!(vis.iter().any(|x| x.name == "tex2map"));
        assert!(vis.iter().any(|x| x.name == "map2tex"));
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let e = entries(&[("maps", true), ("main.rs", false)]);
        let (vis, _) = apply_leap_filter(&e, "xyz", &no_migemo());
        assert!(vis.is_empty());
    }

    #[test]
    fn filter_case_insensitive() {
        let e = entries(&[("MapModels", true), ("utils.rs", false)]);
        let (vis, _) = apply_leap_filter(&e, "mapm", &no_migemo());
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].name, "MapModels");
    }

    #[test]
    fn filter_cursor_on_prefix_match() {
        let e = entries(&[("tex2map", false), ("main.rs", false), ("maps", true)]);
        // "tex2map" contains "ma" as substring; "main.rs" and "maps" start with "ma"
        let (vis, cur) = apply_leap_filter(&e, "ma", &no_migemo());
        // cursor should land on first prefix match: "main.rs" at index 1 in original,
        // but "tex2map" is first in vis (substring)? No — let's check the order.
        // "tex2map": substring match → included
        // "main.rs": prefix match
        // "maps": prefix match
        // "tex2map" is entry[0], it contains "ma" → included first; cursor = index of first prefix
        assert!(vis.iter().any(|x| x.name == "main.rs"));
        // cursor points to a prefix match, not "tex2map"
        assert_ne!(vis[cur].name, "tex2map");
    }

    #[test]
    fn filter_single_space_trailing_no_error() {
        let e = entries(&[("maps", true)]);
        let (vis, _) = apply_leap_filter(&e, "ma ", &no_migemo());
        // trailing space produces segment ["ma"], which matches
        assert_eq!(vis.len(), 1);
    }
}
