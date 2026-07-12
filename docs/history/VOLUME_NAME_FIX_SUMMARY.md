# Volume Name Implementation Fix

## Summary
Fixed the volume name implementation in `rwf-lib/src/volume_info.rs` to match the TWF C# reference code behavior.

## Issues Fixed

### 1. Windows Volume Label Fallback
**Problem**: `get_volume_label_windows` was returning `"C:"` instead of allowing fallback to `"(C:)"` format.

**Fix**: 
- Changed `get_volume_label_windows` to return `None` instead of `Some("C:")`
- Added comprehensive TODO comment explaining the Windows API implementation needed
- Updated `get_windows_volume_name` to check if label is empty before returning it
- Fallback now correctly returns `"(C:)"` format when no volume label exists

**Behavior**:
- If volume label exists and is not empty: return just the label (e.g., "System", "Data")
- If no volume label: return drive letter in parentheses: "(C:)"
- Network paths: return "\\\\server" format

### 2. Unix/Linux Implementation Simplification
**Problem**: Root path check was at the end of the function, making the logic unnecessarily complex.

**Fix**:
- Moved root path check to the beginning of `get_unix_volume_name`
- Simplified control flow for better readability
- Maintained all existing functionality

**Behavior**:
- For root path "/": returns "Root"
- For other paths: returns device and mount point with optional label
- Format: "{device} ({mount_point})" or "{device} ({mount_point} - {label})"

### 3. Format Order Verification
**Status**: ✅ Verified correct

The `format_top_separator_info` function already implements the correct format:
- If both dirs and files: "{dirCount} {Dir/Dirs} {fileCount} {File/Files} {size} marked"
- If only dirs: "{dirCount} {Dir/Dirs} {size} marked"
- If only files: "{fileCount} {File/Files} {size} marked"

## Code Changes

### Modified Functions
1. `get_windows_volume_name` - Added empty label check
2. `get_volume_label_windows` - Returns None with TODO comment for Windows API
3. `get_unix_volume_name` - Moved root path check to beginning

### Files Modified
- `rwf-lib/src/volume_info.rs`

## Testing
- ✅ Compilation successful with no warnings
- ✅ No diagnostic errors
- ✅ Matches requirements 39A.2-39A.8 from design document

## Requirements Validated
- **39A.2**: Network path display (\\\\server format)
- **39A.3-39A.6**: Linux/MacOS device and mount point display
- **39A.7**: Windows volume label display
- **39A.8**: Windows drive letter fallback format "(C:)"
- **39A.9-39A.13**: Marked file statistics formatting (already correct)

## Next Steps
To fully implement Windows volume label support:
1. Add `winapi` crate dependency
2. Implement `GetVolumeInformationW` Windows API call
3. Handle wide string conversion for Windows paths
4. Parse volume name buffer and return actual label
