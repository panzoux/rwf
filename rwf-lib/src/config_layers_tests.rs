//! Tests for layered config (Phase 7.26). Every test loads from a temp directory
//! through `ConfigManager::with_paths`, never the live `%APPDATA%\rwf\`, except
//! `reload_resolves_the_same_lists_as_startup`, which compares two loads of
//! whatever the live config is.

use super::*;
use crate::config::ConfigLoadStatus;
use crate::input::Action;
use crate::model::dialog::MenuContent;
use tempfile::TempDir;

fn manager(dir: &TempDir) -> ConfigManager {
    ConfigManager::with_paths(
        dir.path().join("config.json"),
        dir.path().join("keybindings.json"),
    )
}

fn write(dir: &TempDir, name: &str, body: &str) {
    std::fs::write(dir.path().join(name), body).expect("write fixture");
}

fn load(dir: &TempDir) -> ListConfigs {
    load_list_configs(&manager(dir), ConfigLayering::Layered)
}

fn names(fns: &[CustomFunction]) -> Vec<String> {
    fns.iter().map(|f| f.name.clone()).collect()
}

fn json<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).expect("serializable")
}

fn function<'a>(fns: &'a [CustomFunction], name: &str) -> &'a CustomFunction {
    fns.iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no function {name:?} in {:?}", names(fns)))
}

/// Every file `--export-config-files` writes that this module layers.
fn write_full_export(dir: &TempDir) {
    write(
        dir,
        "keybindings.json",
        crate::help_content::DEFAULT_KEYBINDINGS,
    );
    write(
        dir,
        "custom_functions.json",
        crate::help_content::DEFAULT_CUSTOM_FUNCTIONS,
    );
    write(
        dir,
        "menu_clip.json",
        crate::help_content::DEFAULT_MENU_CLIP,
    );
    write(
        dir,
        "menu_config.json",
        crate::help_content::DEFAULT_MENU_CONFIG,
    );
    write(
        dir,
        "file_type_map.json",
        crate::help_content::DEFAULT_FILE_TYPE_MAP,
    );
    write(
        dir,
        "extension_associations.json",
        crate::help_content::DEFAULT_EXTENSION_ASSOCIATIONS,
    );
}

// ---------------------------------------------------------------------------
// Mode resolution
// ---------------------------------------------------------------------------

#[test]
fn layering_resolves_from_config_and_command_line() {
    use CliLayering::*;
    use ConfigLayering::*;
    assert_eq!(ConfigLayering::resolve(true, FromConfig), Layered);
    assert_eq!(ConfigLayering::resolve(false, FromConfig), UserReplaces);
    assert_eq!(ConfigLayering::resolve(true, NoDefaultConfig), UserReplaces);
    assert_eq!(ConfigLayering::resolve(true, NoUserConfig), BuiltInOnly);
    assert_eq!(ConfigLayering::resolve(false, NoUserConfig), BuiltInOnly);
}

#[test]
fn use_built_in_defaults_is_on_unless_config_json_turns_it_off() {
    assert!(AppConfig::default().use_built_in_defaults);
    let off: AppConfig = serde_json::from_str(r#"{"UseBuiltInDefaults": false}"#).expect("parses");
    assert!(!off.use_built_in_defaults);
}

// ---------------------------------------------------------------------------
// A missing user file gives the full defaults
// ---------------------------------------------------------------------------

/// The 2026-09-19 regression: with no custom_functions.json every built-in
/// function, and both built-in menus, must still be there.
#[test]
fn missing_user_files_give_the_full_built_in_defaults() {
    let dir = TempDir::new().expect("tempdir");
    let lists = load(&dir);

    assert_eq!(names(&lists.custom_functions), names(&built_in_functions()));
    for menu in ["clip menu", "config menu"] {
        let f = function(&lists.custom_functions, menu);
        assert!(
            !f.menu_items().is_empty(),
            "{menu} resolved to an empty menu with no user menu file"
        );
    }
    let ftm_defaults: Vec<FileTypeMapping> =
        serde_json::from_str(crate::help_content::DEFAULT_FILE_TYPE_MAP).expect("valid");
    assert_eq!(json(&lists.file_type_map), json(&ftm_defaults));
    let ext_defaults: Vec<ExtensionAssociation> =
        serde_json::from_str(crate::help_content::DEFAULT_EXTENSION_ASSOCIATIONS).expect("valid");
    assert_eq!(json(&lists.extension_associations), json(&ext_defaults));
    assert_eq!(
        lists.built_in_function_names.len(),
        lists.custom_functions.len()
    );
    // ext, file type map, custom functions: "built-in defaults", not "Skipped".
    for r in &lists.results[..3] {
        assert!(
            matches!(r.status, ConfigLoadStatus::Default(_)),
            "{:?} should report built-in defaults, got {:?}",
            r.path,
            r.status
        );
    }
}

#[test]
fn missing_keybindings_file_gives_the_built_in_bindings() {
    let dir = TempDir::new().expect("tempdir");
    let (kb, result) = load_keybindings(
        &dir.path().join("keybindings.json"),
        ConfigLayering::Layered,
    );
    assert_eq!(kb.normal_mode, KeyBindings::embedded_defaults().normal_mode);
    assert!(matches!(result.status, ConfigLoadStatus::Default(_)));
}

/// What made the regression visible: the shipped keybindings call functions by
/// name. Every one of those names must be a shipped function, or the key does
/// nothing on a fresh install.
#[test]
fn every_function_the_default_keybindings_invoke_is_built_in() {
    let kb = KeyBindings::embedded_defaults();
    let built_in = names(&built_in_functions());
    let mut missing = Vec::new();
    for map in [
        &kb.normal_mode,
        &kb.search_mode,
        &kb.dialog_mode,
        &kb.viewer_mode,
        &kb.leap_mode,
    ] {
        for (key, action) in map {
            if let Action::InvokeCustomFunction(name) = action {
                if !built_in.contains(name) {
                    missing.push(format!("{key} -> {name:?}"));
                }
            }
        }
    }
    missing.sort();
    assert!(
        missing.is_empty(),
        "default keybindings invoke functions that are not built in: {missing:?}"
    );
}

#[test]
fn every_menu_a_built_in_function_opens_is_built_in() {
    for f in built_in_functions() {
        if let Some(MenuContent::File(file)) = &f.menu {
            assert!(
                crate::model::dialog::built_in_menu(file).is_some(),
                "{:?} opens {file}, which is not a built-in menu",
                f.name
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Overriding one entry changes only that entry
// ---------------------------------------------------------------------------

#[test]
fn overriding_one_function_by_name_changes_only_that_entry() {
    let dir = TempDir::new().expect("tempdir");
    let baseline = load(&dir).custom_functions;
    write(
        &dir,
        "custom_functions.json",
        // Same Description as the built-in: menu items inherit the description of
        // the function they call, so dropping it would also change "clip menu".
        r#"{"Version":"1.0","Functions":[
            {"Name":"file name","ClipText":"$F!","Description":"the cursor entry's name"}]}"#,
    );
    let lists = load(&dir);

    assert_eq!(names(&lists.custom_functions), names(&baseline));
    for (got, want) in lists.custom_functions.iter().zip(&baseline) {
        if got.name == "file name" {
            assert_eq!(got.clip_text.as_deref(), Some("$F!"));
        } else {
            assert_eq!(json(got), json(want), "{:?} changed", got.name);
        }
    }
    assert!(!lists.built_in_function_names.contains("file name"));
    assert!(lists.built_in_function_names.contains("full path"));
    assert_eq!(
        lists.results[2].note.as_deref(),
        Some(format!("1 user entry over {} built-in", baseline.len()).as_str())
    );
}

#[test]
fn a_new_function_name_is_added_after_the_built_ins() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "custom_functions.json",
        r#"[{"Name":"my tool","Command":"tool $F"}]"#,
    );
    let lists = load(&dir);
    let built_in = built_in_functions().len();
    assert_eq!(lists.custom_functions.len(), built_in + 1);
    assert_eq!(lists.custom_functions[built_in].name, "my tool");
    let layers = ConfigLayers {
        layering: ConfigLayering::Layered,
        built_in_function_names: lists.built_in_function_names,
    };
    assert_eq!(layers.function_origin("my tool"), ConfigOrigin::User);
    assert_eq!(layers.function_origin("clip menu"), ConfigOrigin::BuiltIn);
}

#[test]
fn a_user_menu_file_replaces_the_built_in_menu_and_only_that_one() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "menu_clip.json",
        r#"{"Menus":[{"Name":"only","Action":"file name"}]}"#,
    );
    let lists = load(&dir);
    let clip = function(&lists.custom_functions, "clip menu").menu_items();
    assert_eq!(clip.len(), 1);
    assert_eq!(clip[0].name, "only");
    assert!(
        function(&lists.custom_functions, "config menu")
            .menu_items()
            .len()
            > 1
    );
}

#[test]
fn a_rebound_key_changes_only_that_key() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "keybindings.json",
        r#"{"NormalMode":{"F4":"file name"}}"#,
    );
    let (kb, result) = load_keybindings(
        &dir.path().join("keybindings.json"),
        ConfigLayering::Layered,
    );
    let mut want = KeyBindings::embedded_defaults().normal_mode;
    want.insert(
        "F4".to_string(),
        Action::InvokeCustomFunction("file name".to_string()),
    );
    assert_eq!(kb.normal_mode, want);
    assert!(result
        .note
        .as_deref()
        .is_some_and(|n| n.starts_with("1 user binding over")));
}

#[test]
fn a_file_type_map_entry_goes_first_and_hides_the_default_with_its_key() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "file_type_map.json",
        r#"[{"Extension":"PNG","FileType":"x/mine","Actions":["OsDefault"]}]"#,
    );
    let (ftm, _) = load_file_type_map(
        &dir.path().join("file_type_map.json"),
        ConfigLayering::Layered,
    );
    let defaults: Vec<FileTypeMapping> =
        serde_json::from_str(crate::help_content::DEFAULT_FILE_TYPE_MAP).expect("valid");
    assert_eq!(ftm.len(), defaults.len());
    assert_eq!(ftm[0].file_type.as_deref(), Some("x/mine"));
    assert_eq!(
        ftm.iter()
            .filter(|m| m.extension.eq_ignore_ascii_case("png"))
            .count(),
        1
    );
}

#[test]
fn associations_put_user_entries_first_and_hide_same_key_defaults() {
    let assoc = |ft: Option<&str>, ext: Option<&str>, cmd: &str| ExtensionAssociation {
        file_type: ft.map(str::to_string),
        extension: ext.map(str::to_string),
        command: cmd.to_string(),
        description: None,
        shell: None,
    };
    let defaults = vec![
        assoc(None, Some("pdf"), "default-pdf"),
        assoc(Some("image"), None, "default-image"),
        assoc(Some("image"), Some("png"), "default-png-image"),
        assoc(None, Some("log"), "default-log"),
    ];
    let user = vec![
        assoc(None, Some(".PDF"), "my-pdf"),
        assoc(None, Some("txt"), "my-txt"),
    ];
    let key =
        |a: &ExtensionAssociation| association_key(a.file_type.as_deref(), a.extension.as_deref());
    let (merged, summary) = merge_keyed(defaults, user, &[association_key(None, Some("log"))], key);
    let commands: Vec<&str> = merged.iter().map(|a| a.command.as_str()).collect();
    // The image entries have a different key from the pdf override, so they stay.
    assert_eq!(
        commands,
        ["my-pdf", "my-txt", "default-image", "default-png-image"]
    );
    assert_eq!(summary.overridden, 1);
    assert_eq!(summary.disabled, 1);
}

// ---------------------------------------------------------------------------
// Removing a default
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_function_is_absent() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "custom_functions.json",
        r#"{"Functions":[{"Name":"fzf jump to dir","Disabled":true}]}"#,
    );
    let lists = load(&dir);
    assert!(!names(&lists.custom_functions).contains(&"fzf jump to dir".to_string()));
    assert_eq!(lists.custom_functions.len(), built_in_functions().len() - 1);
    assert!(lists.results[2]
        .note
        .as_deref()
        .is_some_and(|n| n.ends_with("1 disabled")));
    assert!(matches!(lists.results[2].status, ConfigLoadStatus::Ok));
}

#[test]
fn a_disabled_association_needs_no_command_and_removes_its_default() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "file_type_map.json",
        r#"[{"Extension":"jpg","Disabled":true}]"#,
    );
    let (ftm, result) = load_file_type_map(
        &dir.path().join("file_type_map.json"),
        ConfigLayering::Layered,
    );
    assert!(
        matches!(result.status, ConfigLoadStatus::Ok),
        "{:?}",
        result.status
    );
    assert!(ftm.iter().all(|m| m.extension != "jpg"));
    assert!(ftm.iter().any(|m| m.extension == "jpeg"));
}

#[test]
fn null_or_none_unbinds_a_default_key() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "keybindings.json",
        r#"{"NormalMode":{"F4":null,"Shift+F4":"None"}}"#,
    );
    let (kb, result) = load_keybindings(
        &dir.path().join("keybindings.json"),
        ConfigLayering::Layered,
    );
    assert!(KeyBindings::embedded_defaults()
        .normal_mode
        .contains_key("F4"));
    assert!(!kb.normal_mode.contains_key("F4"));
    assert!(!kb.normal_mode.contains_key("Shift+F4"));
    assert!(kb.normal_mode.contains_key("F6"));
    assert!(result
        .note
        .as_deref()
        .is_some_and(|n| n.ends_with("2 unbound")));
}

#[test]
fn an_unbound_key_is_not_reported_as_a_broken_duplicate_check() {
    let warnings = crate::input::check_keybindings_content_duplicates(
        r#"{"NormalMode":{"F4":null,"F4":"file name"}}"#,
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("'null' overridden by 'file name'"));
}

// ---------------------------------------------------------------------------
// A fully exported file behaves the same as having no file
// ---------------------------------------------------------------------------

#[test]
fn a_fully_exported_config_behaves_like_no_config() {
    let empty = TempDir::new().expect("tempdir");
    let exported = TempDir::new().expect("tempdir");
    write_full_export(&exported);

    let a = load(&empty);
    let b = load(&exported);
    assert_eq!(json(&a.custom_functions), json(&b.custom_functions));
    assert_eq!(json(&a.file_type_map), json(&b.file_type_map));
    assert_eq!(
        json(&a.extension_associations),
        json(&b.extension_associations)
    );
    // Untouched copies still read as built-in in the function list.
    assert_eq!(a.built_in_function_names, b.built_in_function_names);
    for r in &b.results {
        assert!(
            !matches!(r.status, ConfigLoadStatus::Error(_)),
            "{:?}: {:?}",
            r.path,
            r.status
        );
    }

    let (kb_a, _) = load_keybindings(
        &empty.path().join("keybindings.json"),
        ConfigLayering::Layered,
    );
    let (kb_b, _) = load_keybindings(
        &exported.path().join("keybindings.json"),
        ConfigLayering::Layered,
    );
    assert_eq!(kb_a.normal_mode, kb_b.normal_mode);
    assert_eq!(kb_a.search_mode, kb_b.search_mode);
    assert_eq!(kb_a.dialog_mode, kb_b.dialog_mode);
    assert_eq!(kb_a.viewer_mode, kb_b.viewer_mode);
    assert_eq!(kb_a.leap_mode, kb_b.leap_mode);
}

#[test]
fn deleting_an_entry_from_an_export_keeps_the_built_in() {
    let dir = TempDir::new().expect("tempdir");
    write_full_export(&dir);
    // The exported custom_functions.json minus "fzf jump to dir".
    let mut v: serde_json::Value =
        serde_json::from_str(crate::help_content::DEFAULT_CUSTOM_FUNCTIONS).expect("valid");
    v["Functions"]
        .as_array_mut()
        .expect("array")
        .retain(|f| f["Name"] != "fzf jump to dir");
    write(&dir, "custom_functions.json", &v.to_string());

    let lists = load(&dir);
    assert!(names(&lists.custom_functions).contains(&"fzf jump to dir".to_string()));
}

// ---------------------------------------------------------------------------
// The other modes
// ---------------------------------------------------------------------------

#[test]
fn replace_mode_uses_an_existing_user_file_as_is_and_built_ins_for_missing_ones() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "custom_functions.json",
        r#"[{"Name":"my tool","Command":"tool $F"}]"#,
    );
    write(
        &dir,
        "keybindings.json",
        r#"{"NormalMode":{"F4":"my tool"}}"#,
    );
    let lists = load_list_configs(&manager(&dir), ConfigLayering::UserReplaces);
    assert_eq!(names(&lists.custom_functions), ["my tool"]);
    assert!(lists.built_in_function_names.is_empty());
    // No user file_type_map.json: still the built-ins.
    assert!(!lists.file_type_map.is_empty());

    let (kb, _) = load_keybindings(
        &dir.path().join("keybindings.json"),
        ConfigLayering::UserReplaces,
    );
    assert_eq!(kb.binding_count(), 1);
}

#[test]
fn built_in_only_mode_ignores_every_user_file() {
    let dir = TempDir::new().expect("tempdir");
    write(
        &dir,
        "custom_functions.json",
        r#"{"Functions":[{"Name":"clip menu","Disabled":true}]}"#,
    );
    write(&dir, "menu_config.json", r#"{"Menus":[]}"#);
    write(&dir, "file_type_map.json", "not json");
    write(&dir, "keybindings.json", r#"{"NormalMode":{"F6":null}}"#);

    let lists = load_list_configs(&manager(&dir), ConfigLayering::BuiltInOnly);
    assert_eq!(names(&lists.custom_functions), names(&built_in_functions()));
    assert!(
        function(&lists.custom_functions, "config menu")
            .menu_items()
            .len()
            > 1
    );
    assert!(lists
        .results
        .iter()
        .all(|r| !matches!(r.status, ConfigLoadStatus::Error(_))));

    let (kb, _) = load_keybindings(
        &dir.path().join("keybindings.json"),
        ConfigLayering::BuiltInOnly,
    );
    assert!(kb.normal_mode.contains_key("F6"));
}

#[test]
fn an_unparseable_user_file_keeps_the_built_ins_and_reports_the_error() {
    let dir = TempDir::new().expect("tempdir");
    write(&dir, "custom_functions.json", "{ not json");
    let lists = load(&dir);
    assert_eq!(names(&lists.custom_functions), names(&built_in_functions()));
    assert!(matches!(
        lists.results[2].status,
        ConfigLoadStatus::Error(_)
    ));
}

// ---------------------------------------------------------------------------
// Reload behaves the same as startup
// ---------------------------------------------------------------------------

#[test]
fn reload_resolves_the_same_lists_as_startup() {
    let mut state = crate::AppState::new(AppConfig::default());
    let startup_fns = json(&state.custom_functions);
    let startup_ftm = json(&state.file_type_map);
    let startup_ext = json(&state.extension_associations);
    let startup_names = state.config_layers.built_in_function_names.clone();
    let startup_results: Vec<String> = state
        .config_load_results
        .iter()
        .map(|r| format!("{:?} {:?} {:?}", r.path, r.status, r.note))
        .collect();

    crate::state::update_state(&mut state, crate::state::Transition::ReloadConfig);

    assert_eq!(json(&state.custom_functions), startup_fns);
    assert_eq!(json(&state.file_type_map), startup_ftm);
    assert_eq!(json(&state.extension_associations), startup_ext);
    assert_eq!(state.config_layers.built_in_function_names, startup_names);
    // Reload adds config.json in front; the list-type rows must match exactly.
    let reload_results: Vec<String> = state
        .config_load_results
        .iter()
        .filter(|r| r.path.file_name().is_some_and(|n| n != "config.json"))
        .map(|r| format!("{:?} {:?} {:?}", r.path, r.status, r.note))
        .collect();
    assert_eq!(reload_results, startup_results);
}
