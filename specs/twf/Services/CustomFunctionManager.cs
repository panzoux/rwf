using System.Text.Json;
using TWF.Models;
using TWF.UI;
using Microsoft.Extensions.Logging;
using Terminal.Gui;

namespace TWF.Services
{
    /// <summary>
    /// Manages custom user-defined functions with macro expansion
    /// </summary>
    public class CustomFunctionManager
    {
        private readonly MacroExpander _macroExpander;
        private readonly TWF.Providers.ConfigurationProvider? _configProvider;
        private readonly ILogger<CustomFunctionManager>? _logger;
        private CustomFunctionsConfig? _config;
        private MenuManager? _menuManager;
        private Func<string, bool>? _builtInActionExecutor;
        private Func<string, string, bool>? _builtInActionExecutorWithArg;

        public CustomFunctionManager(MacroExpander macroExpander, TWF.Providers.ConfigurationProvider? configProvider = null, ILogger<CustomFunctionManager>? logger = null)
        {
            _macroExpander = macroExpander ?? throw new ArgumentNullException(nameof(macroExpander));
            _configProvider = configProvider;
            _logger = logger;
        }

        /// <summary>
        /// Sets the MenuManager for handling menu-type custom functions
        /// </summary>
        /// <param name="menuManager">The MenuManager instance</param>
        public void SetMenuManager(MenuManager menuManager)
        {
            _menuManager = menuManager;
        }

        /// <summary>
        /// Sets the callback for executing built-in actions from menu items
        /// </summary>
        /// <param name="executor">Function that executes a built-in action by name</param>
        public void SetBuiltInActionExecutor(Func<string, bool> executor)
        {
            _builtInActionExecutor = executor;
        }

        /// <summary>
        /// Sets the callback for executing built-in actions with arguments
        /// </summary>
        public void SetBuiltInActionExecutorWithArg(Func<string, string, bool> executor)
        {
            _builtInActionExecutorWithArg = executor;
        }

        /// <summary>
        /// Loads custom functions from a JSON file
        /// </summary>
        /// <param name="configPath">Path to custom_functions.json</param>
        public void LoadFunctions(string configPath)
        {
            try
            {
                if (!File.Exists(configPath))
                {
                    _logger?.LogInformation("Custom functions file not found: {Path}, creating default", configPath);
                    CreateDefaultFunctionsFile(configPath);
                    LoadFunctions(configPath);
                    return;
                }

                var json = File.ReadAllText(configPath);
                
                try 
                {
                    _config = JsonSerializer.Deserialize<CustomFunctionsConfig>(json, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });

                    if (_config == null || _config.Functions == null)
                    {
                        throw new JsonException("Failed to deserialize custom functions configuration.");
                    }

                    _logger?.LogInformation("Loaded {Count} custom functions from {Path}", _config.Functions.Count, configPath);
                }
                catch (JsonException jsonEx)
                {
                    var errorMsg = $"Error parsing custom_functions.json:\n{jsonEx.Message}";
                    _logger?.LogError(jsonEx, errorMsg);
                    
                    // Show error to user since this is likely a configuration mistake
                    Application.MainLoop.Invoke(() => {
                        MessageBox.ErrorQuery("Custom Functions Error", errorMsg, "OK");
                    });
                    
                    _config = new CustomFunctionsConfig();
                }
            }
            catch (Exception ex)
            {
                var errorMsg = $"Failed to load custom functions from {configPath}";
                _logger?.LogError(ex, errorMsg);
                
                Application.MainLoop.Invoke(() => {
                    MessageBox.ErrorQuery("Error", $"{errorMsg}\n{ex.Message}", "OK");
                });
                
                _config = new CustomFunctionsConfig();
            }
        }

        /// <summary>
        /// Gets all available custom functions
        /// </summary>
        public List<CustomFunction> GetFunctions()
        {
            return _config?.Functions ?? new List<CustomFunction>();
        }

        /// <summary>
        /// Executes a custom function with macro expansion
        /// </summary>
        /// <param name="function">The function to execute</param>
        /// <param name="activePane">Active pane state</param>
        /// <param name="inactivePane">Inactive pane state</param>
        /// <param name="leftPane">Left pane state</param>
        /// <param name="rightPane">Right pane state</param>
        /// <returns>True if executed successfully, false if cancelled or failed</returns>
        public bool ExecuteFunction(CustomFunction function, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            try
            {
                _logger?.LogInformation("Executing custom function: {Name}", function.Name);

                // Check if this is a menu-type function
                if (function.IsMenuType)
                {
                    return ExecuteMenuFunction(function, activePane, inactivePane, leftPane, rightPane);
                }

                // Expand macros
                var expandedCommand = _macroExpander.ExpandMacros(function.Command, activePane, inactivePane, leftPane, rightPane);
                
                if (expandedCommand == null)
                {
                    _logger?.LogInformation("Custom function cancelled by user");
                    return false;
                }

                _logger?.LogDebug("Expanded command: {Command}", expandedCommand);

                // Environment variables are now expanded within MacroExpander
                
                // Determine the appropriate shell based on the function's shell setting or configuration defaults
                string shellExe;
                string shellArgs;

                if (!string.IsNullOrEmpty(function.Shell))
                {
                    // Use the shell specified in the function configuration
                    shellExe = TWF.Utilities.EnvironmentVariableExpander.ExpandEnvironmentVariables(function.Shell);
                    if (shellExe.EndsWith("cmd.exe", StringComparison.OrdinalIgnoreCase))
                    {
                        shellArgs = $"/c {expandedCommand}";
                    }
                    else if (shellExe.EndsWith("powershell.exe", StringComparison.OrdinalIgnoreCase) ||
                             shellExe.EndsWith("pwsh.exe", StringComparison.OrdinalIgnoreCase))
                    {
                        shellArgs = $"-Command \"{expandedCommand}\"";
                    }
                    else
                    {
                        // Assume it's a Unix-like shell
                        shellArgs = $"-c \"{expandedCommand}\"";
                    }
                }
                else
                {
                    // Use shell from configuration based on OS
                    var config = _configProvider?.LoadConfiguration();

                    if (config == null)
                    {
                        // Fallback to default behavior if config is not available
                        if (OperatingSystem.IsWindows())
                        {
                            shellExe = "cmd.exe";
                            shellArgs = $"/c {expandedCommand}";
                        }
                        else
                        {
                            shellExe = "/bin/sh";
                            shellArgs = $"-c \"{expandedCommand}\"";
                        }
                    }
                    else
                    {
                        if (OperatingSystem.IsWindows())
                        {
                            shellExe = config.Shell.Windows;
                            shellArgs = $"/c {expandedCommand}";
                        }
                        else if (OperatingSystem.IsLinux())
                        {
                            shellExe = config.Shell.Linux;
                            shellArgs = $"-c \"{expandedCommand}\"";
                        }
                        else if (OperatingSystem.IsMacOS())
                        {
                            shellExe = config.Shell.Mac;
                            shellArgs = $"-c \"{expandedCommand}\"";
                        }
                        else
                        {
                            // For other OS or if detection fails, use default
                            shellExe = config.Shell.Default;
                            shellArgs = $"-c \"{expandedCommand}\"";
                        }
                    }
                }

                // Log the full command that will be executed
                string fullCommand = $"{shellExe} {shellArgs}";
                _logger?.LogInformation("Executing custom function command: {FullCommand}", fullCommand);

                // Execute the command
                var processInfo = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = shellExe,
                    Arguments = shellArgs,
                    UseShellExecute = false,
                    CreateNoWindow = false, // Must be false to allow command output/UI in the same console
                    RedirectStandardOutput = !string.IsNullOrEmpty(function.PipeToAction)
                };

                using (var process = System.Diagnostics.Process.Start(processInfo))
                {
                    if (process == null)
                    {
                        _logger?.LogError("Failed to start process: {Command}", expandedCommand);
                        return false;
                    }

                    string output = "";
                    if (processInfo.RedirectStandardOutput)
                    {
                        output = process.StandardOutput.ReadToEnd();
                    }

                    process.WaitForExit();

                    // Log the exit code and output for debugging
                    _logger?.LogDebug("Custom function process exited with code: {ExitCode}", process.ExitCode);

                    if (!string.IsNullOrEmpty(output))
                    {
                        _logger?.LogDebug("Custom function output: {Output}", output);
                    }

                    if (process.ExitCode == 0 && !string.IsNullOrEmpty(function.PipeToAction))
                    {
                        output = output.Trim();
                        string pipeAction = TWF.Utilities.EnvironmentVariableExpander.ExpandEnvironmentVariables(function.PipeToAction);
                        _logger?.LogInformation("Custom function output piped to action {Action}: {Output}", pipeAction, output);

                        if (_builtInActionExecutorWithArg != null)
                        {
                            return _builtInActionExecutorWithArg(pipeAction, output);
                        }
                        else
                        {
                            _logger?.LogWarning("Built-in action executor with arg not configured");
                            return false;
                        }
                    }

                    return process.ExitCode == 0;
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error executing custom function: {Name}", function.Name);
                return false;
            }
        }

        /// <summary>
        /// Executes a menu-type custom function by loading and displaying a menu
        /// </summary>
        /// <param name="function">The menu-type function to execute</param>
        /// <param name="activePane">Active pane state</param>
        /// <param name="inactivePane">Inactive pane state</param>
        /// <param name="leftPane">Left pane state</param>
        /// <param name="rightPane">Right pane state</param>
        /// <returns>True if executed successfully, false if cancelled or failed</returns>
        private bool ExecuteMenuFunction(CustomFunction function, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            if (_menuManager == null)
            {
                _logger?.LogError("MenuManager not set, cannot execute menu-type function: {Name}", function.Name);
                return false;
            }

            // Load the menu file
            string menuPath = TWF.Utilities.EnvironmentVariableExpander.ExpandEnvironmentVariables(function.Menu!);
            var menuFile = _menuManager.LoadMenuFile(menuPath);
            if (menuFile == null)
            {
                _logger?.LogError("Failed to load menu file: {MenuPath}", menuPath);
                return false;
            }

            if (menuFile.Menus.Count == 0)
            {
                _logger?.LogWarning("Menu file is empty: {MenuPath}", menuPath);
                return false;
            }

            // Save cursor position and scroll offset before showing menu
            int savedCursorPosition = activePane.CursorPosition;
            int savedScrollOffset = activePane.ScrollOffset;

            // Display the menu dialog
            var menuDialog = new MenuDialog(menuFile.Menus, function.Name);
            Application.Run(menuDialog);

            // Restore cursor position and scroll offset after menu closes
            activePane.CursorPosition = savedCursorPosition;
            activePane.ScrollOffset = savedScrollOffset;

            var selectedItem = menuDialog.SelectedItem;
            if (selectedItem == null)
            {
                _logger?.LogInformation("Menu selection cancelled by user");
                return false;
            }

            _logger?.LogInformation("Menu item selected: {ItemName}", selectedItem.Name);

            // Execute the selected menu item
            return ExecuteMenuItem(selectedItem, activePane, inactivePane, leftPane, rightPane);
        }

        /// <summary>
        /// Executes a menu item (either Function or Action type)
        /// </summary>
        /// <param name="menuItem">The menu item to execute</param>
        /// <param name="activePane">Active pane state</param>
        /// <param name="inactivePane">Inactive pane state</param>
        /// <param name="leftPane">Left pane state</param>
        /// <param name="rightPane">Right pane state</param>
        /// <returns>True if executed successfully, false if failed</returns>
        private bool ExecuteMenuItem(MenuItemDefinition menuItem, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            try
            {
                // If menu item has a Function property, execute that custom function
                if (!string.IsNullOrEmpty(menuItem.Function))
                {
                    _logger?.LogInformation("Executing menu item function: {Function}", menuItem.Function);
                    
                    // Apply macro expansion to the Function property
                    var expandedFunctionName = _macroExpander.ExpandMacros(menuItem.Function, activePane, inactivePane, leftPane, rightPane);
                    
                    if (expandedFunctionName == null)
                    {
                        _logger?.LogInformation("Menu item function execution cancelled by user");
                        return false;
                    }

                    _logger?.LogDebug("Expanded function name: {ExpandedName}", expandedFunctionName);
                    
                    // Look up the custom function
                    var customFunction = _config?.Functions.FirstOrDefault(f => 
                        f.Name.Equals(expandedFunctionName, StringComparison.OrdinalIgnoreCase));
                    
                    if (customFunction == null)
                    {
                        var errorMsg = $"Custom function not found: {expandedFunctionName}";
                        _logger?.LogError(errorMsg);
                        MessageBox.ErrorQuery("Error", errorMsg, "OK");
                        return false;
                    }

                    // Recursively execute the custom function (which may itself be a menu)
                    return ExecuteFunction(customFunction, activePane, inactivePane, leftPane, rightPane);
                }

                // If menu item has an Action property (or legacy Menu property), it's a built-in action
#pragma warning disable CS0618
                string? actionName = menuItem.Action ?? menuItem.Menu;
#pragma warning restore CS0618
                if (!string.IsNullOrEmpty(actionName))
                {
                    _logger?.LogInformation("Executing menu item built-in action: {Action}", actionName);
                    
                    if (_builtInActionExecutor == null)
                    {
                        var errorMsg = "Built-in action executor not configured";
                        _logger?.LogError("{Error}, cannot execute: {Action}", errorMsg, actionName);
                        MessageBox.ErrorQuery("Error", errorMsg, "OK");
                        return false;
                    }

                    // Execute the built-in action via the callback
                    bool result = _builtInActionExecutor(actionName);
                    
                    if (!result)
                    {
                        _logger?.LogWarning("Built-in action failed or not found: {Action}", actionName);
                    }
                    
                    return result;
                }

                // Empty action property - do nothing
                _logger?.LogWarning("Menu item has no Function or Action property: {Name}", menuItem.Name);
                return false;
            }
            catch (Exception ex)
            {
                var errorMsg = $"Error executing menu item: {menuItem.Name}";
                _logger?.LogError(ex, errorMsg);
                MessageBox.ErrorQuery("Error", errorMsg, "OK");
                return false;
            }
        }

        /// <summary>
        /// Creates a default custom_functions.json file with examples
        /// </summary>
        private void CreateDefaultFunctionsFile(string configPath)
        {
            var defaultConfig = new CustomFunctionsConfig
            {
                Version = "1.0",
                Functions = new List<CustomFunction>
                {
                    new CustomFunction
                    {
                        Name = "Open in Notepad",
                        Command = "notepad \"$P\\$F\"",
                        Description = "Open current file in Notepad"
                    },
                    new CustomFunction
                    {
                        Name = "Copy to Other Pane",
                        Command = "cmd /c copy \"$P\\$F\" \"$O\\\"",
                        Description = "Copy current file to other pane"
                    },
                    new CustomFunction
                    {
                        Name = "Open Command Prompt Here",
                        Command = "cmd /k cd /d \"$P\"",
                        Description = "Open command prompt in current directory"
                    },
                    new CustomFunction
                    {
                        Name = "Custom Copy",
                        Command = "cmd /c copy \"$P\\$F\" \"$I\"Destination path\"\"",
                        Description = "Copy file to custom location (prompts for path)"
                    },
                    new CustomFunction
                    {
                        Name = "Batch Rename",
                        Command = "cmd /c ren \"$P\\$F\" \"$I\"New filename\"\"",
                        Description = "Rename current file (prompts for new name)"
                    }
                }
            };

            try
            {
                var directory = Path.GetDirectoryName(configPath);
                if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
                {
                    Directory.CreateDirectory(directory);
                }

                var json = JsonSerializer.Serialize(defaultConfig, new JsonSerializerOptions
                {
                    WriteIndented = true
                });

                File.WriteAllText(configPath, json);
                _logger?.LogInformation("Created default custom functions file at {Path}", configPath);
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Failed to create default custom functions file");
            }
        }
    }
}
