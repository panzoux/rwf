# Directory Color Configuration

TWF now supports configurable colors for directory entries in the file panes!

## Configuration

Add the following properties to your `config.json` file under the `Display` section:

```json
{
  "Display": {
    "ForegroundColor": "White",
    "BackgroundColor": "Black",
    "DirectoryColor": "BrightCyan",
    "DirectoryBackgroundColor": "Black"
  }
}
```

### Color Properties

- **`ForegroundColor`**: Default text color for files (default: `White`)
- **`BackgroundColor`**: Background color for the pane, including empty areas (default: `Black`)
- **`DirectoryColor`**: Text color for directory entries (default: `BrightCyan`)
- **`DirectoryBackgroundColor`**: Background color for directory entries (default: `Black`)

## Available Colors

Terminal.Gui supports the following colors:

- `Black`
- `Blue`
- `Green`
- `Cyan`
- `Red`
- `Magenta`
- `Brown` (also accepts `Yellow`)
- `Gray`
- `DarkGray`
- `BrightBlue`
- `BrightGreen`
- `BrightCyan`
- `BrightRed`
- `BrightMagenta`
- `White`

## Example Configurations

### Classic Look (Bright Cyan)
```json
"DirectoryColor": "BrightCyan",
"DirectoryBackgroundColor": "Black"
```

### High Visibility (Yellow)
```json
"DirectoryColor": "Yellow",
"DirectoryBackgroundColor": "Black"
```

### Traditional Unix Style (Green)
```json
"DirectoryColor": "BrightGreen",
"DirectoryBackgroundColor": "Black"
```

### Bold White
```json
"DirectoryColor": "White",
"DirectoryBackgroundColor": "DarkGray"
```

### Subtle Gray
```json
"DirectoryColor": "Gray",
"DirectoryBackgroundColor": "Black"
```

## Configuration File Location

The configuration file should be placed at:
- Windows: `%APPDATA%\TWF\config.json`
- Linux/Mac: `~/.config/TWF/config.json` (or `$XDG_CONFIG_HOME/TWF/config.json`)

If the file doesn't exist, TWF will create it with default values on first run.

## Default Values

If not specified, directories will be displayed in `BrightCyan` on `Black` background.

## Notes

- **Background Color**: The `BackgroundColor` setting controls the background of the entire pane, including empty areas where there are no files
- **Directory colors** only apply to non-selected entries
- When a directory is selected (cursor is on it), it will use the standard cursor highlight colors
- Marked directories will still show the mark indicator (`*`) but will use the directory colors
- The feature works on both Windows and Linux

## Troubleshooting

**Q: The blank area in my pane has the wrong background color**  
A: Set the `BackgroundColor` property in the `Display` section to your desired color (e.g., `"Black"`)

**Q: My directories aren't showing in color**  
A: Make sure you've set both `DirectoryColor` and `DirectoryBackgroundColor` in your config.json file

**Q: Colors look different than expected**  
A: Terminal.Gui color rendering depends on your terminal emulator. Some terminals may display colors differently.
