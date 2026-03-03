using System.Text.Json;
using TWF.Models;
using Microsoft.Extensions.Logging;

namespace TWF.Services
{
    /// <summary>
    /// Manages loading and caching of menu files
    /// </summary>
    public class MenuManager
    {
        private readonly string _configDirectory;
        private readonly Dictionary<string, MenuFile> _loadedMenus;
        private readonly ILogger<MenuManager>? _logger;

        /// <summary>
        /// Initializes a new instance of the MenuManager class
        /// </summary>
        /// <param name="configDirectory">Directory where menu files are located</param>
        /// <param name="logger">Optional logger instance</param>
        public MenuManager(string configDirectory, ILogger<MenuManager>? logger = null)
        {
            _configDirectory = configDirectory ?? throw new ArgumentNullException(nameof(configDirectory));
            _loadedMenus = new Dictionary<string, MenuFile>();
            _logger = logger;
        }

        /// <summary>
        /// Loads a menu file from the specified path
        /// </summary>
        /// <param name="menuFilePath">Path to the menu file (relative or absolute)</param>
        /// <returns>The loaded MenuFile, or null if loading failed</returns>
        public MenuFile? LoadMenuFile(string menuFilePath)
        {
            if (string.IsNullOrWhiteSpace(menuFilePath))
            {
                _logger?.LogWarning("Menu file path is null or empty");
                return null;
            }

            try
            {
                // Resolve the full path
                string fullPath = ResolveMenuFilePath(menuFilePath);

                // Check cache first
                if (_loadedMenus.TryGetValue(fullPath, out var cachedMenu))
                {
                    _logger?.LogDebug("Returning cached menu file: {Path}", fullPath);
                    return cachedMenu;
                }

                // Check if file exists
                if (!File.Exists(fullPath))
                {
                    _logger?.LogError("Menu file not found: {Path}", fullPath);
                    return null;
                }

                // Read and parse the file
                _logger?.LogInformation("Loading menu file: {Path}", fullPath);
                var jsonContent = File.ReadAllText(fullPath);
                var menuFile = ParseMenuFile(jsonContent);

                if (menuFile != null)
                {
                    // Cache the loaded menu
                    _loadedMenus[fullPath] = menuFile;
                    _logger?.LogInformation("Successfully loaded menu file with {Count} items", menuFile.Menus.Count);
                }

                return menuFile;
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error loading menu file: {Path}", menuFilePath);
                return null;
            }
        }

        /// <summary>
        /// Parses JSON content into a MenuFile object
        /// </summary>
        /// <param name="jsonContent">JSON string to parse</param>
        /// <returns>The parsed MenuFile, or null if parsing failed</returns>
        private MenuFile? ParseMenuFile(string jsonContent)
        {
            try
            {
                var menuFile = JsonSerializer.Deserialize<MenuFile>(jsonContent, new JsonSerializerOptions
                {
                    PropertyNameCaseInsensitive = true,
                    AllowTrailingCommas = true,
                    ReadCommentHandling = JsonCommentHandling.Skip
                });

                if (menuFile == null)
                {
                    _logger?.LogError("Failed to deserialize menu file: result is null");
                    return null;
                }

                // Validate the menu file
                if (menuFile.Menus == null)
                {
                    _logger?.LogWarning("Menu file has null Menus property, initializing empty list");
                    menuFile.Menus = new List<MenuItemDefinition>();
                }

                return menuFile;
            }
            catch (JsonException ex)
            {
                _logger?.LogError(ex, "Invalid JSON in menu file at line {Line}, position {Position}", 
                    ex.LineNumber, ex.BytePositionInLine);
                return null;
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error parsing menu file");
                return null;
            }
        }

        /// <summary>
        /// Resolves a menu file path to a full path
        /// </summary>
        /// <param name="menuFilePath">Relative or absolute path</param>
        /// <returns>Full path to the menu file</returns>
        private string ResolveMenuFilePath(string menuFilePath)
        {
            // If the path is already absolute, use it as-is
            if (Path.IsPathRooted(menuFilePath))
            {
                return menuFilePath;
            }

            // Otherwise, resolve relative to the config directory
            return Path.Combine(_configDirectory, menuFilePath);
        }

        /// <summary>
        /// Clears the menu file cache
        /// </summary>
        public void ClearCache()
        {
            _loadedMenus.Clear();
            _logger?.LogInformation("Menu file cache cleared");
        }
    }
}
