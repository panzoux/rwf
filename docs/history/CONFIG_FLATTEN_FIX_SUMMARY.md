# Config Structure Fix: TWF Format Compatibility

## Problem
The config structure expected color fields to be nested under a `colors` object:
```json
{
  "Display": {
    "colors": {
      "ForegroundColor": "White",
      "PaneInfoBackgroundColor": "Gray"
    }
  }
}
```

But TWF's actual format has color fields directly under Display:
```json
{
  "Display": {
    "ForegroundColor": "White",
    "PaneInfoBackgroundColor": "Gray"
  }
}
```

## Solution
Added `#[serde(flatten)]` attribute to the `colors` field in `DisplayConfig` struct.

### Changes Made

**File: `rwf-lib/src/config.rs`**
- Modified `DisplayConfig` struct to flatten the `colors` field
- Changed comment from "Color scheme" to "Color scheme (flattened into Display for TWF compatibility)"
- Added `#[serde(flatten)]` attribute before `pub colors: ColorScheme`

**File: `rwf-lib/src/config_display_tests.rs`**
- Added new test `test_twf_format_flattened_colors()` to verify TWF format deserialization works correctly

## How It Works
The `#[serde(flatten)]` attribute tells serde to:
1. When **deserializing**: Accept color fields directly at the Display level and map them to the `colors` field
2. When **serializing**: Output color fields directly at the Display level (not nested under "colors")

This maintains TWF compatibility while keeping the Rust code structure clean with `state.config.display.colors.foreground_color`.

## Testing
- All existing config tests pass
- New test `test_twf_format_flattened_colors()` verifies TWF format works
- Build succeeds with no errors
- All code accessing colors through `state.config.display.colors` continues to work

## Impact
- **No breaking changes** to existing code
- **Full TWF compatibility** for config file format
- **Clean API** maintained in Rust code
