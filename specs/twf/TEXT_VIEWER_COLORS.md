# Text Viewer Color Configuration

TWF now supports configurable colors for the text viewer window!

## Configuration

Add the following properties to your `config.json` file under the `Viewer` section:

```json
{
  "Viewer": {
    "TextViewerForegroundColor": "White",
    "TextViewerBackgroundColor": "Black",
    "TextViewerStatusForegroundColor": "White",
    "TextViewerStatusBackgroundColor": "Gray",
    "TextViewerEncodingForegroundColor": "White",
    "TextViewerEncodingBackgroundColor": "Blue"
  }
}
```

## Color Properties

- **`TextViewerForegroundColor`**: Text color for file content (default: `White`)
- **`TextViewerBackgroundColor`**: Background color for file content area (default: `Black`)
- **`TextViewerStatusForegroundColor`**: Text color for status bar (file info) (default: `White`)
- **`TextViewerStatusBackgroundColor`**: Background color for status bar (default: `Gray`)
- **`TextViewerEncodingForegroundColor`**: Text color for encoding/help bar (default: `White`)
- **`TextViewerEncodingBackgroundColor`**: Background color for encoding/help bar (default: `Blue`)

## Available Colors

Terminal.Gui supports the following colors:

- `Black`, `Blue`, `Green`, `Cyan`, `Red`, `Magenta`, `Brown`, `Gray`
- `DarkGray`, `BrightBlue`, `BrightGreen`, `BrightCyan`, `BrightRed`, `BrightMagenta`, `White`
- `Yellow` (mapped to `Brown`)

## Example Configurations

### Classic Look (White on Black)
```json
"TextViewerForegroundColor": "White",
"TextViewerBackgroundColor": "Black"
```

### High Contrast (Yellow on Blue)
```json
"TextViewerForegroundColor": "Yellow",
"TextViewerBackgroundColor": "Blue"
```

### Green Terminal Style
```json
"TextViewerForegroundColor": "BrightGreen",
"TextViewerBackgroundColor": "Black"
```

### Amber Monitor Style
```json
"TextViewerForegroundColor": "Yellow",
"TextViewerBackgroundColor": "Black"
```

## Configuration File Location

The configuration file should be placed at:
- Windows: `%APPDATA%\TWF\config.json`
- Linux/Mac: `~/.config/TWF/config.json` (or `$XDG_CONFIG_HOME/TWF/config.json`)

If the file doesn't exist, TWF will create it with default values on first run.

## Notes

- Colors apply to the entire text viewer window
- The status bar shows file information (name, line count, encoding)
- The encoding bar shows keyboard shortcuts and current encoding
- All three areas can be customized independently
- Changes take effect the next time you open the text viewer
