using System.Text;
using System.Text.RegularExpressions;
using Terminal.Gui;
using TWF.Models;
using Microsoft.Extensions.Logging;

namespace TWF.Services
{
    /// <summary>
    /// Expands macros in command strings with file manager context
    /// </summary>
    public class MacroExpander
    {
        private readonly ILogger<MacroExpander>? _logger;

        public MacroExpander(ILogger<MacroExpander>? logger = null)
        {
            _logger = logger;
        }

        /// <summary>
        /// Expands all macros in a command string
        /// </summary>
        /// <param name="command">Command string with macros</param>
        /// <param name="activePane">Active pane state</param>
        /// <param name="inactivePane">Inactive pane state</param>
        /// <param name="leftPane">Left pane state</param>
        /// <param name="rightPane">Right pane state</param>
        /// <returns>Expanded command string, or null if user cancelled</returns>
        public string? ExpandMacros(string command, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            if (string.IsNullOrEmpty(command))
                return command;

            var result = new StringBuilder();
            int i = 0;

            while (i < command.Length)
            {
                if (command[i] == '$' && i + 1 < command.Length)
                {
                    var expansion = ExpandMacro(command, ref i, activePane, inactivePane, leftPane, rightPane);
                    if (expansion == null)
                    {
                        // User cancelled
                        return null;
                    }
                    result.Append(expansion);
                }
                else
                {
                    result.Append(command[i]);
                    i++;
                }
            }

            var expandedMacros = result.ToString();
            // Expand environment variables (e.g. %VAR%, $VAR) for compatibility
            return TWF.Utilities.EnvironmentVariableExpander.ExpandEnvironmentVariables(expandedMacros);
        }

        /// <summary>
        /// Expands a single macro starting at position i
        /// </summary>
        private string? ExpandMacro(string command, ref int i, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            i++; // Skip the '$'

            if (i >= command.Length)
                return "$";

            char macroChar = command[i];
            i++;

            switch (macroChar)
            {
                case '$': // Literal $
                    return "$";

                case '%': // Literal %
                    return "%";

                case '{': // Literal {
                    return "{";

                case '}': // Literal }
                    return "}";

                case 'F': // Current filename
                case 'f': // Current filename (short)
                    return GetCurrentFilename(activePane, macroChar == 'f');

                case 'W': // Filename without extension
                case 'w': // Filename without extension (short)
                    return GetFilenameWithoutExtension(activePane, macroChar == 'w');

                case 'E': // File extension
                    return GetFileExtension(activePane);

                case 'P': // Active pane path
                case 'p': // Active pane path (short)
                    return GetPanePath(activePane, macroChar == 'p');

                case 'O': // Other pane path
                case 'o': // Other pane path (short)
                    return GetPanePath(inactivePane, macroChar == 'o');

                case 'L': // Left pane path
                case 'l': // Left pane path (short)
                    return GetPanePath(leftPane, macroChar == 'l');

                case 'R': // Right pane path
                case 'r': // Right pane path (short)
                    return GetPanePath(rightPane, macroChar == 'r');

                case 'S': // Sort state
                    return ExpandSortMacro(command, ref i, activePane, inactivePane, leftPane, rightPane);

                case 'M': // Marked files
                case 'm': // Marked files (lowercase = null if none)
                    return ExpandMarkedFilesMacro(command, ref i, activePane, inactivePane, leftPane, rightPane, macroChar == 'M');

                case '*': // File mask
                    return ExpandFileMaskMacro(command, ref i, activePane, inactivePane, leftPane, rightPane);

                case 'I': // Input dialog
                    return ShowInputDialog(command, ref i);

                case 'V': // Environment variable
                    return ExpandEnvironmentVariable(command, ref i);

                case '~': // Home directory
                    return Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);

                case '#': // ASCII character code
                    return ExpandAsciiCode(command, ref i);

                case '"': // Literal quote
                    return "\"";

                default:
                    _logger?.LogWarning("Unknown macro: ${0}", macroChar);
                    return "$" + macroChar;
            }
        }

        private string GetCurrentFilename(PaneState pane, bool shortName)
        {
            var entry = pane.GetCurrentEntry();
            if (entry == null || entry.Name == "..")
                return ".";

            return shortName ? GetShortPath(entry.FullPath) : entry.Name;
        }

        private string GetFilenameWithoutExtension(PaneState pane, bool shortName)
        {
            var entry = pane.GetCurrentEntry();
            if (entry == null || entry.Name == "..")
                return ".";

            var name = shortName ? GetShortPath(entry.FullPath) : entry.Name;
            return Path.GetFileNameWithoutExtension(name);
        }

        private string GetFileExtension(PaneState pane)
        {
            var entry = pane.GetCurrentEntry();
            if (entry == null || entry.Name == "..")
                return "";

            return Path.GetExtension(entry.Name);
        }

        private string GetPanePath(PaneState pane, bool shortName)
        {
            var path = pane.CurrentPath.TrimEnd('\\', '/');
            return shortName ? GetShortPath(path) : path;
        }

        private string? ExpandSortMacro(string command, ref int i, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            if (i >= command.Length)
                return "$S";

            char paneChar = command[i];
            i++;

            PaneState? targetPane = paneChar switch
            {
                'P' => activePane,
                'O' => inactivePane,
                'L' => leftPane,
                'R' => rightPane,
                _ => null
            };

            if (targetPane == null)
            {
                i--; // Back up
                return "$S";
            }

            return targetPane.SortMode.ToString();
        }

        private string? ExpandMarkedFilesMacro(string command, ref int i, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane, bool cancelIfNone)
        {
            if (i >= command.Length)
                return "$M";

            char typeChar = command[i];
            i++;

            bool shortName = char.IsLower(typeChar);
            typeChar = char.ToUpper(typeChar);

            PaneState? targetPane = null;
            bool fullPath = false;

            switch (typeChar)
            {
                case 'S': // Marked filenames only
                    targetPane = activePane;
                    fullPath = false;
                    break;
                case 'F': // Marked full paths
                    targetPane = activePane;
                    fullPath = true;
                    break;
                case 'O': // Other pane marked files
                    targetPane = inactivePane;
                    fullPath = true;
                    break;
                case 'L': // Left pane marked files
                    targetPane = leftPane;
                    fullPath = true;
                    break;
                case 'R': // Right pane marked files
                    targetPane = rightPane;
                    fullPath = true;
                    break;
                default:
                    i--; // Back up
                    return "$M";
            }

            var markedFiles = targetPane.GetMarkedEntries();
            if (markedFiles.Count == 0)
            {
                // No marked files, use current file
                var currentEntry = targetPane.GetCurrentEntry();
                if (currentEntry != null && !currentEntry.IsDirectory)
                {
                    markedFiles = new List<FileEntry> { currentEntry };
                }
                else if (cancelIfNone)
                {
                    // Cancel command if uppercase M and no files
                    return null;
                }
                else
                {
                    // Return empty string if lowercase m
                    return "";
                }
            }

            var files = markedFiles.Select(f =>
            {
                string path = fullPath ? f.FullPath : f.Name;
                if (shortName)
                {
                    path = GetShortPath(path);
                    return path; // Short names don't get quoted
                }
                else
                {
                    return $"\"{path}\""; // Quote full names
                }
            });

            return string.Join(" ", files);
        }

        private string? ExpandFileMaskMacro(string command, ref int i, PaneState activePane, PaneState inactivePane, PaneState leftPane, PaneState rightPane)
        {
            if (i >= command.Length)
                return "$*";

            char paneChar = command[i];
            i++;

            PaneState? targetPane = paneChar switch
            {
                'P' => activePane,
                'O' => inactivePane,
                'L' => leftPane,
                'R' => rightPane,
                _ => null
            };

            if (targetPane == null)
            {
                i--; // Back up
                return "$*";
            }

            return targetPane.FileMask;
        }

        private string? ShowInputDialog(string command, ref int i)
        {
            // Parse optional width digit
            int width = 4;
            if (i < command.Length && char.IsDigit(command[i]))
            {
                width = command[i] - '0';
                if (width == 0) width = 10;
                i++;
            }

            // Parse prompt in quotes
            string prompt = "Input";
            if (i < command.Length && command[i] == '"')
            {
                i++; // Skip opening quote
                int startQuote = i;
                while (i < command.Length && command[i] != '"')
                {
                    i++;
                }
                if (i < command.Length)
                {
                    prompt = command.Substring(startQuote, i - startQuote);
                    i++; // Skip closing quote
                }
            }

            // Show input dialog
            var dialog = new Dialog(prompt, width * 10, 8);
            var inputField = new TextField("")
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1)
            };
            dialog.Add(inputField);

            string? result = null;
            var okButton = new Button("OK", is_default: true);
            okButton.Clicked += () =>
            {
                result = inputField.Text.ToString();
                Application.RequestStop();
            };

            var cancelButton = new Button("Cancel");
            cancelButton.Clicked += () =>
            {
                result = null;
                Application.RequestStop();
            };

            dialog.AddButton(okButton);
            dialog.AddButton(cancelButton);

            Application.Run(dialog);

            return result; // null if cancelled
        }

        private string? ExpandEnvironmentVariable(string command, ref int i)
        {
            // Parse variable name in quotes
            if (i >= command.Length || command[i] != '"')
                return "$V";

            i++; // Skip opening quote
            int startQuote = i;
            while (i < command.Length && command[i] != '"')
            {
                i++;
            }

            if (i >= command.Length)
                return "$V";

            string varName = command.Substring(startQuote, i - startQuote);
            i++; // Skip closing quote

            // Special case: "twf" returns TWF executable path
            if (varName.Equals("twf", StringComparison.OrdinalIgnoreCase))
            {
                return GetTwfPath();
            }

            return Environment.GetEnvironmentVariable(varName) ?? "";
        }

        private string GetTwfPath()
        {
            var exePath = Environment.ProcessPath ?? AppDomain.CurrentDomain.BaseDirectory;
            return Path.GetDirectoryName(exePath)?.TrimEnd('\\', '/') ?? "";
        }

        private string? ExpandAsciiCode(string command, ref int i)
        {
            // Parse two hex digits
            if (i + 1 >= command.Length)
                return "$#";

            string hexCode = command.Substring(i, 2);
            i += 2;

            if (int.TryParse(hexCode, System.Globalization.NumberStyles.HexNumber, null, out int charCode))
            {
                return ((char)charCode).ToString();
            }

            return "$#" + hexCode;
        }

        private string GetShortPath(string path)
        {
            // On Windows, try to get short path (8.3 format)
            // For now, just return the path as-is
            // TODO: Implement proper short path conversion on Windows
            return path;
        }
    }
}
