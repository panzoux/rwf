using Terminal.Gui;
using TWF.Models;
using TWF.Utilities;
using System.Drawing;

namespace TWF.UI
{
    /// <summary>
    /// Custom view component for displaying file lists with support for multiple display modes,
    /// marked file indicators, and cursor highlighting
    /// </summary>
    public class PaneView : View
    {
        private PaneState? _state;
        private bool _isActive;
        private int _visibleLines;
        private Configuration? _configuration;
        
        /// <summary>
        /// Gets or sets the pane state to display
        /// </summary>
        public PaneState? State
        {
            get => _state;
            set
            {
                _state = value;
                SetNeedsDisplay();
            }
        }
        
        /// <summary>
        /// Gets or sets whether this pane is currently active
        /// </summary>
        public bool IsActive
        {
            get => _isActive;
            set
            {
                _isActive = value;
                SetNeedsDisplay();
            }
        }
        
        /// <summary>
        /// Gets or sets the configuration for color settings
        /// </summary>
        public Configuration? Configuration
        {
            get => _configuration;
            set
            {
                _configuration = value;
                SetNeedsDisplay();
            }
        }
        
        /// <summary>
        /// Initializes a new instance of PaneView
        /// </summary>
        public PaneView()
        {
            CanFocus = true;
            _visibleLines = 0;
        }
        
        /// <summary>
        /// Renders the pane view
        /// </summary>
        /// <summary>
        /// Gets the list of currently visible file entries
        /// </summary>
        public IEnumerable<FileEntry> GetVisibleEntries()
        {
            if (_state == null || _state.Entries == null || _state.Entries.Count == 0) return Enumerable.Empty<FileEntry>();

            var bounds = Bounds;
            int visibleHeight = bounds.Height - 2; // Subtract borders
            if (visibleHeight <= 0) return Enumerable.Empty<FileEntry>();

            return _state.Entries.Skip(_state.ScrollOffset).Take(visibleHeight);
        }

        protected override bool OnDrawingContent()
        {
            var bounds = Viewport;
            
            if (_state == null || _state.Entries == null || _state.Entries.Count == 0)
            {
                DrawEmptyPane();
                return true;
            }
            
            _visibleLines = bounds.Height;
            
            // Calculate scroll offset to keep cursor visible
            AdjustScrollOffset();
            
            // Draw each visible line
            int lineNumber = 0;
            for (; lineNumber < _visibleLines && lineNumber + _state.ScrollOffset < _state.Entries.Count; lineNumber++)
            {
                int entryIndex = lineNumber + _state.ScrollOffset;
                DrawEntry(lineNumber, entryIndex);
            }
            
            // Fill remaining lines with blank space (for proper background color)
            for (; lineNumber < _visibleLines; lineNumber++)
            {
                DrawBlankLine(lineNumber);
            }
            
            return true;
        }
        
        /// <summary>
        /// Draws a blank line with the normal background color
        /// </summary>
        private void DrawBlankLine(int lineNumber)
        {
            Move(0, lineNumber);
            SetAttribute(GetNormalColorAttribute());
            
            // Fill the entire line with spaces
            string blankLine = new string(' ', Bounds.Width);
            AddStr(blankLine);
        }
        
        /// <summary>
        /// Draws an empty pane message
        /// </summary>
        private void DrawEmptyPane()
        {
            var message = _state == null ? "No state" : "Empty directory";
            var attr = GetNormalColorAttribute();
            
            // Draw the message on the first line
            Move(0, 0);
            SetAttribute(attr);
            AddStr(message.PadRight(Bounds.Width));
            
            // Fill remaining lines with blank space
            for (int i = 1; i < Bounds.Height; i++)
            {
                Move(0, i);
                SetAttribute(attr);
                AddStr(new string(' ', Bounds.Width));
            }
        }
        
        /// <summary>
        /// Adjusts scroll offset to keep cursor visible
        /// </summary>
        private void AdjustScrollOffset()
        {
            if (_state == null) return;
            
            // Ensure cursor is within bounds
            if (_state.CursorPosition < 0)
                _state.CursorPosition = 0;
            if (_state.CursorPosition >= _state.Entries.Count)
                _state.CursorPosition = _state.Entries.Count - 1;
            
            // Adjust scroll offset to keep cursor visible
            if (_state.CursorPosition < _state.ScrollOffset)
            {
                _state.ScrollOffset = _state.CursorPosition;
            }
            else if (_state.CursorPosition >= _state.ScrollOffset + _visibleLines)
            {
                _state.ScrollOffset = _state.CursorPosition - _visibleLines + 1;
            }
            
            // Ensure scroll offset is valid
            if (_state.ScrollOffset < 0)
                _state.ScrollOffset = 0;
        }
        
        /// <summary>
        /// Draws a single file entry
        /// </summary>
        private void DrawEntry(int lineNumber, int entryIndex)
        {
            if (_state == null || entryIndex >= _state.Entries.Count) return;
            
            var entry = _state.Entries[entryIndex];
            bool isMarked = entry.IsMarked;
            bool isCursor = entryIndex == _state.CursorPosition;
            
            Move(0, lineNumber);
            
            // Set color based on state
            Terminal.Gui.Attribute color;
            if (isCursor && _isActive)
            {
                color = GetCursorColorAttribute();
            }
            else if (isCursor && !_isActive)
            {
                color = GetInactiveCursorColorAttribute();
            }
            else if (entry.IsDirectory)
            {
                color = GetDirectoryColorAttribute();
            }
            else
            {
                color = GetNormalColorAttribute();
            }
            
            SetAttribute(color);
            
            // Draw mark indicator
            string markIndicator = isMarked ? "*" : " ";
            AddStr(markIndicator);
            
            // Draw file entry
            string displayText = FormatEntryForDisplay(entry);
            
            // Truncate or pad to fit width (accounting for CJK character widths)
            int availableWidth = Bounds.Width - 2; // Account for mark indicator and padding
            int currentWidth = TWF.Utilities.CharacterWidthHelper.GetStringWidth(displayText);
            
            if (currentWidth > availableWidth)
            {
                string ellipsis = _configuration?.Display?.Ellipsis ?? "...";
                displayText = TWF.Utilities.CharacterWidthHelper.SmartTruncate(displayText, availableWidth, ellipsis);
            }
            else if (currentWidth < availableWidth)
            {
                displayText = TWF.Utilities.CharacterWidthHelper.PadToWidth(displayText, availableWidth);
            }
            
            AddStr(displayText);
        }
        
        /// <summary>
        /// Formats a file entry for display based on the current display mode
        /// </summary>
        private string FormatEntryForDisplay(FileEntry entry)
        {
            if (_state == null) return entry.Name;
            
            return _state.DisplayMode switch
            {
                DisplayMode.Details => FormatDetailView(entry),
                DisplayMode.NameOnly => entry.Name,
                DisplayMode.OneColumn => entry.Name,
                DisplayMode.TwoColumns => FormatColumnView(entry, 2),
                DisplayMode.ThreeColumns => FormatColumnView(entry, 3),
                DisplayMode.FourColumns => FormatColumnView(entry, 4),
                DisplayMode.FiveColumns => FormatColumnView(entry, 5),
                DisplayMode.SixColumns => FormatColumnView(entry, 6),
                DisplayMode.SevenColumns => FormatColumnView(entry, 7),
                DisplayMode.EightColumns => FormatColumnView(entry, 8),
                DisplayMode.Thumbnail => entry.Name + " [IMG]",
                DisplayMode.Icon => GetIconPrefix(entry) + entry.Name,
                _ => entry.Name
            };
        }
        
        /// <summary>
        /// Formats entry for detail view with size, date, and attributes
        /// </summary>
        private string FormatDetailView(FileEntry entry)
        {
            var sizeStr = entry.IsDirectory ? "<DIR>".PadLeft(10) : FormatSize(entry.Size).PadLeft(10);
            var dateStr = entry.LastModified.ToString("yyyy-MM-dd HH:mm");
            var attrStr = FormatAttributes(entry);
            
            // Calculate available space for name (accounting for CJK character widths)
            int nameWidth = Math.Max(20, Bounds.Width - 35); // Reserve space for size, date, attrs
            
            // Use CharacterWidthHelper to handle CJK characters properly
            string name;
            int currentWidth = CharacterWidthHelper.GetStringWidth(entry.Name);
            
            if (currentWidth > nameWidth)
            {
                // Truncate to fit, accounting for display width
                string ellipsis = _configuration?.Display?.Ellipsis ?? "...";
                name = CharacterWidthHelper.SmartTruncate(entry.Name, nameWidth, ellipsis);
            }
            else
            {
                // Pad to fixed width, accounting for display width
                name = CharacterWidthHelper.PadToWidth(entry.Name, nameWidth);
            }
            
            return $"{name} {sizeStr} {dateStr} {attrStr}";
        }
        
        /// <summary>
        /// Formats entry for multi-column view
        /// </summary>
        private string FormatColumnView(FileEntry entry, int columns)
        {
            // For multi-column views, just show the name
            // The actual column layout would be handled by the rendering logic
            return entry.Name;
        }
        
        /// <summary>
        /// Gets an icon prefix for the entry based on its type
        /// </summary>
        private string GetIconPrefix(FileEntry entry)
        {
            if (entry.IsDirectory)
                return "[DIR] ";
            if (entry.IsArchive)
                return "[ZIP] ";
            
            return entry.Extension.ToLower() switch
            {
                ".txt" or ".md" or ".log" => "[TXT] ",
                ".jpg" or ".jpeg" or ".png" or ".gif" or ".bmp" => "[IMG] ",
                ".exe" or ".dll" or ".com" => "[EXE] ",
                ".zip" or ".rar" or ".7z" or ".tar" or ".gz" => "[ARC] ",
                _ => "[   ] "
            };
        }
        
        /// <summary>
        /// Formats file size for display
        /// </summary>
        private string FormatSize(long bytes)
        {
            if (bytes < 1024)
                return $"{bytes} B";
            if (bytes < 1024 * 1024)
                return $"{bytes / 1024.0:F1} KB";
            if (bytes < 1024 * 1024 * 1024)
                return $"{bytes / (1024.0 * 1024.0):F1} MB";
            return $"{bytes / (1024.0 * 1024.0 * 1024.0):F1} GB";
        }
        
        /// <summary>
        /// Formats file attributes for display
        /// </summary>
        private string FormatAttributes(FileEntry entry)
        {
            var attrs = "";
            if (entry.Attributes.HasFlag(FileAttributes.ReadOnly)) attrs += "R";
            if (entry.Attributes.HasFlag(FileAttributes.Hidden)) attrs += "H";
            if (entry.Attributes.HasFlag(FileAttributes.System)) attrs += "S";
            if (entry.Attributes.HasFlag(FileAttributes.Archive)) attrs += "A";
            return attrs.PadRight(4);
        }
        
        /// <summary>
        /// Gets the normal color attribute
        /// </summary>
        private Terminal.Gui.Attribute GetNormalColorAttribute()
        {
            if (_isActive)
            {
                return Application.Driver.MakeAttribute(Color.White, Color.Black);
            }
            else
            {
                return Application.Driver.MakeAttribute(Color.Gray, Color.Black);
            }
        }
        
        /// <summary>
        /// Gets the directory color attribute from configuration
        /// </summary>
        private Terminal.Gui.Attribute GetDirectoryColorAttribute()
        {
            if (_configuration?.Display != null)
            {
                if (_isActive)
                {
                    var foreground = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration.Display.DirectoryColor, Color.BrightCyan);
                    var background = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration.Display.DirectoryBackgroundColor, Color.Black);
                    return Application.Driver.MakeAttribute(foreground, background);
                }
                else
                {
                    var foreground = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration.Display.InactiveDirectoryColor, Color.Cyan);
                    var background = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration.Display.InactiveDirectoryBackgroundColor, Color.Black);
                    return Application.Driver.MakeAttribute(foreground, background);
                }
            }
            
            // Default directory color if no configuration
            if (_isActive)
            {
                return Application.Driver.MakeAttribute(Color.BrightCyan, Color.Black);
            }
            else
            {
                return Application.Driver.MakeAttribute(Color.Cyan, Color.Black);
            }
        }
        
        /// <summary>
        /// Gets the cursor color attribute for active pane
        /// </summary>
        private Terminal.Gui.Attribute GetCursorColorAttribute()
        {
            if (_configuration?.Display != null)
            {
                var foreground = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration.Display.HighlightForegroundColor, Color.Black);
                var background = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration.Display.HighlightBackgroundColor, Color.Cyan);
                return Application.Driver.MakeAttribute(foreground, background);
            }
            
            return Application.Driver.MakeAttribute(Color.Black, Color.Cyan);
        }
        
        /// <summary>
        /// Gets the cursor color attribute for inactive pane
        /// </summary>
        private Terminal.Gui.Attribute GetInactiveCursorColorAttribute()
        {
            return Application.Driver.MakeAttribute(Color.Gray, Color.DarkGray);
        }
        

        
        /// <summary>
        /// Moves the cursor up by one position
        /// </summary>
        public void MoveCursorUp()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            if (_state.CursorPosition > 0)
            {
                _state.CursorPosition--;
                SetNeedsDisplay();
            }
        }
        
        /// <summary>
        /// Moves the cursor down by one position
        /// </summary>
        public void MoveCursorDown()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            if (_state.CursorPosition < _state.Entries.Count - 1)
            {
                _state.CursorPosition++;
                SetNeedsDisplay();
            }
        }
        
        /// <summary>
        /// Moves the cursor to the first entry
        /// </summary>
        public void MoveCursorToFirst()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            _state.CursorPosition = 0;
            SetNeedsDisplay();
        }
        
        /// <summary>
        /// Moves the cursor to the last entry
        /// </summary>
        public void MoveCursorToLast()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            _state.CursorPosition = _state.Entries.Count - 1;
            SetNeedsDisplay();
        }
        
        /// <summary>
        /// Moves the cursor up by one page
        /// </summary>
        public void PageUp()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            _state.CursorPosition = Math.Max(0, _state.CursorPosition - _visibleLines);
            SetNeedsDisplay();
        }
        
        /// <summary>
        /// Moves the cursor down by one page
        /// </summary>
        public void PageDown()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            _state.CursorPosition = Math.Min(_state.Entries.Count - 1, _state.CursorPosition + _visibleLines);
            SetNeedsDisplay();
        }
        
        /// <summary>
        /// Toggles the mark on the current entry
        /// </summary>
        public void ToggleMark()
        {
            if (_state == null || _state.Entries.Count == 0) return;
            
            var entry = _state.Entries[_state.CursorPosition];
            entry.IsMarked = !entry.IsMarked;
            
            SetNeedsDisplay();
        }
        
        /// <summary>
        /// Gets the current entry under the cursor
        /// </summary>
        public FileEntry? GetCurrentEntry()
        {
            return _state?.GetCurrentEntry();
        }
        
        /// <summary>
        /// Gets all marked entries
        /// </summary>
        public List<FileEntry> GetMarkedEntries()
        {
            return _state?.GetMarkedEntries() ?? new List<FileEntry>();
        }
    }
}
