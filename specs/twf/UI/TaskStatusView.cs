using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using Terminal.Gui;
using TWF.Models;
using TWF.Services;
using TWF.Utilities;

namespace TWF.UI
{
    /// <summary>
    /// Non-modal view for displaying status logs and task progress.
    /// Replaces the traditional status bar with a resizable, scrollable log pane.
    /// Supports colored status tags, async buffering, and log persistence.
    /// </summary>
    public class TaskStatusView : View
    {
        private readonly JobManager _jobManager;
        private readonly Configuration _config;
        
        // Log storage (in-memory)
        private readonly List<string> _logEntries = new List<string>();
        private readonly ConcurrentQueue<string> _pendingLogs = new ConcurrentQueue<string>();
        
        // Display state
        private bool _isExpanded = false;
        private int _expandedHeight = 10;
        private int _scrollOffset = 0;
        
        // Colors
        private Terminal.Gui.Attribute _normalColor;
        private Terminal.Gui.Attribute _okColor;
        private Terminal.Gui.Attribute _warnColor;
        private Terminal.Gui.Attribute _errorColor;
        private bool _colorsInitialized = false;
        
        // Configuration for busy spinner
        private readonly string[] _spinnerFrames = { "|", "/", "-", "\u005C" };
        private int _spinnerFrameIndex = 0;
        private object _lock = new object();

        public string CurrentSpinnerFrame => _spinnerFrames[_spinnerFrameIndex];
        
        public bool IsExpanded 
        { 
            get => _isExpanded; 
            set 
            {
                _isExpanded = value;
                SetNeedsDisplay();
            }
        }

        public int ExpandedHeight
        {
            get => _expandedHeight;
            set
            {
                _expandedHeight = Math.Max(3, Math.Min(20, value));
                if (_isExpanded) SetNeedsDisplay();
            }
        }

        public TaskStatusView(JobManager jobManager, Configuration config)
        {
            _jobManager = jobManager;
            _config = config;
            CanFocus = false; // Log view isn't focusable by default
            
            // Initialize persistence directory
            EnsureLogDirectoryExists();
            
            // Subscribe to job updates
            _jobManager.JobStarted += (s, j) => AddLogEntry(j, "Started");
            _jobManager.JobCompleted += (s, j) => AddLogEntry(j, j.Status.ToString());
            
            // Set up buffer drain timer
            // Use configured interval or default to 500ms
            int refreshInterval = _config.Display.TaskStatusViewRefreshIntervalMs > 0 ? _config.Display.TaskStatusViewRefreshIntervalMs : 500;
            Application.MainLoop.AddTimeout(TimeSpan.FromMilliseconds(refreshInterval), (loop) =>
            {
                ProcessPendingLogs();
                return true;
            });
        }
        
        private void EnsureLogDirectoryExists()
        {
             try
             {
                 string logPath = GetLogFilePath();
                 string? dir = Path.GetDirectoryName(logPath);
                 if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
                 {
                     Directory.CreateDirectory(dir);
                 }

                 // Centralized rotation and cleanup for session log
                 TWF.Utilities.LogHelper.RotateAndCleanup(logPath, _config.Display.MaxLogFiles);
             }
             catch
             {
                 // Swallow recursive errors during init
             }
        }

        private string GetLogFilePath()
        {
            string path = _config.Display.LogSavePath;
            if (string.IsNullOrEmpty(path)) path = "logs/session.log";
            
            if (!Path.IsPathRooted(path))
            {
                string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                path = Path.Combine(appData, "TWF", path);
            }
            return path;
        }

        /// <summary>
        /// Adds a raw log message to the queue
        /// </summary>
        public void AddLog(string message)
        {
            string timestampedMessage = $"[{DateTime.Now:HH:mm:ss}] {message}";
            _pendingLogs.Enqueue(timestampedMessage);
        }

        private void AddLogEntry(BackgroundJob job, string action)
        {
            // Format: [Job #1] <Message>
            // Note: ShortId will be added to BackgroundJob in next step
            // For now, we manually format if ShortId doesn't exist yet, but assuming it will.
            // Using dynamic to bypass compile error until BackgroundJob is updated
            dynamic dJob = job;
            int shortId = 0;
            try { shortId = dJob.ShortId; } catch { /* Ignore if property missing */ }
            
            string idPrefix = shortId > 0 ? $"[Job #{shortId}]" : "";
            string tabPrefix = job.TabId >= 0 ? $"[Tab {job.TabId+1}]" : "";
            string prefix = $"{idPrefix}{tabPrefix}".Trim();
            if(!string.IsNullOrEmpty(prefix)) prefix += " ";

            string message = $"{prefix}{job.Name}: {action}";
            
            if (action == "Started")
            {
                message += $" (ID: {job.Id})";
            }
            else if (!string.IsNullOrEmpty(job.ProgressMessage))
            {
                if (!string.Equals(job.ProgressMessage, action, StringComparison.OrdinalIgnoreCase))
                {
                    message += $" - {job.ProgressMessage}";
                }
            }
            
            // Add status tag
            if (job.Status == JobStatus.Completed) message += " [OK]";
            else if (job.Status == JobStatus.Failed) message += " [FAIL]";
            else if (job.Status == JobStatus.Cancelled) message += " [WARN]";
            
            AddLog(message);
        }

        /// <summary>
        /// Drains the pending log queue and updates the UI
        /// </summary>
        private void ProcessPendingLogs()
        {
            if (_pendingLogs.IsEmpty) return;

            bool added = false;
            while (_pendingLogs.TryDequeue(out string? log))
            {
                if (log != null)
                {
                    _logEntries.Add(log);
                    added = true;
                }
            }

            if (added)
            {
                // Check for memory limit
                int maxLines = _config.Display.MaxLogLinesInMemory;
                if (maxLines > 0 && _logEntries.Count > maxLines)
                {
                    FlushOldLogs(maxLines);
                }
                
                // Auto-scroll logic
                // If we were at the bottom, stay at the bottom
                int visibleLines = GetVisibleLogLines();
                bool wasAtBottom = _scrollOffset >= Math.Max(0, _logEntries.Count - visibleLines - _pendingLogs.Count - 10); // heuristic
                
                if (wasAtBottom || _scrollOffset == 0) // or newly started
                {
                    ScrollToEnd();
                }

                SetNeedsDisplay();
            }
        }
        
        private void FlushOldLogs(int maxLines)
        {
            try
            {
                int countToRemove = _logEntries.Count - maxLines;
                if (countToRemove <= 0) return;
                
                var logsToWrite = _logEntries.Take(countToRemove).ToList();
                _logEntries.RemoveRange(0, countToRemove);
                
                // Adjust scroll offset
                _scrollOffset = Math.Max(0, _scrollOffset - countToRemove);
                
                // Write to file
                string path = GetLogFilePath();
                File.AppendAllLines(path, logsToWrite);
            }
            catch
            {
                // Ignore persistence errors to keep UI running
            }
        }
        
        /// <summary>
        /// Manually saves all current logs to the file
        /// </summary>
        public bool SaveLog()
        {
            try
            {
                if (_logEntries.Count == 0) return true;
                string path = GetLogFilePath();
                File.AppendAllLines(path, _logEntries);
                return true;
            }
            catch 
            {
                return false;
            }
        }

        public void ScrollUp()
        {
            _scrollOffset = Math.Max(0, _scrollOffset - 1);
            SetNeedsDisplay();
        }

        public void ScrollDown()
        {
            int maxScroll = Math.Max(0, _logEntries.Count - GetVisibleLogLines());
            _scrollOffset = Math.Min(maxScroll, _scrollOffset + 1);
            SetNeedsDisplay();
        }
        
        public void ScrollToEnd()
        {
            _scrollOffset = Math.Max(0, _logEntries.Count - GetVisibleLogLines());
            SetNeedsDisplay();
        }

        private int GetVisibleLogLines()
        {
            return Bounds.Height;
        }
        
        private void InitializeColors()
        {
            var display = _config.Display;
            
            // Create attributes using config
            // Normal
            _normalColor = ColorScheme.Normal;
            
            // OK (Green)
            var okFg = ColorHelper.ParseConfigColor(display.OkColor, Color.Green);
            var bg = ColorHelper.ParseConfigColor(display.BackgroundColor, Color.Black); 
            _okColor = Application.Driver.MakeAttribute(okFg, bg);
            
            // Warn (Yellow)
            var warnFg = ColorHelper.ParseConfigColor(display.WarningColor, Color.BrightYellow);
            _warnColor = Application.Driver.MakeAttribute(warnFg, bg);
            
            // Error (Red)
            var errFg = ColorHelper.ParseConfigColor(display.ErrorColor, Color.Red);
            _errorColor = Application.Driver.MakeAttribute(errFg, bg);
            _colorsInitialized = true;
        }

        protected override bool OnDrawingContent()
        {
            var bounds = Viewport;
            
            // Ensure colors are initialized
            if (!_colorsInitialized) InitializeColors();

            var normal = ColorScheme.Normal;
            SetAttribute(normal);

            // Draw Log Area Background
            for (int y = 0; y < bounds.Height; y++)
            {
                Move(0, y);
                AddStr(new string(' ', bounds.Width));
            }

            int visibleLines = bounds.Height;
            lock (_lock) // Ensure thread safety during read
            {
                for (int i = 0; i < visibleLines; i++)
                {
                    int logIndex = _scrollOffset + i;
                    if (logIndex < _logEntries.Count)
                    {
                        Move(0, i);
                        string line = _logEntries[logIndex];
                        DrawColoredLine(line, bounds.Width);
                    }
                }
            }
            
            return true;
        }
        
        /// <summary>
        /// Parses and draws a line with colored tags
        /// </summary>
        private void DrawColoredLine(string line, int width)
        {
            // Truncate if too long (simple truncation)
            if (CharacterWidthHelper.GetStringWidth(line) > width)
            {
                line = CharacterWidthHelper.SmartTruncate(line, width, _config.Display.Ellipsis);
            }

            // Patterns to look for
            // [OK], [FAIL], [WARN]
            
            int lastIndex = 0;
            
            // Regex for tags
            var regex = new Regex(@"(\[(?:OK|FAIL|WARN)\])");
            var matches = regex.Matches(line);

            foreach (Match match in matches)
            {
                // Draw text before match
                if (match.Index > lastIndex)
                {
                    string text = line.Substring(lastIndex, match.Index - lastIndex);
                    SetAttribute(_normalColor);
                    AddStr(text);
                }
                
                // Draw match with color
                string tag = match.Value;
                if (tag == "[OK]") SetAttribute(_okColor);
                else if (tag == "[FAIL]") SetAttribute(_errorColor);
                else if (tag == "[WARN]") SetAttribute(_warnColor);
                
                AddStr(tag);
                
                lastIndex = match.Index + match.Length;
            }
            
            // Draw remaining text
            if (lastIndex < line.Length)
            {
                string text = line.Substring(lastIndex);
                SetAttribute(_normalColor);
                AddStr(text);
            }
        }
        
        public void Tick()
        {
            _spinnerFrameIndex = (_spinnerFrameIndex + 1) % _spinnerFrames.Length;
            // No need to redraw whole view just for tick if invisible, 
            // but if we had a spinner visible in the UI we would.
            // Currently TaskStatusView itself doesn't show the spinner, the MainController uses it.
        }
    }
}