using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using Microsoft.Extensions.Logging;
using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    /// Manages help explanations and joins them with current keybindings
    /// </summary>
    public class HelpManager
    {
        private readonly KeyBindingManager _keyBindingManager;
        private readonly ILogger<HelpManager>? _logger;
        private readonly string _configDirectory;
        private List<HelpItem> _definitions;
        private string _currentLanguage;

        public HelpManager(KeyBindingManager keyBindingManager, string configDirectory, string initialLanguage = "en", ILogger<HelpManager>? logger = null)
        {
            _keyBindingManager = keyBindingManager;
            _configDirectory = configDirectory;
            _logger = logger;
            _definitions = new List<HelpItem>();
            _currentLanguage = initialLanguage;
            
            Reload(initialLanguage);
        }

        /// <summary>
        /// Reloads help definitions for a specific language
        /// </summary>
        /// <param name="languageCode">Language code (e.g., "en", "jp")</param>
        public void Reload(string languageCode)
        {
            try
            {
                _currentLanguage = languageCode;
                string fileName = $"help.{languageCode}.json";
                string helpDir = Path.Combine(_configDirectory, "help");
                string path = Path.Combine(helpDir, fileName);

                _logger?.LogDebug("Searching for help file: {Path}", path);

                if (!File.Exists(path))
                {
                    if (!Directory.Exists(helpDir))
                    {
                        Directory.CreateDirectory(helpDir);
                    }

                    // Fallback to app directory
                    string appDirFile = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, fileName);
                    _logger?.LogDebug("Not found in config. Checking app directory: {Path}", appDirFile);

                    if (File.Exists(appDirFile))
                    {
                        _logger?.LogInformation("Found help file in app directory, copying to: {Path}", path);
                        try { File.Copy(appDirFile, path); } catch (Exception ex) { _logger?.LogWarning(ex, "Failed to copy help file"); }
                    }
                    else if (languageCode != "en")
                    {
                        _logger?.LogWarning("Help definitions for '{Lang}' not found at '{Path}', falling back to English", languageCode, path);
                        Reload("en");
                        return;
                    }
                    else
                    {
                        _logger?.LogWarning("Help definitions file not found at '{Path}' or '{AppPath}'", path, appDirFile);
                        _definitions = new List<HelpItem>();
                        return;
                    }
                }

                string json = File.ReadAllText(path);
                _definitions = JsonSerializer.Deserialize<List<HelpItem>>(json, new JsonSerializerOptions
                {
                    PropertyNameCaseInsensitive = true
                }) ?? new List<HelpItem>();
                
                _logger?.LogInformation("Loaded {Count} help definitions for '{Lang}'", _definitions.Count, languageCode);
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error loading help definitions for '{Lang}'", languageCode);
                if (languageCode != "en") Reload("en");
            }
        }

        public string CurrentLanguage => _currentLanguage;

        /// <summary>
        /// Gets all help items, refreshed with current keybindings
        /// </summary>
        public List<HelpItem> GetHelpItems(UiMode mode = UiMode.Normal)
        {
            foreach (var item in _definitions)
            {
                var keys = _keyBindingManager.GetKeysForAction(item.Action, mode);
                item.BoundKeys = keys.Count > 0 ? string.Join(", ", keys) : "---";
            }
            
            return _definitions;
        }

        /// <summary>
        /// Gets filtered help items
        /// </summary>
        public List<HelpItem> GetFilteredItems(string query, SearchEngine searchEngine, UiMode mode = UiMode.Normal)
        {
            var items = GetHelpItems(mode);
            if (string.IsNullOrWhiteSpace(query)) return items;

            return items.Where(item => 
                Matches(item.Category, query, searchEngine) ||
                Matches(item.Action, query, searchEngine) ||
                Matches(item.BoundKeys, query, searchEngine) ||
                Matches(item.Description, query, searchEngine)
            ).ToList();
        }

        private bool Matches(string text, string query, SearchEngine searchEngine)
        {
            if (string.IsNullOrEmpty(text)) return false;
            return searchEngine.IsMatch(text, query);
        }
    }
}
