using Terminal.Gui;
using TWF.Services;
using TWF.Models;
using System.Text;
using Microsoft.Extensions.Logging;

namespace TWF.UI
{
    /// <summary>
    /// Text/Binary viewer window for displaying file contents efficiently
    /// </summary>
    public class TextViewerWindow : Window
    {
        private readonly LargeFileEngine _fileEngine;
        private readonly KeyBindingManager _keyBindings;
        private readonly SearchEngine _searchEngine; // Injected
        private readonly Configuration? _configuration;
        private readonly ILogger<TextViewerWindow>? _logger;
        
        private VirtualFileView _fileView = null!;
        private Label _statusLabel = null!;
        private Label _messageLabel = null!;
        
        // Search State
        private bool _isSearchMode = false;
        private StringBuilder _searchQuery = new StringBuilder();
        private string _lastSearchTerm = string.Empty;
        private List<string> _searchHistory = new List<string>();
        private int _historyIndex = -1;
        private string _searchStatusText = string.Empty;
        private bool _searchBackwards = false;
        private long _searchStartLine = 0;
        private CancellationTokenSource? _searchCts;
        private object _statusUpdateToken;

        /// <summary>
        /// Initializes a new instance of TextViewerWindow
        /// </summary>
        public TextViewerWindow(LargeFileEngine fileEngine, KeyBindingManager keyBindings, SearchEngine searchEngine, Configuration? configuration = null, ILogger<TextViewerWindow>? logger = null, bool startInHexMode = false) : base(startInHexMode ? "Binary Viewer" : "Text Viewer")
        {
            _fileEngine = fileEngine ?? throw new ArgumentNullException(nameof(fileEngine));
            _keyBindings = keyBindings ?? throw new ArgumentNullException(nameof(keyBindings));
            _searchEngine = searchEngine ?? throw new ArgumentNullException(nameof(searchEngine));
            _configuration = configuration;
            _logger = logger;
            
            InitializeComponents();
            
            if (startInHexMode)
            {
                _fileView.Mode = FileViewMode.Hex;
            }
            
            // Hook into engine events
            // We do NOT hook into IndexingProgressChanged here because it fires too frequently (every 1MB)
            // and flooding MainLoop.Invoke causes high CPU usage.
            // Instead, we rely on the timer below to poll for updates.
            
            _fileEngine.IndexingCompleted += (s, e) => Application.MainLoop.Invoke(() => { 
                _logger?.LogInformation("IndexingCompleted event received");
                UpdateStatusLabel(); 
                UpdateMessageLabel(); 
                _fileView.SetNeedsDisplay(); // Final redraw to ensure all lines are accessible
            });

            // Start status update timer for indexing progress
            _statusUpdateToken = Application.MainLoop.AddTimeout(TimeSpan.FromMilliseconds(500), (loop) => 
            {
                if (_fileEngine.IsIndexing)
                {
                    UpdateStatusLabel();
                    UpdateMessageLabel();
                    
                    // Smart Redraw: Only redraw file view if new lines are visible
                    // This prevents high CPU usage from constant redrawing when viewing top of large files
                    long topVisibleLine = _fileView.ScrollOffset;
                    long bottomVisibleLine = topVisibleLine + _fileView.Frame.Height;
                    
                    // If the current indexed line count is within or just past the visible range, redraw
                    // This handles the initial load and "tailing" behavior if user is at the bottom
                    if (_fileEngine.LineCount >= topVisibleLine && _fileEngine.LineCount <= bottomVisibleLine + 100) 
                    {
                        _fileView.SetNeedsDisplay();
                    }
                    
                    return true;
                }
                return true;
            });

            // Cleanup timer on close
            this.Closed += (e) => {
                if (_statusUpdateToken != null)
                {
                    Application.MainLoop.RemoveTimeout(_statusUpdateToken);
                    _statusUpdateToken = null!;
                }
            };
        }

        private void InitializeComponents()
        {
            X = 0;
            Y = 0;
            Width = Dim.Fill();
            Height = Dim.Fill();
            Modal = true;

            // Colors
            var textFg = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration?.Viewer.TextViewerForegroundColor, Color.White);
            var textBg = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration?.Viewer.TextViewerBackgroundColor, Color.Black);
            var statusFg = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration?.Viewer.TextViewerStatusForegroundColor, Color.Black);
            var statusBg = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration?.Viewer.TextViewerStatusBackgroundColor, Color.Gray);
            var messageFg = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration?.Viewer.TextViewerMessageForegroundColor, Color.White);
            var messageBg = TWF.Utilities.ColorHelper.ParseConfigColor(_configuration?.Viewer.TextViewerMessageBackgroundColor, Color.Blue);

            // Virtual File View
            _fileView = new VirtualFileView(_fileEngine)
            {
                X = 0,
                Y = 0,
                Width = Dim.Fill(),
                Height = Dim.Fill(2)
            };
            
            if (Application.Driver != null)
            {
                _fileView.ColorScheme = new ColorScheme()
                {
                    Normal = Application.Driver.MakeAttribute(textFg, textBg),
                    Focus = Application.Driver.MakeAttribute(textFg, textBg),
                    HotNormal = Application.Driver.MakeAttribute(Color.BrightYellow, textBg) // For line numbers/offsets
                };
            }
            Add(_fileView);

            // Status Label
            _statusLabel = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(2),
                Width = Dim.Fill(),
                Height = 1,
                Text = "Loading..."
            };
            
            if (Application.Driver != null)
            {
                _statusLabel.ColorScheme = new ColorScheme()
                {
                    Normal = Application.Driver.MakeAttribute(statusFg, statusBg)
                };
            }
            Add(_statusLabel);

            // Message Label (formerly Encoding Label)
            _messageLabel = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(1),
                Width = Dim.Fill(),
                Height = 1,
                Text = ""
            };
            
            if (Application.Driver != null)
            {
                _messageLabel.ColorScheme = new ColorScheme()
                {
                    Normal = Application.Driver.MakeAttribute(messageFg, messageBg)
                };
            }
            Add(_messageLabel);

            // Real-time status updates for scrolling
            _fileView.OffsetChanged += () => UpdateStatusLabel();

            UpdateStatusLabel();
            UpdateMessageLabel();
        }

        protected override bool OnKeyDown(Key key)
        {
            string keyString = TWF.Utilities.KeyHelper.ConvertKeyToString(key);
            _logger?.LogDebug("TextViewer Window Key: {Key} (Raw: {RawKey})", keyString, key);
            
            // --- Search Mode Input Handling (Pre-emptive) ---
            if (_isSearchMode)
            {
                if (keyString == "Escape")
                {
                    ExitSearchMode();
                    return true;
                }
                else if (keyString == "Enter")
                {
                    CommitSearch();
                    return true;
                }
                else if (keyString == "Backspace")
                {
                    if (_searchQuery.Length > 0)
                    {
                        _historyIndex = -1; // Reset history index on edit
                        _searchQuery.Length--;
                        TriggerAsyncSearch();
                    }
                    else
                    {
                        ExitSearchMode();
                    }
                    return true;
                }
                else if (keyString == "Ctrl+K")
                {
                    _historyIndex = -1;
                    _searchQuery.Clear();
                    TriggerAsyncSearch();
                    return true;
                }
                else if (keyString == "Up")
                {
                    if (_searchHistory.Count > 0)
                    {
                        if (_historyIndex < _searchHistory.Count - 1)
                        {
                            _historyIndex++;
                            _searchQuery.Clear();
                            _searchQuery.Append(_searchHistory[_historyIndex]);
                            TriggerAsyncSearch();
                        }
                    }
                    return true;
                }
                else if (keyString == "Down")
                {
                    if (_historyIndex > 0)
                    {
                        _historyIndex--;
                        _searchQuery.Clear();
                        _searchQuery.Append(_searchHistory[_historyIndex]);
                        TriggerAsyncSearch();
                    }
                    else if (_historyIndex == 0)
                    {
                        _historyIndex = -1;
                        _searchQuery.Clear();
                        TriggerAsyncSearch();
                    }
                    return true;
                }
                
                // Handle character input
                if (key.IsValid && !key.IsKeyCodeKey && key.AsRune.IsValid)
                {
                    char keyChar = (char)key.AsRune.Value;
                    if (!char.IsControl(keyChar))
                    {
                        _historyIndex = -1; // Reset history index when typing new input
                        _searchQuery.Append(keyChar);
                        TriggerAsyncSearch();
                        return true;
                    }
                }
                
                return true; // Consume keys in search mode
            }

            // --- Standard Mode Handling ---
            // (Log already moved to top)

            // Allow cancelling indexing with Esc
            if (keyString == "Escape" && _fileEngine.IsIndexing)
            {
                _fileEngine.CancelIndexing();
                UpdateStatusLabel();
                UpdateMessageLabel();
                return true;
            }

            // Check custom bindings (/, ?, n, N etc)
            string? action = _keyBindings.GetActionForKey(keyString, UiMode.TextViewer);
            if (action != null)
            {
                if (ExecuteTextViewerAction(action)) return true;
            }

            // Default bindings
            if (ExecuteDefaultBinding(key)) return true;

            return base.OnKeyDown(key);
        }



        private bool ExecuteDefaultBinding(Key key)
        {
            switch (key)
            {
                case Key.F5:
                    _fileView.ScrollToTop();
                    return true;
                case Key.F6:
                    _fileView.ScrollToBottom();
                    return true;
                case Key.PageUp:
                    _fileView.PageUp();
                    return true;
                case Key.PageDown:
                    _fileView.PageDown();
                    return true;
                case Key.Home:
                    _fileView.ScrollToTop();
                    return true;
                case Key.End:
                    _fileView.ScrollToBottom();
                    return true;
                case Key.CursorUp:
                    _fileView.ScrollUp(1);
                    return true;
                case Key.CursorDown:
                    _fileView.ScrollDown(1);
                    return true;
                case Key.CursorLeft:
                    _fileView.HorizontalOffset -= 10;
                    return true;
                case Key.CursorRight:
                    _fileView.HorizontalOffset += 10;
                    return true;
                case Key.Esc:
                    Application.RequestStop();
                    return true;
                case (Key.B | Key.ShiftMask):
                    ToggleHexMode();
                    return true;
                case Key.F7:
                    CycleEncoding();
                    return true;
                case Key.F8:
                    ToggleHexMode();
                    return true;
                case Key.F9:
                    StartSearch(true);
                    return true;
                case (Key)'c':
                case (Key)'C':
                    ClearHighlight();
                    return true;
            }
            return false;
        }

        private void CycleEncoding()
        {
            _fileEngine.CycleEncoding();
            _fileView.ScrollOffset = 0; // Reset scroll as line counts will change
            UpdateStatusLabel();
            UpdateMessageLabel();
            _fileView.SetNeedsDisplay();
        }

        private void StartSearch(bool backwards)
        {
            _isSearchMode = true;
            _searchBackwards = backwards;
            _searchStartLine = _fileView.ScrollOffset; // Anchor position
            _searchQuery.Clear();
            _historyIndex = -1;
            _searchStatusText = string.Empty;
            _fileView.HighlightPattern = null; // Clear previous highlight when starting new search
            UpdateMessageLabel(); 
        }

        private void ExitSearchMode()
        {
            _isSearchMode = false;
            _searchQuery.Clear();
            _searchCts?.Cancel();
            _searchStatusText = string.Empty;
            _fileView.HighlightPattern = null;
            _fileView.ScrollOffset = _searchStartLine; // Restore original position on Cancel
            _fileView.SetNeedsDisplay();
            UpdateMessageLabel();
        }

        private void ClearHighlight()
        {
            _fileView.HighlightPattern = null;
            _fileView.SetNeedsDisplay();
            _searchStatusText = string.Empty;
            UpdateMessageLabel();
        }

        private void CommitSearch()
        {
            string query = _searchQuery.ToString();
            if (!string.IsNullOrEmpty(query))
            {
                _lastSearchTerm = query;
                // Add to history if not same as last
                if (_searchHistory.Count == 0 || _searchHistory[0] != query)
                {
                    _searchHistory.Insert(0, query);
                    if (_searchHistory.Count > 50) _searchHistory.RemoveAt(50);
                }
            }
            _isSearchMode = false;
            _searchStatusText = string.Empty; // Reset status text so label reverts to help/indexing
            UpdateMessageLabel();
        }

        private void TriggerAsyncSearch()
        {
            _historyIndex = -1;
            TriggerAsyncSearchInternal();
        }

        private void TriggerAsyncSearchInternal()
        {
            _searchCts?.Cancel();
            _searchCts = new CancellationTokenSource();
            var token = _searchCts.Token;
            string query = _searchQuery.ToString();
            
            _searchStatusText = string.Empty;
            UpdateMessageLabel();

            if (string.IsNullOrEmpty(query)) 
            {
                _fileView.HighlightPattern = null;
                _fileView.ScrollOffset = _searchStartLine; // Back to start if query empty
                _fileView.SetNeedsDisplay();
                return;
            }

            Task.Run(async () => 
            {
                try
                {
                    bool useMigemo = _searchEngine.IsMigemoAvailable;
                    string pattern = query;
                    if (useMigemo) pattern = _searchEngine.ExpandPattern(query);
                    
                    // ALWAYS search from the anchored start line during incremental typing
                    long? line = await _fileEngine.FindNextAsync(pattern, _searchStartLine, _searchBackwards, useMigemo, token);
                    
                    if (token.IsCancellationRequested) return;

                    Application.MainLoop.Invoke(() => 
                    {
                        _fileView.HighlightPattern = pattern;
                        _fileView.IsRegex = useMigemo;

                        if (line.HasValue)
                        {
                            _fileView.ScrollOffset = line.Value;
                            _searchStatusText = "(found)";
                        }
                        else
                        {
                            _fileView.ScrollOffset = _searchStartLine; // Stay/Return to start if not found
                            _searchStatusText = "(not found)";
                        }
                        _fileView.SetNeedsDisplay();
                        UpdateMessageLabel();
                    });
                }
                catch (Exception) { }
            });
        }

        private void Search()
        {
            StartSearch(false);
        }

        private void FindNext()
        {
            if (string.IsNullOrEmpty(_lastSearchTerm)) return;
            ExecuteFind(_lastSearchTerm, _searchBackwards);
        }

        private void FindPrevious()
        {
            if (string.IsNullOrEmpty(_lastSearchTerm)) return;
            ExecuteFind(_lastSearchTerm, !_searchBackwards);
        }

        private void ExecuteFind(string term, bool backwards)
        {
            _searchCts?.Cancel();
            _searchCts = new CancellationTokenSource();
            var token = _searchCts.Token;
            
            long startLine = _fileView.ScrollOffset;
            if (backwards) startLine--; else startLine++;
            if (startLine < 0) startLine = 0;

            _searchStatusText = "...";
            UpdateMessageLabel();
            
            Task.Run(async () => 
            {
                bool useMigemo = _searchEngine.IsMigemoAvailable;
                string pattern = term;
                if (useMigemo) pattern = _searchEngine.ExpandPattern(term);

                long? line = await _fileEngine.FindNextAsync(pattern, startLine, backwards, useMigemo, token);
                if (token.IsCancellationRequested) return;

                Application.MainLoop.Invoke(() => 
                {
                    _fileView.HighlightPattern = pattern;
                    _fileView.IsRegex = useMigemo;

                    if (line.HasValue)
                    {
                        _fileView.ScrollOffset = line.Value;
                        _searchStatusText = string.Empty; // Clear status on found
                    }
                    else
                    {
                        _searchStatusText = "(not found)";
                    }
                    _fileView.SetNeedsDisplay();
                    UpdateMessageLabel();
                });
            });
        }

        private bool ExecuteTextViewerAction(string actionName)
        {
            try
            {
                _logger?.LogDebug("Executing TextViewer action: {Action}", actionName);
                
                switch (actionName)
                {
                    case "TextViewer.GoToTop":
                    case "TextViewer.GoToFileTop":
                        _fileView.ScrollToTop();
                        return true;
                        
                    case "TextViewer.GoToBottom":
                    case "TextViewer.GoToFileBottom":
                        _fileView.ScrollToBottom();
                        return true;
                        
                    case "TextViewer.GoToLineStart":
                        // Standard view doesn't support horiz scroll yet
                        return false;
                        
                    case "TextViewer.GoToLineEnd":
                        return false;
                        
                    case "TextViewer.PageUp":
                        _fileView.PageUp();
                        return true;
                        
                    case "TextViewer.PageDown":
                        _fileView.PageDown();
                        return true;
                        
                    case "TextViewer.ScrollLeft":
                        _fileView.HorizontalOffset -= 10;
                        return true;

                    case "TextViewer.ScrollRight":
                        _fileView.HorizontalOffset += 10;
                        return true;

                    case "TextViewer.Close":
                        Application.RequestStop();
                        return true;
                        
                    case "TextViewer.Search":
                    case "TextViewer.StartForwardSearch":
                        _logger?.LogDebug("Action: StartForwardSearch");
                        StartSearch(false);
                        return true;

                    case "TextViewer.StartBackwardSearch":
                        _logger?.LogDebug("Action: StartBackwardSearch");
                        StartSearch(true);
                        return true;

                    case "TextViewer.FindNext":
                        FindNext();
                        return true;

                    case "TextViewer.FindPrevious":
                        FindPrevious();
                        return true;
                        
                    case "TextViewer.CycleEncoding":
                        CycleEncoding();
                        return true;
                        
                    case "TextViewer.ToggleHexMode":
                        ToggleHexMode();
                        return true;
                        
                    case "TextViewer.ClearHighlight":
                        ClearHighlight();
                        return true;
                        
                    default:
                        _logger?.LogWarning("Unknown TextViewer action '{Action}'", actionName);
                        return false;
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error executing action {Action}", actionName);
                return false;
            }
        }

        private void ToggleHexMode()
        {
            if (_fileView.Mode == FileViewMode.Text)
            {
                _fileView.Mode = FileViewMode.Hex;
                Title = "Binary Viewer";
            }
            else
            {
                _fileView.Mode = FileViewMode.Text;
                Title = "Text Viewer";
            }
            UpdateStatusLabel();
            UpdateMessageLabel();
        }

        private void UpdateStatusLabel()
        {
            string status = "";
            string fileName = Path.GetFileName(_fileEngine.FilePath);
            if (_fileView.Mode == FileViewMode.Text)
            {
                status = $"Lines: {_fileEngine.LineCount:N0}";
                long row = _fileView.ScrollOffset + 1;
                int col = _fileView.HorizontalOffset + 1;
                // Status label shows File, Lines, Encoding, and Position
                _statusLabel.Text = $"File: {fileName} | {status} | {_fileEngine.CurrentEncoding.EncodingName} | {row}:{col}";
            }
            else
            {
                status = $"Size: {_fileEngine.FileSize:N0} bytes";
                long row = _fileView.ScrollOffset + 1;
                _statusLabel.Text = $"File: {fileName} | {status} | Row: {row}";
            }
        }

        private void UpdateMessageLabel()
        {
            if (_isSearchMode)
            {
                string query = _searchQuery.ToString();
                string prefix = _searchStatusText == "..." ? "Searching...: " : "Search: ";
                _messageLabel.Text = $"{prefix}{query} {_searchStatusText}";
            }
            else if (!string.IsNullOrEmpty(_searchStatusText) && !string.IsNullOrEmpty(_lastSearchTerm))
            {
                // Show result of last continuous find (n/N)
                _messageLabel.Text = $"Search: {_lastSearchTerm} {_searchStatusText}";
            }
            else if (_fileEngine.IsIndexing)
            {
                _messageLabel.Text = $"Reading... {_fileEngine.IndexingProgress:P0} (Esc to stop)";
            }
            else
            {
                _messageLabel.Text = "Ctrl+B: Hex/Text | F5/F6: Top/Bot | F4: Search | Esc: Close";
            }
            _messageLabel.SetNeedsDisplay();
        }



    }
}
