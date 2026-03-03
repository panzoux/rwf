# Custom Functions with Macro Support

TWF now supports custom user-defined functions with powerful macro expansion! Execute external commands with context from the file manager.

## Quick Start

### Method 1: Direct Keybinding (Fastest)

1. Define a function in `custom_functions.json`
2. Bind a key to it in `keybindings.json` using the function name
3. Press the key to execute instantly!

### Method 2: Function Dialog

1. Press `Shift+T` to open the custom functions dialog
2. Select a function from the list
3. The command will be executed with macros expanded

## Complete Example

**custom_functions.json:**
```json
{
  "version": "1.0",
  "functions": [
    {
      "name": "Open in Notepad",
      "command": "notepad \"$P\\$F\"",
      "description": "Open current file in Notepad"
    },
    {
      "name": "Open in VS Code",
      "command": "code \"$P\"",
      "description": "Open current directory in VS Code"
    }
  ]
}
```

**keybindings.json:**
```json
{
  "bindings": {
    ":": "Open in Notepad",
    "Shift+N": "Open in VS Code",
    "Shift+F": "ShowCustomFunctionsDialog"
  }
}
```

Now pressing `:` opens the current file in Notepad instantly!

## Configuration File

Custom functions are defined in `custom_functions.json`:

**Location:**
- Windows: `%APPDATA%\TWF\custom_functions.json`
- Linux/Mac: `~/.config/TWF/custom_functions.json`

**Format:**
```json
{
  "version": "1.0",
  "functions": [
    {
      "name": "Open in Notepad",
      "command": "notepad \"$P\\$F\"",
      "description": "Open current file in Notepad"
    },
    {
      "name": "Copy to Other Pane",
      "command": "cmd /c copy \"$P\\$F\" \"$O\\\"",
      "description": "Copy current file to other pane"
    },
    {
      "name": "Echo Current Directory Linux",
      "command": "echo $P",
      "shell": "/bin/sh",
      "description": "Echo current directory using specific shell"
    }
  ]
}
```

### Function Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Unique name for the function |
| `command` | string | Yes* | Command to execute (required unless `menu` is specified) |
| `menu` | string | Yes* | Menu file to display (required unless `command` is specified) |
| `description` | string | No | Description shown in function list |
| `pipeToAction` | string | No | Action to pipe command output to |
| `shell` | string | No | Shell to use for executing the command |

**Note:** Either `command` or `menu` must be specified, but not both.

### Shell Property

The `shell` property allows you to specify which shell executable should be used to run the command. This provides cross-platform compatibility and flexibility.

**Examples:**
```json
{
  "name": "Windows Command",
  "command": "echo %P%",
  "shell": "cmd.exe",
  "description": "Use Windows cmd.exe"
}
```

```json
{
  "name": "Linux Command",
  "command": "echo $P",
  "shell": "/bin/bash",
  "description": "Use Linux bash shell"
}
```

```json
{
  "name": "PowerShell Command",
  "command": "Get-Location",
  "shell": "powershell.exe",
  "description": "Use Windows PowerShell"
}
```

**Shell Arguments:**
- For `cmd.exe`: Command is executed with `/c` argument
- For PowerShell (`powershell.exe`, `pwsh.exe`): Command is executed with `-Command` argument
- For Unix shells (`/bin/sh`, `/bin/bash`, etc.): Command is executed with `-c` argument

**Shell Configuration Hierarchy:**
1. Function-specific `shell` property (if specified)
2. OS-specific shell from `config.json` `Shell` section
3. Default fallback based on operating system detection

## Macro Reference

### File/Path Macros

| Macro | Description | Example |
|-------|-------------|---------|
| `$$` | Literal `$` character | `$` |
| `$F` | Current filename | `document.txt` |
| `$W` | Filename without extension | `document` |
| `$E` | File extension | `.txt` |
| `$P` | Active pane path (no trailing `\`) | `C:\Users\John\Documents` |
| `$O` | Other pane path | `D:\Backup` |
| `$L` | Left pane path | `C:\Projects` |
| `$R` | Right pane path | `D:\Data` |

**Lowercase variants** (`$f`, `$w`, `$p`, `$o`, `$l`, `$r`) return short (8.3) filenames when available.

### Sort State Macros

| Macro | Description |
|-------|-------------|
| `$SP` | Active pane sort mode |
| `$SO` | Other pane sort mode |
| `$SL` | Left pane sort mode |
| `$SR` | Right pane sort mode |

### Marked Files Macros

| Macro | Description | Example Output |
|-------|-------------|----------------|
| `$MS` | Marked filenames (space-separated, quoted) | `"file1.txt" "file2.txt"` |
| `$MF` | Marked full paths (space-separated, quoted) | `"C:\path\file1.txt" "C:\path\file2.txt"` |
| `$MO` | Other pane marked files (full paths) | `"D:\other\file.txt"` |
| `$ML` | Left pane marked files | `"C:\left\file.txt"` |
| `$MR` | Right pane marked files | `"D:\right\file.txt"` |

**Notes:**
- If no files are marked, uses current file
- Uppercase `M` (`$MO`, `$ML`, `$MR`): Cancels command if no marked files
- Lowercase `m` (`$mO`, `$mL`, `$mR`): Returns empty string if no marked files
- Lowercase second letter (`$Ms`, `$Mf`, etc.): Returns short filenames (unquoted)

### File Mask Macros

| Macro | Description |
|-------|-------------|
| `$*P` | Active pane file mask |
| `$*O` | Other pane file mask |
| `$*L` | Left pane file mask |
| `$*R` | Right pane file mask |

### Input Dialog Macro

| Macro | Description | Example |
|-------|-------------|---------|
| `$I"prompt"` | Show input dialog | `$I"Enter destination"` |
| `$I5"prompt"` | Input dialog with width (1-9, 0=10) | `$I5"File name"` |

**Example:**
```json
{
  "name": "Custom Copy",
  "command": "cmd /c copy \"$P\\$F\" \"$I\"Destination path\"\"",
  "description": "Copy file to custom location"
}
```

### Environment Variable Macro

| Macro | Description |
|-------|-------------|
| `$V"varname"` | Environment variable value |
| `$V"twf"` | TWF executable directory |
| `$~` | Same as `$V"twf"` |

### Special Macros

| Macro | Description |
|-------|-------------|
| `$#XX` | ASCII character (XX = hex code) |
| `$"` | Literal quote character |

## Example Functions

### Open Command Prompt Here
```json
{
  "name": "Open Command Prompt Here",
  "command": "cmd /k cd /d \"$P\"",
  "description": "Open command prompt in current directory"
}
```

### Batch Rename
```json
{
  "name": "Batch Rename",
  "command": "cmd /c ren \"$P\\$F\" \"$I\"New filename\"\"",
  "description": "Rename current file (prompts for new name)"
}
```

### Copy Marked Files
```json
{
  "name": "Copy Marked to Folder",
  "command": "cmd /c for %f in ($MF) do copy %f \"$I\"Destination folder\"\\\"",
  "description": "Copy all marked files to a custom folder"
}
```

### Open in VS Code
```json
{
  "name": "Open in VS Code",
  "command": "code \"$P\"",
  "description": "Open current directory in Visual Studio Code"
}
```

### Compare Files
```json
{
  "name": "Compare with Other Pane",
  "command": "fc \"$P\\$F\" \"$O\\$F\"",
  "description": "Compare current file with same filename in other pane"
}
```

## Keybindings

### Option 1: Direct Key Binding (Recommended)

Bind a key directly to a custom function by using the function name:

```json
{
  "bindings": {
    ":": "Open in Notepad",
    "Shift+N": "Open in VS Code",
    "Ctrl+C": "Copy to Other Pane"
  }
}
```

The function name must match exactly (case-insensitive) with a function defined in `custom_functions.json`.

### Option 2: Show Function Dialog

Open a dialog to select from all available functions:

```json
{
  "bindings": {
    "Shift+F": "ShowCustomFunctionsDialog"
  }
}
```

## How Direct Keybindings Work

When you press a key, TWF:
1. Looks up the action in `keybindings.json`
2. Checks if it's a built-in action (like `ShowFileMaskDialog`)
3. If not found, searches for a custom function with that name
4. Executes the custom function if found

This means you can bind any custom function to any key!

## Menu Files

TWF supports hierarchical menus that group related custom functions together. Instead of executing a command directly, a custom function can reference a menu file that displays a list of options.

### Creating Menu Files

Menu files are JSON files stored in the same directory as `custom_functions.json`:

**Location:**
- Windows: `%APPDATA%\TWF\`
- Linux/Mac: `~/.config/TWF/`

**Menu File Format:**
```json
{
  "Version": "1.0",
  "Menus": [
    {
      "Name": "Open in Notepad",
      "Function": "Open in Notepad"
    },
    {
      "Name": "-----",
      "Function": ""
    },
    {
      "Name": "View as Text",
      "Action": "ViewFileAsText"
    }
  ]
}
```

### Menu Item Properties

Each menu item can have the following properties:

| Property | Description | Example |
|----------|-------------|---------|
| `Name` | Display name in the menu | `"Open in Notepad"` |
| `Function` | Custom function to execute | `"Open in Notepad"` |
| `Action` | Built-in action to execute | `"ViewFileAsText"` |

**Important:**
- Use `Function` to execute a custom function defined in `custom_functions.json`
- Use `Action` to execute a built-in TWF action (like `ViewFileAsText`, `DeleteFile`, etc.)
- Use `Name: "-----"` with `Function: ""` to create a visual separator
- Menu items with separators or empty `Function`/`Action` properties are non-selectable

### Referencing Menu Files from Custom Functions

To create a menu-type custom function, use the `Menu` property instead of `Command`:

```json
{
  "name": "File Operations",
  "menu": "file_operations.json",
  "description": "Show file operations menu"
}
```

**Path Resolution:**
- Relative paths (e.g., `"file_operations.json"`) are resolved from the config directory
- Absolute paths are used as-is

### Keyboard Navigation in Menus

When a menu dialog opens, you can navigate using:

- **Up/Down Arrow**: Move to previous/next selectable item (skips separators)
- **Letter Keys**: Jump to the next item starting with that letter
- **Enter**: Execute the selected menu item
- **Escape**: Close the menu without executing anything

### Example Menu Files

**file_operations.json** - Common file operations:
```json
{
  "Version": "1.0",
  "Menus": [
    {
      "Name": "Open in Notepad",
      "Function": "Open in Notepad"
    },
    {
      "Name": "Copy to Other Pane",
      "Function": "Copy to Other Pane"
    },
    {
      "Name": "Custom Copy",
      "Function": "Custom Copy"
    },
    {
      "Name": "Batch Rename",
      "Function": "Batch Rename"
    },
    {
      "Name": "-----",
      "Function": ""
    },
    {
      "Name": "Open Command Prompt Here",
      "Function": "Open Command Prompt Here"
    },
    {
      "Name": "-----",
      "Function": ""
    },
    {
      "Name": "Delete File",
      "Action": "DeleteFile"
    },
    {
      "Name": "Move File",
      "Action": "MoveFile"
    }
  ]
}
```

**text_viewer_options.json** - Text viewer actions:
```json
{
  "Version": "1.0",
  "Menus": [
    {
      "Name": "View as Text",
      "Action": "ViewFileAsText"
    },
    {
      "Name": "View as Hex",
      "Action": "ViewFileAsHex"
    },
    {
      "Name": "-----",
      "Function": ""
    },
    {
      "Name": "Edit in External Editor",
      "Action": "EditFile"
    },
    {
      "Name": "-----",
      "Function": ""
    },
    {
      "Name": "Search in File",
      "Action": "SearchInFile"
    }
  ]
}
```

### Using Menu-Type Custom Functions

**In custom_functions.json:**
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

**In keybindings.json:**
```json
{
  "bindings": {
    "F2": "File Operations",
    "F3": "Viewer Options"
  }
}
```

Now pressing `F2` opens the file operations menu, and `F3` opens the viewer options menu!

### Function vs Action Properties

The key difference between `Function` and `Action` properties in menu items:

- **Function**: References a custom function name from `custom_functions.json`
  - Executes user-defined commands with macro expansion
  - Example: `"Function": "Open in Notepad"` executes the custom function that runs `notepad "$P\\$F"`

- **Action**: References a built-in TWF action
  - Executes internal TWF functionality
  - Example: `"Action": "ViewFileAsText"` opens the built-in text viewer
  - Common built-in actions: `ViewFileAsText`, `ViewFileAsHex`, `DeleteFile`, `MoveFile`, `EditFile`, `SearchInFile`

### Separators

Create visual separators to group related menu items:

```json
{
  "Name": "-----",
  "Function": ""
}
```

Separators:
- Are displayed as horizontal lines
- Cannot be selected
- Are automatically skipped during keyboard navigation
- Help organize menu items into logical groups

## Tips

1. **Quote paths**: Always quote paths with spaces: `"$P\\$F"`
2. **Test commands**: Test commands in cmd.exe first before adding to custom functions
3. **Use $I for flexibility**: Input dialogs make functions reusable
4. **Marked files**: Use `$MF` to operate on multiple files at once
5. **Cancel on no marks**: Use uppercase `$MO` to prevent accidental execution
6. **Direct keybindings**: Bind frequently-used functions to keys for quick access
7. **Organize with menus**: Group related functions into menu files for better organization
8. **Use separators**: Add visual separators to group related menu items
9. **Cross-platform functions**: Use the `shell` property to specify different shells for different operating systems
10. **Shell configuration**: Configure default shells per OS in `config.json` under the `Shell` section

## Troubleshooting

**Q: My function doesn't work**  
A: Check that paths are properly quoted and use `\\` for path separators in JSON

**Q: How do I see what command was executed?**  
A: Check the TWF logs for the expanded command

**Q: Can I use PowerShell instead of cmd?**  
A: Yes! Use `powershell -Command "your command here"`

**Q: Input dialog was cancelled, what happens?**  
A: The entire command is cancelled, nothing executes

**Q: My menu file doesn't load**  
A: Check that the menu file is valid JSON and located in the config directory. Check TWF logs for specific errors.

**Q: Menu items aren't selectable**  
A: Ensure menu items have either a `Function` or non-empty `Action` property. Items with `Name: "-----"` are separators and cannot be selected.

**Q: What built-in actions can I use in Action property?**  
A: Common actions include `ViewFileAsText`, `ViewFileAsHex`, `DeleteFile`, `MoveFile`, `EditFile`, `SearchInFile`. Check TWF documentation for a complete list.

**Q: Can I nest menus (menu within a menu)?**
A: Currently, menu files can reference custom functions or built-in actions, but not other menu files directly.

## PipeToAction Feature

### Supported Actions for Piping
You can pipe the output of a custom command to internal actions using the `PipeToAction` property:

- `JumpToPath`: Jumps to the directory provided in the argument.
- `ExecuteFile`: Executes a file using the system's default handler.
- `ExecuteFileWithEditor`: Executes a file using the configured editor.

### ExecuteFile Actions

The `ExecuteFile` and `ExecuteFileWithEditor` actions allow you to execute files returned by commands:

**ExecuteFile Example:**
```json
{
    "Name": "Execute Selected File",
    "Command": "find . -name \"*.txt\" | head -1",
    "PipeToAction": "ExecuteFile",
    "Description": "Find and execute the first .txt file"
}
```

**ExecuteFileWithEditor Example:**
```json
{
    "Name": "Open in Editor",
    "Command": "find . -name \"*.cs\" | head -1",
    "PipeToAction": "ExecuteFileWithEditor",
    "Description": "Find and open the first .cs file in editor"
}
```

## Environment Variable Expansion in Custom Functions

Custom function commands now support environment variable expansion in the following formats:

### Supported Formats
- **Windows batch format**: `%VAR%` (e.g., `%USERPROFILE%\Documents`)
- **Unix shell format**: `$VAR` (e.g., `$HOME/Documents`)
- **Curly brace format**: `${VAR}` (e.g., `${USERPROFILE}/Documents`)
- **PowerShell format**: `$env:VAR` (e.g., `$env:USERPROFILE\Documents`)

### Example Custom Function with Environment Variables
```json
{
    "Name": "List Home Directory",
    "Command": "ls $HOME",
    "Description": "List contents of home directory using environment variable"
}
```

```json
{
    "Name": "Open Temp File",
    "Command": "notepad %TEMP%\\temp.txt",
    "PipeToAction": "ExecuteFile",
    "Description": "Open temporary file using Windows environment variable"
}
```

The environment variable expansion happens before command execution, allowing for flexible and cross-platform command definitions.
