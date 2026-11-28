//! Tests that `DiagnosticsConfig` / `AppConfig` serialise as the diagnostics
//! contract expects. Kept outside `diagnostics` so that module never depends on `config`.

use crate::config::{AppConfig, DiagnosticsConfig};

#[test]
fn diagnostics_config_defaults_to_enabled_with_a_prompt() {
    let cfg = DiagnosticsConfig::default();
    assert!(cfg.enabled);
    assert!(cfg.prompt_for_report);
    assert!(
        cfg.output_directory.is_empty(),
        "empty means default_diagnostics_dir()"
    );
}

/// Config JSON is PascalCase for TWF compatibility (see CLAUDE.md).
#[test]
fn diagnostics_config_serialises_as_pascal_case() {
    let json = serde_json::to_value(DiagnosticsConfig::default()).expect("serialize");
    assert!(json.get("Enabled").is_some(), "got: {json}");
    assert!(json.get("OutputDirectory").is_some(), "got: {json}");
    assert!(json.get("PromptForReport").is_some(), "got: {json}");
}

/// Every field needs `#[serde(default)]` so older config files still load.
#[test]
fn diagnostics_section_tolerates_missing_and_partial_config() {
    let absent: AppConfig = serde_json::from_str("{}").expect("empty config loads");
    assert!(absent.diagnostics.enabled);

    let partial: AppConfig =
        serde_json::from_str(r#"{"Diagnostics":{"Enabled":false}}"#).expect("partial loads");
    assert!(!partial.diagnostics.enabled);
    assert!(
        partial.diagnostics.prompt_for_report,
        "unspecified fields must fall back to defaults"
    );
}

/// `key_bindings` carries `#[serde(skip)]`, so a bundle that serialises only
/// `AppConfig` would silently omit the keymap — which is what makes a
/// no-action `Key` event interpretable. Pinning the fact the capture code
/// depends on.
#[test]
fn app_config_json_does_not_carry_keybindings() {
    let json = serde_json::to_value(AppConfig::default()).expect("serialize");
    assert!(
        json.get("KeyBindings").is_none() && json.get("key_bindings").is_none(),
        "AppConfig unexpectedly serialises keybindings; \
             App::record_effective_config adds them separately and would now duplicate them"
    );
}
