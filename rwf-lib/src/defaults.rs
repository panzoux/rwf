//! Embedded default config files (moved down from `help_content` so `input`,
//! `config_layers` etc. can read defaults without depending on the help builder).

/// Embedded default config.json — exported by `--export-config-files`. The source of
/// truth stays `AppConfig::default()` (plus each field's serde default); this file is
/// its serialized form, kept identical by `default_config_json_matches_app_config_default`
/// in `rwf-bin/tests/config_contracts.rs`, which also regenerates it.
pub const DEFAULT_CONFIG: &str = include_str!("../resources/default_config.json");

/// Embedded default custom_functions.json — exported by `--export-config-files`
pub const DEFAULT_CUSTOM_FUNCTIONS: &str =
    include_str!("../resources/default_custom_functions.json");

/// Embedded default menu_config.json — exported by `--export-config-files`
pub const DEFAULT_MENU_CONFIG: &str = include_str!("../resources/default_menu_config.json");

/// Embedded default menu_clip.json — exported by `--export-config-files`
pub const DEFAULT_MENU_CLIP: &str = include_str!("../resources/default_menu_clip.json");

/// Every built-in `menu_*.json`, by the file name a custom function's `Menu` names.
/// Always loaded unless a user file of the same name replaces it (Phase 7.26).
pub const BUILT_IN_MENUS: &[(&str, &str)] = &[
    ("menu_clip.json", DEFAULT_MENU_CLIP),
    ("menu_config.json", DEFAULT_MENU_CONFIG),
];

/// Embedded default keybindings.json — exported by `--export-config-files`
pub const DEFAULT_KEYBINDINGS: &str = include_str!("../resources/default_keybindings.json");

/// Embedded default file_type_map.json — exported by `--export-config-files`
pub const DEFAULT_FILE_TYPE_MAP: &str = include_str!("../resources/default_file_type_map.json");

/// Embedded default extension_associations.json — exported by `--export-config-files`
pub const DEFAULT_EXTENSION_ASSOCIATIONS: &str =
    include_str!("../resources/default_extension_associations.json");
