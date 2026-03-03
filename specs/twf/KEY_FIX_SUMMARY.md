# Key Binding Fix Summary

## Problem
Most keys (C, D, F, J, M, P, S, etc.) were not working in the TWF file manager. Only arrow keys and ESC were responding.

## Root Cause
The ListView controls in Terminal.Gui were capturing key events and not propagating them to the main window's key handler. When a control has focus in Terminal.Gui, it processes keys first, and if it doesn't explicitly pass them up, the parent window never sees them.

## Solution
Attached the `HandleKeyPress` event handler directly to both ListView controls (`_leftPane` and `_rightPane`) in addition to the main window. This ensures that key presses are processed regardless of which control has focus.

### Code Changes
**File:** `Controllers/MainController.cs`

**Before:**
```csharp
_leftPane = new ListView() { ... };
_mainWindow.Add(_leftPane);

_rightPane = new ListView() { ... };
_mainWindow.Add(_rightPane);
```

**After:**
```csharp
_leftPane = new ListView() { ... };
_leftPane.KeyPress += HandleKeyPress;  // Added
_mainWindow.Add(_leftPane);

_rightPane = new ListView() { ... };
_rightPane.KeyPress += HandleKeyPress;  // Added
_mainWindow.Add(_rightPane);
```

## Result
All hardcoded key bindings now work properly:
- Navigation keys (arrows, Tab, Enter, Backspace, Home, PageUp/Down)
- File operations (C=copy, M=move, D=delete, J=create dir)
- Display modes (1-8)
- Marking (Space, Shift+Space, Ctrl+Space, @)
- Search (F)
- Sorting (S)
- File mask (:)
- Compression (P)
- And all other implemented keys

## Testing
1. Rebuild: `dotnet build -c Release`
2. Publish: `dotnet publish -c Release -r win-x64 --self-contained`
3. Run: `bin\Release\net8.0\win-x64\publish\twf.exe`
4. Test various keys to confirm they work

## Configuration Files
Configuration files are stored in: `%APPDATA%\TWF\` (typically `C:\Users\[username]\AppData\Roaming\TWF\`)

- `config.json` - Main configuration
- `session.json` - Session state (paths, sort modes, etc.)
- `twf_errors.log` - Error log

## Future Enhancement
Custom key bindings via AFXW.KEY file are not yet implemented. The KeyBindingManager infrastructure exists but needs to be wired up to map Terminal.Gui's Key enum to virtual key codes and integrate with the HandleKeyPress method.
