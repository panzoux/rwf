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
    public class RegisteredFolderDialog : Dialog
    {
        private readonly List<RegisteredFolder> _allFolders;
        private readonly SearchEngine _searchEngine;
        private readonly Configuration _configuration;
        private readonly Action<RegisteredFolder> _onNavigate;
        private readonly Action<RegisteredFolder> _onDelete;
        private readonly ILogger _logger;

        private ListView _folderList = null!;
        private Label _searchLabel = null!;
        private Label _searchTextLabel = null!;
        private Label _helpBar = null!;

        private List<RegisteredFolder> _filteredFolders = new List<RegisteredFolder>();
        private string _searchPattern = "";

        public RegisteredFolderDialog(
            List<RegisteredFolder> folders,
            SearchEngine searchEngine,
            Configuration configuration,
            Action<RegisteredFolder> onNavigate,
            Action<RegisteredFolder> onDelete,
            ILogger logger)
            : base("Registered Folders", 70, 20)
        {
            _allFolders = new List<RegisteredFolder>(folders);
            _searchEngine = searchEngine;
            _configuration = configuration;
            _onNavigate = onNavigate;
            _onDelete = onDelete;
            _logger = logger;
            _filteredFolders = new List<RegisteredFolder>(_allFolders);

            InitializeComponents();
            ApplyColors();
        }

        private void InitializeComponents()
        {
            // Folder List
            _folderList = new ListView()
            {
                X = 0,
                Y = 0,
                Width = Dim.Fill(0),
                Height = Dim.Fill(2),
                AllowsMarking = false,
                Source = new ListWrapper(_filteredFolders.Select(FormatFolder).ToList())
            };

            if (_filteredFolders.Count > 0)
            {
                _folderList.SelectedItem = 0;
            }

            _folderList.KeyPress += (e) =>
            {
                var keyEvent = e.KeyEvent;
                
                // Esc: Close
                if (keyEvent.Key == Key.Esc)
                {
                    Application.RequestStop();
                    e.Handled = true;
                    return;
                }

                // Enter: Select
                var key = keyEvent.Key;
                var cleanKey = (Key)((uint)key & 0xFFFF);
                if (cleanKey == Key.Enter || cleanKey == (Key)10 || cleanKey == (Key)13)
                {
                    NavigateToSelected();
                    e.Handled = true;
                    return;
                }

                // Delete: Remove item
                if (keyEvent.Key == Key.DeleteChar || keyEvent.Key == Key.Delete)
                {
                    DeleteSelected();
                    e.Handled = true;
                    return;
                }

                // Ctrl+K: Clear search
                if (keyEvent.Key == (Key.K | Key.CtrlMask))
                {
                    _searchPattern = "";
                    FilterFolders();
                    e.Handled = true;
                    return;
                }

                // Backspace: Search query editing
                if (keyEvent.Key == Key.Backspace)
                {
                    if (_searchPattern.Length > 0)
                    {
                        _searchPattern = _searchPattern.Substring(0, _searchPattern.Length - 1);
                        FilterFolders();
                    }
                    e.Handled = true;
                    return;
                }

                // Alphabetic/Numeric/Symbol input for search
                if ((keyEvent.Key & (Key.AltMask | Key.CtrlMask)) == 0)
                {
                    uint val = (uint)keyEvent.Key;
                    if ((val >= 32 && val <= 126) || (val >= 0x20 && val <= 0x7E))
                    {
                        _searchPattern += (char)(val & 0xFFFF);
                        FilterFolders();
                        e.Handled = true;
                        return;
                    }
                }
            };

            Add(_folderList);

            // Help Bar
            _helpBar = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(2),
                Width = Dim.Fill(),
                Height = 1,
                Text = "[Enter] Jump to folder [Delete] Remove selected [Esc] Cancel"
            };
            Add(_helpBar);

            // Search Line
            _searchLabel = new Label("Search: ")
            {
                X = 0,
                Y = Pos.AnchorEnd(1),
                Width = 8,
                Height = 1
            };
            Add(_searchLabel);

            _searchTextLabel = new Label("")
            {
                X = 8,
                Y = Pos.AnchorEnd(1),
                Width = Dim.Fill(),
                Height = 1
            };
            Add(_searchTextLabel);

            /*
            // Buttons
            var okButton = new Button("OK")
            {
                X = Pos.Center() - 16,
                Y = Pos.AnchorEnd(1),
                IsDefault = true
            };
            
            var cancelButton = new Button("Cancel")
            {
                X = Pos.Center(),
                Y = Pos.AnchorEnd(1)
            };
            
            var deleteButton = new Button("Delete")
            {
                X = Pos.Center() + 8,
                Y = Pos.AnchorEnd(1)
            };
            
            okButton.Clicked += () => NavigateToSelected();
            cancelButton.Clicked += () => Application.RequestStop();
            deleteButton.Clicked += () => DeleteSelected();
            
            Add(okButton);
            Add(cancelButton);
            Add(deleteButton);
            */
        }

        private string FormatFolder(RegisteredFolder folder)
        {
            return $"{folder.Name} - {folder.Path}";
        }

        private void FilterFolders()
        {
            _searchTextLabel.Text = _searchPattern;
            
            if (string.IsNullOrEmpty(_searchPattern))
            {
                _filteredFolders = new List<RegisteredFolder>(_allFolders);
            }
            else
            {
                var tempEntries = _allFolders.Select(f => new FileEntry { Name = FormatFolder(f) }).ToList();
                var matches = _searchEngine.FindMatches(tempEntries, _searchPattern, _configuration.Migemo.Enabled);
                _filteredFolders = matches.Select(idx => _allFolders[idx]).ToList();
            }

            _folderList.Source = new ListWrapper(_filteredFolders.Select(FormatFolder).ToList());
            if (_filteredFolders.Count > 0)
            {
                _folderList.SelectedItem = 0;
            }
        }

        private void NavigateToSelected()
        {
            if (_folderList.SelectedItem >= 0 && _folderList.SelectedItem < _filteredFolders.Count)
            {
                var selected = _filteredFolders[_folderList.SelectedItem];
                _onNavigate?.Invoke(selected);
                Application.RequestStop();
            }
        }

        private void DeleteSelected()
        {
            if (_folderList.SelectedItem >= 0 && _folderList.SelectedItem < _filteredFolders.Count)
            {
                var selected = _filteredFolders[_folderList.SelectedItem];
                int originalIndex = _folderList.SelectedItem;

                _onDelete?.Invoke(selected);
                
                _allFolders.Remove(selected);
                _filteredFolders.Remove(selected);
                
                FilterFolders(); // Refresh list

                // Adjust selection
                if (_filteredFolders.Count > 0)
                {
                    _folderList.SelectedItem = Math.Min(originalIndex, _filteredFolders.Count - 1);
                    _folderList.SetFocus();
                }
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
            _folderList.ColorScheme = listScheme;

            var searchScheme = new ColorScheme()
            {
                Normal = Application.Driver.MakeAttribute(foreground, background)
            };
            _searchLabel.ColorScheme = searchScheme;
            _searchTextLabel.ColorScheme = searchScheme;
            
            var helpFg = ColorHelper.ParseConfigColor(display.DialogHelpForegroundColor, Color.BrightYellow);
            var helpBg = ColorHelper.ParseConfigColor(display.DialogHelpBackgroundColor, Color.Blue);
            _helpBar.ColorScheme = new ColorScheme()
            {
                Normal = Application.Driver.MakeAttribute(helpFg, helpBg)
            };
        }
    }
}
