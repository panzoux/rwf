using System;
using System.Collections.Generic;
using System.Drawing;
using System.Text;
using Terminal.Gui;
using TWF.Services;

namespace TWF.UI
{
    public class VirtualFileView : View
    {
        private readonly LargeFileEngine _engine;
        private FileViewMode _mode = FileViewMode.Text;
        private long _scrollOffset = 0; // Line index (Text) or Row index (Hex)
        private int _horizontalOffset = 0;
        private int _contentHeight = 0;
        
        public VirtualFileView(LargeFileEngine engine)
        {
            _engine = engine;
            CanFocus = true;
        }

        public event Action? OffsetChanged;

        public FileViewMode Mode
        {
            get => _mode;
            set
            {
                _mode = value;
                _scrollOffset = 0;
                _horizontalOffset = 0;
                SetNeedsDisplay();
                OffsetChanged?.Invoke();
            }
        }

        public long ScrollOffset
        {
            get => _scrollOffset;
            set
            {
                long max = GetMaxScrollOffset();
                long newValue = Math.Clamp(value, 0, max);
                if (_scrollOffset != newValue)
                {
                    _scrollOffset = newValue;
                    SetNeedsDisplay();
                    OffsetChanged?.Invoke();
                }
            }
        }

        public int HorizontalOffset
        {
            get => _horizontalOffset;
            set
            {
                int newValue = Math.Max(0, value);
                if (_horizontalOffset != newValue)
                {
                    _horizontalOffset = newValue;
                    SetNeedsDisplay();
                    OffsetChanged?.Invoke();
                }
            }
        }

        public string? HighlightPattern { get; set; }
        public bool IsRegex { get; set; }

        private long GetMaxScrollOffset()
        {
            // Use Frame.Height if _contentHeight hasn't been set by Redraw yet
            int height = _contentHeight > 0 ? _contentHeight : Frame.Height;

            if (_mode == FileViewMode.Text)
            {
                 // Clamping so the last line stays at the bottom of the screen if possible
                 return Math.Max(0, _engine.LineCount - height);
            }
            else
            {
                 // Hex mode: 16 bytes per row
                 long totalRows = (_engine.FileSize + 15) / 16;
                 return Math.Max(0, totalRows - height);
            }
        }

        // Navigation Methods
        public void ScrollUp(int lines) => ScrollOffset -= lines;
        public void ScrollDown(int lines) => ScrollOffset += lines;
        public void ScrollToTop() => ScrollOffset = 0;
        public void ScrollToBottom() 
        {
            if (_mode == FileViewMode.Text)
                ScrollOffset = Math.Max(0, _engine.LineCount - _contentHeight);
            else
                ScrollOffset = Math.Max(0, (_engine.FileSize / 16) - _contentHeight);
        }

        public void PageUp() => ScrollUp(_contentHeight);
        public void PageDown() => ScrollDown(_contentHeight);

        protected override bool OnDrawingContent()
        {
            var bounds = Viewport;

            _contentHeight = bounds.Height;
            int width = bounds.Width;

            // Clear view
            SetAttribute(GetNormalColor());
            for (int y = 0; y < _contentHeight; y++)
            {
                Move(0, y);
                AddStr(new string(' ', width));
            }

            if (_mode == FileViewMode.Text)
            {
                DrawTextMode(bounds);
            }
            else
            {
                DrawHexMode(bounds);
            }
            
            return true;
        }

        private void DrawTextMode(Rectangle bounds)
        {
            // Calculate how many lines we can show
            int linesToShow = bounds.Height;
            
            // Fetch lines from engine
            // scrollOffset is line index
            int startLine = (int)_scrollOffset; // Note: LargeFileEngine uses int for line count, but long is safer for future
            
            var lines = _engine.GetTextLines(startLine, linesToShow);

            int lineNumberWidth = _engine.LineCount.ToString().Length + 1;
            
            for (int i = 0; i < lines.Count; i++)
            {
                int y = i;
                if (y >= bounds.Height) break;

                string lineContent = lines[i];
                // Replace tabs with spaces
                lineContent = lineContent.Replace("\t", "    ");

                // Draw Line Number
                Move(0, y);
                SetAttribute(ColorScheme.HotNormal); // Highlight color for line numbers
                string lineNumStr = (startLine + i + 1).ToString().PadLeft(lineNumberWidth - 1);
                AddStr(lineNumStr);
                AddStr(" | ");

                // Draw Content
                int maxContentWidth = Math.Max(0, bounds.Width - (lineNumberWidth + 3));
                
                // Horizontal scrolling slicing
                string visiblePart = "";
                if (_horizontalOffset < lineContent.Length)
                {
                    int length = Math.Min(maxContentWidth, lineContent.Length - _horizontalOffset);
                    visiblePart = lineContent.Substring(_horizontalOffset, length);
                }

                if (!string.IsNullOrEmpty(HighlightPattern))
                {
                    System.Text.RegularExpressions.Regex? regex = null;
                    if (IsRegex)
                    {
                        try { regex = new System.Text.RegularExpressions.Regex(HighlightPattern, System.Text.RegularExpressions.RegexOptions.IgnoreCase); } catch { }
                    }

                    int lastPos = 0;
                    var normal = GetNormalColor();
                    var inverted = App?.Driver?.MakeAttribute(normal.Background, normal.Foreground) ?? normal;

                    if (regex != null)
                    {
                        var matches = regex.Matches(visiblePart);
                        foreach (System.Text.RegularExpressions.Match match in matches)
                        {
                            SetAttribute(normal);
                            AddStr(visiblePart.Substring(lastPos, match.Index - lastPos));
                            SetAttribute(inverted);
                            AddStr(match.Value);
                            lastPos = match.Index + match.Length;
                        }
                    }
                    else
                    {
                        int idx = visiblePart.IndexOf(HighlightPattern, StringComparison.OrdinalIgnoreCase);
                        while (idx >= 0)
                        {
                            SetAttribute(normal);
                            AddStr(visiblePart.Substring(lastPos, idx - lastPos));
                            SetAttribute(inverted);
                            AddStr(visiblePart.Substring(idx, HighlightPattern.Length));
                            lastPos = idx + HighlightPattern.Length;
                            idx = visiblePart.IndexOf(HighlightPattern, lastPos, StringComparison.OrdinalIgnoreCase);
                        }
                    }
                    SetAttribute(normal);
                    AddStr(visiblePart.Substring(lastPos));
                }
                else
                {
                    SetAttribute(GetNormalColor());
                    AddStr(visiblePart);
                }
            }
        }

        private void DrawHexMode(Rectangle bounds)
        {
            int linesToShow = bounds.Height;
            long startRow = _scrollOffset;
            long startByte = startRow * 16;
            int bytesNeeded = linesToShow * 16;

            byte[] data = _engine.GetBytes(startByte, bytesNeeded);

            for (int i = 0; i < linesToShow; i++)
            {
                int y = i;
                long currentRow = startRow + i;
                long currentOffset = currentRow * 16;

                if (currentOffset >= _engine.FileSize) break;

                // Draw Offset
                Move(0, y);
                SetAttribute(ColorScheme.HotNormal);
                AddStr(currentOffset.ToString("X8"));
                AddStr("  ");

                // Draw Hex
                SetAttribute(GetNormalColor());
                
                int bytesInRow = 16;
                if (currentOffset + 16 > _engine.FileSize)
                    bytesInRow = (int)(_engine.FileSize - currentOffset);

                StringBuilder hexPart = new StringBuilder();
                StringBuilder asciiPart = new StringBuilder();

                for (int b = 0; b < 16; b++)
                {
                    if (b < bytesInRow)
                    {
                        int byteIdx = (i * 16) + b;
                        if (byteIdx < data.Length)
                        {
                            byte val = data[byteIdx];
                            hexPart.Append(val.ToString("X2"));
                            asciiPart.Append((val >= 32 && val <= 126) ? (char)val : '.');
                        }
                        else 
                        {
                            // Should not happen if logic is correct
                            hexPart.Append("  ");
                            asciiPart.Append(" ");
                        }
                    }
                    else
                    {
                        hexPart.Append("  ");
                        asciiPart.Append(" ");
                    }

                    hexPart.Append(" ");
                    if (b == 7) hexPart.Append(" ");
                }

                AddStr(hexPart.ToString());
                AddStr(" |");
                AddStr(asciiPart.ToString());
                AddStr("|");
            }
        }
    }
}
