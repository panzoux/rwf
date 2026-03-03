using System;
using Terminal.Gui;
using TWF.Models;

namespace TWF.UI
{
    public class SortDialog : Dialog
    {
        public SortMode SelectedMode { get; private set; }
        public bool IsOk { get; private set; }

        public SortDialog(SortMode currentMode, string paneTitle) : base($"Sort ({paneTitle})", 40, 16)
        {
            var modes = new[] 
            {
                SortMode.NameAscending, SortMode.NameDescending,
                SortMode.ExtensionAscending, SortMode.ExtensionDescending,
                SortMode.SizeAscending, SortMode.SizeDescending,
                SortMode.DateAscending, SortMode.DateDescending,
                SortMode.Unsorted
            };

            var listItems = new List<string> 
            {
                "Name (Ascending)", "Name (Descending)",
                "Extension (Ascending)", "Extension (Descending)",
                "Size (Ascending)", "Size (Descending)",
                "Date (Ascending)", "Date (Descending)",
                "Unsorted"
            };

            int selectedIndex = Array.IndexOf(modes, currentMode);
            if (selectedIndex < 0) selectedIndex = 0;

            var listView = new ListView(listItems)
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1),
                Height = Dim.Fill(2), // Space for buttons
                AllowsMarking = false,
                SelectedItem = selectedIndex
            };
            Add(listView);

            // Handle Enter to select
            listView.OpenSelectedItem += (e) =>
            {
                SelectedMode = modes[e.Item];
                IsOk = true;
                Application.RequestStop();
            };

            var okBtn = new Button("OK") 
            { 
                X = Pos.Center() - 6,
                Y = Pos.AnchorEnd(1),
                IsDefault = true 
            };
            okBtn.Clicked += () =>
            {
                if (listView.SelectedItem >= 0 && listView.SelectedItem < modes.Length)
                {
                    SelectedMode = modes[listView.SelectedItem];
                    IsOk = true;
                }
                Application.RequestStop();
            };

            var cancelBtn = new Button("Cancel") 
            { 
                X = Pos.Center() + 4,
                Y = Pos.AnchorEnd(1) 
            };
            cancelBtn.Clicked += () => { Application.RequestStop(); };

            AddButton(okBtn);
            AddButton(cancelBtn);
        }
    }
}
