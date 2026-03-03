using Terminal.Gui;
using TWF.Models;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for displaying and selecting menu items from a menu file
    /// </summary>
    public class MenuDialog : Dialog
    {
        private readonly List<MenuItemDefinition> _menuItems;
        private ListView _menuList = null!;
        private MenuItemDefinition? _selectedItem;
        private int previousSelectedItem;

        /// <summary>
        /// Gets the selected menu item, or null if cancelled
        /// </summary>
        public MenuItemDefinition? SelectedItem => _selectedItem;

        public MenuDialog(List<MenuItemDefinition> menuItems, string title = "Menu") : base(title, 60, 15)
        {
            _menuItems = menuItems ?? throw new ArgumentNullException(nameof(menuItems));
            InitializeComponents();
        }

        private void InitializeComponents()
        {
            // Menu list
            _menuList = new ListView()
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1),
                Height = Dim.Fill(3),
                AllowsMarking = false
            };

            // Format menu items - separators displayed as horizontal lines
            var displayItems = _menuItems.Select(item => 
                item.IsSeparator ? "─────────────────────────────────────────────────────" : item.Name
            ).ToList();

            _menuList.SetSource(displayItems);

            // Set initial selection to first selectable item
            int firstSelectableIndex = FindFirstSelectableIndex();
            if (firstSelectableIndex >= 0)
            {
                _menuList.SelectedItem = firstSelectableIndex;
            }

            // Override the ListView's selection changed event to skip separators
            _menuList.SelectedItemChanged += (args) =>
            {
                // If a separator is selected, move to next selectable item
                if (_menuList.SelectedItem >= 0 && _menuList.SelectedItem < _menuItems.Count)
                {
                    if (_menuItems[_menuList.SelectedItem].IsSeparator)
                    {
                        // set direction
                        int direction = 1;
                        if (_menuList.SelectedItem < previousSelectedItem || (_menuList.SelectedItem > previousSelectedItem && _menuList.SelectedItem == firstSelectableIndex))
                        {
                            direction = -1;
                        }

                        // Find next selectable item
                        int nextIndex = GetNextSelectableIndex(_menuList.SelectedItem, direction);
                        if (nextIndex != _menuList.SelectedItem)
                        {
                            _menuList.SelectedItem = nextIndex;
                        }
                    }
                }
                previousSelectedItem = _menuList.SelectedItem;
            };

            Add(_menuList);

            // Buttons
            var selectButton = new Button("Select", is_default: true);
            selectButton.Clicked += () =>
            {
                if (_menuList.SelectedItem >= 0 && _menuList.SelectedItem < _menuItems.Count)
                {
                    var selectedMenuItem = _menuItems[_menuList.SelectedItem];
                    if (selectedMenuItem.IsSelectable)
                    {
                        _selectedItem = selectedMenuItem;
                        Application.RequestStop();
                    }
                }
            };

            var cancelButton = new Button("Cancel");
            cancelButton.Clicked += () =>
            {
                _selectedItem = null;
                Application.RequestStop();
            };

            AddButton(selectButton);
            AddButton(cancelButton);
        }

        /// <summary>
        /// Finds the index of the first selectable menu item
        /// </summary>
        /// <returns>Index of first selectable item, or -1 if none found</returns>
        private int FindFirstSelectableIndex()
        {
            for (int i = 0; i < _menuItems.Count; i++)
            {
                if (_menuItems[i].IsSelectable)
                {
                    return i;
                }
            }
            return -1;
        }

        /// <summary>
        /// Gets the next selectable index in the specified direction, skipping separators
        /// </summary>
        /// <param name="currentIndex">Current index</param>
        /// <param name="direction">Direction to move: 1 for down, -1 for up</param>
        /// <returns>Next selectable index, wrapping around if necessary</returns>
        private int GetNextSelectableIndex(int currentIndex, int direction)
        {
            if (_menuItems.Count == 0)
                return -1;

            int nextIndex = currentIndex;
            int attempts = 0;
            int maxAttempts = _menuItems.Count;

            do
            {
                nextIndex += direction;
                
                // Wrap around at boundaries
                if (nextIndex < 0)
                    nextIndex = _menuItems.Count - 1;
                else if (nextIndex >= _menuItems.Count)
                    nextIndex = 0;

                attempts++;

                // If we've checked all items and none are selectable, return current
                if (attempts >= maxAttempts)
                    return currentIndex;

            } while (!_menuItems[nextIndex].IsSelectable);

            return nextIndex;
        }

        /// <summary>
        /// Jumps to the next menu item that starts with the specified letter
        /// </summary>
        /// <param name="letter">The letter to search for</param>
        private void JumpToNextMatch(char letter)
        {
            if (_menuItems.Count == 0)
                return;

            int currentIndex = _menuList.SelectedItem;
            char searchChar = char.ToUpper(letter);

            // Search from current position + 1 to end
            for (int i = currentIndex + 1; i < _menuItems.Count; i++)
            {
                if (_menuItems[i].IsSelectable && 
                    _menuItems[i].Name.Length > 0 && 
                    char.ToUpper(_menuItems[i].Name[0]) == searchChar)
                {
                    _menuList.SelectedItem = i;
                    return;
                }
            }

            // Wrap around: search from beginning to current position
            for (int i = 0; i <= currentIndex; i++)
            {
                if (_menuItems[i].IsSelectable && 
                    _menuItems[i].Name.Length > 0 && 
                    char.ToUpper(_menuItems[i].Name[0]) == searchChar)
                {
                    _menuList.SelectedItem = i;
                    return;
                }
            }

            // No match found - selection stays at current position
        }

        /// <summary>
        /// Override OnKeyDown to handle Up/Down arrow navigation, letter jump, Enter, and Escape
        /// </summary>
        protected override bool OnKeyDown(Key key)
        {
            if (key == Key.Enter)
            {
                // Handle Enter key to confirm selection
                if (_menuList.SelectedItem >= 0 && _menuList.SelectedItem < _menuItems.Count)
                {
                    var selectedMenuItem = _menuItems[_menuList.SelectedItem];
                    if (selectedMenuItem.IsSelectable)
                    {
                        _selectedItem = selectedMenuItem;
                        Application.RequestStop();
                    }
                }
                return true;
            }
            else if (key == Key.Esc)
            {
                // Handle Escape key to cancel
                _selectedItem = null;
                Application.RequestStop();
                return true;
            }
            else if (key == Key.CursorDown)
            {
                int currentIndex = _menuList.SelectedItem;
                int nextIndex = GetNextSelectableIndex(currentIndex, 1);
                if (nextIndex != currentIndex)
                {
                    _menuList.SelectedItem = nextIndex;
                }
                return true;
            }
            else if (key == Key.CursorUp)
            {
                int currentIndex = _menuList.SelectedItem;
                int nextIndex = GetNextSelectableIndex(currentIndex, -1);
                if (nextIndex != currentIndex)
                {
                    _menuList.SelectedItem = nextIndex;
                }
                return true;
            }
            else if (key >= (Key)'A' && key <= (Key)'Z')
            {
                // Handle uppercase letters
                char letter = (char)key;
                JumpToNextMatch(letter);
                return true;
            }
            else if (key >= (Key)'a' && key <= (Key)'z')
            {
                // Handle lowercase letters
                char letter = (char)key;
                JumpToNextMatch(letter);
                return true;
            }

            return base.OnKeyDown(key);
        }
    }
}
