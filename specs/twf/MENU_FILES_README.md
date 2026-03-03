# Menu Files - Example Files

This directory contains example menu files for TWF's custom menu system.

## Installation

Copy these example menu files to your TWF configuration directory:

**Windows:**
```
%APPDATA%\TWF\
```

**Linux/Mac:**
```
~/.config/TWF/
```

## Example Files

### file_operations.json
A menu containing common file operations including:
- Opening files in Notepad
- Copying files between panes
- Custom copy with prompt
- Batch rename
- Opening command prompt
- Delete and move operations

### text_viewer_options.json
A menu for text viewer actions including:
- View as text
- View as hex
- Edit in external editor
- Search in file

## Usage

To use these menu files, add menu-type custom functions to your `custom_functions.json`:

```json
{
  "Version": "1.0",
  "Functions": [
    {
      "Name": "File Operations",
      "Menu": "file_operations.json",
      "Description": "Show file operations menu"
    },
    {
      "Name": "Viewer Options",
      "Menu": "text_viewer_options.json",
      "Description": "Show text viewer options"
    }
  ]
}
```

Then bind them to keys in `keybindings.json`:

```json
{
  "bindings": {
    "F2": "File Operations",
    "F3": "Viewer Options"
  }
}
```

## Menu File Format

Menu files follow this JSON structure:

```json
{
  "Version": "1.0",
  "Menus": [
    {
      "Name": "Menu Item Name",
      "Function": "CustomFunctionName"
    },
    {
      "Name": "Another Item",
      "Menu": "BuiltInAction"
    },
    {
      "Name": "-----",
      "Menu": ""
    }
  ]
}
```

### Properties

- **Version**: Menu file format version (currently "1.0")
- **Menus**: Array of menu items

### Menu Item Properties

- **Name**: Display name shown in the menu
- **Function**: (Optional) Name of a custom function from `custom_functions.json`
- **Menu**: (Optional) Name of a built-in TWF action
- Use `Name: "-----"` with `Menu: ""` to create a separator

### Function vs Menu

- **Function**: Executes a custom function with macro expansion
  - Example: `"Function": "Open in Notepad"`
  - Must match a function name in `custom_functions.json`

- **Menu**: Executes a built-in TWF action
  - Example: `"Menu": "ViewFileAsText"`
  - Common actions: `ViewFileAsText`, `ViewFileAsHex`, `DeleteFile`, `MoveFile`, `EditFile`, `SearchInFile`

## Keyboard Navigation

When a menu opens:
- **Up/Down Arrow**: Navigate between items (skips separators)
- **Letter Keys**: Jump to next item starting with that letter
- **Enter**: Execute selected item
- **Escape**: Close menu without executing

## More Information

See `CUSTOM_FUNCTIONS.md` for complete documentation on custom functions and menu files.
