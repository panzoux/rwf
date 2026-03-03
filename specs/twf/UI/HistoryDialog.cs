using System;
using System.Collections.Generic;
using System.Linq;
using Terminal.Gui;
using TWF.Models;
using Microsoft.Extensions.Logging;
using TWF.Services;
using TWF.Utilities;

namespace TWF.UI
{
    public class HistoryDialog : Dialog
    {
        private readonly HistoryManager _historyManager;
        private readonly SearchEngine _searchEngine;
        private readonly Configuration _configuration;
        private readonly Action<string, bool> _onSelect;
        private readonly ILogger _logger;

        private ListView _historyList = null!;
        private Label _helpBar = null!;
        private Label _searchLabel = null!;
        private Label _searchTextLabel = null!;
        
        private List<string> _fullHistory;
        private List<string> _filteredHistory;
        private bool _isLeftPane;
        private string _searchPattern = "";

        public HistoryDialog(
            HistoryManager historyManager, 
            SearchEngine searchEngine, 
            Configuration configuration,
            bool initialIsLeftPane,
            Action<string, bool> onSelect,
            ILogger logger) 
            : base("") // Title is set in UpdateTitle
        {
            _historyManager = historyManager;
            _searchEngine = searchEngine;
            _configuration = configuration;
            _isLeftPane = initialIsLeftPane;
            _onSelect = onSelect;
            _logger = logger;

            _fullHistory = (_isLeftPane ? _historyManager.LeftHistory : _historyManager.RightHistory).ToList();
            _filteredHistory = new List<string>(_fullHistory);

            InitializeComponents();
            UpdateTitle();
            ApplyColors();

            // Ensure the list is focused initially
            _historyList.SetFocus();
        }

        private void InitializeComponents()
        {
            // History List
            _historyList = new ListView()
            {
                X = 0,
                Y = 0,
                Width = Dim.Fill(),
                Height = Dim.Fill(2),
                AllowsMarking = false,
                Source = new ListWrapper(_filteredHistory),
                ColorScheme = Colors.Base 
            };
            if (_filteredHistory.Count > 0)
            {
                _historyList.SelectedItem = 0;
            }

            _historyList.KeyPress += (e) =>
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
                    if (_historyList.SelectedItem >= 0 && _historyList.SelectedItem < _filteredHistory.Count)
                    {
                        bool isOtherPane = (key & (Key.ShiftMask | Key.CtrlMask)) != 0 || key != cleanKey;
                        _onSelect?.Invoke(_filteredHistory[_historyList.SelectedItem], !isOtherPane);
                        Application.RequestStop();
                        e.Handled = true;
                        return;
                    }
                }

                // Left/Right: Context switching
                if (keyEvent.Key == Key.CursorLeft)
                {
                    SwitchHistory(true);
                    e.Handled = true;
                    return;
                }
                if (keyEvent.Key == Key.CursorRight)
                {
                    SwitchHistory(false);
                    e.Handled = true;
                    return;
                }

                // Ctrl+K: Clear search
                if (keyEvent.Key == (Key.K | Key.CtrlMask))
                {
                    _searchPattern = "";
                    FilterHistory();
                    e.Handled = true;
                    return;
                }

                // Backspace: Search query editing
                if (keyEvent.Key == Key.Backspace)
                {
                    if (_searchPattern.Length > 0)
                    {
                        _searchPattern = _searchPattern.Substring(0, _searchPattern.Length - 1);
                        FilterHistory();
                    }
                    e.Handled = true;
                    return;
                }

                // Alphabetic/Numeric/Symbol input for search
                // Check if key is a character without special modifiers (except Shift)
                if ((keyEvent.Key & (Key.AltMask | Key.CtrlMask)) == 0)
                {
                    uint val = (uint)keyEvent.Key;
                    // Check if it's a printable character
                    if ((val >= 32 && val <= 126) || (val >= 0x20 && val <= 0x7E))
                    {
                        _searchPattern += (char)(val & 0xFFFF);
                        FilterHistory();
                        e.Handled = true;
                        return;
                    }
                }
            };

            Add(_historyList);

            // Help Bar
            _helpBar = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(2),
                Width = Dim.Fill(),
                Height = 1,
                Text = "[Enter] Select [Ctrl+Enter] Open other [Esc] Cancel [L/R] Switch Pane"
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
        }

        private void UpdateTitle()
        {
            Title = _isLeftPane ? "History (Left Pane)" : "History (Right Pane)";
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
            _historyList.ColorScheme = listScheme;

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

        private void FilterHistory()
        {
            _searchTextLabel.Text = _searchPattern;
            
            if (string.IsNullOrEmpty(_searchPattern))
            {
                _filteredHistory = new List<string>(_fullHistory);
            }
            else
            {
                var tempEntries = _fullHistory.Select(p => new FileEntry { Name = p }).ToList();
                var matches = _searchEngine.FindMatches(tempEntries, _searchPattern, _configuration.Migemo.Enabled);
                _filteredHistory = matches.Select(idx => _fullHistory[idx]).ToList();
            }

            _historyList.Source = new ListWrapper(_filteredHistory);
            if (_filteredHistory.Count > 0)
            {
                _historyList.SelectedItem = 0;
            }
        }

        private void SwitchHistory(bool toLeft)
        {
            if (_isLeftPane == toLeft) return;

            _isLeftPane = toLeft;
            _fullHistory = (_isLeftPane ? _historyManager.LeftHistory : _historyManager.RightHistory).ToList();
            _searchPattern = "";
            FilterHistory();
            UpdateTitle();
        }

        protected override bool OnKeyDown(Key key)
        {
            return base.OnKeyDown(key);
        }
    }
}