//! Custom-function definitions and their JSON loading (moved down from `dialog` so
//! `config_layers`/`help_content` need not depend on the dialog stack).

use crate::pipe_to_action::PipeToAction;
use std::collections::HashMap;

/// A single item in a menu_xxx.json file.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MenuItem {
    pub name: String,
    /// Custom function name or built-in action name. Empty string = separator.
    #[serde(default)]
    pub action: String,
    /// Shown dimmed beside the name. Filled from the target function's own
    /// description at load (`fill_menu_descriptions`) when the menu file does
    /// not give one, so the common case needs no extra JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl MenuItem {
    pub fn new(name: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            action: action.into(),
            description: None,
        }
    }

    pub fn is_separator(&self) -> bool {
        self.name.starts_with("-----") || self.action.is_empty()
    }
    pub fn is_selectable(&self) -> bool {
        !self.is_separator()
    }
}

/// Wrapper for the menu_xxx.json file format.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MenuFile {
    #[serde(default)]
    #[allow(dead_code)]
    pub version: String,
    pub menus: Vec<MenuItem>,
}

/// Menu content: either a resolved item list or a filename to load from.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MenuContent {
    /// Resolved list of menu items (after loading)
    Items(Vec<MenuItem>),
    /// Path to a separate menu JSON file (relative to the parent file's directory)
    File(String),
}

/// Custom function definition with macro expansion support.
///
/// Exactly one of `Command` (run a shell command), `Menu` (open a submenu) or
/// `ClipText` (copy expanded text to the clipboard) must be present. The
/// combination is validated at load time by [`validate_custom_functions`] rather
/// than resolved by a silent precedence rule, so an ambiguous entry is a loud
/// config error instead of a surprise.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CustomFunction {
    pub name: String,
    /// Shell command to execute (leaf entry). Mutually exclusive with Menu/ClipText.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Submenu: inline item list or filename reference. Resolved to Items after loading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<MenuContent>,
    /// Macro template whose expansion is copied to the clipboard. No process is
    /// spawned. Mutually exclusive with Command/Menu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_text: Option<String>,
    /// Hand the terminal to this command: rwf leaves the alternate screen, the
    /// child inherits stdin/stderr (so a TUI like fzf or vim can draw), stdout is
    /// captured, and rwf resumes when it exits. Orthogonal to `PipeToAction` —
    /// either may be used without the other.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suspend: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Category for display in the help viewer (default: "Custom Functions" if absent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipe_to_action: Option<PipeToAction>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub os_specific: HashMap<String, OsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_binding: Option<String>,
}

impl CustomFunction {
    pub fn is_menu(&self) -> bool {
        self.menu.is_some()
    }
    pub fn is_command(&self) -> bool {
        self.command.is_some()
    }
    pub fn is_clip_text(&self) -> bool {
        self.clip_text.is_some()
    }

    /// Return the resolved menu items, or empty slice if not a menu or not yet resolved.
    pub fn menu_items(&self) -> &[MenuItem] {
        match &self.menu {
            Some(MenuContent::Items(items)) => items,
            _ => &[],
        }
    }
}

impl CustomFunction {
    /// Create a new leaf custom function with a command
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: Some(command.into()),
            menu: None,
            clip_text: None,
            suspend: false,
            description: None,
            category: None,
            shell: None,
            working_dir: None,
            pipe_to_action: None,
            os_specific: HashMap::new(),
            key_binding: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the shell
    pub fn with_shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, working_dir: impl Into<String>) -> Self {
        self.working_dir = Some(working_dir.into());
        self
    }

    /// Set the pipe to action
    pub fn with_pipe_to_action(mut self, action: PipeToAction) -> Self {
        self.pipe_to_action = Some(action);
        self
    }

    /// Get the effective command for the current OS, or None for menu entries.
    pub fn get_command(&self) -> Option<&str> {
        #[cfg(target_os = "windows")]
        let os_key = "windows";
        #[cfg(target_os = "macos")]
        let os_key = "macos";
        #[cfg(target_os = "linux")]
        let os_key = "linux";

        if let Some(os_config) = self.os_specific.get(os_key) {
            Some(&os_config.command)
        } else {
            self.command.as_deref()
        }
    }

    /// Get the shell for the current OS
    pub fn get_shell(&self) -> Option<&str> {
        #[cfg(target_os = "windows")]
        let os_key = "windows";
        #[cfg(target_os = "macos")]
        let os_key = "macos";
        #[cfg(target_os = "linux")]
        let os_key = "linux";

        if let Some(os_config) = self.os_specific.get(os_key) {
            os_config.shell.as_deref().or(self.shell.as_deref())
        } else {
            self.shell.as_deref()
        }
    }
}

/// Wrapper format for custom_functions.json (TWF-compatible)
#[derive(Debug, serde::Deserialize)]
struct CustomFunctionsFile {
    #[serde(rename = "Version", default)]
    #[allow(dead_code)]
    version: String,
    #[serde(rename = "Functions")]
    functions: Vec<CustomFunction>,
}

/// Load custom functions from one JSON file, as-is (no built-in layering — that is
/// `crate::config_layers::load_custom_functions`).
/// Accepts the wrapper format `{"Version":"1.0","Functions":[...]}` and a bare
/// array `[...]`. Menu entries whose `Menu` field is a filename string are loaded
/// from the same directory as the parent file, falling back to the built-in menu
/// of that name.
pub fn load_custom_functions(
    path: &std::path::Path,
) -> Result<Vec<CustomFunction>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
    let content = std::fs::read_to_string(path)?;

    let mut functions = parse_custom_functions(&content)?;
    resolve_menu_files(&mut functions, Some(base_dir));
    Ok(functions)
}

pub(crate) fn parse_custom_functions(
    content: &str,
) -> Result<Vec<CustomFunction>, Box<dyn std::error::Error>> {
    // Preferred: wrapper object with Version + Functions
    if let Ok(file) = serde_json::from_str::<CustomFunctionsFile>(content) {
        return Ok(file.functions);
    }
    // Fallback: bare array
    if let Ok(functions) = serde_json::from_str::<Vec<CustomFunction>>(content) {
        return Ok(functions);
    }
    // Surface error from the wrapper format as primary
    let file: CustomFunctionsFile = serde_json::from_str(content)?;
    Ok(file.functions)
}

/// Validate loaded custom functions, returning one human-readable message per problem.
///
/// Three classes of error, all deliberately loud rather than silently resolved:
///
/// 1. **Ambiguous kind** — `Command`/`Menu`/`ClipText` are mutually exclusive, and
///    `Suspend` is meaningless on a `ClipText` entry (there is no process to suspend).
///    Reporting beats inventing a precedence order nobody can remember.
/// 2. **Stale `$M`** — the removed marked-files macro. Left alone it would survive as
///    literal `$M` text in the command rather than erroring, so it is caught here.
///    Replacements: `$MFS` (names for a shell), `$MPS` (full paths for a shell),
///    `$MFL` / `$MPL` (newline-joined raw forms, for `ClipText`).
/// 3. **Picker without `Suspend`** — see [`INTERACTIVE_PICKERS`]. The function still
///    loads and runs; the message names the fix.
pub fn validate_custom_functions(functions: &[CustomFunction]) -> Vec<String> {
    let mut problems = Vec::new();
    for f in functions {
        let kinds = [
            ("Command", f.command.is_some()),
            ("Menu", f.menu.is_some()),
            ("ClipText", f.clip_text.is_some()),
        ];
        let present: Vec<&str> = kinds.iter().filter(|(_, p)| *p).map(|(n, _)| *n).collect();
        match present.len() {
            0 => problems.push(format!(
                "'{}': needs one of Command, Menu or ClipText",
                f.name
            )),
            1 => {}
            _ => problems.push(format!(
                "'{}': {} are mutually exclusive — keep only one",
                f.name,
                present.join(" and ")
            )),
        }
        if !f.suspend {
            let commands = f
                .command
                .iter()
                .chain(f.os_specific.values().map(|os| &os.command));
            if let Some(picker) = commands.filter_map(|c| interactive_picker_in(c)).next() {
                problems.push(format!(
                    "'{}': runs {picker}, which needs the terminal — add \"Suspend\": true \
                     (without it rwf and {picker} share the console and both act on your keys)",
                    f.name
                ));
            }
        }
        if f.suspend && f.clip_text.is_some() {
            problems.push(format!(
                "'{}': Suspend has no meaning with ClipText (no process is run)",
                f.name
            ));
        }
        for (field, text) in [("Command", &f.command), ("ClipText", &f.clip_text)] {
            if let Some(text) = text {
                if crate::macro_syntax::has_stale_marked_macro(text) {
                    problems.push(format!(
                        "'{}': {} uses the removed $M macro — use $MFS (names, for a shell), \
                         $MPS (full paths, for a shell), or $MFL/$MPL (newline-joined, for ClipText)",
                        f.name, field
                    ));
                }
            }
        }
    }
    problems
}

/// The built-in menu file shipped under `name`, if any. Menu files layer by file
/// name: a user file of the same name replaces the built-in menu as a whole.
pub fn built_in_menu(name: &str) -> Option<&'static str> {
    crate::defaults::BUILT_IN_MENUS
        .iter()
        .find(|(file, _)| file.eq_ignore_ascii_case(name))
        .map(|(_, content)| *content)
}

/// Resolve `Menu: "filename.json"` references into item lists.
///
/// The file is read from `user_dir` when it exists there; otherwise the built-in
/// menu of the same name is used (Phase 7.26 — before that a missing file gave an
/// empty menu even though the default was embedded). `user_dir: None` means user
/// config is being ignored (`--no-user-config`). A user file that fails to parse
/// also falls back to the built-in menu; the load report flags the file as NG.
/// Inline `Items` entries are left as-is (nested menus not supported in 6.6 scope).
pub(crate) fn resolve_menu_files(
    functions: &mut [CustomFunction],
    user_dir: Option<&std::path::Path>,
) {
    for func in functions.iter_mut() {
        if let Some(MenuContent::File(filename)) = &func.menu {
            let user_menu = user_dir
                .map(|dir| dir.join(filename))
                .filter(|p| p.exists())
                .and_then(|p| {
                    let parsed = std::fs::read_to_string(&p)
                        .map_err(|e| e.to_string())
                        .and_then(|c| {
                            serde_json::from_str::<MenuFile>(&c).map_err(|e| e.to_string())
                        });
                    match parsed {
                        Ok(menu_file) => Some(menu_file.menus),
                        Err(e) => {
                            tracing::warn!("Failed to load menu file {:?}: {}", p, e);
                            None
                        }
                    }
                });
            let items = user_menu
                .or_else(|| {
                    built_in_menu(filename).and_then(|c| {
                        serde_json::from_str::<MenuFile>(c)
                            .map(|m| m.menus)
                            .map_err(|e| tracing::warn!("built-in menu {filename}: {e}"))
                            .ok()
                    })
                })
                .unwrap_or_else(|| {
                    tracing::warn!("Menu file {:?} not found and has no built-in", filename);
                    Vec::new()
                });
            func.menu = Some(MenuContent::Items(items));
        }
        // Inline Items stay as-is; no recursive nesting in 6.6
    }
    fill_menu_descriptions(functions);
}

/// Full-screen pickers that read the console directly. Run without `Suspend` they
/// share the console with rwf: both draw, both act on each key, and on exit the
/// picker leaves the alternate screen rwf is still drawing to (bundle
/// 20260919-133522, where keys meant for fzf opened a Rename dialog in rwf).
const INTERACTIVE_PICKERS: &[&str] = &["fzf", "sk", "peco", "percol", "fzy"];

/// The first known interactive picker a command runs, matched on program name so
/// `C:\tools\fzf.exe` and `(fzf.exe)` inside a PowerShell expression both count.
fn interactive_picker_in(command: &str) -> Option<&'static str> {
    command
        .split(|c: char| c.is_whitespace() || "()|;&'\"`".contains(c))
        .filter_map(|token| token.rsplit(['/', '\\']).next())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            lower
                .strip_suffix(".exe")
                .map(str::to_string)
                .unwrap_or(lower)
        })
        .find_map(|name| INTERACTIVE_PICKERS.iter().copied().find(|p| *p == name))
}

/// OS-specific configuration for custom functions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OsConfig {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}

/// Give every menu item without a `Description` the description of the function its
/// `Action` names.
///
/// Runs once at load (and so again on every config reload) rather than at render
/// time, so the menu renderer and its width calculation only read a field. An
/// explicit `Description` in the menu file wins; an action that names no function
/// (a built-in like `ReloadConfig`, or a stale menu file) keeps `None`. Name lookup
/// takes the first match, the same rule the action dispatch uses.
pub(crate) fn fill_menu_descriptions(functions: &mut [CustomFunction]) {
    let mut by_name: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for func in functions.iter() {
        if let Some(desc) = func.description.as_ref().filter(|d| !d.is_empty()) {
            by_name
                .entry(func.name.clone())
                .or_insert_with(|| desc.clone());
        }
    }
    for func in functions.iter_mut() {
        if let Some(MenuContent::Items(items)) = &mut func.menu {
            for item in items.iter_mut() {
                if item.description.is_none() && item.is_selectable() {
                    item.description = by_name.get(&item.action).cloned();
                }
            }
        }
    }
}
