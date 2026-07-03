//! Search model

use super::FileEntry;
use regex::Regex;
use std::collections::HashMap;
use std::cell::RefCell;
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::query::query as migemo_query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use std::time::Instant;
use tracing::debug;

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
    
    /// Load migemo dictionary from file
    pub fn load_migemo_dict(&mut self, dict_path: &std::path::Path) -> Result<(), String> {
        use std::fs;
        
        let dict_data = fs::read(dict_path)
            .map_err(|e| format!("Failed to read migemo dictionary from {:?}: {}", dict_path, e))?;
        
        self.migemo_dict = Some(CompactDictionary::new(&dict_data));
        self.migemo_dict_path = Some(dict_path.to_string_lossy().to_string());
        // Clear cache when dictionary changes
        self.migemo_cache.borrow_mut().clear();
        Ok(())
    }
    
    /// Load migemo dictionary from common paths
    pub fn load_migemo_dict_auto(&mut self, config_path: Option<&str>) -> Result<(), String> {
        use std::path::PathBuf;
        
        let mut possible_paths = Vec::new();

        // 1. Config path
        if let Some(path) = config_path {
            possible_paths.push(PathBuf::from(path));
        }

        // 2. Environment variable
        if let Ok(path) = std::env::var("RWF_MIGEMO_DICT") {
            possible_paths.push(PathBuf::from(path));
        }

        // 3. Local paths
        possible_paths.push(PathBuf::from("dict/migemo-compact-dict"));
        possible_paths.push(PathBuf::from("migemo-compact-dict"));
        possible_paths.push(PathBuf::from("dict/utf-8/migemo-compact-dict"));

        // 4. Windows AppData
        #[cfg(target_os = "windows")]
        if let Some(data_dir) = dirs::data_dir() {
            possible_paths.push(data_dir.join("rwf").join("dict").join("migemo-compact-dict"));
            possible_paths.push(data_dir.join("rwf").join("dict").join("utf-8").join("migemo-compact-dict"));
        }

        // 5. Unix Home (inc. macOS)
        if let Some(home) = dirs::home_dir() {
            possible_paths.push(home.join(".migemo").join("migemo-compact-dict"));
            possible_paths.push(home.join(".migemo").join("utf-8").join("migemo-compact-dict"));
            possible_paths.push(home.join(".config").join("migemo").join("migemo-compact-dict"));
            possible_paths.push(home.join(".config").join("migemo").join("utf-8").join("migemo-compact-dict"));
        }

        // 6. Linux/Unix System paths
        possible_paths.push(PathBuf::from("/usr/share/migemo/utf-8/migemo-dict"));
        possible_paths.push(PathBuf::from("/usr/local/share/migemo/utf-8/migemo-dict"));
        possible_paths.push(PathBuf::from("/opt/homebrew/share/migemo/utf-8/migemo-dict"));

        for path in possible_paths {
            let exists = path.exists() && path.is_file();
            tracing::info!("Migemo: checking {:?} — {}", path, if exists { "found" } else { "not found" });
            if exists {
                match self.load_migemo_dict(&path) {
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
        if !self.use_migemo { return None; }
        let dict = self.migemo_dict.as_ref()?;
        let pattern = migemo_query(query.to_string(), dict, &RegexOperator::Default);
        Some(if case_sensitive { pattern } else { format!("(?i){}", pattern) })
    }
    
    /// Start a new search with the given query
    pub fn start_search(&mut self, query: String) {
        self.query = query.clone();
        self.add_to_history(query);
        self.results.clear();
        self.current_index = None;
    }

    /// Find next matching index starting from the given index (inclusive)
    pub fn find_next_index(&self, entries: &[FileEntry], start_index: usize, query: &str) -> Option<usize> {
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
    pub fn find_prev_index(&self, entries: &[FileEntry], start_index: usize, query: &str) -> Option<usize> {
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
            Some(idx) => if idx == 0 { self.results.len() - 1 } else { idx - 1 },
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
            self.include_pattern = if include.is_empty() { None } else { Some(include) };
            self.exclude_pattern = if exclude.is_empty() { None } else { Some(exclude) };
        } else {
            self.include_pattern = if query.is_empty() { None } else { Some(query.to_string()) };
            self.exclude_pattern = None;
        }
    }
    
    /// Check if entry matches current query
    pub fn matches(&self, entry: &FileEntry) -> bool {
        if self.query.is_empty() {
            return true;
        }
        let (include, exclude) = if let Some(colon_pos) = self.query.find(':') {
            (Some(&self.query[..colon_pos]), Some(&self.query[colon_pos + 1..]))
        } else {
            (Some(self.query.as_str()), None)
        };
        
        let include_match = include.is_none_or(|p| p.is_empty() || self.matches_pattern(entry, p));
        let exclude_match = exclude.is_some_and(|p| !p.is_empty() && self.matches_pattern(entry, p));
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
                    debug!("migemo regex generation for \"{}\" took {} ms", pattern, gen_elapsed.as_millis());
                }
                if cache.len() >= 100 {
                    if let Some(key) = cache.keys().next().cloned() { cache.remove(&key); }
                }
                let res = re.is_match(&entry.name);
                cache.insert(cache_key, re);
                return res;
            }
        }

        if self.case_sensitive { entry.name.contains(pattern) } 
        else { entry.name.to_lowercase().contains(&pattern.to_lowercase()) }
    }
    
    fn matches_regex(&self, entry: &FileEntry, pattern: &str, case_insensitive: bool) -> bool {
        let regex_str = if case_insensitive { format!("(?i){}", pattern) } else { pattern.to_string() };
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
    
    pub fn filter_entries(&mut self, entries: &[FileEntry]) {
        self.results = entries.iter().filter(|e| self.matches(e)).cloned().collect();
        if self.results.is_empty() { self.current_index = None; }
        else if self.current_index.is_none_or(|idx| idx >= self.results.len()) { self.current_index = Some(0); }
    }

    /// Filter a list of path strings with AND-token matching, using migemo when enabled.
    pub fn filter_paths(&self, candidates: &[String], query: &str) -> Vec<String> {
        let tokens: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();
        if tokens.is_empty() {
            return candidates.to_vec();
        }
        candidates.iter()
            .filter(|path| tokens.iter().all(|token| self.path_matches_token(path, token)))
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
                        if let Some(key) = cache.keys().next().cloned() { cache.remove(&key); }
                    }
                    cache.insert(cache_key, re);
                    return result;
                }
            }
        }
        if self.case_sensitive { path.contains(token) }
        else { path.to_lowercase().contains(&token.to_lowercase()) }
    }
    
    pub fn current_result(&self) -> Option<&FileEntry> {
        self.current_index.and_then(|idx| self.results.get(idx))
    }
    
    pub fn find_match_ranges(&self, _filename: &str) -> Vec<(usize, usize)> { vec![] }
}

fn wildcard_to_regex(pattern: &str) -> String {
    let mut regex = String::from("");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\'); regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }
    regex
}
