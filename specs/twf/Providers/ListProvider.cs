using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using TWF.Models;

namespace TWF.Providers
{
    /// <summary>
    /// Provides generic list data for various UI components including drives, registered folders, history, and context menus
    /// </summary>
    public class ListProvider
    {
        private readonly ConfigurationProvider _configProvider;
        private readonly ILogger<ListProvider> _logger;
        private readonly List<string> _directoryHistory;
        private readonly List<string> _searchHistory;
        private readonly List<string> _commandHistory;
        private const int MaxHistoryItems = 50;

        public ListProvider(ConfigurationProvider configProvider, ILogger<ListProvider>? logger = null)
        {
            _configProvider = configProvider ?? throw new ArgumentNullException(nameof(configProvider));
            _logger = logger ?? NullLogger<ListProvider>.Instance;
            _directoryHistory = new List<string>();
            _searchHistory = new List<string>();
            _commandHistory = new List<string>();
        }

        /// <summary>
        /// Gets a list of all available drives on the system
        /// </summary>
        public List<Models.DriveInfo> GetDriveList()
        {
            var driveList = new List<Models.DriveInfo>();

            try
            {
                var drives = System.IO.DriveInfo.GetDrives();
                
                foreach (var drive in drives)
                {
                    try
                    {
                        // Skip if not ready (unreachable, empty, etc.)
                        if (!drive.IsReady) continue;

                        var driveInfo = new Models.DriveInfo
                        {
                            DriveLetter = drive.Name,
                            DriveType = drive.DriveType,
                            TotalSize = drive.TotalSize,
                            FreeSpace = drive.AvailableFreeSpace,
                            VolumeLabel = !string.IsNullOrEmpty(drive.VolumeLabel) 
                                ? drive.VolumeLabel 
                                : drive.DriveType.ToString()
                        };
                        
                        driveList.Add(driveInfo);
                    }
                    catch (Exception ex)
                    {
                        // Log error but continue with other drives
                        _logger.LogError(ex, "Error accessing drive {DriveName}", drive.Name);
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error enumerating drives");
            }

            return driveList;
        }

        /// <summary>
        /// Gets the list of registered folders from configuration
        /// </summary>
        public List<RegisteredFolder> GetJumpList()
        {
            try
            {
                var config = _configProvider.LoadConfiguration();
                return config.RegisteredFolders
                    .OrderBy(f => f.SortOrder)
                    .ThenBy(f => f.Name)
                    .ToList();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error loading registered folders");
                return new List<RegisteredFolder>();
            }
        }

        /// <summary>
        /// Gets the history list for the specified history type
        /// </summary>
        public List<string> GetHistoryList(HistoryType type)
        {
            var history = type switch
            {
                HistoryType.DirectoryHistory => _directoryHistory,
                HistoryType.SearchHistory => _searchHistory,
                HistoryType.CommandHistory => _commandHistory,
                _ => new List<string>()
            };

            // Return a copy to prevent external modification
            return new List<string>(history);
        }

        /// <summary>
        /// Adds an item to the specified history list
        /// </summary>
        public void AddToHistory(HistoryType type, string item)
        {
            if (string.IsNullOrWhiteSpace(item))
                return;

            var history = type switch
            {
                HistoryType.DirectoryHistory => _directoryHistory,
                HistoryType.SearchHistory => _searchHistory,
                HistoryType.CommandHistory => _commandHistory,
                _ => null
            };

            if (history == null)
                return;

            // Remove duplicate if it exists
            history.Remove(item);

            // Add to the beginning of the list
            history.Insert(0, item);

            // Trim to max size
            if (history.Count > MaxHistoryItems)
            {
                history.RemoveRange(MaxHistoryItems, history.Count - MaxHistoryItems);
            }
        }

        /// <summary>
        /// Clears the specified history list
        /// </summary>
        public void ClearHistory(HistoryType type)
        {
            var history = type switch
            {
                HistoryType.DirectoryHistory => _directoryHistory,
                HistoryType.SearchHistory => _searchHistory,
                HistoryType.CommandHistory => _commandHistory,
                _ => null
            };

            history?.Clear();
        }

        /// <summary>
        /// Generates a context menu based on the current file entry and selection state
        /// </summary>
        public List<MenuItem> GetContextMenu(FileEntry? entry, bool hasMarkedFiles)
        {
            var menu = new List<MenuItem>();

            if (entry == null)
            {
                // Empty pane menu
                menu.Add(new MenuItem { Label = "Refresh", Action = "Refresh", Shortcut = "F5" });
                menu.Add(new MenuItem { IsSeparator = true });
                menu.Add(new MenuItem { Label = "Create Directory", Action = "CreateDirectory", Shortcut = "J" });
                return menu;
            }

            // File/Directory operations
            if (entry.IsDirectory)
            {
                menu.Add(new MenuItem { Label = "Open", Action = "Navigate", Shortcut = "Enter" });
                menu.Add(new MenuItem { IsSeparator = true });
            }
            else
            {
                menu.Add(new MenuItem { Label = "Open", Action = "Execute", Shortcut = "Enter" });
                menu.Add(new MenuItem { Label = "Open with Editor", Action = "OpenEditor", Shortcut = "Shift+Enter" });
                menu.Add(new MenuItem { IsSeparator = true });
            }

            // Mark operations
            if (hasMarkedFiles)
            {
                menu.Add(new MenuItem { Label = "Copy Marked Files", Action = "Copy", Shortcut = "C" });
                menu.Add(new MenuItem { Label = "Move Marked Files", Action = "Move", Shortcut = "M" });
                menu.Add(new MenuItem { Label = "Delete Marked Files", Action = "Delete", Shortcut = "D" });
                menu.Add(new MenuItem { IsSeparator = true });
                menu.Add(new MenuItem { Label = "Rename Marked Files", Action = "Rename", Shortcut = "Shift+R" });
                menu.Add(new MenuItem { Label = "Clear Marks", Action = "ClearMarks" });
            }
            else
            {
                menu.Add(new MenuItem { Label = "Copy", Action = "Copy", Shortcut = "C" });
                menu.Add(new MenuItem { Label = "Move", Action = "Move", Shortcut = "M" });
                menu.Add(new MenuItem { Label = "Delete", Action = "Delete", Shortcut = "D" });
                menu.Add(new MenuItem { IsSeparator = true });
                menu.Add(new MenuItem { Label = "Rename", Action = "Rename", Shortcut = "Shift+R" });
            }

            menu.Add(new MenuItem { IsSeparator = true });

            // Archive operations
            if (entry.IsArchive)
            {
                menu.Add(new MenuItem { Label = "Browse Archive", Action = "BrowseArchive", Shortcut = "Enter" });
                menu.Add(new MenuItem { Label = "Extract Archive", Action = "ExtractArchive", Shortcut = "Shift+Enter" });
                menu.Add(new MenuItem { IsSeparator = true });
            }

            if (hasMarkedFiles || !entry.IsDirectory)
            {
                menu.Add(new MenuItem { Label = "Compress", Action = "Compress", Shortcut = "P" });
                menu.Add(new MenuItem { IsSeparator = true });
            }

            // View operations for files
            if (!entry.IsDirectory)
            {
                // Check if it's a text file
                var textExtensions = new[] { ".txt", ".log", ".md", ".cs", ".json", ".xml", ".config", ".ini" };
                if (textExtensions.Contains(entry.Extension.ToLowerInvariant()))
                {
                    menu.Add(new MenuItem { Label = "View as Text", Action = "ViewText", Shortcut = "Enter" });
                }

                // Check if it's an image file
                var imageExtensions = new[] { ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".tiff", ".ico" };
                if (imageExtensions.Contains(entry.Extension.ToLowerInvariant()))
                {
                    menu.Add(new MenuItem { Label = "View Image", Action = "ViewImage", Shortcut = "Enter" });
                }

                if (menu.Count > 0 && !menu[menu.Count - 1].IsSeparator)
                {
                    menu.Add(new MenuItem { IsSeparator = true });
                }
            }

            // File comparison
            menu.Add(new MenuItem { Label = "Compare Files", Action = "Compare", Shortcut = "W" });
            
            // File split/join
            if (!entry.IsDirectory)
            {
                menu.Add(new MenuItem { Label = "Split File", Action = "Split", Shortcut = "Shift+W" });
                
                // Check if it's a split file part
                if (entry.Name.Contains(".part") || entry.Name.Contains(".001"))
                {
                    menu.Add(new MenuItem { Label = "Join Files", Action = "Join", Shortcut = "Shift+W" });
                }
            }

            menu.Add(new MenuItem { IsSeparator = true });

            // Properties
            menu.Add(new MenuItem { Label = "Properties", Action = "Properties", Shortcut = "Alt+Enter" });

            return menu;
        }
    }
}
