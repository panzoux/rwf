//! Guards for the config-file serialization contract.
//!
//! `config.json` is read from a user's real `%APPDATA%\rwf\` directory, written by
//! older versions of rwf and by the TWF prototype. Two rules follow from that and
//! are stated in CLAUDE.md, but nothing enforced them:
//!
//! 1. **Keys are PascalCase.** A field that serializes as `snake_case` is silently
//!    unreadable by TWF and by any config the user hand-wrote in the documented style.
//! 2. **Every field is optional.** A new field without a `serde` default makes every
//!    existing config file fail to parse; rwf then falls back to defaults wholesale,
//!    so the user loses *all* their settings, not just the new one — and the only
//!    signal is a line in the log.
//!
//! Both are properties of the serialized shape, so both are cheap to test directly.
//! See `docs/IMPLICIT_CONTRACTS.md`.

use rwf_lib::config::AppConfig;
use serde_json::Value;

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{}: {:?}", context, err),
    }
}

fn default_config_json() -> Value {
    ok(
        serde_json::to_value(AppConfig::default()),
        "AppConfig::default() is not serializable",
    )
}

/// Collect `(json_pointer, key)` for every object key in the tree.
fn collect_keys(value: &Value, pointer: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_pointer =
                    format!("{}/{}", pointer, key.replace('~', "~0").replace('/', "~1"));
                out.push((child_pointer.clone(), key.clone()));
                collect_keys(child, &child_pointer, out);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_keys(child, &format!("{}/{}", pointer, index), out);
            }
        }
        _ => {}
    }
}

#[test]
fn app_config_serializes_with_pascal_case_keys() {
    let json = default_config_json();
    let mut keys = Vec::new();
    collect_keys(&json, "", &mut keys);

    assert!(
        keys.len() > 30,
        "only found {} keys in the serialized AppConfig — the walk is broken",
        keys.len()
    );

    let offenders: Vec<String> = keys
        .iter()
        .filter(|(_, key)| {
            // A struct field rendered in the documented style always starts with an
            // ASCII uppercase letter. Anything else is either snake_case (a missing
            // `rename_all`/`rename`) or camelCase.
            !key.chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
        })
        .map(|(pointer, key)| format!("{} (at {})", key, pointer))
        .collect();

    assert!(
        offenders.is_empty(),
        "config.json keys must be PascalCase for TWF compatibility, but these are not:\n  {}\n\n\
         Add `#[serde(rename_all = \"PascalCase\")]` to the struct, or \
         `#[serde(rename = \"FieldName\")]` to the field.",
        offenders.join("\n  ")
    );
}

#[test]
fn app_config_parses_from_an_empty_object() {
    let parsed: Result<AppConfig, _> = serde_json::from_value(serde_json::json!({}));
    assert!(
        parsed.is_ok(),
        "an empty config.json must deserialize to defaults, but failed: {:?}\n\n\
         Some field on AppConfig lost its `#[serde(default)]`. Every existing user \
         config would now fail to load, silently discarding all their settings.",
        parsed.err()
    );
}

/// `ArchiveConfig` and `TextInputConfig` shipped without `rename_all` for long
/// enough that real installed configs contain their snake_case keys — the config at
/// `%APPDATA%\rwf\config.json` on the development machine has
/// `{"Archive": {"default_format": ..., "compression_level": ..., ...}}` today.
///
/// Adding `rename_all` fixed the PascalCase contract but would have silently reset
/// those settings to defaults on the next launch, which is why each field also
/// carries a snake_case `#[serde(alias)]`. Dropping an alias is invisible: the
/// config still parses, the value just quietly reverts. This test is the only thing
/// that would notice.
#[test]
fn legacy_snake_case_keys_still_deserialize() {
    let legacy = serde_json::json!({
        "Archive": {
            "default_format": "SevenZip",
            "compression_level": 9,
            "last_archive_name": "release.7z"
        },
        "TextInput": { "edit_mode": "Vi" }
    });

    let parsed: AppConfig = ok(
        serde_json::from_value(legacy),
        "a config.json using the pre-rename_all snake_case keys must still load",
    );

    assert_eq!(
        parsed.archive.compression_level, 9,
        "Archive.compression_level fell back to its default — the snake_case alias is gone"
    );
    assert_eq!(
        parsed.archive.last_archive_name, "release.7z",
        "Archive.last_archive_name fell back to its default — the snake_case alias is gone"
    );
    assert_eq!(
        format!("{:?}", parsed.archive.default_format),
        "SevenZip",
        "Archive.default_format fell back to its default — the snake_case alias is gone"
    );
    assert_eq!(
        format!("{:?}", parsed.text_input.edit_mode),
        "Vi",
        "TextInput.edit_mode fell back to its default — the snake_case alias is gone"
    );
}

#[test]
fn every_app_config_field_can_be_omitted() {
    let json = default_config_json();
    let mut keys = Vec::new();
    collect_keys(&json, "", &mut keys);

    let mut missing_defaults: Vec<String> = Vec::new();
    for (pointer, _key) in &keys {
        let mut trimmed = json.clone();
        // Split "/a/b/c" into the parent pointer "/a/b" and the leaf key "c".
        let Some(split) = pointer.rfind('/') else {
            continue;
        };
        let (parent_pointer, leaf) = pointer.split_at(split);
        let leaf = leaf
            .trim_start_matches('/')
            .replace("~1", "/")
            .replace("~0", "~");
        let Some(parent) = trimmed.pointer_mut(parent_pointer) else {
            continue;
        };
        let Some(parent_map) = parent.as_object_mut() else {
            continue;
        };
        if parent_map.remove(&leaf).is_none() {
            continue;
        }
        if let Err(err) = serde_json::from_value::<AppConfig>(trimmed) {
            missing_defaults.push(format!("{} -> {}", pointer, err));
        }
    }

    assert!(
        missing_defaults.is_empty(),
        "these config keys are mandatory — omitting any one of them makes the whole \
         config.json unparseable, so a user upgrading from an older rwf loses every \
         setting at once:\n  {}\n\n\
         Give the field a `#[serde(default)]` (or `default = \"...\"`), or put \
         `#[serde(default)]` on the containing struct.",
        missing_defaults.join("\n  ")
    );
}
