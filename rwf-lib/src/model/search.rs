//! Search model

use super::FileEntry;
use regex::Regex;

#[cfg(feature = "migemo")]
use rustmigemo::migemo::{compact_dictionary::CompactDictionary, query::query as migemo_query, regex_generator::RegexOperator};

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
    #[cfg(feature = "migemo")]
    migemo_dict: Option<CompactDictionary>,
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
            #[cfg(feature = "migemo")]
            migemo_dict: None,
        }
    }
    
    /// Load migemo dictionary from file
    #[cfg(feature = "migemo")]
    pub fn load_migemo_dict(&mut self, dict_path: &std::path::Path) -> Result<(), String> {
        use std::fs;
        
        let dict_data = fs::read(dict_path)
            .map_err(|e| format!("Failed to read migemo dictionary: {}", e))?;
        
        self.migemo_dict = Some(CompactDictionary::new(&dict_data));
        Ok(())
    }
    
    /// Load migemo dictionary from common paths
    #[cfg(feature = "migemo")]
    pub fn load_migemo_dict_auto(&mut self) -> Result<(), String> {
        use std::path::PathBuf;
        
        // Try common dictionary paths
        let possible_paths = vec![
            PathBuf::from("migemo-compact-dict"),
            PathBuf::from("dict/migemo-compact-dict"),
            PathBuf::from("/usr/share/migemo/utf-8/migemo-dict"),
            PathBuf::from("/usr/local/share/migemo/utf-8/migemo-dict"),
            PathBuf::from("/opt/homebrew/share/migemo/utf-8/migemo-dict"),
        ];
        
        // Also check user directories
        if let Some(home) = dirs::home_dir() {
            let user_paths = vec![
                home.join(".migemo/migemo-compact-dict"),
                home.join(".config/migemo/migemo-compact-dict"),
            ];
            
            for path in user_paths.iter().chain(possible_paths.iter()) {
                if path.exists() {
                    return self.load_migemo_dict(path);
                }
            }
        } else {
            for path in &possible_paths {
                if path.exists() {
                    return self.load_migemo_dict(path);
                }
            }
        }
        
        Err("Migemo dictionary not found in common paths".to_string())
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
    /// Format: include:exclude
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
        
        // Parse patterns from query
        let (include_pattern, exclude_pattern) = if let Some(colon_pos) = self.query.find(':') {
            let include = &self.query[..colon_pos];
            let exclude = &self.query[colon_pos + 1..];
            (Some(include), if exclude.is_empty() { None } else { Some(exclude) })
        } else {
            (Some(self.query.as_str()), None)
        };
        
        // Check include pattern
        let include_match = if let Some(pattern) = include_pattern {
            if pattern.is_empty() {
                true
            } else {
                self.matches_pattern(entry, pattern)
            }
        } else {
            true
        };
        
        // Check exclude pattern
        let exclude_match = if let Some(pattern) = exclude_pattern {
            if pattern.is_empty() {
                false
            } else {
                self.matches_pattern(entry, pattern)
            }
        } else {
            false
        };
        
        include_match && !exclude_match
    }
    
    /// Check if entry matches a single pattern
    fn matches_pattern(&self, entry: &FileEntry, pattern: &str) -> bool {
        // Check for regex pattern (/pattern/ or /pattern/i)
        if let Some(stripped) = pattern.strip_prefix('/') {
            if let Some(end_pos) = stripped.rfind('/') {
                let regex_pattern = &stripped[..end_pos];
                let case_insensitive = pattern.ends_with("/i");
                
                return self.matches_regex(entry, regex_pattern, case_insensitive);
            }
        }
        
        // Use migemo if enabled and dictionary is loaded
        #[cfg(feature = "migemo")]
        if self.use_migemo {
            if let Some(ref dict) = self.migemo_dict {
                return self.matches_migemo(entry, pattern, dict);
            }
        }
        
        // Use configured regex mode
        if self.use_regex {
            return self.matches_regex(entry, pattern, !self.case_sensitive);
        }
        
        // Wildcard matching
        self.matches_wildcard(entry, pattern)
    }
    
    /// Match using migemo (Japanese romaji search)
    #[cfg(feature = "migemo")]
    fn matches_migemo(&self, entry: &FileEntry, pattern: &str, dict: &CompactDictionary) -> bool {
        // Generate regex pattern from romaji input
        let regex_operator = RegexOperator::Default;
        let migemo_pattern = migemo_query(pattern.to_string(), dict, &regex_operator);
        
        // Apply the generated pattern
        if let Ok(re) = Regex::new(&migemo_pattern) {
            re.is_match(&entry.name)
        } else {
            // Fallback to simple substring match if regex generation fails
            if self.case_sensitive {
                entry.name.contains(pattern)
            } else {
                entry.name.to_lowercase().contains(&pattern.to_lowercase())
            }
        }
    }
    
    /// Match using regex
    fn matches_regex(&self, entry: &FileEntry, pattern: &str, case_insensitive: bool) -> bool {
        let regex_str = if case_insensitive {
            format!("(?i){}", pattern)
        } else {
            pattern.to_string()
        };
        
        if let Ok(re) = Regex::new(&regex_str) {
            re.is_match(&entry.name)
        } else {
            false
        }
    }
    
    /// Match using wildcards
    fn matches_wildcard(&self, entry: &FileEntry, pattern: &str) -> bool {
        let regex_pattern = if self.case_sensitive {
            wildcard_to_regex(pattern)
        } else {
            wildcard_to_regex(&pattern.to_lowercase())
        };
        
        if let Ok(re) = Regex::new(&regex_pattern) {
            if self.case_sensitive {
                re.is_match(&entry.name)
            } else {
                re.is_match(&entry.name.to_lowercase())
            }
        } else {
            false
        }
    }
    
    /// Filter entries based on current query
    pub fn filter_entries(&mut self, entries: &[FileEntry]) {
        self.results = entries
            .iter()
            .filter(|e| self.matches(e))
            .cloned()
            .collect();
        
        // Reset current index if results changed
        if self.results.is_empty() {
            self.current_index = None;
        } else if self.current_index.is_none() || self.current_index.unwrap() >= self.results.len() {
            self.current_index = Some(0);
        }
    }
    
    /// Get the currently selected search result
    pub fn current_result(&self) -> Option<&FileEntry> {
        self.current_index.and_then(|idx| self.results.get(idx))
    }
    
    /// Find matching portions of filename for highlighting
    pub fn find_match_ranges(&self, _filename: &str) -> Vec<(usize, usize)> {
        if self.query.is_empty() {
            return vec![];
        }
        
        // For now, return empty - highlighting will be implemented in UI layer
        // This is a placeholder for the highlighting logic
        vec![]
    }
}

fn wildcard_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => {
                // For case-insensitive matching, we'll handle it in the regex flags
                regex.push(ch);
            }
        }
    }
    regex.push('$');
    regex
}
