using System;
using System.Collections.Generic;
using System.Linq;
using Terminal.Gui;
using TWF.Models;
using TWF.Services;
using TWF.Utilities;

namespace TWF.UI
{
    /// <summary>
    /// Enhanced help view with incremental search and multi-language support from JSON files
    /// </summary>
    public class HelpView : Dialog
    {
        private readonly HelpManager _helpManager;
        private readonly SearchEngine _searchEngine;
        private readonly UiMode _mode;
        
        private TextField _searchField = null!;
        private ListView _listView = null!;
        private Label _helpBar = null!;
        private List<HelpItem> _currentItems = new List<HelpItem>();
        private readonly List<string> _availableLanguages = new List<string> { "en", "jp" };

        public HelpView(HelpManager helpManager, SearchEngine searchEngine, UiMode mode = UiMode.Normal) 
            : base("TWF Help", Application.Driver.Cols - 10, Application.Driver.Rows - 6)
        {
            _helpManager = helpManager;
            _searchEngine = searchEngine;
            _mode = mode;
            
            InitializeUI();
            UpdateList();
        }

        private void InitializeUI()
        {
            var searchLabel = new Label("Filter:") { X = 1, Y = 1 };
            _searchField = new TextField("")
            {
                X = Pos.Right(searchLabel) + 1,
                Y = 1,
                Width = Dim.Fill(1)
            };
            _searchField.TextChanged += (e) => UpdateList();

            var header = new Label(string.Format("  {0} | {1} | {2}", 
                CharacterWidthHelper.PadToWidth("Category", 15), 
                CharacterWidthHelper.PadToWidth("Keys", 15), 
                "Description"))
            {
                X = 0,
                Y = 2,
                Width = Dim.Fill(),
                // Remove Colors.Menu to match ListView
            };

            var separator = new Label(new string('─', Application.Driver.Cols))
            {
                X = 0,
                Y = 3,
                Width = Dim.Fill()
            };

            _listView = new ListView()
            {
                X = 0,
                Y = 4,
                Width = Dim.Fill(),
                Height = Dim.Fill(2),
                AllowsMarking = false,
                CanFocus = false // Prevent list from taking focus and showing cursor
            };

            _helpBar = new Label("")
            {
                X = 0,
                Y = Pos.AnchorEnd(1),
                Width = Dim.Fill(),
                Height = 1,
                // Remove Colors.Menu override for consistency
            };
            UpdateHelpBar();

            var closeButton = new Button("Close") { X = Pos.Center(), Y = Pos.AnchorEnd(1) };
            closeButton.Clicked += () => Application.RequestStop();

            Add(searchLabel, _searchField, header, separator, _listView, _helpBar);
            AddButton(closeButton);

            // Initially focus search
            _searchField.SetFocus();
        }

        private void UpdateList()
        {
            string query = _searchField.Text?.ToString() ?? string.Empty;
            _currentItems = _helpManager.GetFilteredItems(query, _searchEngine, _mode);
            
            var displayList = _currentItems.Select(item => 
                string.Format("  {0} | {1} | {2}", 
                    CharacterWidthHelper.PadToWidth(item.Category, 15), 
                    CharacterWidthHelper.PadToWidth(item.BoundKeys, 15), 
                    item.Description)
            ).ToList();

            _listView.SetSource(displayList);
        }

        private void UpdateHelpBar()
        {
            string currentLang = _helpManager.CurrentLanguage.ToUpper();
            int currentIndex = _availableLanguages.IndexOf(_helpManager.CurrentLanguage);
            string nextLang = _availableLanguages[(currentIndex + 1) % _availableLanguages.Count].ToUpper();

            Title = $"TWF Help ({currentLang})";

            _helpBar.Text = $" [Ctrl+L]: Switch to {nextLang}  [Up/Down/Pg]: Nav  [Enter/Esc]: Close ";
        }

        protected override bool OnKeyDown(Key key)
        {
            // Ctrl+L to rotate language
            if (key == Key.L.WithCtrl)
            {
                int currentIndex = _availableLanguages.IndexOf(_helpManager.CurrentLanguage);
                int nextIndex = (currentIndex + 1) % _availableLanguages.Count;
                _helpManager.Reload(_availableLanguages[nextIndex]);
                UpdateHelpBar();
                UpdateList();
                return true;
            }

            // / or Ctrl+F to refocus search
            if (key == (Key) '/' || key == Key.F.WithCtrl)
            {
                _searchField.SetFocus();
                return true;
            }

            // Proxy navigation keys to ListView
            if (key == Key.CursorDown || key == Key.CursorUp || 
                key == Key.PageDown || key == Key.PageUp ||
                key == Key.Home || key == Key.End)
            {
                return _listView.OnKeyDown(key);
            }

            if (key == Key.Esc || key == Key.Enter)
            {
                Application.RequestStop();
                return true;
            }

            return base.OnKeyDown(key);
        }
    }
}
