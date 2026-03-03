using System;
using System.Collections.Generic;
using System.Linq;
using Terminal.Gui;
using TWF.Models;
using TWF.Services;
using TWF.Utilities;
using Microsoft.Extensions.Logging;

namespace TWF.UI
{
    public class DriveDialog : Dialog
    {
        private readonly List<Models.DriveInfo> _drives;
        private readonly HistoryManager _historyManager;
        private readonly SearchEngine _searchEngine;
        private readonly Configuration _configuration;
        private readonly Action<string> _onSelect;
        private readonly ILogger _logger;

        private ListView _driveList = null!;
        private Label _helpBar = null!;
        private Label _searchLabel = null!;
        private Label _searchTextLabel = null!;

        private List<DriveItem> _allItems = new List<DriveItem>();
        private List<DriveItem> _filteredItems = new List<DriveItem>();
        private string _searchPattern = "";

        private class DriveItem
        {
            public string Display { get; set; } = string.Empty;
            public string Path { get; set; } = string.Empty;
            public override string ToString() => Display;
        }

        public DriveDialog(
            List<Models.DriveInfo> drives,
            HistoryManager historyManager,
            SearchEngine searchEngine,
            Configuration configuration,
            Action<string> onSelect,
            ILogger logger,
            string titleSuffix = "")
            : base($"Select Drive ({titleSuffix})", 60, 15)
        {
            _drives = drives;
            _historyManager = historyManager;
            _searchEngine = searchEngine;
            _configuration = configuration;
            _onSelect = onSelect;
            _logger = logger;

            InitializeItems();
            InitializeComponents();
            ApplyColors();
        }

        private void InitializeItems()
        {
            _allItems = new List<DriveItem>();

            // 1. User Directory
            _allItems.Add(new DriveItem 
            {
                Display = "~ User Directory", 
                Path = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile) 
            });

            // 2. Network Shares from History
            var networkRoots = _historyManager.LeftHistory.Concat(_historyManager.RightHistory)
                .Select(GetShareRoot)
                .Where(p => !string.IsNullOrEmpty(p))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(p => p);
            
            foreach (var path in networkRoots)
            {
                _allItems.Add(new DriveItem 
                {
                    Display = path, 
                    Path = path 
                });
            }

            // 3. Drives
            foreach (var drive in _drives)
            {
                _allItems.Add(new DriveItem 
                {
                    Display = $"{drive.DriveLetter} - {drive.VolumeLabel} ({drive.DriveType})", 
                    Path = drive.DriveLetter 
                });
            }

            _filteredItems = new List<DriveItem>(_allItems);
        }

        private string GetShareRoot(string path)
        {
            if (string.IsNullOrWhiteSpace(path)) return string.Empty;
            
            // Normalize slashes
            var normalized = path.Replace('/', '\\');
            
            // Check if it is a network path (starts with \\)
            if (!normalized.StartsWith(@"\\")) return string.Empty;
            
            // Trim leading backslashes to process components
            var cleanPath = normalized.TrimStart('\\');
            var parts = cleanPath.Split(new[] { '\\' }, StringSplitOptions.RemoveEmptyEntries);
            
            // Need at least server name (parts[0])
            // Ideally server\share (parts[0]\parts[1])
            if (parts.Length >= 2)
            {
                return $"\\\\{parts[0]}\\{parts[1]}";
            }
            else if (parts.Length == 1)
            {
                // Just the server name
                return $"\\\\{parts[0]}";
            }
            
            return string.Empty;
        }

        private void InitializeComponents()
        {
            // Drive List
            _driveList = new ListView()
            {
                X = 0,
                Y = 0,
                Width = Dim.Fill(0),
                Height = Dim.Fill(2),
                AllowsMarking = false,
                Source = new ListWrapper(_filteredItems),
            };
            if (_filteredItems.Count > 0)
            {
                _driveList.SelectedItem = 0;
            }

            _driveList.KeyPress += (e) =>
            {
                var keyEvent = e.KeyEvent;
                
                // Esc: Close dialog immediately
                if (keyEvent.Key == Key.Esc)
                {
                    Application.RequestStop();
                    e.Handled = true;
                    return;
                }

                // Enter: Selection logic
                var key = keyEvent.Key;
                var cleanKey = (Key)((uint)key & 0xFFFF);
                if (cleanKey == Key.Enter || cleanKey == (Key)10 || cleanKey == (Key)13)
                {
                    if (_driveList.SelectedItem >= 0 && _driveList.SelectedItem < _filteredItems.Count)
                    {
                        var selected = _filteredItems[_driveList.SelectedItem];
                        _onSelect?.Invoke(selected.Path);
                        Application.RequestStop();
                        e.Handled = true;
                        return;
                    }
                }

                // Ctrl+K: Clear search
                if (keyEvent.Key == (Key.K | Key.CtrlMask))
                {
                    _searchPattern = "";
                    FilterItems();
                    e.Handled = true;
                    return;
                }

                // Backspace: Search query editing
                if (keyEvent.Key == Key.Backspace)
                {
                    if (_searchPattern.Length > 0)
                    {
                        _searchPattern = _searchPattern.Substring(0, _searchPattern.Length - 1);
                        FilterItems();
                    }
                    e.Handled = true;
                    return;
                }

                // Alphabetic/Numeric/Symbol input for search
                if ((keyEvent.Key & (Key.AltMask | Key.CtrlMask)) == 0)
                {
                    uint val = (uint)keyEvent.Key;
                    // Check if it's a printable character
                    if ((val >= 32 && val <= 126) || (val >= 0x20 && val <= 0x7E))
                    {
                        _searchPattern += (char)(val & 0xFFFF);
                        FilterItems();
                        e.Handled = true;
                        return;
                    }
                }
            };

            Add(_driveList);

            // Help Bar
            _helpBar = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(2),
                Width = Dim.Fill(),
                Height = 1,
                Text = "[Enter] Select [Esc] Cancel"
            };
            Add(_helpBar);

            // Search Line
            //_searchLabel = new Label("Search: ")
            _searchLabel = new Label("/")
            {
                X = 0,
                Y = Pos.AnchorEnd(1),
                Width = 1,
                Height = 1
            };
            Add(_searchLabel);

            _searchTextLabel = new Label("")
            {
                X = 1,
                Y = Pos.AnchorEnd(1),
                Width = Dim.Fill(0), // Leave space for OK/Cancel buttons logic if we were using them, but we are using Enter/Esc
                Height = 1
            };
            Add(_searchTextLabel);

            /*
            // Buttons
            var okButton = new Button("OK")
            {
                X = Pos.Center() - 10,
                Y = Pos.AnchorEnd(1),
                IsDefault = true
            };
            
            var cancelButton = new Button("Cancel")
            {
                X = Pos.Center() + 2,
                Y = Pos.AnchorEnd(1)
            };
            
            okButton.Clicked += () =>
            {
                if (_driveList.SelectedItem >= 0 && _driveList.SelectedItem < _filteredItems.Count)
                {
                    var selected = _filteredItems[_driveList.SelectedItem];
                    _onSelect?.Invoke(selected.Path);
                    Application.RequestStop();
                }
            };
            
            cancelButton.Clicked += () =>
            {
                Application.RequestStop();
            };
            
            Add(okButton);
            Add(cancelButton);
            */
        }

        private void FilterItems()
        {
            _searchTextLabel.Text = _searchPattern;
            
            if (string.IsNullOrEmpty(_searchPattern))
            {
                _filteredItems = new List<DriveItem>(_allItems);
            }
            else
            {
                // Create temp entries for search engine
                var tempEntries = _allItems.Select(p => new FileEntry { Name = p.Display }).ToList();
                var matches = _searchEngine.FindMatches(tempEntries, _searchPattern, _configuration.Migemo.Enabled);
                _filteredItems = matches.Select(idx => _allItems[idx]).ToList();
            }

            _driveList.Source = new ListWrapper(_filteredItems);
            if (_filteredItems.Count > 0)
            {
                _driveList.SelectedItem = 0;
            }
        }

        private void ApplyColors()
        {
            var display = _configuration.Display;
            var foreground = ColorHelper.ParseConfigColor(display.ForegroundColor, Color.White);
            var background = ColorHelper.ParseConfigColor(display.BackgroundColor, Color.Black);
            var highlightFg = ColorHelper.ParseConfigColor(display.HighlightForegroundColor, Color.Black);
            var highlightBg = ColorHelper.ParseConfigColor(display.HighlightBackgroundColor, Color.Cyan);

            var listScheme = new ColorScheme()
            {
                Normal = Application.Driver.MakeAttribute(foreground, background),
                Focus = Application.Driver.MakeAttribute(highlightFg, highlightBg),
                HotNormal = Application.Driver.MakeAttribute(foreground, background),
                HotFocus = Application.Driver.MakeAttribute(highlightFg, highlightBg)
            };
            _driveList.ColorScheme = listScheme;

            var helpFg = ColorHelper.ParseConfigColor(display.DialogHelpForegroundColor, Color.BrightYellow);
            var helpBg = ColorHelper.ParseConfigColor(display.DialogHelpBackgroundColor, Color.Blue);
            _helpBar.ColorScheme = new ColorScheme()
            {
                Normal = Application.Driver.MakeAttribute(helpFg, helpBg)
            };

            var searchScheme = new ColorScheme()
            {
                Normal = Application.Driver.MakeAttribute(foreground, background)
            };
            _searchLabel.ColorScheme = searchScheme;
            _searchTextLabel.ColorScheme = searchScheme;
        }
    }
}