# Custom Key Bindings Test Plan

## Prerequisites
- TWF application built and published to `bin/publish/`
- `keybindings.json` and `keybindings_test.json` in `bin/publish/`

## Test 1: Default Keys (No Custom Bindings)

**Setup:**
1. Ensure `%APPDATA%\TWF\config.json` has:
```json
{
  "KeyBindings": {
    "UseCustomBindings": false
  }
}
```

**Test Steps:**
1. Run `bin\publish\twf.exe`
2. Test these keys:
   - `C` - Should show copy dialog
   - `M` - Should show move dialog
   - `D` - Should show delete dialog
   - `F` - Should enter search mode
   - `S` - Should cycle sort mode
   - `A` - Should mark all files
   - `Tab` - Should switch panes
   - `Space` - Should toggle mark and move down

**Expected:** All keys work as documented ✓

## Test 2: Custom Bindings with Default Mappings

**Setup:**
1. Edit `%APPDATA%\TWF\config.json`:
```json
{
  "KeyBindings": {
    "KeyBindingFile": "keybindings.json",
    "UseCustomBindings": true
  }
}
```
2. Copy `bin\publish\keybindings.json` to `%APPDATA%\TWF\keybindings.json`

**Test Steps:**
1. Run `bin\publish\twf.exe`
2. Test the same keys as Test 1

**Expected:** All keys work exactly the same as Test 1 ✓

## Test 3: Custom Bindings with Alternative Mappings

**Setup:**
1. Edit `%APPDATA%\TWF\config.json`:
```json
{
  "KeyBindings": {
    "KeyBindingFile": "keybindings_test.json",
    "UseCustomBindings": true
  }
}
```
2. Copy `bin\publish\keybindings_test.json` to `%APPDATA%\TWF\keybindings_test.json`

**Test Steps:**
1. Run `bin\publish\twf.exe`
2. Test these NEW key mappings:
   - `X` - Should show copy dialog (was `C`)
   - `N` - Should show move dialog (was `M`)
   - `R` - Should show delete dialog (was `D`)
   - `T` - Should cycle sort mode (was `S`)
   - `Q` - Should mark all files (was `A`)
3. Test that OLD keys don't work:
   - `C` - Should do nothing
   - `M` - Should do nothing
   - `D` - Should do nothing
   - `S` - Should do nothing
   - `A` - Should do nothing

**Expected:** New keys work, old keys don't ✓

## Test 4: Modifier Keys

**Setup:** Same as Test 2 (default keybindings.json)

**Test Steps:**
1. Run `bin\publish\twf.exe`
2. Test modifier combinations:
   - `Shift+Enter` - Should extract archive or open with editor
   - `Ctrl+Enter` - Should use Explorer association
   - `Ctrl+PageUp` - Should move cursor to first entry
   - `Ctrl+PageDown` - Should move cursor to last entry
   - `Shift+Space` - Should toggle mark and move up
   - `Ctrl+Space` - Should mark range
   - `Shift+M` - Should show registered folder dialog
   - `Shift+B` - Should register current directory

**Expected:** All modifier combinations work ✓

## Test 5: File Location Priority

**Test 5a: AppData Location**
1. Place `keybindings.json` in `%APPDATA%\TWF\`
2. Remove `keybindings.json` from current directory
3. Enable custom bindings in config.json
4. Run application
**Expected:** Loads from AppData ✓

**Test 5b: Current Directory Fallback**
1. Remove `keybindings.json` from `%APPDATA%\TWF\`
2. Place `keybindings.json` in same directory as twf.exe
3. Enable custom bindings in config.json
4. Run application
**Expected:** Loads from current directory ✓

**Test 5c: Missing File**
1. Remove `keybindings.json` from both locations
2. Enable custom bindings in config.json
3. Run application
**Expected:** Falls back to hardcoded keys, shows warning in log ✓

## Test 6: Edit and Reload

**Setup:** Custom bindings enabled with keybindings.json

**Test Steps:**
1. Run application, verify `C` copies files
2. Exit application
3. Edit `%APPDATA%\TWF\keybindings.json`:
   - Change `"C": "HandleCopyOperation"` to `"Y": "HandleCopyOperation"`
4. Run application again
5. Test `Y` key - should copy
6. Test `C` key - should do nothing

**Expected:** Changes take effect after restart ✓

## Test 7: Invalid Configuration

**Test 7a: Invalid JSON**
1. Create invalid JSON in keybindings.json (missing comma, etc.)
2. Enable custom bindings
3. Run application
**Expected:** Falls back to hardcoded keys, shows error in log ✓

**Test 7b: Invalid Action Name**
1. Edit keybindings.json: `"C": "NonExistentAction"`
2. Enable custom bindings
3. Run application
4. Press `C`
**Expected:** Shows warning in log, key does nothing ✓

## Test 8: All Actions Available

**Test Steps:**
Verify all 50+ actions can be mapped and work:
- Navigation actions (MoveCursorUp, NavigateToParent, etc.)
- File operations (HandleCopyOperation, HandleDeleteOperation, etc.)
- Marking actions (ToggleMarkAndMoveDown, MarkAll, etc.)
- Display actions (DisplayMode1-8, CycleSortMode, etc.)
- Archive actions (HandleCompressionOperation, etc.)
- Special actions (EnterSearchMode, ExitApplication, etc.)

**Expected:** All actions work when mapped ✓

## Success Criteria

✓ All default keys work without custom bindings
✓ Custom bindings can override default keys
✓ All modifier combinations work (Shift, Ctrl, Alt)
✓ File location priority works correctly
✓ Changes take effect after restart
✓ Invalid configurations handled gracefully
✓ All 50+ actions are available and functional
✓ All 169 unit tests pass
✓ All 46 property-based tests pass

## Notes

- Custom key bindings are disabled by default for safety
- Users must explicitly enable them in config.json
- JSON format is easy to read and edit
- Well documented in CONFIGURATION.md
- Example files provided (keybindings.json, keybindings_test.json)
