//! Layered config (Phase 7.26): the built-in defaults are always loaded, and the
//! user's files only override, add to, or disable them.
//!
//! | File | Merge key | A user entry … |
//! |---|---|---|
//! | keybindings.json | key, per mode | rebinds it; `null` / `"None"` unbinds it |
//! | custom_functions.json | `Name` | same name replaces the default in place; a new name is appended |
//! | menu_*.json | file name | a user file replaces the built-in menu as a whole |
//! | extension_associations.json | `FileType` + `Extension` | goes first; a default with the same key is hidden |
//! | file_type_map.json | `Extension` | goes first; a default with the same key is hidden |
//!
//! `"Disabled": true` on a custom function or association entry removes every
//! entry with that name or key. Disabled entries are stripped from the JSON value
//! *before* the typed parse, so they need no `Command`/`Actions`.
//!
//! [`load_list_configs`] is the single entry point used by both startup
//! (`AppState::new`) and reload (`Transition::ReloadConfig`), so the two cannot
//! drift apart. `config.json` layers field-by-field through serde defaults and is
//! not handled here beyond [`load_app_config`].

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

use crate::config::{
    AppConfig, ConfigLoadResult, ConfigManager, ExtensionAssociation, FileTypeMapping,
};
use crate::input::KeyBindings;
use crate::model::dialog::CustomFunction;

/// What the command line asked for, set once by `main` before any config loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CliLayering {
    /// No flag: `UseBuiltInDefaults` in config.json decides.
    #[default]
    FromConfig,
    /// `--no-default-config`: user files replace the built-ins for this run.
    NoDefaultConfig,
    /// `--no-user-config`: ignore the user's config directory entirely.
    NoUserConfig,
}

static CLI_LAYERING: OnceLock<CliLayering> = OnceLock::new();

/// Record the command-line layering override. Only the first call takes effect.
pub fn set_cli_layering(cli: CliLayering) {
    let _ = CLI_LAYERING.set(cli);
}

/// The command-line layering override, `FromConfig` when none was given.
pub fn cli_layering() -> CliLayering {
    CLI_LAYERING.get().copied().unwrap_or_default()
}

/// How the list-type config files combine with their built-in defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigLayering {
    /// Built-in defaults, with the user's entries merged over them.
    #[default]
    Layered,
    /// A user file that exists replaces its built-in; a missing one still falls
    /// back to the built-in (so a fresh install works in either mode).
    UserReplaces,
    /// Built-ins only; the user's config directory is not read.
    BuiltInOnly,
}

impl ConfigLayering {
    pub fn resolve(use_built_in_defaults: bool, cli: CliLayering) -> Self {
        match cli {
            CliLayering::NoUserConfig => Self::BuiltInOnly,
            CliLayering::NoDefaultConfig => Self::UserReplaces,
            CliLayering::FromConfig if use_built_in_defaults => Self::Layered,
            CliLayering::FromConfig => Self::UserReplaces,
        }
    }

    /// The mode in force for `config` and this process's command line.
    pub fn current(config: &AppConfig) -> Self {
        Self::resolve(config.use_built_in_defaults, cli_layering())
    }

    pub fn reads_user_files(self) -> bool {
        self != Self::BuiltInOnly
    }

    /// One-line description for the load report and `config_effective.json`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Layered => "built-in defaults + user overrides",
            Self::UserReplaces => "user files replace built-in defaults",
            Self::BuiltInOnly => "built-in defaults only (user config ignored)",
        }
    }
}

/// Where a custom function's effective definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOrigin {
    BuiltIn,
    User,
}

impl ConfigOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::User => "user",
        }
    }
}

/// Layering facts kept on `AppState` for the UI and diagnostics.
#[derive(Debug, Clone, Default)]
pub struct ConfigLayers {
    pub layering: ConfigLayering,
    /// Custom functions whose effective definition is the built-in one. Anything
    /// else (a user override, a user addition, a function pushed by a test) is `User`.
    pub built_in_function_names: HashSet<String>,
}

impl ConfigLayers {
    pub fn function_origin(&self, name: &str) -> ConfigOrigin {
        if self.built_in_function_names.contains(name) {
            ConfigOrigin::BuiltIn
        } else {
            ConfigOrigin::User
        }
    }
}

/// How one file's user entries combined with its built-ins, for the load report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayerSummary {
    /// Entries the user file contributed (overrides + additions).
    pub user: usize,
    /// Entries in the built-in defaults.
    pub built_in: usize,
    /// Built-in entries a user entry replaced or hid.
    pub overridden: usize,
    /// Entries removed by `"Disabled": true` (or unbound keys).
    pub disabled: usize,
    /// User entries that are an exact copy of their built-in (e.g. an exported
    /// file left as-is). They still count as `user`, but keep the built-in origin.
    pub identical: usize,
}

impl LayerSummary {
    /// e.g. "3 user entries over 21 built-in, 1 disabled".
    pub fn describe(&self) -> String {
        self.describe_as(("entry", "entries"), "disabled")
    }

    /// [`describe`](Self::describe) with the file's own words for an entry and for
    /// a removed one (keybindings: "binding"/"bindings", "unbound").
    pub fn describe_as(&self, (singular, plural): (&str, &str), removed: &str) -> String {
        let noun = if self.user == 1 { singular } else { plural };
        let mut s = format!("{} user {noun} over {} built-in", self.user, self.built_in);
        if self.identical > 0 {
            s.push_str(&format!(", {} same as built-in", self.identical));
        }
        if self.disabled > 0 {
            s.push_str(&format!(", {} {removed}", self.disabled));
        }
        s
    }
}

/// Everything the list-type config files resolve to.
#[derive(Debug, Clone, Default)]
pub struct ListConfigs {
    pub extension_associations: Vec<ExtensionAssociation>,
    pub file_type_map: Vec<FileTypeMapping>,
    pub custom_functions: Vec<CustomFunction>,
    pub built_in_function_names: HashSet<String>,
    /// extension_associations, file_type_map, custom_functions, context_menu, then
    /// one per menu file — the order the load report has always used.
    pub results: Vec<ConfigLoadResult>,
}

/// Load every list-type config file. The single path for startup and reload.
pub fn load_list_configs(manager: &ConfigManager, layering: ConfigLayering) -> ListConfigs {
    let (extension_associations, ext_result) =
        load_extension_associations(manager.extension_associations_path(), layering);
    let (file_type_map, ftm_result) = load_file_type_map(manager.file_type_map_path(), layering);
    let fns = load_custom_functions(manager.custom_functions_path(), layering);
    let context_menu_result = ConfigManager::validate_json_file(manager.context_menu_path());

    let user_dir = manager
        .custom_functions_path()
        .parent()
        .unwrap_or(Path::new("."));
    let mut results = vec![ext_result, ftm_result, fns.result, context_menu_result];
    results.extend(menu_file_results(user_dir, layering));

    ListConfigs {
        extension_associations,
        file_type_map,
        custom_functions: fns.functions,
        built_in_function_names: fns.built_in_names,
        results,
    }
}

// ---------------------------------------------------------------------------
// config.json and keybindings.json
// ---------------------------------------------------------------------------

/// Load config.json, honouring `--no-user-config`. `None` means the file exists
/// but failed to load: startup falls back to defaults, reload keeps what it had.
pub fn load_app_config(manager: &ConfigManager) -> (Option<AppConfig>, ConfigLoadResult) {
    let path = manager.config_path().to_path_buf();
    if cli_layering() == CliLayering::NoUserConfig {
        return (
            Some(AppConfig::default()),
            ConfigLoadResult::default_fallback(path, "user config ignored (--no-user-config)"),
        );
    }
    let exists = path.exists();
    match manager.load_config() {
        Ok(config) if exists => (Some(config), ConfigLoadResult::ok(path)),
        Ok(config) => (
            Some(config),
            ConfigLoadResult::default_fallback(path, "no user file; built-in defaults"),
        ),
        Err(e) => {
            tracing::warn!("Failed to load config: {:?}, using defaults", e);
            (None, ConfigLoadResult::error(path, format!("{:?}", e)))
        }
    }
}

/// Load keybindings.json over (or, in replace mode, instead of) the built-ins.
pub fn load_keybindings(path: &Path, layering: ConfigLayering) -> (KeyBindings, ConfigLoadResult) {
    let path_buf = path.to_path_buf();
    let defaults = KeyBindings::embedded_defaults();
    let built_in = defaults.binding_count();
    if !layering.reads_user_files() {
        return (
            defaults,
            ConfigLoadResult::default_fallback(path_buf, "user config ignored")
                .with_note(format!("{built_in} built-in bindings")),
        );
    }
    if !path.exists() {
        return (
            defaults,
            ConfigLoadResult::default_fallback(path_buf, "no user file")
                .with_note(format!("{built_in} built-in bindings")),
        );
    }
    let value = match std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|c| serde_json::from_str::<Value>(&c).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Failed to load {:?}, using built-in defaults: {}", path, e);
            return (defaults, ConfigLoadResult::error(path_buf, e));
        }
    };
    let (user, unbound) = count_user_bindings(&value);
    match layering {
        ConfigLayering::UserReplaces => (
            KeyBindings::apply_over(KeyBindings::empty(), &value),
            ConfigLoadResult::ok(path_buf).with_note("replaces built-in defaults"),
        ),
        _ => {
            let summary = LayerSummary {
                user,
                built_in,
                disabled: unbound,
                ..LayerSummary::default()
            };
            let note = summary.describe_as(("binding", "bindings"), "unbound");
            (
                KeyBindings::apply_over(defaults, &value),
                ConfigLoadResult::ok(path_buf).with_note(note),
            )
        }
    }
}

/// (bindings set, keys unbound) across every section of a keybindings document.
fn count_user_bindings(v: &Value) -> (usize, usize) {
    const SECTIONS: &[&str] = &[
        "bindings",
        "textViewerBindings",
        "NormalMode",
        "SearchMode",
        "DialogMode",
        "ViewerMode",
        "LeapMode",
    ];
    let mut set = 0;
    let mut unbound = 0;
    for section in SECTIONS {
        if let Some(map) = v.get(section).and_then(Value::as_object) {
            for val in map.values() {
                if crate::input::is_unbind_value(val) {
                    unbound += 1;
                } else if val.is_string() {
                    set += 1;
                }
            }
        }
    }
    (set, unbound)
}

// ---------------------------------------------------------------------------
// custom_functions.json and menu_*.json
// ---------------------------------------------------------------------------

/// Resolved custom functions plus what the load report and UI need to know.
#[derive(Debug, Clone)]
pub struct LoadedFunctions {
    pub functions: Vec<CustomFunction>,
    pub built_in_names: HashSet<String>,
    pub result: ConfigLoadResult,
}

fn built_in_functions() -> Vec<CustomFunction> {
    crate::model::dialog::parse_custom_functions(crate::help_content::DEFAULT_CUSTOM_FUNCTIONS)
        .expect("embedded default_custom_functions.json is always valid")
}

/// Load custom_functions.json layered over the built-ins, resolve menu files, and
/// validate the result.
pub fn load_custom_functions(path: &Path, layering: ConfigLayering) -> LoadedFunctions {
    let path_buf = path.to_path_buf();
    let user_dir = layering
        .reads_user_files()
        .then(|| path.parent().unwrap_or(Path::new(".")));
    let defaults = built_in_functions();
    let built_in = defaults.len();

    let (merged, result) = if !layering.reads_user_files() {
        (
            all_built_in(defaults),
            ConfigLoadResult::default_fallback(path_buf, "user config ignored")
                .with_note(format!("{built_in} built-in entries")),
        )
    } else if !path.exists() {
        (
            all_built_in(defaults),
            ConfigLoadResult::default_fallback(path_buf, "no user file")
                .with_note(format!("{built_in} built-in entries")),
        )
    } else {
        match read_user_functions(path) {
            Err(e) => {
                tracing::warn!("Failed to load {:?}, using built-in defaults: {}", path, e);
                (all_built_in(defaults), ConfigLoadResult::error(path_buf, e))
            }
            Ok((user, _disabled)) if layering == ConfigLayering::UserReplaces => (
                MergedFunctions {
                    functions: user,
                    built_in_names: HashSet::new(),
                    summary: LayerSummary::default(),
                },
                ConfigLoadResult::ok(path_buf).with_note("replaces built-in defaults"),
            ),
            Ok((user, disabled)) => {
                let merged = merge_custom_functions(defaults, user, &disabled);
                let note = merged.summary.describe();
                (merged, ConfigLoadResult::ok(path_buf).with_note(note))
            }
        }
    };

    let mut functions = merged.functions;
    crate::model::dialog::resolve_menu_files(&mut functions, user_dir);
    let problems = crate::model::dialog::validate_custom_functions(&functions);
    let result = if problems.is_empty() {
        result
    } else {
        ConfigLoadResult::error(result.path, problems.join("; "))
    };
    LoadedFunctions {
        functions,
        built_in_names: merged.built_in_names,
        result,
    }
}

/// Parse a user custom_functions.json, returning its entries and the names it disables.
fn read_user_functions(path: &Path) -> Result<(Vec<CustomFunction>, Vec<String>), String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut value: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let entries = match &mut value {
        Value::Array(a) => Some(a),
        Value::Object(o) => o.get_mut("Functions").and_then(Value::as_array_mut),
        _ => None,
    };
    let disabled = entries
        .map(|a| take_disabled(a, |e| str_field(e, "Name")))
        .unwrap_or_default();
    let functions = crate::model::dialog::parse_custom_functions(&value.to_string())
        .map_err(|e| e.to_string())?;
    Ok((functions, disabled))
}

/// The outcome of merging user custom functions over the built-ins.
#[derive(Debug, Clone)]
pub struct MergedFunctions {
    pub functions: Vec<CustomFunction>,
    pub built_in_names: HashSet<String>,
    pub summary: LayerSummary,
}

fn all_built_in(defaults: Vec<CustomFunction>) -> MergedFunctions {
    let built_in_names = defaults.iter().map(|f| f.name.clone()).collect();
    MergedFunctions {
        summary: LayerSummary {
            built_in: defaults.len(),
            ..LayerSummary::default()
        },
        functions: defaults,
        built_in_names,
    }
}

/// Merge by `Name`: a user entry replaces the first built-in of the same name in
/// place (so list order stays stable), a new name is appended, and every entry
/// named in `disabled` is dropped. A user entry identical to its built-in keeps
/// the built-in origin, so an exported file left untouched still reads as built-in.
pub fn merge_custom_functions(
    defaults: Vec<CustomFunction>,
    user: Vec<CustomFunction>,
    disabled: &[String],
) -> MergedFunctions {
    let built_in = defaults.len();
    let user_count = user.len();
    let mut merged: Vec<(CustomFunction, ConfigOrigin)> = defaults
        .into_iter()
        .map(|f| (f, ConfigOrigin::BuiltIn))
        .collect();
    let mut overridden = 0;
    let mut identical = 0;
    let mut replaced = vec![false; merged.len()];
    for func in user {
        let slot = (0..replaced.len()).find(|&i| !replaced[i] && merged[i].0.name == func.name);
        match slot {
            Some(i) => {
                replaced[i] = true;
                overridden += 1;
                let same =
                    serde_json::to_value(&merged[i].0).ok() == serde_json::to_value(&func).ok();
                if same {
                    identical += 1;
                } else {
                    merged[i] = (func, ConfigOrigin::User);
                }
            }
            None => merged.push((func, ConfigOrigin::User)),
        }
    }
    let before = merged.len();
    merged.retain(|(f, _)| !disabled.contains(&f.name));
    let disabled_count = before - merged.len();

    let built_in_names = merged
        .iter()
        .filter(|(_, o)| *o == ConfigOrigin::BuiltIn)
        .map(|(f, _)| f.name.clone())
        .collect();
    MergedFunctions {
        functions: merged.into_iter().map(|(f, _)| f).collect(),
        built_in_names,
        summary: LayerSummary {
            user: user_count,
            built_in,
            overridden,
            disabled: disabled_count,
            identical,
        },
    }
}

/// One load-report row per menu file: each built-in menu (replaced by a user file
/// of the same name, or in use as-is) and each other `menu_*.json` in `user_dir`.
fn menu_file_results(user_dir: &Path, layering: ConfigLayering) -> Vec<ConfigLoadResult> {
    let mut results: Vec<ConfigLoadResult> = Vec::new();
    let mut user_paths: Vec<std::path::PathBuf> = Vec::new();
    if layering.reads_user_files() {
        if let Ok(entries) = std::fs::read_dir(user_dir) {
            user_paths = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("menu_") && n.ends_with(".json"))
                })
                .collect();
        }
    }
    for (name, _) in crate::help_content::BUILT_IN_MENUS {
        let path = user_dir.join(name);
        let user_file = user_paths.iter().find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
        results.push(match user_file {
            Some(p) => {
                let r = ConfigManager::validate_json_file(p);
                match r.status {
                    crate::config::ConfigLoadStatus::Ok => {
                        r.with_note("user file replaces built-in menu")
                    }
                    _ => r.with_note("built-in menu in use"),
                }
            }
            None if layering.reads_user_files() => {
                ConfigLoadResult::default_fallback(path, "no user file; built-in menu")
            }
            None => ConfigLoadResult::default_fallback(path, "user config ignored; built-in menu"),
        });
    }
    for p in user_paths {
        let is_built_in = p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| crate::model::dialog::built_in_menu(n).is_some());
        if !is_built_in {
            results.push(ConfigManager::validate_json_file(&p));
        }
    }
    results.sort_by(|a, b| a.path.cmp(&b.path));
    results
}

// ---------------------------------------------------------------------------
// extension_associations.json and file_type_map.json
// ---------------------------------------------------------------------------

/// Merge key of an association: `FileType` and `Extension`, case-insensitive.
fn association_key(file_type: Option<&str>, extension: Option<&str>) -> String {
    format!(
        "{}|{}",
        file_type.unwrap_or("").to_lowercase(),
        extension
            .unwrap_or("")
            .trim_start_matches('.')
            .to_lowercase()
    )
}

fn association_value_key(v: &Value) -> Option<String> {
    let ft = str_field(v, "FileType");
    let ext = str_field(v, "Extension");
    (ft.is_some() || ext.is_some()).then(|| association_key(ft.as_deref(), ext.as_deref()))
}

fn file_type_value_key(v: &Value) -> Option<String> {
    str_field(v, "Extension").map(|e| association_key(None, Some(&e)))
}

/// Load extension_associations.json layered over the built-ins.
pub fn load_extension_associations(
    path: &Path,
    layering: ConfigLayering,
) -> (Vec<ExtensionAssociation>, ConfigLoadResult) {
    let (assocs, result) = load_keyed_list(
        path,
        layering,
        crate::help_content::DEFAULT_EXTENSION_ASSOCIATIONS,
        association_value_key,
        |a: &ExtensionAssociation| association_key(a.file_type.as_deref(), a.extension.as_deref()),
    );
    // Phase 7.3b: an entry with neither `FileType` nor `Extension` can never match
    // anything — skip it (with a warning) instead of failing the whole file.
    let assocs = assocs
        .into_iter()
        .filter(|a| {
            let usable = a.file_type.is_some() || a.extension.is_some();
            if !usable {
                tracing::warn!(
                    "extension_associations.json: skipping entry with neither FileType nor Extension set (command: {:?})",
                    a.command
                );
            }
            usable
        })
        .collect();
    (assocs, result)
}

/// Load file_type_map.json layered over the built-ins.
pub fn load_file_type_map(
    path: &Path,
    layering: ConfigLayering,
) -> (Vec<FileTypeMapping>, ConfigLoadResult) {
    load_keyed_list(
        path,
        layering,
        crate::help_content::DEFAULT_FILE_TYPE_MAP,
        file_type_value_key,
        |m: &FileTypeMapping| association_key(None, Some(&m.extension)),
    )
}

/// Shared loader for the two array-shaped, key-merged files.
fn load_keyed_list<T: serde::de::DeserializeOwned>(
    path: &Path,
    layering: ConfigLayering,
    defaults_json: &str,
    value_key: fn(&Value) -> Option<String>,
    item_key: impl Fn(&T) -> String,
) -> (Vec<T>, ConfigLoadResult) {
    let path_buf = path.to_path_buf();
    let defaults: Vec<T> = serde_json::from_str(defaults_json)
        .expect("embedded default config lists are always valid JSON");
    let built_in = defaults.len();
    if !layering.reads_user_files() {
        return (
            defaults,
            ConfigLoadResult::default_fallback(path_buf, "user config ignored")
                .with_note(format!("{built_in} built-in entries")),
        );
    }
    if !path.exists() {
        return (
            defaults,
            ConfigLoadResult::default_fallback(path_buf, "no user file")
                .with_note(format!("{built_in} built-in entries")),
        );
    }
    let parsed = std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|c| serde_json::from_str::<Value>(&c).map_err(|e| e.to_string()))
        .and_then(|mut v| {
            let disabled = match v.as_array_mut() {
                Some(a) => take_disabled(a, value_key),
                None => return Err("expected a JSON array of entries".to_string()),
            };
            serde_json::from_value::<Vec<T>>(v)
                .map(|user| (user, disabled))
                .map_err(|e| e.to_string())
        });
    let (user, disabled) = match parsed {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to load {:?}, using built-in defaults: {}", path, e);
            return (defaults, ConfigLoadResult::error(path_buf, e));
        }
    };
    if layering == ConfigLayering::UserReplaces {
        return (
            user,
            ConfigLoadResult::ok(path_buf).with_note("replaces built-in defaults"),
        );
    }
    let (merged, summary) = merge_keyed(defaults, user, &disabled, item_key);
    let note = summary.describe();
    (merged, ConfigLoadResult::ok(path_buf).with_note(note))
}

/// User entries first, then every default whose key no user entry claims and no
/// `Disabled` entry names. A same-key default is hidden rather than kept second:
/// associations show every match in the Open With picker, so keeping it would turn
/// each override into a prompt.
pub fn merge_keyed<T>(
    defaults: Vec<T>,
    user: Vec<T>,
    disabled: &[String],
    key: impl Fn(&T) -> String,
) -> (Vec<T>, LayerSummary) {
    let user_keys: HashSet<String> = user.iter().map(&key).collect();
    let built_in = defaults.len();
    let user_count = user.len();
    let mut overridden = 0;
    let mut disabled_count = 0;
    let mut merged = user;
    for d in defaults {
        let k = key(&d);
        if disabled.contains(&k) {
            disabled_count += 1;
        } else if user_keys.contains(&k) {
            overridden += 1;
        } else {
            merged.push(d);
        }
    }
    (
        merged,
        LayerSummary {
            user: user_count,
            built_in,
            overridden,
            disabled: disabled_count,
            identical: 0,
        },
    )
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Remove every object with `"Disabled": true` from `entries`, returning the merge
/// key of each (entries with no key are dropped with a warning).
fn take_disabled(entries: &mut Vec<Value>, key: impl Fn(&Value) -> Option<String>) -> Vec<String> {
    let mut disabled = Vec::new();
    entries.retain(|e| {
        if e.get("Disabled").and_then(Value::as_bool) != Some(true) {
            return true;
        }
        match key(e) {
            Some(k) => disabled.push(k),
            None => tracing::warn!("\"Disabled\": true entry names nothing to disable: {e}"),
        }
        false
    });
    disabled
}

fn str_field(v: &Value, field: &str) -> Option<String> {
    v.get(field).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
#[path = "config_layers_tests.rs"]
mod tests;
