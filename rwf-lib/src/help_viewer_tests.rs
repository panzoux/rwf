//! Tests for Phase 6.7 Dynamic Help Viewer

use crate::help_content::{ActionDescriptions, build_help_entries};
use crate::input::KeyBindings;
use crate::model::dialog::{CustomFunction, HelpEntry, HelpTab};

// ── action_to_keys inversion ─────────────────────────────────────────────────

#[test]
fn test_action_to_keys_non_empty() {
    let kb = KeyBindings::embedded_defaults();
    let map = kb.normal_action_to_keys();
    assert!(!map.is_empty(), "normal_action_to_keys should not be empty");
}

#[test]
fn test_action_to_keys_sorted() {
    let kb = KeyBindings::embedded_defaults();
    let map = kb.normal_action_to_keys();
    for (action, keys) in &map {
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, &sorted, "Keys for action {} should be sorted", action);
    }
}

#[test]
fn test_action_to_keys_no_pending_sequence() {
    let kb = KeyBindings::embedded_defaults();
    let map = kb.normal_action_to_keys();
    // PendingSequence and CountDownJob should not appear as keys
    for (action, _) in &map {
        assert!(!action.contains("PendingSequence"), "PendingSequence should not appear in action name: {}", action);
    }
}

#[test]
fn test_viewer_action_to_keys_non_empty() {
    let kb = KeyBindings::embedded_defaults();
    let map = kb.viewer_action_to_keys();
    assert!(!map.is_empty(), "viewer_action_to_keys should not be empty");
}

// ── ActionDescriptions loading ────────────────────────────────────────────────

#[test]
fn test_action_descriptions_load_en() {
    let desc = ActionDescriptions::load("en");
    assert!(!desc.normal_mode.categories.is_empty(), "English normal_mode should have categories");
    assert!(!desc.viewer_mode.categories.is_empty(), "English viewer_mode should have categories");
}

#[test]
fn test_action_descriptions_load_jp() {
    let desc = ActionDescriptions::load("jp");
    assert!(!desc.normal_mode.categories.is_empty(), "Japanese normal_mode should have categories");
}

#[test]
fn test_action_descriptions_fallback_unknown_lang() {
    // Unknown language falls back to English
    let desc = ActionDescriptions::load("xx");
    assert!(!desc.normal_mode.categories.is_empty());
}

#[test]
fn test_available_languages_includes_en_and_jp() {
    let langs = ActionDescriptions::available_languages();
    assert!(langs.contains(&"en".to_string()), "English should always be available");
    assert!(langs.contains(&"jp".to_string()), "Japanese should always be available (embedded)");
}

#[test]
fn test_next_language_cycles() {
    // en → jp → en
    let next = ActionDescriptions::next_language("en");
    assert_eq!(next, "jp");
    let back = ActionDescriptions::next_language("jp");
    assert_eq!(back, "en");
}

// ── build_help_entries ────────────────────────────────────────────────────────

fn make_entries(show_unbound: bool) -> Vec<HelpEntry> {
    let kb = KeyBindings::embedded_defaults();
    let desc = ActionDescriptions::load("en");
    let custom: Vec<CustomFunction> = vec![];
    build_help_entries(&kb, &desc, &custom, show_unbound, &crate::config::AppConfig::default())
}

#[test]
fn test_build_help_entries_non_empty() {
    let entries = make_entries(true);
    assert!(!entries.is_empty(), "Help entries should not be empty");
}

#[test]
fn test_build_help_entries_has_normal_mode() {
    let entries = make_entries(true);
    let normal: Vec<_> = entries.iter().filter(|e| e.tab == HelpTab::NormalMode).collect();
    assert!(!normal.is_empty(), "Should have NormalMode entries");
}

#[test]
fn test_build_help_entries_has_viewer_mode() {
    let entries = make_entries(true);
    let viewer: Vec<_> = entries.iter().filter(|e| e.tab == HelpTab::ViewerMode).collect();
    assert!(!viewer.is_empty(), "Should have ViewerMode entries");
}

#[test]
fn test_show_unbound_false_hides_unbound() {
    let entries_with = make_entries(true);
    let entries_without = make_entries(false);
    // With show_unbound=false every entry must have at least one key
    for e in &entries_without {
        assert!(!e.keys.is_empty(), "Entry {:?} should have keys when show_unbound=false", e.action_name);
    }
    // With show_unbound=true there should be more (or equal) entries
    assert!(entries_with.len() >= entries_without.len());
}

#[test]
fn test_show_unbound_true_includes_unbound() {
    let entries = make_entries(true);
    let unbound: Vec<_> = entries.iter().filter(|e| e.keys.is_empty()).collect();
    // Not every action is bound by default so at least one should be unbound
    // (this could theoretically fail if everything is bound, but that's not the case)
    assert!(!unbound.is_empty(), "With show_unbound=true some entries should be unbound");
}

// ── AND search logic (mirrors the render-time filter) ───────────────────────

fn and_filter<'a>(entries: &'a [HelpEntry], query: &str) -> Vec<&'a HelpEntry> {
    let tokens: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    entries.iter().filter(|e| {
        let haystack = format!("{} {} {}", e.category, e.description, e.keys.join(" ")).to_lowercase();
        tokens.iter().all(|tok| haystack.contains(tok.as_str()))
    }).collect()
}

#[test]
fn test_and_search_single_token() {
    let entries = make_entries(true);
    let results = and_filter(&entries, "navi");
    for e in &results {
        assert!(
            e.category.to_lowercase().contains("navi")
                || e.description.to_lowercase().contains("navi")
                || e.keys.join(" ").to_lowercase().contains("navi"),
            "Entry should contain 'navi': {:?}", e
        );
    }
    assert!(!results.is_empty(), "Should find some entries for 'navi'");
}

#[test]
fn test_and_search_two_tokens_narrows_results() {
    let entries = make_entries(true);
    let one_token = and_filter(&entries, "navi");
    let two_tokens = and_filter(&entries, "navi cur");
    // More tokens = fewer or equal results
    assert!(two_tokens.len() <= one_token.len(), "AND should narrow results");
}

#[test]
fn test_and_search_no_match_returns_empty() {
    let entries = make_entries(true);
    let results = and_filter(&entries, "zzz_no_match_xyz");
    assert!(results.is_empty(), "Non-matching query should return empty");
}

#[test]
fn test_and_search_empty_query_returns_all() {
    let entries = make_entries(true);
    let results = and_filter(&entries, "");
    assert_eq!(results.len(), entries.len(), "Empty query should return all entries");
}

// ── Regex search ─────────────────────────────────────────────────────────────

fn regex_filter<'a>(entries: &'a [HelpEntry], pattern: &str) -> Vec<&'a HelpEntry> {
    match regex::Regex::new(&format!("(?i){}", pattern)) {
        Ok(re) => entries.iter().filter(|e| {
            let h = format!("{} {} {}", e.category, e.description, e.keys.join(" "));
            re.is_match(&h)
        }).collect(),
        Err(_) => entries.iter().collect(),
    }
}

#[test]
fn test_regex_search_basic() {
    let entries = make_entries(true);
    let results = regex_filter(&entries, "nav");
    assert!(!results.is_empty(), "Regex 'nav' should match some entries");
}

#[test]
fn test_regex_search_case_insensitive() {
    let entries = make_entries(true);
    let lower = regex_filter(&entries, "navi");
    let upper = regex_filter(&entries, "NAVI");
    assert_eq!(lower.len(), upper.len(), "Regex search should be case-insensitive");
}

#[test]
fn test_regex_invalid_returns_all() {
    let entries = make_entries(true);
    // Invalid regex should not crash and returns all entries (graceful fallback)
    let results = regex_filter(&entries, "[invalid");
    assert_eq!(results.len(), entries.len());
}

// ── CustomFunction optional fields ───────────────────────────────────────────

#[test]
fn test_custom_function_category_defaults() {
    let kb = KeyBindings::embedded_defaults();
    let desc = ActionDescriptions::load("en");
    let custom = vec![
        CustomFunction {
            name: "MyFunc".to_string(),
            command: Some("echo hi".to_string()),
            description: None,
            category: None,
            menu: None,
            shell: None,
            working_dir: None,
            pipe_to_action: None,
            os_specific: std::collections::HashMap::new(),
            key_binding: None,
        },
    ];
    let entries = build_help_entries(&kb, &desc, &custom, true, &crate::config::AppConfig::default());
    let cf: Vec<_> = entries.iter().filter(|e| e.tab == HelpTab::CustomFunctions).collect();
    // The function should appear with default category
    assert!(!cf.is_empty(), "Custom function entry should appear");
    let my_func = cf.iter().find(|e| e.action_name == "MyFunc");
    assert!(my_func.is_some(), "MyFunc should be in custom entries");
    if let Some(entry) = my_func {
        assert_eq!(entry.category, "Custom Functions", "Default category should be 'Custom Functions'");
    }
}

#[test]
fn test_custom_function_explicit_category() {
    let kb = KeyBindings::embedded_defaults();
    let desc = ActionDescriptions::load("en");
    let custom = vec![
        CustomFunction {
            name: "MyFunc".to_string(),
            command: Some("echo hi".to_string()),
            description: Some("My description".to_string()),
            category: Some("My Category".to_string()),
            menu: None,
            shell: None,
            working_dir: None,
            pipe_to_action: None,
            os_specific: std::collections::HashMap::new(),
            key_binding: None,
        },
    ];
    let entries = build_help_entries(&kb, &desc, &custom, true, &crate::config::AppConfig::default());
    let my_func = entries.iter().find(|e| e.action_name == "MyFunc");
    assert!(my_func.is_some());
    if let Some(entry) = my_func {
        assert_eq!(entry.category, "My Category");
        assert_eq!(entry.description, "My description");
    }
}

// ── HelpTab cycling ──────────────────────────────────────────────────────────

#[test]
fn test_help_tab_next_cycles() {
    // LeapMode was inserted between ViewerMode and DialogMode when Leap
    // Navigation default keybindings were wired up (see HelpTab in
    // model/dialog.rs), extending the tab cycle to 5 tabs.
    assert_eq!(HelpTab::NormalMode.next(), HelpTab::ViewerMode);
    assert_eq!(HelpTab::ViewerMode.next(), HelpTab::LeapMode);
    assert_eq!(HelpTab::LeapMode.next(), HelpTab::DialogMode);
    assert_eq!(HelpTab::DialogMode.next(), HelpTab::CustomFunctions);
    assert_eq!(HelpTab::CustomFunctions.next(), HelpTab::NormalMode);
}

#[test]
fn test_help_tab_prev_cycles() {
    assert_eq!(HelpTab::NormalMode.prev(), HelpTab::CustomFunctions);
    assert_eq!(HelpTab::CustomFunctions.prev(), HelpTab::DialogMode);
}

#[test]
fn test_help_tab_from_index() {
    assert_eq!(HelpTab::from_index(0), HelpTab::NormalMode);
    assert_eq!(HelpTab::from_index(1), HelpTab::ViewerMode);
    assert_eq!(HelpTab::from_index(2), HelpTab::LeapMode);
    assert_eq!(HelpTab::from_index(3), HelpTab::DialogMode);
    assert_eq!(HelpTab::from_index(4), HelpTab::CustomFunctions);
    assert_eq!(HelpTab::from_index(99), HelpTab::CustomFunctions);
}
