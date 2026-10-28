//! Search model

use super::FileEntry;
use regex::Regex;
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::compact_dictionary_builder;
use rustmigemo::migemo::query::query as migemo_query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use std::time::SystemTime;
use tracing::debug;

const NORMAL_DICT_NAME: &str = "migemo-dict";
const COMPACT_DICT_NAME: &str = "migemo-compact-dict";

type CompactDictCache = HashMap<PathBuf, ((SystemTime, u64), Arc<Vec<u8>>)>;

/// Compact form of text dictionaries already converted in this process.
///
/// Converting the 5 MB text dictionary takes seconds, and every `AppState` (one
/// per test, one per session) loads the dictionary, so it is converted once per
/// file version (path + mtime + length).
fn compact_dict_cache() -> &'static Mutex<CompactDictCache> {
    static CACHE: OnceLock<Mutex<CompactDictCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A compact dictionary starts with a big-endian i32 edge count, so its first
/// byte is 0; the text dictionary starts with a comment or a hiragana key.
fn is_compact_dict(data: &[u8]) -> bool {
    data.first() == Some(&0)
}

/// Whether `rustmigemo`'s compact-dictionary builder can encode this key
/// (ASCII, hiragana, long-vowel mark). The builder prints skipped words to
/// stdout, which is reserved, so unsupported keys are dropped here first.
fn is_encodable_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| matches!(c, '\u{20}'..='\u{7e}' | '\u{3041}'..='\u{3096}' | '\u{30fc}'))
}

/// Parse the C/Migemo text dictionary (`key<TAB>value<TAB>value...`, `;` comments).
fn parse_text_dict(data: &[u8]) -> Result<HashMap<String, Vec<String>>, String> {
    let text = std::str::from_utf8(data).map_err(|_| "not valid UTF-8".to_string())?;
    let mut entries: HashMap<String, Vec<String>> = HashMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let mut fields = line.split('\t');
        let key = fields.next().unwrap_or_default();
        if !is_encodable_key(key) {
            continue;
        }
        let values: Vec<String> = fields
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .collect();
        if !values.is_empty() {
            entries.entry(key.to_string()).or_default().extend(values);
        }
    }
    if entries.is_empty() {
        return Err("no dictionary entries".to_string());
    }
    Ok(entries)
}

/// Paths searched for a migemo dictionary, in priority order:
///
/// 1. the configured path, then `RWF_MIGEMO_DICT` (a file, or a directory
///    holding the dictionary) — normal dictionary before compact;
/// 2. every known location for the normal dictionary (`migemo-dict`);
/// 3. every known location for the compact dictionary (`migemo-compact-dict`),
///    used only when no normal dictionary was found.
fn migemo_dict_candidates(
    config_path: Option<&str>,
    env_path: Option<&str>,
    data_dir: Option<&Path>,
    home_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for explicit in [config_path, env_path].into_iter().flatten() {
        let path = PathBuf::from(explicit);
        if path.is_dir() {
            for name in [NORMAL_DICT_NAME, COMPACT_DICT_NAME] {
                candidates.push(path.join(name));
                candidates.push(path.join("utf-8").join(name));
            }
        } else {
            candidates.push(path);
        }
    }

    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("dict"),
        PathBuf::from("."),
        PathBuf::from("dict/utf-8"),
    ];
    // Windows AppData (%APPDATA%\rwf\dict)
    #[cfg(target_os = "windows")]
    if let Some(data_dir) = data_dir {
        let base = data_dir.join("rwf").join("dict");
        dirs.push(base.clone());
        dirs.push(base.join("utf-8"));
    }
    #[cfg(not(target_os = "windows"))]
    let _ = data_dir;
    if let Some(home) = home_dir {
        dirs.push(home.join(".migemo"));
        dirs.push(home.join(".migemo").join("utf-8"));
        dirs.push(home.join(".config").join("migemo"));
        dirs.push(home.join(".config").join("migemo").join("utf-8"));
    }
    // Linux/macOS system paths
    for system in [
        "/usr/share/migemo/utf-8",
        "/usr/local/share/migemo/utf-8",
        "/opt/homebrew/share/migemo/utf-8",
    ] {
        dirs.push(PathBuf::from(system));
    }

    for name in [NORMAL_DICT_NAME, COMPACT_DICT_NAME] {
        candidates.extend(dirs.iter().map(|d| d.join(name)));
    }
    candidates
}

/// Search state
#[derive(Debug)]
pub struct SearchModel {
    pub query: String,
    pub results: Vec<FileEntry>,
    pub history: Vec<String>,
    pub current_index: Option<usize>,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub use_migemo: bool,
    pub include_pattern: Option<String>,
    pub exclude_pattern: Option<String>,
    migemo_dict: Option<CompactDictionary>,
    migemo_cache: RefCell<HashMap<String, Regex>>,
    migemo_dict_path: Option<String>,
}

impl Default for SearchModel {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchModel {
    /// Create a new empty search model
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            history: Vec::new(),
            current_index: None,
            case_sensitive: false,
            use_regex: false,
            use_migemo: false,
            include_pattern: None,
            exclude_pattern: None,
            migemo_dict: None,
            migemo_cache: RefCell::new(HashMap::new()),
            migemo_dict_path: None,
        }
    }

    /// Load migemo dictionary from file.
    ///
    /// Accepts both the normal text dictionary (`migemo-dict`, UTF-8) and the
    /// rustmigemo compact dictionary (`migemo-compact-dict`); the format is
    /// detected from the content.
    pub fn load_migemo_dict(&mut self, dict_path: &Path) -> Result<(), String> {
        let dict_data = std::fs::read(dict_path).map_err(|e| {
            format!(
                "Failed to read migemo dictionary from {:?}: {}",
                dict_path, e
            )
        })?;

        let compact = if is_compact_dict(&dict_data) {
            dict_data
        } else {
            let stamp = std::fs::metadata(dict_path)
                .ok()
                .and_then(|m| Some((m.modified().ok()?, m.len())));
            let cached = stamp.and_then(|stamp| {
                let cache = compact_dict_cache().lock().ok()?;
                let (cached_stamp, bytes) = cache.get(dict_path)?;
                (*cached_stamp == stamp).then(|| bytes.clone())
            });
            match cached {
                Some(bytes) => bytes,
                None => {
                    let entries = parse_text_dict(&dict_data).map_err(|e| {
                        format!("Failed to parse migemo dictionary {:?}: {}", dict_path, e)
                    })?;
                    let bytes = Arc::new(compact_dictionary_builder::build(entries));
                    if let (Some(stamp), Ok(mut cache)) = (stamp, compact_dict_cache().lock()) {
                        cache.insert(dict_path.to_path_buf(), (stamp, bytes.clone()));
                    }
                    bytes
                }
            }
            .to_vec()
        };

        self.migemo_dict = Some(CompactDictionary::new(&compact));
        self.migemo_dict_path = Some(dict_path.to_string_lossy().to_string());
        // Clear cache when dictionary changes
        self.migemo_cache.borrow_mut().clear();
        Ok(())
    }

    /// Load migemo dictionary from common paths.
    ///
    /// The normal dictionary (`migemo-dict`) is preferred; the compact one
    /// (`migemo-compact-dict`) is only used when no normal dictionary loads.
    /// See [`migemo_dict_candidates`] for the exact order.
    pub fn load_migemo_dict_auto(&mut self, config_path: Option<&str>) -> Result<(), String> {
        let env_path = std::env::var("RWF_MIGEMO_DICT").ok();
        let candidates = migemo_dict_candidates(
            config_path,
            env_path.as_deref(),
            dirs::data_dir().as_deref(),
            dirs::home_dir().as_deref(),
        );
        self.load_first_migemo_dict(&candidates)
    }

    /// Load the first candidate that exists and loads successfully.
    fn load_first_migemo_dict(&mut self, candidates: &[PathBuf]) -> Result<(), String> {
        for path in candidates {
            let exists = path.is_file();
            tracing::info!(
                "Migemo: checking {:?} — {}",
                path,
                if exists { "found" } else { "not found" }
            );
            if exists {
                match self.load_migemo_dict(path) {
                    Ok(_) => {
                        tracing::info!("Migemo dictionary loaded from {:?}", path);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!("Migemo: failed to load {:?}: {}", path, e);
                    }
                }
            }
        }

        Err("Migemo dictionary not found in any of the searched paths".to_string())
    }

    /// Check if migemo library is available
    pub fn is_migemo_available(&self) -> bool {
        true // Mandatory dependency
    }

    /// Check if migemo dictionary is loaded
    pub fn is_migemo_dict_loaded(&self) -> bool {
        self.migemo_dict.is_some()
    }

    /// Get current migemo dictionary path
    pub fn migemo_dict_path(&self) -> Option<&str> {
        self.migemo_dict_path.as_deref()
    }

    /// Build a migemo regex pattern for the given query. Returns None if migemo
    /// is not available or the query should be matched as plain text.
    pub fn get_migemo_regex(&self, query: &str, case_sensitive: bool) -> Option<String> {
        if !self.use_migemo {
            return None;
        }
        let dict = self.migemo_dict.as_ref()?;
        let pattern = migemo_query(query.to_string(), dict, &RegexOperator::Default);
        Some(if case_sensitive {
            pattern
        } else {
            format!("(?i){}", pattern)
        })
    }

    /// Start a new search with the given query
    pub fn start_search(&mut self, query: String) {
        self.query = query.clone();
        self.add_to_history(query);
        self.results.clear();
        self.current_index = None;
    }

    /// Find next matching index starting from the given index (inclusive)
    pub fn find_next_index(
        &self,
        entries: &[FileEntry],
        start_index: usize,
        query: &str,
    ) -> Option<usize> {
        if query.is_empty() || entries.is_empty() {
            return None;
        }

        for i in 0..entries.len() {
            let idx = (start_index + i) % entries.len();
            if self.matches_pattern(&entries[idx], query) {
                return Some(idx);
            }
        }
        None
    }

    /// Find previous matching index starting from the given index (inclusive)
    pub fn find_prev_index(
        &self,
        entries: &[FileEntry],
        start_index: usize,
        query: &str,
    ) -> Option<usize> {
        if query.is_empty() || entries.is_empty() {
            return None;
        }

        for i in 0..entries.len() {
            let idx = if start_index >= i {
                start_index - i
            } else {
                entries.len().saturating_sub(i.saturating_sub(start_index))
            };
            if idx < entries.len() && self.matches_pattern(&entries[idx], query) {
                return Some(idx);
            }
        }
        None
    }

    /// Select the next search result
    pub fn next_result(&mut self) {
        if self.results.is_empty() {
            return;
        }
        self.current_index = Some(match self.current_index {
            Some(idx) => (idx + 1) % self.results.len(),
            None => 0,
        });
    }

    /// Select the previous search result
    pub fn prev_result(&mut self) {
        if self.results.is_empty() {
            return;
        }
        self.current_index = Some(match self.current_index {
            Some(idx) => {
                if idx == 0 {
                    self.results.len() - 1
                } else {
                    idx - 1
                }
            }
            None => self.results.len() - 1,
        });
    }

    /// Clear current search state
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.current_index = None;
    }

    /// Add query to history
    pub fn add_to_history(&mut self, query: String) {
        if !query.is_empty() && !self.history.contains(&query) {
            self.history.push(query);
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
    }

    /// Parse query to extract include/exclude patterns
    pub fn parse_query(&mut self, query: &str) {
        if let Some(colon_pos) = query.find(':') {
            let include = query[..colon_pos].to_string();
            let exclude = query[colon_pos + 1..].to_string();
            self.include_pattern = if include.is_empty() {
                None
            } else {
                Some(include)
            };
            self.exclude_pattern = if exclude.is_empty() {
                None
            } else {
                Some(exclude)
            };
        } else {
            self.include_pattern = if query.is_empty() {
                None
            } else {
                Some(query.to_string())
            };
            self.exclude_pattern = None;
        }
    }

    /// Check if entry matches current query
    pub fn matches(&self, entry: &FileEntry) -> bool {
        if self.query.is_empty() {
            return true;
        }
        let (include, exclude) = if let Some(colon_pos) = self.query.find(':') {
            (
                Some(&self.query[..colon_pos]),
                Some(&self.query[colon_pos + 1..]),
            )
        } else {
            (Some(self.query.as_str()), None)
        };

        let include_match = include.is_none_or(|p| p.is_empty() || self.matches_pattern(entry, p));
        let exclude_match =
            exclude.is_some_and(|p| !p.is_empty() && self.matches_pattern(entry, p));
        include_match && !exclude_match
    }

    /// Check if entry matches a single pattern
    pub fn matches_pattern(&self, entry: &FileEntry, pattern: &str) -> bool {
        if let Some(stripped) = pattern.strip_prefix('/') {
            if let Some(end_pos) = stripped.rfind('/') {
                let regex_pattern = &stripped[..end_pos];
                let case_insensitive = pattern.ends_with("/i");
                return self.matches_regex(entry, regex_pattern, case_insensitive);
            }
        }

        if self.use_migemo && self.migemo_dict.is_some() {
            return self.matches_migemo(entry, pattern);
        }

        if self.use_regex {
            return self.matches_regex(entry, pattern, !self.case_sensitive);
        }

        self.matches_wildcard(entry, pattern)
    }

    /// Match using migemo (Japanese romaji search)
    fn matches_migemo(&self, entry: &FileEntry, pattern: &str) -> bool {
        let mut cache = self.migemo_cache.borrow_mut();

        // Cache key includes case sensitivity
        let cache_key = format!("{}:{}", self.case_sensitive, pattern);
        if let Some(re) = cache.get(&cache_key) {
            return re.is_match(&entry.name);
        }

        if let Some(ref dict) = self.migemo_dict {
            let t0 = Instant::now();
            let migemo_pattern = migemo_query(pattern.to_string(), dict, &RegexOperator::Default);
            // Add case-insensitive flag if needed
            let final_pattern = if self.case_sensitive {
                migemo_pattern
            } else {
                format!("(?i){}", migemo_pattern)
            };

            if let Ok(re) = Regex::new(&final_pattern) {
                let gen_elapsed = t0.elapsed();
                if gen_elapsed.as_millis() > 50 {
                    debug!(
                        "migemo regex generation for \"{}\" took {} ms",
                        pattern,
                        gen_elapsed.as_millis()
                    );
                }
                if cache.len() >= 100 {
                    if let Some(key) = cache.keys().next().cloned() {
                        cache.remove(&key);
                    }
                }
                let res = re.is_match(&entry.name);
                cache.insert(cache_key, re);
                return res;
            }
        }

        if self.case_sensitive {
            entry.name.contains(pattern)
        } else {
            entry.name.to_lowercase().contains(&pattern.to_lowercase())
        }
    }

    fn matches_regex(&self, entry: &FileEntry, pattern: &str, case_insensitive: bool) -> bool {
        let regex_str = if case_insensitive {
            format!("(?i){}", pattern)
        } else {
            pattern.to_string()
        };
        Regex::new(&regex_str).is_ok_and(|re| re.is_match(&entry.name))
    }

    fn matches_wildcard(&self, entry: &FileEntry, pattern: &str) -> bool {
        if pattern.contains('*') || pattern.contains('?') {
            let anchored = format!("^{}$", wildcard_to_regex(pattern));
            let re = if self.case_sensitive {
                Regex::new(&anchored)
            } else {
                Regex::new(&format!("(?i){}", anchored))
            };
            re.is_ok_and(|re| re.is_match(&entry.name))
        } else if self.case_sensitive {
            entry.name.contains(pattern)
        } else {
            entry.name.to_lowercase().contains(&pattern.to_lowercase())
        }
    }

    /// Filter entries against the current query and update results
    pub fn filter_entries(&mut self, entries: &[FileEntry]) {
        self.results = entries
            .iter()
            .filter(|e| self.matches(e))
            .cloned()
            .collect();
        if self.results.is_empty() {
            self.current_index = None;
        } else if self
            .current_index
            .is_none_or(|idx| idx >= self.results.len())
        {
            self.current_index = Some(0);
        }
    }

    /// Filter a list of path strings with AND-token matching, using migemo when enabled.
    pub fn filter_paths(&self, candidates: &[String], query: &str) -> Vec<String> {
        let tokens: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();
        if tokens.is_empty() {
            return candidates.to_vec();
        }
        candidates
            .iter()
            .filter(|path| {
                tokens
                    .iter()
                    .all(|token| self.path_matches_token(path, token))
            })
            .cloned()
            .collect()
    }

    fn path_matches_token(&self, path: &str, token: &str) -> bool {
        if self.use_migemo && self.migemo_dict.is_some() {
            let mut cache = self.migemo_cache.borrow_mut();
            let cache_key = format!("p:{}:{}", self.case_sensitive, token);
            if let Some(re) = cache.get(&cache_key) {
                return re.is_match(path);
            }
            if let Some(ref dict) = self.migemo_dict {
                let migemo_pattern = migemo_query(token.to_string(), dict, &RegexOperator::Default);
                let final_pattern = if self.case_sensitive {
                    migemo_pattern
                } else {
                    format!("(?i){}", migemo_pattern)
                };
                if let Ok(re) = Regex::new(&final_pattern) {
                    let result = re.is_match(path);
                    if cache.len() >= 100 {
                        if let Some(key) = cache.keys().next().cloned() {
                            cache.remove(&key);
                        }
                    }
                    cache.insert(cache_key, re);
                    return result;
                }
            }
        }
        if self.case_sensitive {
            path.contains(token)
        } else {
            path.to_lowercase().contains(&token.to_lowercase())
        }
    }

    /// Get the currently selected search result, if any
    pub fn current_result(&self) -> Option<&FileEntry> {
        self.current_index.and_then(|idx| self.results.get(idx))
    }

    /// Find byte ranges of query matches in a filename, for highlighting.
    /// Currently unimplemented (always returns empty) — no callers rely on it yet.
    pub fn find_match_ranges(&self, _filename: &str) -> Vec<(usize, usize)> {
        vec![]
    }
}

fn wildcard_to_regex(pattern: &str) -> String {
    let mut regex = String::from("");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }
    regex
}

#[cfg(test)]
mod migemo_dict_tests {
    use super::*;
    use std::fs;

    const TEXT_DICT: &str = ";; test dictionary\nにほん\t日本\t二本\nけんさく\t検索\n";

    fn write_normal(dir: &Path) -> PathBuf {
        let path = dir.join(NORMAL_DICT_NAME);
        fs::write(&path, TEXT_DICT).unwrap();
        path
    }

    fn write_compact(dir: &Path) -> PathBuf {
        let entries = parse_text_dict(TEXT_DICT.as_bytes()).unwrap();
        let path = dir.join(COMPACT_DICT_NAME);
        fs::write(&path, compact_dictionary_builder::build(entries)).unwrap();
        path
    }

    fn loaded_path(candidates: &[PathBuf]) -> Result<String, String> {
        let mut search = SearchModel::new();
        search.load_first_migemo_dict(candidates)?;
        Ok(search.migemo_dict_path().unwrap().to_string())
    }

    #[test]
    fn normal_dict_wins_over_compact() {
        let normal_dir = tempfile::tempdir().unwrap();
        let compact_dir = tempfile::tempdir().unwrap();
        let normal = write_normal(normal_dir.path());
        let compact = write_compact(compact_dir.path());

        let candidates = [normal.clone(), compact];
        assert_eq!(loaded_path(&candidates).unwrap(), normal.to_string_lossy());
    }

    #[test]
    fn compact_used_when_only_compact_exists() {
        let dir = tempfile::tempdir().unwrap();
        let compact = write_compact(dir.path());
        let candidates = [dir.path().join(NORMAL_DICT_NAME), compact.clone()];
        assert_eq!(loaded_path(&candidates).unwrap(), compact.to_string_lossy());
    }

    #[test]
    fn compact_used_when_normal_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(NORMAL_DICT_NAME), [0xff, 0xfe, b'x']).unwrap();
        let compact_dir = tempfile::tempdir().unwrap();
        let compact = write_compact(compact_dir.path());
        let candidates = [dir.path().join(NORMAL_DICT_NAME), compact.clone()];
        assert_eq!(loaded_path(&candidates).unwrap(), compact.to_string_lossy());
    }

    #[test]
    fn neither_dict_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let candidates = [
            dir.path().join(NORMAL_DICT_NAME),
            dir.path().join(COMPACT_DICT_NAME),
        ];
        assert!(loaded_path(&candidates).is_err());
    }

    #[test]
    fn both_formats_produce_working_queries() {
        for write in [write_normal, write_compact] {
            let dir = tempfile::tempdir().unwrap();
            let path = write(dir.path());
            let mut search = SearchModel::new();
            search.load_migemo_dict(&path).unwrap();
            search.use_migemo = true;
            let pattern = search.get_migemo_regex("nihon", true).unwrap();
            let re = Regex::new(&pattern).unwrap();
            assert!(re.is_match("日本"), "{path:?}: {pattern}");
        }
    }

    #[test]
    fn candidates_list_every_normal_path_before_any_compact_path() {
        let data = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let candidates = migemo_dict_candidates(None, None, Some(data.path()), Some(home.path()));
        let name = |p: &PathBuf| p.file_name().unwrap().to_string_lossy().to_string();
        let first_compact = candidates
            .iter()
            .position(|p| name(p) == COMPACT_DICT_NAME)
            .unwrap();
        assert!(first_compact > 0);
        assert!(candidates[..first_compact]
            .iter()
            .all(|p| name(p) == NORMAL_DICT_NAME));
        assert!(candidates[first_compact..]
            .iter()
            .all(|p| name(p) == COMPACT_DICT_NAME));
        #[cfg(target_os = "windows")]
        assert!(candidates.contains(
            &data
                .path()
                .join("rwf")
                .join("dict")
                .join("utf-8")
                .join(NORMAL_DICT_NAME)
        ));
    }

    #[test]
    fn explicit_path_comes_first_and_directory_prefers_normal() {
        let dir = tempfile::tempdir().unwrap();
        let candidates =
            migemo_dict_candidates(Some(dir.path().to_str().unwrap()), None, None, None);
        assert_eq!(candidates[0], dir.path().join(NORMAL_DICT_NAME));
        assert_eq!(
            candidates[1],
            dir.path().join("utf-8").join(NORMAL_DICT_NAME)
        );

        let file = dir.path().join("custom");
        let candidates = migemo_dict_candidates(file.to_str(), None, None, None);
        assert_eq!(candidates[0], file);
    }
}
