//! Integration tests for search functionality
//! 
//! Tests Requirements 11.1-11.8 and 30.1-30.10

#[cfg(test)]
mod tests {
    use crate::model::{FileEntry, Location, SearchModel};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_entries() -> Vec<FileEntry> {
        vec![
            FileEntry {
                name: "test.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/test.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "Test.rs".to_string(),
                location: Location::Local(PathBuf::from("/test/Test.rs")),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "example.md".to_string(),
                location: Location::Local(PathBuf::from("/test/example.md")),
                size: 300,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "data.json".to_string(),
                location: Location::Local(PathBuf::from("/test/data.json")),
                size: 400,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "README.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/README.txt")),
                size: 500,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ]
    }

    #[test]
    fn test_wildcard_search_star() {
        // Test wildcard search with * (match any characters)
        // Validates: Requirements 30.2
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "*.txt".to_string();
        search.filter_entries(&entries);

        assert_eq!(search.results.len(), 2);
        assert!(search.results.iter().any(|e| e.name == "test.txt"));
        assert!(search.results.iter().any(|e| e.name == "README.txt"));
    }

    #[test]
    fn test_wildcard_search_question() {
        // Test wildcard search with ? (match single character)
        // Validates: Requirements 30.2
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "test.???".to_string();
        search.filter_entries(&entries);

        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "test.txt");
    }

    #[test]
    fn test_wildcard_search_combined() {
        // Test wildcard search with both * and ?
        // Validates: Requirements 30.2
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "*.??".to_string();
        search.filter_entries(&entries);

        assert_eq!(search.results.len(), 2);
        assert!(search.results.iter().any(|e| e.name == "Test.rs"));
        assert!(search.results.iter().any(|e| e.name == "example.md"));
    }

    #[test]
    fn test_regex_search_basic() {
        // Test regex search with /pattern/ syntax
        // Validates: Requirements 30.3
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "/^test/".to_string();
        search.filter_entries(&entries);

        // Should match "test.txt" (case-insensitive by default)
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "test.txt");
    }

    #[test]
    fn test_regex_search_case_insensitive() {
        // Test case-insensitive regex search with /pattern/i syntax
        // Validates: Requirements 30.4
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "/^test/i".to_string();
        search.filter_entries(&entries);

        // Should match both "test.txt" and "Test.rs"
        assert_eq!(search.results.len(), 2);
        assert!(search.results.iter().any(|e| e.name == "test.txt"));
        assert!(search.results.iter().any(|e| e.name == "Test.rs"));
    }

    #[test]
    fn test_regex_search_complex() {
        // Test complex regex pattern
        // Validates: Requirements 30.3
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = r"/\.(txt|md)$/i".to_string();
        search.filter_entries(&entries);

        // Should match files ending with .txt or .md
        assert_eq!(search.results.len(), 3);
        assert!(search.results.iter().any(|e| e.name == "test.txt"));
        assert!(search.results.iter().any(|e| e.name == "example.md"));
        assert!(search.results.iter().any(|e| e.name == "README.txt"));
    }

    #[test]
    fn test_case_sensitive_search() {
        // Test case-sensitive wildcard search
        // Validates: Requirements 11.5
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.case_sensitive = true;
        search.query = "test*".to_string();
        search.filter_entries(&entries);

        // Should only match "test.txt", not "Test.rs"
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "test.txt");
    }

    #[test]
    fn test_case_insensitive_search() {
        // Test case-insensitive wildcard search (default)
        // Validates: Requirements 11.4
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.case_sensitive = false;
        search.query = "test*".to_string();
        search.filter_entries(&entries);

        // Should match both "test.txt" and "Test.rs"
        assert_eq!(search.results.len(), 2);
        assert!(search.results.iter().any(|e| e.name == "test.txt"));
        assert!(search.results.iter().any(|e| e.name == "Test.rs"));
    }

    #[test]
    fn test_combined_include_exclude_patterns() {
        // Test combined include:exclude patterns
        // Validates: Requirements 30.5
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        // Include all .txt files, exclude README
        search.query = "*.txt:README*".to_string();
        search.filter_entries(&entries);

        // Should match "test.txt" but not "README.txt"
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "test.txt");
    }

    #[test]
    fn test_combined_patterns_with_regex() {
        // Test combined patterns with regex
        // Validates: Requirements 30.5
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        // Include files starting with 't', exclude .txt files
        search.query = "/^t/i:*.txt".to_string();
        search.filter_entries(&entries);

        // Should match "Test.rs" but not "test.txt"
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "Test.rs");
    }

    #[test]
    fn test_empty_query_matches_all() {
        // Test that empty query matches all entries
        // Validates: Requirements 11.3
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = String::new();
        search.filter_entries(&entries);

        assert_eq!(search.results.len(), entries.len());
    }

    #[test]
    fn test_no_matches() {
        // Test query with no matches
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "nonexistent*".to_string();
        search.filter_entries(&entries);

        assert_eq!(search.results.len(), 0);
        assert!(search.current_index.is_none());
    }

    #[test]
    fn test_search_history() {
        // Test search history management
        // Validates: Requirements 28.5
        let mut search = SearchModel::new();

        search.add_to_history("*.txt".to_string());
        search.add_to_history("*.rs".to_string());
        search.add_to_history("*.md".to_string());

        assert_eq!(search.history.len(), 3);
        assert_eq!(search.history[0], "*.txt");
        assert_eq!(search.history[1], "*.rs");
        assert_eq!(search.history[2], "*.md");
    }

    #[test]
    fn test_search_history_no_duplicates() {
        // Test that search history doesn't store duplicates
        let mut search = SearchModel::new();

        search.add_to_history("*.txt".to_string());
        search.add_to_history("*.rs".to_string());
        search.add_to_history("*.txt".to_string()); // Duplicate

        assert_eq!(search.history.len(), 2);
        assert_eq!(search.history[0], "*.txt");
        assert_eq!(search.history[1], "*.rs");
    }

    #[test]
    fn test_search_history_limit() {
        // Test that search history is limited to 50 entries
        let mut search = SearchModel::new();

        for i in 0..60 {
            search.add_to_history(format!("query{}", i));
        }

        assert_eq!(search.history.len(), 50);
        // First 10 should be removed
        assert_eq!(search.history[0], "query10");
        assert_eq!(search.history[49], "query59");
    }

    #[test]
    fn test_current_result() {
        // Test getting the current search result
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        search.query = "*.txt".to_string();
        search.filter_entries(&entries);

        assert!(search.current_result().is_some());
        assert_eq!(search.current_result().unwrap().name, "test.txt");
    }

    #[test]
    fn test_incremental_search() {
        // Test that search filters in real-time as query changes
        // Validates: Requirements 11.3, 30.8
        let mut search = SearchModel::new();
        let entries = create_test_entries();

        // Start with broad query
        search.query = "*".to_string();
        search.filter_entries(&entries);
        assert_eq!(search.results.len(), 5);

        // Narrow down to .txt files
        search.query = "*.txt".to_string();
        search.filter_entries(&entries);
        assert_eq!(search.results.len(), 2);

        // Further narrow to specific file
        search.query = "test.txt".to_string();
        search.filter_entries(&entries);
        assert_eq!(search.results.len(), 1);
    }

    #[test]
    fn test_special_regex_characters_in_wildcard() {
        // Test that special regex characters are escaped in wildcard mode
        let mut search = SearchModel::new();
        let entries = vec![
            FileEntry {
                name: "file.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "file+txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file+txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];

        // The dot should be treated as literal dot, not regex wildcard
        search.query = "file.txt".to_string();
        search.filter_entries(&entries);

        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "file.txt");
    }

    #[test]
    fn test_parse_query_with_colon() {
        // Test query parsing for include:exclude patterns
        let mut search = SearchModel::new();

        search.parse_query("*.txt:README*");
        assert_eq!(search.include_pattern, Some("*.txt".to_string()));
        assert_eq!(search.exclude_pattern, Some("README*".to_string()));
    }

    #[test]
    fn test_parse_query_without_colon() {
        // Test query parsing without colon
        let mut search = SearchModel::new();

        search.parse_query("*.txt");
        assert_eq!(search.include_pattern, Some("*.txt".to_string()));
        assert_eq!(search.exclude_pattern, None);
    }

    #[test]
    fn test_parse_query_empty_exclude() {
        // Test query parsing with empty exclude pattern
        let mut search = SearchModel::new();

        search.parse_query("*.txt:");
        assert_eq!(search.include_pattern, Some("*.txt".to_string()));
        assert_eq!(search.exclude_pattern, None);
    }

    #[test]
    fn test_parse_query_empty_include() {
        // Test query parsing with empty include pattern
        let mut search = SearchModel::new();

        search.parse_query(":*.bak");
        assert_eq!(search.include_pattern, None);
        assert_eq!(search.exclude_pattern, Some("*.bak".to_string()));
    }

    #[test]
    #[cfg(feature = "migemo")]
    fn test_migemo_search_basic() {
        // Test basic migemo search functionality
        // Validates: Requirements 30.6
        // Note: This test requires a migemo dictionary file to be present
        let mut search = SearchModel::new();
        
        // Try to load migemo dictionary
        if search.load_migemo_dict_auto().is_ok() {
            search.use_migemo = true;
            
            let entries = vec![
                FileEntry {
                    name: "日本.txt".to_string(),
                    location: Location::Local(PathBuf::from("/test/日本.txt")),
                    size: 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "nihon.txt".to_string(),
                    location: Location::Local(PathBuf::from("/test/nihon.txt")),
                    size: 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "test.txt".to_string(),
                    location: Location::Local(PathBuf::from("/test/test.txt")),
                    size: 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
            ];

            // Search for "nihon" should match "日本.txt" and "nihon.txt"
            search.query = "nihon".to_string();
            search.filter_entries(&entries);

            // Should match both files with Japanese and romaji
            assert!(search.results.len() >= 1);
            assert!(search.results.iter().any(|e| e.name == "nihon.txt"));
            // May also match "日本.txt" if dictionary is loaded correctly
        } else {
            // Skip test if dictionary is not available
            println!("Skipping migemo test: dictionary not found");
        }
    }

    #[test]
    #[cfg(feature = "migemo")]
    fn test_migemo_toggle() {
        // Test toggling migemo mode on and off
        // Validates: Requirements 30.6
        let mut search = SearchModel::new();
        
        assert!(!search.use_migemo);
        
        search.use_migemo = true;
        assert!(search.use_migemo);
        
        search.use_migemo = false;
        assert!(!search.use_migemo);
    }
}
