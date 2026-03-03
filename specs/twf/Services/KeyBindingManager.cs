using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Extensions.Logging;
using Terminal.Gui;
using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    /// Manages key bindings and maps key events to actions based on configuration
    /// Supports loading custom key bindings from JSON format files
    /// </summary>
    public class KeyBindingManager
    {
        private readonly ILogger<KeyBindingManager>? _logger;
        private Dictionary<string, string> _keyBindings;
        private Dictionary<int, ActionBinding> _normalModeBindings;
        private Dictionary<int, ActionBinding> _viewerModeBindings;
        private Dictionary<int, ActionBinding> _textViewerModeBindings;
        private bool _isEnabled;

        public KeyBindingManager(ILogger<KeyBindingManager>? logger = null)
        {
            _logger = logger;
            _keyBindings = new Dictionary<string, string>();
            _normalModeBindings = new Dictionary<int, ActionBinding>();
            _viewerModeBindings = new Dictionary<int, ActionBinding>();
            _textViewerModeBindings = new Dictionary<int, ActionBinding>();
            _isEnabled = false;
        }

        /// <summary>
        /// Loads key bindings from a configuration file (JSON or legacy AFXW.KEY format)
        /// If file not found, loads default bindings
        /// </summary>
        /// <param name="configPath">Path to the key binding configuration file</param>
        public void LoadBindings(string configPath)
        {
            if (!File.Exists(configPath))
            {
                _logger?.LogWarning("Key binding file not found: {ConfigPath}, loading defaults", configPath);
                LoadDefaultBindings();
                return;
            }

            try
            {
                string content = File.ReadAllText(configPath, Encoding.UTF8);
                
                // Determine format based on file extension or content
                if (configPath.EndsWith(".json", StringComparison.OrdinalIgnoreCase))
                {
                    LoadJsonBindings(content);
                }
                else
                {
                    // Legacy AFXW.KEY format
                    ParseBindings(content);
                }
                
                _logger?.LogInformation("Successfully loaded key bindings from {ConfigPath}", configPath);
            }
            catch (Exception ex)
            {
                var errorMsg = $"Failed to load key bindings from {configPath}";
                _logger?.LogError(ex, errorMsg);
                
                if (Application.Top != null)
                {
                    Application.MainLoop.Invoke(() => {
                        MessageBox.ErrorQuery("Key Bindings Error", $"{errorMsg}\n{ex.Message}", "OK");
                    });
                }
                
                LoadDefaultBindings();
            }
        }

        /// <summary>
        /// Loads default key bindings (hardcoded fallback)
        /// </summary>
        private void LoadDefaultBindings()
        {
            _keyBindings = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
            {
                { "F", "EnterSearchMode" },
                { "Tab", "SwitchPane" },
                { "Enter", "HandleEnterKey" },
                { "Shift+Enter", "HandleShiftEnter" },
                { "Ctrl+Enter", "HandleCtrlEnter" },
                { "Backspace", "NavigateToParent" },
                { "Home", "InvertMarks" },
                { "Ctrl+Home", "NavigateToRoot" },
                { "Up", "MoveCursorUp" },
                { "Down", "MoveCursorDown" },
                { "Left", "SwitchToLeftPane" },
                { "Right", "SwitchToRightPane" },
                { "PageUp", "PageUp" },
                { "PageDown", "PageDown" },
                { "Ctrl+PageUp", "MoveCursorToFirst" },
                { "Ctrl+PageDown", "MoveCursorToLast" },
                { "End", "ClearMarks" },
                { "Space", "ToggleMarkAndMoveDown" },
                { "Shift+Space", "ToggleMarkAndMoveUp" },
                { "Ctrl+Space", "MarkRange" },
                { "1", "DisplayMode1" },
                { "2", "DisplayMode2" },
                { "3", "DisplayMode3" },
                { "4", "DisplayMode4" },
                { "5", "DisplayMode5" },
                { "6", "DisplayMode6" },
                { "7", "DisplayMode7" },
                { "8", "DisplayMode8" },
                { "0", "DisplayModeDetailed" },
                { "@", "ShowWildcardMarkingDialog" },
                { "`", "HandleContextMenu" },
                { "C", "HandleCopyOperation" },
                { "M", "HandleMoveOperation" },
                { "Shift+M", "MoveToRegisteredFolder" },
                { "D", "HandleDeleteOperation" },
                { "K", "HandleCreateDirectory" },
                { "L", "ShowDriveChangeDialog" },
                { "S", "CycleSortMode" },
                { "Shift+S", "ShowSortDialog" },
                { ":", "ShowFileMaskDialog" },
                { "P", "HandleCompressionOperation" },
                { "I", "ShowRegisteredFolderDialog" },
                { "Shift+B", "RegisterCurrentDirectory" },
                { "Shift+R", "HandlePatternRename" },
                { "W", "HandleFileComparison" },
                { "Shift+W", "HandleFileSplitOrJoin" },
                { "Y", "HandleLaunchConfigurationProgram" },
                { "Z", "HandleLaunchConfigurationProgram" },
                { "H", "ShowFileInfoForCursor" },
                { "Ctrl+Shift+V", "ShowVersion" },
                { "G", "ShowRegisteredFolderDialog" },
                { "O", "HandleArchiveExtraction" },
                { "V", "ViewFileAsText" },
                { "A", "MarkAll" },
                { "E", "SyncPanes" },
                { "Ctrl+B", "ShowTabSelector" },
                { "Escape", "ExitApplication" }
            };
            _isEnabled = true;
            _logger?.LogInformation("Loaded default key bindings");
        }

        /// <summary>
        /// Loads key bindings from JSON format
        /// </summary>
        private void LoadJsonBindings(string jsonContent)
        {
            try
            {
                var config = JsonSerializer.Deserialize<KeyBindingConfig>(jsonContent, new JsonSerializerOptions
                {
                    PropertyNameCaseInsensitive = true
                });

                if (config?.Bindings == null)
                {
                    _logger?.LogWarning("Invalid key binding configuration - missing or null 'bindings' section");
                    return;
                }

                _keyBindings = new Dictionary<string, string>(config.Bindings, StringComparer.OrdinalIgnoreCase);
                _isEnabled = true;
                
                _logger?.LogInformation("Loaded {Count} key bindings from JSON", _keyBindings.Count);

                // Load text viewer bindings if present
                if (config.TextViewerBindings != null && config.TextViewerBindings.Count > 0)
                {
                    LoadTextViewerBindings(config.TextViewerBindings);
                }
                else
                {
                    _logger?.LogInformation("No textViewerBindings section found in configuration, text viewer will fall back to default bindings");
                }

                // Load image viewer bindings if present
                if (config.ImageViewerBindings != null && config.ImageViewerBindings.Count > 0)
                {
                    LoadImageViewerBindings(config.ImageViewerBindings);
                }
                else
                {
                    _logger?.LogInformation("No imageViewerBindings section found in configuration, image viewer will fall back to default bindings");
                }
            }
            catch (JsonException jsonEx)
            {
                var errorMsg = $"JSON syntax error in key bindings file:\n{jsonEx.Message}";
                _logger?.LogError(jsonEx, errorMsg);
                
                if (Application.Top != null)
                {
                    Application.MainLoop.Invoke(() => {
                        MessageBox.ErrorQuery("Key Bindings Syntax Error", errorMsg, "OK");
                    });
                }
                throw;
            }
        }

        /// <summary>
        /// Loads text viewer specific key bindings
        /// </summary>
        private void LoadTextViewerBindings(Dictionary<string, string> textViewerBindings)
        {
            // Ensure _keyBindings is initialized if it's not already
            if (_keyBindings == null)
            {
                _keyBindings = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            }

            int validBindingsCount = 0;
            int invalidBindingsCount = 0;

            // Store text viewer bindings in a separate dictionary for mode-specific lookup
            // This allows the text viewer to have different bindings than normal mode
            foreach (var kvp in textViewerBindings)
            {
                // Validate that the action name starts with "TextViewer."
                if (!kvp.Value.StartsWith("TextViewer.", StringComparison.OrdinalIgnoreCase))
                {
                    _logger?.LogWarning("Invalid TextViewer action name '{Action}' for key '{Key}'. TextViewer actions must start with 'TextViewer.' - binding ignored", kvp.Value, kvp.Key);
                    invalidBindingsCount++;
                    continue;
                }

                // Validate that the action name is recognized
                if (!IsValidTextViewerAction(kvp.Value))
                {
                    _logger?.LogWarning("Unknown TextViewer action name '{Action}' for key '{Key}' - binding ignored. Valid actions: GoToTop, GoToBottom, GoToFileTop, GoToFileBottom, GoToLineStart, GoToLineEnd, PageUp, PageDown, Close, Search, FindNext, FindPrevious, CycleEncoding", kvp.Value, kvp.Key);
                    invalidBindingsCount++;
                    continue;
                }

                // Store in a way that can be retrieved by GetActionForKey with mode parameter
                // We'll use a naming convention: the key will be stored with mode prefix
                string modeKey = $"TextViewer:{kvp.Key}";
                _keyBindings[modeKey] = kvp.Value;
                validBindingsCount++;
            }

            if (invalidBindingsCount > 0)
            {
                _logger?.LogWarning("Loaded {ValidCount} valid TextViewer bindings, ignored {InvalidCount} invalid bindings", validBindingsCount, invalidBindingsCount);
            }
        }

        /// <summary>
        /// Loads image viewer specific key bindings
        /// </summary>
        private void LoadImageViewerBindings(Dictionary<string, string> imageViewerBindings)
        {
            // Ensure _keyBindings is initialized if it's not already
            if (_keyBindings == null)
            {
                _keyBindings = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            }

            int validBindingsCount = 0;
            int invalidBindingsCount = 0;

            foreach (var kvp in imageViewerBindings)
            {
                // Validate that the action name starts with "ImageViewer."
                if (!kvp.Value.StartsWith("ImageViewer.", StringComparison.OrdinalIgnoreCase))
                {
                    _logger?.LogWarning("Invalid ImageViewer action name '{Action}' for key '{Key}'. ImageViewer actions must start with 'ImageViewer.' - binding ignored", kvp.Value, kvp.Key);
                    invalidBindingsCount++;
                    continue;
                }

                // Validate that the action name is recognized
                if (!IsValidImageViewerAction(kvp.Value))
                {
                    _logger?.LogWarning("Unknown ImageViewer action name '{Action}' for key '{Key}' - binding ignored. Valid actions: OriginalSize, FitToWindow, RotateClockwise, FlipHorizontal, FlipVertical, ConsoleView, ScrollUp, ScrollDown, ScrollLeft, ScrollRight, Close", kvp.Value, kvp.Key);
                    invalidBindingsCount++;
                    continue;
                }

                string modeKey = $"ImageViewer:{kvp.Key}";
                _keyBindings[modeKey] = kvp.Value;
                validBindingsCount++;
            }

            if (invalidBindingsCount > 0)
            {
                _logger?.LogWarning("Loaded {ValidCount} valid ImageViewer bindings, ignored {InvalidCount} invalid bindings", validBindingsCount, invalidBindingsCount);
            }
        }

        /// <summary>
        /// Validates if a ImageViewer action name is recognized
        /// </summary>
        private bool IsValidImageViewerAction(string actionName)
        {
            var validActions = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            {
                "ImageViewer.OriginalSize",
                "ImageViewer.FitToWindow",
                "ImageViewer.RotateClockwise",
                "ImageViewer.FlipHorizontal",
                "ImageViewer.FlipVertical",
                "ImageViewer.ConsoleView",
                "ImageViewer.ScrollUp",
                "ImageViewer.ScrollDown",
                "ImageViewer.ScrollLeft",
                "ImageViewer.ScrollRight",
                "ImageViewer.Close"
            };

            return validActions.Contains(actionName);
        }

        /// <summary>
        /// Validates if a TextViewer action name is recognized
        /// </summary>
        private bool IsValidTextViewerAction(string actionName)
        {
            var validActions = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            {
                "TextViewer.GoToTop",
                "TextViewer.GoToBottom",
                "TextViewer.GoToFileTop",
                "TextViewer.GoToFileBottom",
                "TextViewer.GoToLineStart",
                "TextViewer.GoToLineEnd",
                "TextViewer.PageUp",
                "TextViewer.PageDown",
                "TextViewer.Close",
                "TextViewer.Search",
                "TextViewer.StartForwardSearch",
                "TextViewer.StartBackwardSearch",
                "TextViewer.FindNext",
                "TextViewer.FindPrevious",
                "TextViewer.CycleEncoding",
                "TextViewer.ToggleHexMode",
                "TextViewer.ClearHighlight"
            };

            return validActions.Contains(actionName);
        }

        /// <summary>
        /// Parses the key binding configuration content
        /// </summary>
        private void ParseBindings(string content)
        {
            var lines = content.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries);
            UiMode? currentMode = null;

            foreach (var line in lines)
            {
                var trimmedLine = line.Trim();

                // Skip empty lines and comments
                if (string.IsNullOrWhiteSpace(trimmedLine) || trimmedLine.StartsWith(";"))
                    continue;

                // Check for [KEYCUST] section and ON=1
                if (trimmedLine.Equals("[KEYCUST]", StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }

                if (trimmedLine.StartsWith("ON=", StringComparison.OrdinalIgnoreCase))
                {
                    _isEnabled = trimmedLine.Substring(3).Trim() == "1";
                    continue;
                }

                // Check for mode sections
                if (trimmedLine.Equals("[NORMAL]", StringComparison.OrdinalIgnoreCase))
                {
                    currentMode = UiMode.Normal;
                    continue;
                }
                else if (trimmedLine.Equals("[GVIEW]", StringComparison.OrdinalIgnoreCase))
                {
                    currentMode = UiMode.ImageViewer;
                    continue;
                }
                else if (trimmedLine.Equals("[TVIEW]", StringComparison.OrdinalIgnoreCase))
                {
                    currentMode = UiMode.TextViewer;
                    continue;
                }

                // Parse key binding line: K0000="0065:0038" or K0001="0066notepad "$P\$F""
                if (currentMode.HasValue && trimmedLine.StartsWith("K", StringComparison.OrdinalIgnoreCase))
                {
                    ParseKeyBindingLine(trimmedLine, currentMode.Value);
                }
            }
        }

        /// <summary>
        /// Parses a single key binding line
        /// Format: K0000="0065:0038" (key redirect) or K0001="0066notepad "$P\$F"" (command)
        /// </summary>
        private void ParseKeyBindingLine(string line, UiMode mode)
        {
            // Match pattern: K#### = "value"
            var match = Regex.Match(line, @"K(\d+)\s*=\s*""(.+?)""", RegexOptions.IgnoreCase);
            if (!match.Success)
                return;

            try
            {
                string keyIndexStr = match.Groups[1].Value;
                string value = match.Groups[2].Value;

                // The key index is not used in our implementation
                // We extract the actual key code from the value

                // Check if it's a key redirect (contains colon)
                if (value.Contains(":"))
                {
                    // Format: "0065:0038" - redirect key 0065 to key 0038
                    var parts = value.Split(':');
                    if (parts.Length == 2)
                    {
                        int sourceKey = ParseKeyCode(parts[0]);
                        int targetKey = ParseKeyCode(parts[1]);

                        var binding = new ActionBinding
                        {
                            Type = ActionType.KeyRedirect,
                            Target = targetKey.ToString()
                        };

                        SetBinding(sourceKey, binding, mode);
                    }
                }
                else
                {
                    // Format: "0066notepad "$P\$F"" - execute command
                    // First 4 digits are the key code, rest is the command
                    if (value.Length >= 4)
                    {
                        int keyCode = ParseKeyCode(value.Substring(0, 4));
                        string command = value.Substring(4);

                        var binding = new ActionBinding
                        {
                            Type = ActionType.Command,
                            Target = command
                        };

                        SetBinding(keyCode, binding, mode);
                    }
                }
            }
            catch (Exception ex)
            {
                _logger?.LogWarning(ex, "Failed to parse key binding line: {Line}", line);
            }
        }

        /// <summary>
        /// Parses a key code string in the format "0065" or "2065" (with modifiers)
        /// First digit(s) represent modifiers: 0=none, 1=SHIFT, 2=CTRL, 3=SHIFT+CTRL, 4=ALT, etc.
        /// Last 3 digits represent the virtual key code
        /// </summary>
        private int ParseKeyCode(string keyCodeStr)
        {
            if (string.IsNullOrWhiteSpace(keyCodeStr))
                return 0;

            // Parse as integer - the full code includes modifiers
            if (int.TryParse(keyCodeStr.Trim(), out int keyCode))
            {
                return keyCode;
            }

            return 0;
        }

        /// <summary>
        /// Gets the key binding for a specific key code and UI mode
        /// </summary>
        /// <param name="keyCode">The key code (including modifiers)</param>
        /// <param name="mode">The current UI mode</param>
        /// <returns>The action binding, or null if no binding exists</returns>
        public ActionBinding? GetBinding(int keyCode, UiMode mode)
        {
            if (!_isEnabled)
                return null;

            var bindings = GetBindingsForMode(mode);
            
            if (bindings.TryGetValue(keyCode, out var binding))
            {
                // If it's a key redirect, resolve it recursively (but only once to avoid loops)
                if (binding.Type == ActionType.KeyRedirect)
                {
                    if (int.TryParse(binding.Target, out int targetKey))
                    {
                        if (bindings.TryGetValue(targetKey, out var targetBinding))
                        {
                            return targetBinding;
                        }
                    }
                }
                
                return binding;
            }

            return null;
        }

        /// <summary>
        /// Sets a key binding for a specific key code and UI mode
        /// </summary>
        /// <param name="keyCode">The key code (including modifiers)</param>
        /// <param name="action">The action binding</param>
        /// <param name="mode">The UI mode</param>
        public void SetBinding(int keyCode, ActionBinding action, UiMode mode)
        {
            var bindings = GetBindingsForMode(mode);
            bindings[keyCode] = action;
        }

        /// <summary>
        /// Gets the binding dictionary for a specific UI mode
        /// </summary>
        private Dictionary<int, ActionBinding> GetBindingsForMode(UiMode mode)
        {
            return mode switch
            {
                UiMode.Normal => _normalModeBindings,
                UiMode.ImageViewer => _viewerModeBindings,
                UiMode.TextViewer => _textViewerModeBindings,
                _ => _normalModeBindings
            };
        }

        /// <summary>
        /// Gets the action name for a key string (e.g., "Ctrl+C" -> "HandleCopyOperation")
        /// </summary>
        /// <param name="keyString">The key string (e.g., "C", "Ctrl+C", "Shift+Enter")</param>
        /// <returns>The action name, or null if no binding exists</returns>
        public string? GetActionForKey(string keyString)
        {
            if (!_isEnabled || _keyBindings == null)
                return null;

            if (_keyBindings.TryGetValue(keyString, out var action))
            {
                return action;
            }

            return null;
        }

        /// <summary>
        /// Gets the action name for a key string in a specific UI mode
        /// </summary>
        /// <param name="keyString">The key string (e.g., "C", "Ctrl+C", "Shift+Enter")</param>
        /// <param name="mode">The UI mode to check bindings for</param>
        /// <returns>The action name, or null if no binding exists</returns>
        public string? GetActionForKey(string keyString, UiMode mode)
        {
            if (!_isEnabled || _keyBindings == null)
                return null;

            // For TextViewer mode, check mode-specific bindings first
            if (mode == UiMode.TextViewer)
            {
                string modeKey = $"TextViewer:{keyString}";
                if (_keyBindings.TryGetValue(modeKey, out var modeAction))
                {
                    return modeAction;
                }
                
                // Fall back to default bindings if no mode-specific binding exists
                return null;
            }

            // For ImageViewer mode, check mode-specific bindings first
            if (mode == UiMode.ImageViewer)
            {
                string modeKey = $"ImageViewer:{keyString}";
                if (_keyBindings.TryGetValue(modeKey, out var modeAction))
                {
                    return modeAction;
                }
                
                // Fall back to default bindings if no mode-specific binding exists
                return null;
            }

        // For other modes, use the standard lookup
        return GetActionForKey(keyString);
    }

    /// <summary>
    /// Gets all keys bound to a specific action
    /// </summary>
    public List<string> GetKeysForAction(string actionName, UiMode mode = UiMode.Normal)
    {
        var result = new List<string>();
        if (_keyBindings == null) return result;

        string prefix = mode switch
        {
            UiMode.TextViewer => "TextViewer:",
            UiMode.ImageViewer => "ImageViewer:",
            _ => ""
        };

        foreach (var kvp in _keyBindings)
        {
            if (kvp.Value.Equals(actionName, StringComparison.OrdinalIgnoreCase))
            {
                // If it's a mode-specific binding, check if the prefix matches
                if (!string.IsNullOrEmpty(prefix))
                {
                    if (kvp.Key.StartsWith(prefix))
                    {
                        result.Add(kvp.Key.Substring(prefix.Length));
                    }
                }
                else
                {
                    // For Normal mode, only include keys without prefixes
                    if (!kvp.Key.Contains(":"))
                    {
                        result.Add(kvp.Key);
                    }
                }
            }
        }

        return result;
    }

        /// <summary>
        /// Checks if custom key bindings are enabled
        /// </summary>
        public bool IsEnabled => _isEnabled;

        /// <summary>
        /// Enables or disables custom key bindings
        /// </summary>
        public void SetEnabled(bool enabled)
        {
            _isEnabled = enabled;
        }

        /// <summary>
        /// Clears all key bindings for a specific mode
        /// </summary>
        public void ClearBindings(UiMode mode)
        {
            var bindings = GetBindingsForMode(mode);
            bindings.Clear();
        }

        /// <summary>
        /// Clears all key bindings for all modes
        /// </summary>
        public void ClearAllBindings()
        {
            _normalModeBindings.Clear();
            _viewerModeBindings.Clear();
            _textViewerModeBindings.Clear();
        }

        /// <summary>
        /// Gets the count of bindings for a specific mode
        /// </summary>
        public int GetBindingCount(UiMode mode)
        {
            return GetBindingsForMode(mode).Count;
        }

        /// <summary>
        /// Encodes a key code with modifiers
        /// </summary>
        /// <param name="virtualKeyCode">The virtual key code (0-255)</param>
        /// <param name="shift">Whether SHIFT is pressed</param>
        /// <param name="ctrl">Whether CTRL is pressed</param>
        /// <param name="alt">Whether ALT is pressed</param>
        /// <returns>The encoded key code</returns>
        public static int EncodeKeyCode(int virtualKeyCode, bool shift = false, bool ctrl = false, bool alt = false)
        {
            int modifier = 0;
            if (shift) modifier += 1;
            if (ctrl) modifier += 2;
            if (alt) modifier += 4;

            return (modifier * 1000) + virtualKeyCode;
        }

        /// <summary>
        /// Decodes a key code into its components
        /// </summary>
        /// <param name="keyCode">The encoded key code</param>
        /// <returns>Tuple of (virtualKeyCode, shift, ctrl, alt)</returns>
        public static (int virtualKeyCode, bool shift, bool ctrl, bool alt) DecodeKeyCode(int keyCode)
        {
            int modifier = keyCode / 1000;
            int virtualKeyCode = keyCode % 1000;

            bool shift = (modifier & 1) != 0;
            bool ctrl = (modifier & 2) != 0;
            bool alt = (modifier & 4) != 0;

            return (virtualKeyCode, shift, ctrl, alt);
        }
    }
}
