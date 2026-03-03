using Terminal.Gui;
using TWF.Models;
using TWF.Services;
using TWF.Providers;
using TWF.Utilities;
using TWF.UI;
using Microsoft.Extensions.Logging;
using System.Text;
using TWF.Infrastructure;

namespace TWF.Controllers
{
    /// <summary>
    /// Main controller that orchestrates the entire application, manages UI state, and coordinates between components
    /// </summary>
    public class MainController : IDisposable
    {
        // Application Instance
        private IApplication? _app;
        private bool _disposed;
        
        // UI Components
        private Window? _mainWindow;
        private PaneView? _leftPane;
        private PaneView? _rightPane;
        private TabBarView? _tabBar;
        private Label? _pathsLabel;
        private Label? _topSeparator;
        private Label? _filenameLabel;
        private Label? _statusBar;
        private TaskStatusView? _taskStatusView;
        
        // Layout State
        private int _taskPanelHeight = 1;
        private bool _taskPanelExpanded = false;
        private const int DefaultTaskPanelHeight = 5;
        private const int MinFilePanelHeight = 3;
        
        // State
        private List<TabSession> _tabs;
        private int _activeTabIndex;
        
        private TabSession CurrentTab => _tabs[_activeTabIndex];
        
        private PaneState _leftState => CurrentTab.LeftState;
        private PaneState _rightState => CurrentTab.RightState;
        
        private bool _leftPaneActive
        {
            get => CurrentTab.IsLeftPaneActive;
            set => CurrentTab.IsLeftPaneActive = value;
        }

        private UiMode _currentMode;
        
        // Search state
        private string _searchPattern = string.Empty;
        private string _searchStatus = string.Empty;
        private int _searchStartIndex = 0;
        
        // Dependencies
        private readonly KeyBindingManager _keyBindings;
        private readonly FileOperations _fileOps;
        private readonly MarkingEngine _markingEngine;
        private readonly SortEngine _sortEngine;
        private readonly SearchEngine _searchEngine;
        private readonly ArchiveManager _archiveManager;
        private readonly ViewerManager _viewerManager;
        private HistoryManager _historyManager => CurrentTab.History;
        private readonly ConfigurationProvider _configProvider;
        private Configuration _config = null!;
        private readonly FileSystemProvider _fileSystemProvider;
        private readonly ListProvider _listProvider;
        private readonly CustomFunctionManager _customFunctionManager;
        private readonly MenuManager _menuManager;
        private readonly JobManager _jobManager;
        private HelpManager? _helpManager;
        private readonly ILogger<MainController> _logger;

        // Async Loading State
        private Dictionary<PaneState, CancellationTokenSource> _loadingCts = new Dictionary<PaneState, CancellationTokenSource>();
        private string _spinnerFrame = "|";
        private readonly string[] _spinnerFrames = { "|", "/", "-", "\\" };
        private int _spinnerIndex = 0;

        // Caching & Navigation State
        private readonly DirectoryCache _directoryCache = new DirectoryCache();
        private readonly DriveInfoService _driveInfoService = new DriveInfoService();
        private readonly PathValidator _pathValidator = new PathValidator();
        private Dictionary<string, (int cursor, int scroll)> _navigationStateCache = new Dictionary<string, (int, int)>();
        private Dictionary<PaneState, string> _lastLoadedPaths = new Dictionary<PaneState, string>();
        private Dictionary<PaneState, DateTime> _lastDirectoryWriteTimes = new Dictionary<PaneState, DateTime>();

        /// <summary>
        /// Output file path for changing directory on exit
        /// </summary>
        public string? ChangeDirectoryOutputFile { get; set; }
        
        /// <summary>
        /// Initializes a new instance of MainController with all required dependencies
        /// </summary>
        public MainController(
            KeyBindingManager keyBindings,
            FileOperations fileOps,
            MarkingEngine markingEngine,
            SortEngine sortEngine,
            SearchEngine searchEngine,
            ArchiveManager archiveManager,
            ViewerManager viewerManager,
            ConfigurationProvider configProvider,
            FileSystemProvider fileSystemProvider,
            ListProvider listProvider,
            CustomFunctionManager customFunctionManager,
            MenuManager menuManager,
            HistoryManager historyManager,
            JobManager jobManager,
            ILogger<MainController> logger)
        {
            _keyBindings = keyBindings ?? throw new ArgumentNullException(nameof(keyBindings));
            _fileOps = fileOps ?? throw new ArgumentNullException(nameof(fileOps));
            _markingEngine = markingEngine ?? throw new ArgumentNullException(nameof(markingEngine));
            _sortEngine = sortEngine ?? throw new ArgumentNullException(nameof(sortEngine));
            _searchEngine = searchEngine ?? throw new ArgumentNullException(nameof(searchEngine));
            _archiveManager = archiveManager ?? throw new ArgumentNullException(nameof(archiveManager));
            _viewerManager = viewerManager ?? throw new ArgumentNullException(nameof(viewerManager));
            if (historyManager == null) throw new ArgumentNullException(nameof(historyManager));
            _configProvider = configProvider ?? throw new ArgumentNullException(nameof(configProvider));
            _fileSystemProvider = fileSystemProvider ?? throw new ArgumentNullException(nameof(fileSystemProvider));
            _listProvider = listProvider ?? throw new ArgumentNullException(nameof(listProvider));
            _customFunctionManager = customFunctionManager ?? throw new ArgumentNullException(nameof(customFunctionManager));
            _menuManager = menuManager ?? throw new ArgumentNullException(nameof(menuManager));
            _jobManager = jobManager ?? throw new ArgumentNullException(nameof(jobManager));
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            
            _tabs = new List<TabSession> { new TabSession(historyManager) };
            _activeTabIndex = 0;
            
            _currentMode = UiMode.Normal;
        }
        
        /// <summary>
        /// Updates the layout of UI components based on current task pane height
        /// </summary>
        private void UpdateLayout()
        {
            if (_mainWindow == null || _taskStatusView == null || _statusBar == null || _filenameLabel == null || _leftPane == null || _rightPane == null) return;

            int windowHeight = Application.Driver?.Rows ?? 0;
            if (windowHeight <= 0) windowHeight = _mainWindow.Frame.Height;
            if (windowHeight <= 0) return;

            // Overhead calculation: TabBar(1) + Paths(1) + Separator(1) + Filename(1) + Status(1) + MinFile(3) = 8
            int maxAllowedHeight = Math.Max(1, windowHeight - 8);
            
            // Constrain task panel height
            _taskPanelHeight = Math.Max(1, Math.Min(_taskPanelHeight, maxAllowedHeight));

            int height = _taskStatusView.IsExpanded ? _taskPanelHeight : 1;
            
            _logger.LogDebug($"TaskPanel Layout: WinH={windowHeight}, PanelH={_taskPanelHeight}, MaxAllowed={maxAllowedHeight}, Expanded={_taskStatusView.IsExpanded}");

            // Set positions and sizes
            _taskStatusView.Y = Pos.AnchorEnd(height);
            _taskStatusView.Height = height;
            
            _filenameLabel.Y = Pos.AnchorEnd(height + 1);
            _statusBar.Y = Pos.AnchorEnd(height + 2);
            
            // Adjust pane heights
            int bottomOffset = height + 2;
            _leftPane.Height = Dim.Fill(bottomOffset);
            _rightPane.Height = Dim.Fill(bottomOffset);
            
            _mainWindow.LayoutSubviews();
            _mainWindow.SetNeedsDisplay();
        }        /// <summary>
        /// Applies color scheme from configuration to the main window
        /// </summary>
        private void ApplyColorScheme(DisplaySettings display)
        {
            if (_mainWindow == null) return;

            // Parse colors
            var backgroundColor = ColorHelper.ParseConfigColor(display.BackgroundColor, Color.Black);
            var foregroundColor = ColorHelper.ParseConfigColor(display.ForegroundColor, Color.White);
            var borderColor = ColorHelper.ParseConfigColor(display.PaneBorderColor, Color.Green);

            // Update labels
            if (_pathsLabel != null)
            {
                _pathsLabel.ColorScheme = new ColorScheme
                {
                    Normal = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                             ?? new Attribute(foregroundColor, backgroundColor)
                };
            }

            if (_topSeparator != null)
            {
                var topSeparatorFg = ColorHelper.ParseConfigColor(display.TopSeparatorForegroundColor, Color.White);
                var topSeparatorBg = ColorHelper.ParseConfigColor(display.TopSeparatorBackgroundColor, Color.DarkGray);
                _topSeparator.ColorScheme = new ColorScheme
                {
                    Normal = _app?.Driver?.MakeAttribute(topSeparatorFg, topSeparatorBg)
                             ?? new Attribute(topSeparatorFg, topSeparatorBg)
                };
            }
            
            if (_filenameLabel != null)
            {
                var labelFg = ColorHelper.ParseConfigColor(display.FilenameLabelForegroundColor, Color.White);
                var labelBg = ColorHelper.ParseConfigColor(display.FilenameLabelBackgroundColor, Color.Blue);
                _filenameLabel.ColorScheme = new ColorScheme 
                { 
                    Normal = _app?.Driver?.MakeAttribute(labelFg, labelBg)
                             ?? new Attribute(labelFg, labelBg)
                };
            }
            
            // Update Pane color schemes
            if (_leftPane != null)
            {
                _leftPane.ColorScheme = new ColorScheme
                {
                    Normal = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                             ?? new Attribute(foregroundColor, backgroundColor),
                    Focus = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                            ?? new Attribute(foregroundColor, backgroundColor),
                    HotNormal = _app?.Driver?.MakeAttribute(borderColor, backgroundColor)
                                ?? new Attribute(borderColor, backgroundColor)
                };
            }
            
            if (_rightPane != null)
            {
                _rightPane.ColorScheme = new ColorScheme
                {
                    Normal = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                             ?? new Attribute(foregroundColor, backgroundColor),
                    Focus = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                            ?? new Attribute(foregroundColor, backgroundColor),
                    HotNormal = _app?.Driver?.MakeAttribute(borderColor, backgroundColor)
                                ?? new Attribute(borderColor, backgroundColor)
                };
            }
            
            _mainWindow.SetNeedsDisplay();
        }

        /// <summary>
        /// Initializes the Terminal.Gui application and creates all UI components
        /// </summary>
        public void Initialize()
        {
            try
            {
                _logger.LogDebug("Initializing MainController");
                
                // Initialize Terminal.Gui v2 - Create and store IApplication instance
                _app = Application.Create().Init();
                
                // Load configuration once
                _config = _configProvider.LoadConfiguration();
                _logger.LogInformation("Configuration loaded and cached");

                // Initialize HelpManager with preferred language
                _helpManager = new HelpManager(_keyBindings, _configProvider.ConfigDirectory, _config.Display.HelpLanguage, LoggingConfiguration.GetLogger<HelpManager>());

                // Initialize CharacterWidthHelper defaults from config
                CharacterWidthHelper.DefaultEllipsis = _config.Display.Ellipsis;
                
                // Configure CJK character width
                TWF.Utilities.CharacterWidthHelper.CJKCharacterWidth = _config.Display.CJK_CharacterWidth;
                _logger.LogInformation($"CJK character width set to: {_config.Display.CJK_CharacterWidth}");
                
                // Initialize layout constants from config
                _taskPanelHeight = _config.Display.TaskPanelHeight > 0 ? _config.Display.TaskPanelHeight : 10;
                
                // Always load key bindings (they are always configurable)
                string keyBindingPath = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                    "TWF",
                    _config.KeyBindings.KeyBindingFile
                );
                
                // If the file doesn't exist in AppData, check current directory
                if (!File.Exists(keyBindingPath))
                {
                    keyBindingPath = _config.KeyBindings.KeyBindingFile;
                }
                
                _keyBindings.LoadBindings(keyBindingPath);
                _logger.LogInformation($"Key bindings loaded from: {keyBindingPath}");
                
                // Load custom functions
                string customFunctionsPath = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                    "TWF",
                    "custom_functions.json"
                );
                
                // If the file doesn't exist in AppData, check current directory
                if (!File.Exists(customFunctionsPath))
                {
                    customFunctionsPath = "custom_functions.json";
                }
                
                _customFunctionManager.LoadFunctions(customFunctionsPath);
                _logger.LogInformation($"Custom functions loaded from: {customFunctionsPath}");
                
                // Set MenuManager for custom function manager
                _customFunctionManager.SetMenuManager(_menuManager);
                _logger.LogDebug("MenuManager configured for custom functions");
                
                // Set built-in action executor for menu items
                _customFunctionManager.SetBuiltInActionExecutor(ExecuteAction);
                _customFunctionManager.SetBuiltInActionExecutorWithArg(ExecuteActionWithArg);
                _logger.LogDebug("Built-in action executor configured for menu items");
                
                // Load session state if enabled
                if (_config.SaveSessionState)
                {
                    var sessionState = _configProvider.LoadSessionState();
                    if (sessionState != null)
                    {
                        // Check if we have multiple tabs saved
                        if (sessionState.Tabs != null && sessionState.Tabs.Count > 0)
                        {
                            _tabs.Clear();
                            foreach (var tabState in sessionState.Tabs)
                            {
                                var tabSession = new TabSession(_config);
                                
                                tabSession.LeftState.CurrentPath = tabState.LeftPath ?? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
                                tabSession.RightState.CurrentPath = tabState.RightPath ?? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
                                tabSession.LeftState.FileMask = tabState.LeftMask ?? "*";
                                tabSession.RightState.FileMask = tabState.RightMask ?? "*";
                                tabSession.LeftState.SortMode = tabState.LeftSort;
                                tabSession.RightState.SortMode = tabState.RightSort;
                                tabSession.LeftState.DisplayMode = tabState.LeftDisplayMode;
                                tabSession.RightState.DisplayMode = tabState.RightDisplayMode;
                                tabSession.LeftFocusTarget = tabState.LeftFocusTarget;
                                tabSession.RightFocusTarget = tabState.RightFocusTarget;
                                tabSession.IsLeftPaneActive = tabState.LeftPaneActive;
                                
                                tabSession.History.SetHistory(true, tabState.LeftHistory);
                                tabSession.History.SetHistory(false, tabState.RightHistory);
                                
                                _tabs.Add(tabSession);
                            }
                            
                            // Restore active tab index
                            if (sessionState.ActiveTabIndex >= 0 && sessionState.ActiveTabIndex < _tabs.Count)
                            {
                                _activeTabIndex = sessionState.ActiveTabIndex;
                            }
                            else
                            {
                                _activeTabIndex = 0;
                            }
                        }
                        else
                        {
                            // Fallback to legacy single tab load
                            var tabSession = _tabs[0];
                            
                            tabSession.LeftState.CurrentPath = sessionState.LeftPath ?? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
                            tabSession.RightState.CurrentPath = sessionState.RightPath ?? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
                            tabSession.LeftState.FileMask = sessionState.LeftMask ?? "*";
                            tabSession.RightState.FileMask = sessionState.RightMask ?? "*";
                            tabSession.LeftState.SortMode = sessionState.LeftSort;
                            tabSession.RightState.SortMode = sessionState.RightSort;
                            
                            // Restore history
                            tabSession.History.SetHistory(true, sessionState.LeftHistory);
                            tabSession.History.SetHistory(false, sessionState.RightHistory);
                        }

                        // Restore Task Panel state
                        _taskPanelHeight = sessionState.TaskPaneHeight > 0 ? sessionState.TaskPaneHeight : DefaultTaskPanelHeight;
                        _taskPanelExpanded = sessionState.TaskPaneExpanded;
                        
                        _logger.LogInformation("Session state restored");
                    }
                    else
                    {
                        InitializeDefaultPaths();
                    }
                }
                else
                {
                    InitializeDefaultPaths();
                }
                
                // Load initial directory contents
                LoadPaneDirectory(_leftState, CurrentTab.LeftFocusTarget);
                LoadPaneDirectory(_rightState, CurrentTab.RightFocusTarget);
                
                // Clear initial targets
                CurrentTab.LeftFocusTarget = null;
                CurrentTab.RightFocusTarget = null;
                
                // Create main window
                CreateMainWindow();

                // Apply initial configuration
                ApplyConfiguration(_config);
                
                // Initialize dynamic layout after everything is loaded and window is created
                UpdateLayout();
                
                // Show version info in log area on startup
                ShowVersionInfo();
                
                _logger.LogDebug("MainController initialized successfully");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to initialize MainController");
                throw;
            }
        }
        
        /// <summary>
        /// Initializes pane paths to default values
        /// </summary>
        private void InitializeDefaultPaths()
        {
            var userProfile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            _leftState.CurrentPath = userProfile;
            _rightState.CurrentPath = userProfile;
            _logger.LogInformation($"Initialized default paths to: {userProfile}");
        }
        
        /// <summary>
        /// Creates the main window with borderless layout
        /// </summary>
        private void CreateMainWindow()
        {
            // Hide cursor in the main window
            try
            {
                Console.CursorVisible = false;
            }
            catch
            {
                // Ignore errors hiding cursor
            }
            
            // Create main window without border
            _mainWindow = new Window("")
            {
                X = 0,
                Y = 0,
                Width = Dim.Fill(),
                Height = Dim.Fill(),
                Border = new Border()
                {
                    BorderStyle = BorderStyle.None
                }
            };
            
            // Line 0: Tab Bar
            _tabBar = new TabBarView(_config)
            {
                X = 0,
                Y = 0
            };
            _mainWindow.Add(_tabBar);

            // Line 1: Paths display
            _pathsLabel = new Label()
            {
                X = 0,
                Y = 1,
                Width = Dim.Fill(),
                Height = 1,
                Text = "",
                ColorScheme = new ColorScheme()
                {
                    Normal = _app?.Driver?.MakeAttribute(Color.White, Color.Black)
                             ?? new Attribute(Color.White, Color.Black)
                }
            };
            _mainWindow.Add(_pathsLabel);
            
            // Parse colors from configuration
            var backgroundColor = ColorHelper.ParseConfigColor(_config.Display.BackgroundColor, Color.Black);
            var foregroundColor = ColorHelper.ParseConfigColor(_config.Display.ForegroundColor, Color.White);
            var borderColor = ColorHelper.ParseConfigColor(_config.Display.PaneBorderColor, Color.Green);
            
            // Line 2: Top separator
            _topSeparator = new Label()
            {
                X = 0,
                Y = 2,
                Width = Dim.Fill(),
                Height = 1,
                Text = "",
                ColorScheme = new ColorScheme()
                {

                    Normal = _app?.Driver?.MakeAttribute(
                        ColorHelper.ParseConfigColor(_config.Display.TopSeparatorForegroundColor, Color.White), 
                        ColorHelper.ParseConfigColor(_config.Display.TopSeparatorBackgroundColor, Color.DarkGray))
                             ?? new Attribute(
                        ColorHelper.ParseConfigColor(_config.Display.TopSeparatorForegroundColor, Color.White), 
                        ColorHelper.ParseConfigColor(_config.Display.TopSeparatorBackgroundColor, Color.DarkGray))
                }
            };
            _mainWindow.Add(_topSeparator);
            
            // Line 3+: Left pane (file list)
            _leftPane = new PaneView()
            {
                X = 0,
                Y = 3,
                Width = Dim.Percent(50),
                Height = Dim.Fill(3), // Space for bottom elements (filename, status, task)
                CanFocus = true,
                State = _leftState,
                IsActive = _leftPaneActive,
                Configuration = _config,
                ColorScheme = new ColorScheme()
                {
                    Normal = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                             ?? new Attribute(foregroundColor, backgroundColor),
                    Focus = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                            ?? new Attribute(foregroundColor, backgroundColor),
                    HotNormal = _app?.Driver?.MakeAttribute(borderColor, backgroundColor)
                                ?? new Attribute(borderColor, backgroundColor)
                }
            };
            _leftPane.KeyPress += HandleKeyPress;
            _mainWindow.Add(_leftPane);
            
            // Line 3+: Right pane (file list)
            _rightPane = new PaneView()
            {
                X = Pos.Percent(50),
                Y = 3,
                Width = Dim.Percent(50),
                Height = Dim.Fill(3), 
                CanFocus = true,
                State = _rightState,
                IsActive = !_leftPaneActive,
                Configuration = _config,
                ColorScheme = new ColorScheme()
                {
                    Normal = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                             ?? new Attribute(foregroundColor, backgroundColor),
                    Focus = _app?.Driver?.MakeAttribute(foregroundColor, backgroundColor)
                            ?? new Attribute(foregroundColor, backgroundColor),
                    HotNormal = _app?.Driver?.MakeAttribute(borderColor, backgroundColor)
                                ?? new Attribute(borderColor, backgroundColor)
                }
            };
            _rightPane.KeyPress += HandleKeyPress;
            _mainWindow.Add(_rightPane);
            
            // Line N-2: Filename display
            _filenameLabel = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(3), 
                Width = Dim.Fill(),
                Height = 1,
                Text = "",
                ColorScheme = new ColorScheme()
                {
                    Normal = _app?.Driver?.MakeAttribute(
                        ColorHelper.ParseConfigColor(_config.Display.FilenameLabelForegroundColor, Color.White), 
                        ColorHelper.ParseConfigColor(_config.Display.FilenameLabelBackgroundColor, Color.Blue))
                             ?? new Attribute(
                        ColorHelper.ParseConfigColor(_config.Display.FilenameLabelForegroundColor, Color.White), 
                        ColorHelper.ParseConfigColor(_config.Display.FilenameLabelBackgroundColor, Color.Blue))
                }
            };
            _mainWindow.Add(_filenameLabel);
            
            // Line N-2: Drive usage (status bar)
            _statusBar = new Label()
            {
                X = 0,
                Y = Pos.AnchorEnd(2), 
                Width = Dim.Fill(),
                Height = 1,
                Text = "",
                ColorScheme = new ColorScheme()
                {
                    Normal = _app?.Driver?.MakeAttribute(Color.Black, Color.Gray)
                             ?? new Attribute(Color.Black, Color.Gray)
                }
            };
            _mainWindow.Add(_statusBar);
            
            // Line N: Task Status / Log Area (last line, expandable)
            _taskStatusView = new TaskStatusView(_jobManager, _config)
            {
                X = 0,
                Y = Pos.AnchorEnd(1),
                Width = Dim.Fill(),
                Height = 1,
                IsExpanded = _taskPanelExpanded,
                ColorScheme = new ColorScheme()
                {
                    Normal = _app?.Driver?.MakeAttribute(Color.White, Color.Black)
                             ?? new Attribute(Color.White, Color.Black)
                }
            };
            _mainWindow.Add(_taskStatusView);
            
            // Update initial layout
            UpdateLayout();
            
            // Set initial focus
            _leftPane.SetFocus();
            
            // Add global key handler for the main window
            _mainWindow.KeyPress += HandleKeyPress;
            
            // Add resize handler to refresh display when window size changes
            Application.Resized += (e) =>
            {
                _logger.LogDebug("Terminal resized, updating display");
                // Only update display, don't reload data
                UpdateDisplay();
            };
            
            // Initial display update
            RefreshPanes();
            
            // Add tick handler for spinner animation using configurable interval (min 100ms)
            int interval = _config.Display.TaskPanelUpdateIntervalMs;
            if (interval < 100)
            {
                _logger.LogWarning($"TaskPanelUpdateIntervalMs ({interval}ms) is below minimum. Using 100ms.");
                SetStatus("Warning: TaskPanelUpdateIntervalMs < 100ms. Using 100ms instead.");
                interval = 100;
            }

            _app?.MainLoop.AddTimeout(TimeSpan.FromMilliseconds(interval), (loop) =>
            {
                _taskStatusView?.Tick();

                // Tick spinner
                _spinnerIndex = (_spinnerIndex + 1) % _spinnerFrames.Length;
                _spinnerFrame = _spinnerFrames[_spinnerIndex];
                
                // Force update if loading
                if (_loadingCts.Count > 0)
                {
                    UpdateStatusBar();
                }

                UpdateTabBar();
                return true;
            });
            
            // Subscribe to job updates for tab bar
            _jobManager.JobStarted += (s, j) => _app?.MainLoop.Invoke(() => UpdateTabBar());
            _jobManager.JobCompleted += (s, j) => _app?.MainLoop.Invoke(() => UpdateTabBar());
            
            _logger.LogDebug("Main window created with borderless layout");
        }
        
        /// <summary>
        /// Updates the paths label to show both pane paths and the top separator with detailed information
        /// </summary>
        private void UpdateWindowTitle()
        {
            if (_pathsLabel == null) return;

            int windowWidth = Math.Max(40, Application.Driver.Cols);
            int halfWidth = windowWidth / 2;

            // Format left path with file mask if applicable
            string leftPath = _leftState.CurrentPath;
            if (!string.IsNullOrEmpty(_leftState.FileMask) && _leftState.FileMask != "*")
            {
                leftPath += $" {_leftState.FileMask}";
            }
            leftPath = TWF.Utilities.CharacterWidthHelper.TruncateToWidth(leftPath, halfWidth - 4);

            // Format right path with file mask if applicable
            string rightPath = _rightState.CurrentPath;
            if (!string.IsNullOrEmpty(_rightState.FileMask) && _rightState.FileMask != "*")
            {
                rightPath += $" {_rightState.FileMask}";
            }
            rightPath = TWF.Utilities.CharacterWidthHelper.TruncateToWidth(rightPath, halfWidth - 4);

            // Add indicator for active pane
            leftPath = _leftPaneActive ? $"►{leftPath}" : $" {leftPath}";
            rightPath = !_leftPaneActive ? $"►{rightPath}" : $" {rightPath}";

            // Set paths label with both paths separated by │
             _pathsLabel.Text = $"{TWF.Utilities.CharacterWidthHelper.PadToWidth(leftPath,halfWidth - 1)}│{rightPath}";

            // Update top separator with detailed information (similar to paths label format)
            if (_topSeparator != null)
            {
                string leftInfo = FormatTopSeparatorInfo(_leftState);
                string rightInfo = FormatTopSeparatorInfo(_rightState);

                // Truncate information if needed to fit in half width
                leftInfo = TWF.Utilities.CharacterWidthHelper.TruncateToWidth(leftInfo, halfWidth - 4);
                rightInfo = TWF.Utilities.CharacterWidthHelper.TruncateToWidth(rightInfo, halfWidth - 4);

                // Pad the left info and format with vertical bar separator
                leftInfo = TWF.Utilities.CharacterWidthHelper.PadToWidth(leftInfo, halfWidth - 1);
                _topSeparator.Text = $"{leftInfo}│{rightInfo}";
            }
        }

        /// <summary>
        /// Formats information for the top separator
        /// </summary>
        private string FormatTopSeparatorInfo(PaneState paneState)
        {
            // Extract drive name or shared folder name from path
            string driveOrPath = GetDriveOrShareName(paneState.CurrentPath);

            // Calculate marked file information
            var markedEntries = paneState.GetMarkedEntries();
            long totalSize = markedEntries.Sum(e => e.Size);
            int fileCount = markedEntries.Count(e => !e.IsDirectory);
            int dirCount = markedEntries.Count(e => e.IsDirectory);

            string sizeInfo = "";
            if (fileCount > 0 && dirCount > 0)
            {
                string fileText = fileCount == 1 ? "File" : "Files";
                string dirText = dirCount == 1 ? "Dir" : "Dirs";
                sizeInfo = $" {dirCount} {dirText} {fileCount} {fileText} {FormatSize(totalSize)} marked";
            }
            else if (dirCount > 0)
            {
                string dirText = dirCount == 1 ? "Dir" : "Dirs";
                sizeInfo = $" {dirCount} {dirText} {FormatSize(totalSize)} marked";
            }
            else if (fileCount > 0)
            {
                string fileText = fileCount == 1 ? "File" : "Files";
                sizeInfo = $" {fileCount} {fileText} {FormatSize(totalSize)} marked";
            }

            // Format: "<drive_name> [size_info]"
            return $" {driveOrPath}{sizeInfo}".TrimEnd();
        }

        /// <summary>
        /// Extracts the drive name or network share name from a path
        /// </summary>
        private string GetDriveOrShareName(string path)
        {
            try
            {
                // Handle network paths like \\server\share
                if (path.StartsWith(@"\\"))
                {
                    var parts = path.Substring(2).Split(new char[] { '\\' }, 2);
                    if (parts.Length >= 1)
                    {
                        return $@"\\{parts[0]}";
                    }
                }

                // Handle Linux/Unix paths specifically
                if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
                {
                    // On Linux/MacOS, get the mount point
                    string? linuxRootPath = Path.GetPathRoot(path);
                    if (!string.IsNullOrEmpty(linuxRootPath))
                    {
                        // Use async service
                        var stats = _driveInfoService.GetDriveStats(linuxRootPath);
                        
                        // If cached and ready, use info
                        if (stats.IsReady)
                        {
                            // On Linux/MacOS, try to get both device path and mount point
                            string devicePath = GetDevicePathForMountPoint(linuxRootPath);

                            if (!string.IsNullOrEmpty(stats.VolumeLabel) && stats.VolumeLabel != linuxRootPath)
                            {
                                return !string.IsNullOrEmpty(devicePath)
                                    ? $"{devicePath} ({linuxRootPath} - {stats.VolumeLabel})"
                                    : $"{linuxRootPath} - {stats.VolumeLabel}";
                            }
                            else
                            {
                                return !string.IsNullOrEmpty(devicePath)
                                    ? $"{devicePath} ({linuxRootPath})"
                                    : (linuxRootPath == "/" ? "Root" : linuxRootPath);
                            }
                        }
                    }
                }

                // Handle regular drive letters like C:\
                if (path.Length >= 3 && path[1] == ':' && (path[2] == '\\' || path[2] == '/'))
                {
                    var stats = _driveInfoService.GetDriveStats(path);
                    if (!string.IsNullOrEmpty(stats.VolumeLabel))
                    {
                        return stats.VolumeLabel;
                    }

                    // Fallback to drive letter in brackets if volume label is empty or unavailable
                    return $"({path.Substring(0, 2)})"; // Return "(C:)"
                }

                // For other paths, return the root directory name
                var rootPath = Path.GetPathRoot(path);
                if (!string.IsNullOrEmpty(rootPath))
                {
                    return rootPath.TrimEnd('\\', '/');
                }

                // Fallback to first directory in path
                var directoryInfo = new DirectoryInfo(path);
                if (directoryInfo.Parent == null)
                {
                    return directoryInfo.Name; // Already at root
                }

                return directoryInfo.Root.Name;
            }
            catch
            {
                // Fallback if path parsing fails
                return Path.GetPathRoot(path) ?? "Unknown";
            }
        }

        /// <summary>
        /// Gets the device path for a mount point on Linux/MacOS systems
        /// </summary>
        private string GetDevicePathForMountPoint(string? mountPoint)
        {
            try
            {
                if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
                {
                    // Use df command to get the device path for the mount point
                    var processStartInfo = new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = "df",
                        Arguments = $"--output=source,target {mountPoint}",
                        UseShellExecute = false,
                        RedirectStandardOutput = true,
                        CreateNoWindow = true
                    };

                    using (var process = System.Diagnostics.Process.Start(processStartInfo))
                    {
                        if (process != null)
                        {
                            process.WaitForExit(2000); // Wait up to 2 seconds

                            if (process.ExitCode == 0)
                            {
                                string output = process.StandardOutput.ReadToEnd();
                                var lines = output.Split('\n', StringSplitOptions.RemoveEmptyEntries);

                                // Skip header line (first element) and look for the mount point in remaining lines
                                for (int i = 1; i < lines.Length; i++)
                                {
                                    var parts = lines[i].Trim().Split(new char[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                                    if (parts.Length >= 2 && parts[1] == mountPoint)
                                    {
                                        return parts[0]; // Return the device path
                                    }
                                }
                            }
                        }
                    }
                }
            }
            catch
            {
                // If df command fails, return empty string
            }

            return string.Empty;
        }

        /// <summary>
        /// Formats file size with appropriate units (B, KB, MB, GB, TB)
        /// </summary>
        private string FormatSize(long size)
        {
            if (size < 1024)
                return $"{size} B";
            if (size < 1024 * 1024)
                return $"{size / 1024.0:F1} KB";
            if (size < 1024 * 1024 * 1024)
                return $"{size / (1024.0 * 1024.0):F1} MB";
            if (size < 1024L * 1024L * 1024L * 1024L)
                return $"{size / (1024.0 * 1024.0 * 1024.0):F1} GB";
            return $"{size / (1024.0 * 1024.0 * 1024.0 * 1024.0):F1} TB";
        }

        /// <summary>
        /// Updates the status bar with directory/file counts and drive usage information
        /// </summary>
        private void UpdateStatusBar()
        {
            if (_statusBar == null) return;

            int halfWidth = Math.Max(20, (Application.Driver.Cols - 6) / 2);

            // Format left pane stats
            string leftDirFileStats = FormatDirFileCounts(_leftState.DirectoryCount, _leftState.FileCount);
            string leftDriveStats = GetDriveStatsForPath(_leftState.CurrentPath);
            string leftStats = leftDirFileStats + leftDriveStats;

            // Truncate if too long to fit in half width using CharacterWidthHelper for proper CJK support
            if (TWF.Utilities.CharacterWidthHelper.GetStringWidth(leftStats) > halfWidth)
            {
                leftStats = TWF.Utilities.CharacterWidthHelper.TruncateToWidth(leftStats, halfWidth);
            }

            // Format right pane stats
            string rightDirFileStats = FormatDirFileCounts(_rightState.DirectoryCount, _rightState.FileCount);
            string rightDriveStats = GetDriveStatsForPath(_rightState.CurrentPath);
            string rightStats = rightDirFileStats + rightDriveStats;

            // Truncate if too long to fit in half width using CharacterWidthHelper for proper CJK support
            if (TWF.Utilities.CharacterWidthHelper.GetStringWidth(rightStats) > halfWidth)
            {
                rightStats = TWF.Utilities.CharacterWidthHelper.TruncateToWidth(rightStats, halfWidth);
            }

            // Format: "LeftStats  │  RightStats"
            var statusText = $" {leftStats.PadRight(halfWidth)} │ {rightStats}";
            
            if (_loadingCts.Count > 0)
            {
                 statusText += $" {_spinnerFrame}";
            }
            
            _statusBar.Text = statusText;
        }

        /// <summary>
        /// Gets drive statistics for a path, handling network paths appropriately
        /// </summary>
        private string GetDriveStatsForPath(string path)
        {
            var stats = _driveInfoService.GetDriveStats(path);
            return FormatDriveStats(stats);
        }
        
        /// <summary>
        /// Formats directory and file counts with proper pluralization
        /// </summary>
        private string FormatDirFileCounts(int directories, int files)
        {
            string dirText = directories == 1 ? "Dir" : "Dirs";
            string fileText = files == 1 ? "File" : "Files";
            return $"  {directories} {dirText}   {files} {fileText}  ";
        }

        /// <summary>
        /// Formats drive statistics
        /// </summary>
        private string FormatDriveStats(TWF.Models.DriveStats drive)
        {
            if (drive.IsLoading) return " [Loading...]";
            if (!drive.IsReady) return " Drive not ready";

            long usedBytes = drive.TotalSize - drive.AvailableFreeSpace;
            double usedGB = usedBytes / (1024.0 * 1024.0 * 1024.0);
            double freeGB = drive.AvailableFreeSpace / (1024.0 * 1024.0 * 1024.0);
            double freePercent = drive.TotalSize > 0 ? (drive.AvailableFreeSpace * 100.0) / drive.TotalSize : 0;

            return $"{usedGB:F1} GB used  {freeGB:F1} GB free ({freePercent:F1}%)";
        }
        
        /// <summary>
        /// Updates the filename label with current filename
        /// </summary>
        private void UpdateBottomBorder()
        {
            if (_filenameLabel == null) return;
            
            if (_currentMode == UiMode.Search)
            {
                _filenameLabel.Text = $" Search: {_searchPattern} {_searchStatus}";
                return;
            }

            var activePane = GetActivePane();
            var currentEntry = activePane.GetCurrentEntry();
            
            if (currentEntry != null)
            {
                // Get the current filename
                string filename = currentEntry.Name;
                
                // Truncate filename if needed (accounting for CJK character widths)
                int maxWidth = Math.Max(10, Application.Driver.Cols - 2);
                filename = TWF.Utilities.CharacterWidthHelper.SmartTruncate(filename, maxWidth);

                
                // Display filename
                _filenameLabel.Text = filename;
            }
            else
            {
                _filenameLabel.Text = "";
            }
        }
        
        /// <summary>
        /// Truncates a path to fit within specified width (accounting for CJK character widths)
        /// </summary>
        private string TruncatePath(string path, int maxWidth)
        {
            int currentWidth = TWF.Utilities.CharacterWidthHelper.GetStringWidth(path);
            if (currentWidth <= maxWidth)
                return path;
            
            // Try to show drive and end of path
            if (path.Length > 3 && path[1] == ':')
            {
                string drive = path.Substring(0, 3); // "C:\"
                int driveWidth = TWF.Utilities.CharacterWidthHelper.GetStringWidth(drive);
                string ellipsis = TWF.Utilities.CharacterWidthHelper.DefaultEllipsis; // Use default ellipsis
                int ellipsisWidth = TWF.Utilities.CharacterWidthHelper.GetStringWidth(ellipsis);
                int remaining = maxWidth - driveWidth - ellipsisWidth;
                
                if (remaining > 0)
                {
                    // Find the substring from the end that fits in remaining width
                    string endPart = "";
                    int endWidth = 0;
                    for (int i = path.Length - 1; i >= drive.Length && endWidth < remaining; i--)
                    {
                        int charWidth = TWF.Utilities.CharacterWidthHelper.GetCharWidth(path[i]);
                        if (endWidth + charWidth <= remaining)
                        {
                            endPart = path[i] + endPart;
                            endWidth += charWidth;
                        }
                        else
                        {
                            break;
                        }
                    }
                    
                    if (endPart.Length > 0)
                    {
                        return drive + ellipsis + endPart;
                    }
                }
            }
            
            // Fallback to smart truncation
            return TWF.Utilities.CharacterWidthHelper.SmartTruncate(path, maxWidth, _config.Display.Ellipsis);
        }
        

        
        /// <summary>
        /// Handles key press events for the main window
        /// Routes keys to appropriate handlers based on current UI mode
        /// </summary>
        private void HandleKeyPress(Key key)
        {
            try
            {
                _logger.LogDebug($"Key pressed: {key}");
                
                // Handle search mode keys
                if (_currentMode == UiMode.Search)
                {
                    HandleSearchModeKey(key);
                    return;
                }
                
                // All keys are now configurable - check key bindings
                string keyString = TWF.Utilities.KeyHelper.ConvertKeyToString(key);
                _logger.LogDebug($"Converted key to string: {keyString}, IsEnabled: {_keyBindings.IsEnabled}");
                
                string? action = _keyBindings.GetActionForKey(keyString);
                _logger.LogDebug($"Action for key '{keyString}': {action ?? "null"}");
                
                if (action != null && ExecuteAction(action))
                {
                    return;
                }
                
                // If no binding found, log it for debugging
                _logger.LogWarning($"No key binding found for: {keyString}");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling key press");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles key press events when in search mode
        /// </summary>
        private void HandleSearchModeKey(Key key)
        {
            try
            {
                if (key == Key.Enter || key == Key.Esc)
                {
                    ExitSearchMode();
                    return;
                }
                        
                if (key == Key.Space) // Mark and find next
                {
                    HandleSearchMarkAndNext();
                    return;
                }
                        
                if (key == Key.CursorDown) // Find next match
                {
                    HandleSearchNext();
                    return;
                }
                        
                if (key == Key.CursorUp) // Find previous match
                {
                    HandleSearchPrevious();
                    return;
                }
                        
                if (key == Key.Backspace) // Remove last character from search pattern
                {
                    HandleSearchBackspace();
                    return;
                }

                if (key == Key.K.WithCtrl) // Ctrl+K - Clear search input
                {
                    _searchPattern = string.Empty;
                    UpdateBottomBorder();
                    return;
                }
                        
                // Handle regular character input
                if (key.IsValid && !key.IsKeyCodeKey && key.AsRune.IsValid)
                {
                    char character = (char)key.AsRune.Value;
                    if (character >= 32 && character < 127)
                    {
                        HandleSearchInput(character);
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling search mode key");
                SetStatus($"Search error: {ex.Message}");
            }
        }
        

        
        /// <summary>
        /// Executes an action by name
        /// </summary>
        private bool ExecuteAction(string actionName)
        {
            try
            {
                switch (actionName)
                {
                    case "ShowHelp": ShowHelp(); return true;
                    case "EnterSearchMode": EnterSearchMode(); return true;
                    case "SwitchPane": SwitchPane(); return true;
                    case "HandleEnterKey": HandleEnterKey(); return true;
                    case "HandleShiftEnter": HandleShiftEnter(); return true;
                    case "HandleCtrlEnter": HandleCtrlEnter(); return true;
                    case "NavigateToParent": NavigateToParent(); return true;
                    case "InvertMarks": InvertMarks(); return true;
                    case "NavigateToRoot": NavigateToRoot(); return true;
                    case "MoveCursorUp": MoveCursorUp(); return true;
                    case "MoveCursorDown": MoveCursorDown(); return true;
                    case "SwitchToLeftPane": if (!_leftPaneActive) SwitchPane(); return true;
                    case "SwitchToRightPane": if (_leftPaneActive) SwitchPane(); return true;
                    case "PageUp":
                        var paneUp = GetActivePane();
                        paneUp.CursorPosition = Math.Max(0, paneUp.CursorPosition - 10);
                        RefreshPanes();
                        return true;
                    case "PageDown":
                        var paneDown = GetActivePane();
                        paneDown.CursorPosition = Math.Min(paneDown.Entries.Count - 1, paneDown.CursorPosition + 10);
                        RefreshPanes();
                        return true;
                    case "MoveCursorToFirst": MoveCursorToFirst(); return true;
                    case "MoveCursorToLast": MoveCursorToLast(); return true;
                    case "RefreshPane": LoadPaneDirectory(GetActivePane()); RefreshPanes(); SetStatus("Refreshed"); return true;
                    case "ToggleMarkAndMoveDown": ToggleMarkAndMoveDown(); return true;
                    case "ToggleMarkAndMoveUp": ToggleMarkAndMoveUp(); return true;
                    case "MarkRange": MarkRange(); return true;
                    case "DisplayMode1": HandleNumberKey(1); return true;
                    case "DisplayMode2": HandleNumberKey(2); return true;
                    case "DisplayMode3": HandleNumberKey(3); return true;
                    case "DisplayMode4": HandleNumberKey(4); return true;
                    case "DisplayMode5": HandleNumberKey(5); return true;
                    case "DisplayMode6": HandleNumberKey(6); return true;
                    case "DisplayMode7": HandleNumberKey(7); return true;
                    case "DisplayMode8": HandleNumberKey(8); return true;
                    case "DisplayModeDetailed": SetDisplayModeDetailed(); return true;
                    case "ShowWildcardMarkingDialog": ShowWildcardMarkingDialog(); return true;
                    case "HandleContextMenu": HandleContextMenu(); return true;
                    case "HandleCopyOperation": HandleCopyOperation(); return true;
                    case "HandleMoveOperation": HandleMoveOperation(); return true;
                    case "MoveToRegisteredFolder": MoveToRegisteredFolder(); return true;
                    case "HandleDeleteOperation": HandleDeleteOperation(); return true;
                    case "HandleCreateDirectory": HandleCreateDirectory(); return true;
                    case "HandleEditNewFile": HandleEditNewFile(); return true;
                    case "ShowDriveChangeDialog": ShowDriveChangeDialog(); return true;
                    case "JumpToPath": JumpToPath(); return true;
                    case "CycleSortMode": CycleSortMode(); return true;
                    case "ShowSortDialog": ShowSortDialog(); return true;
                    case "ShowFileMaskDialog": ShowFileMaskDialog(); return true;
                    case "HandleCompressionOperation": HandleCompressionOperation(); return true;
                    case "ShowRegisteredFolderDialog": ShowRegisteredFolderDialog(); return true;
                    case "RegisterCurrentDirectory": RegisterCurrentDirectory(); return true;
                    case "ShowCustomFunctionsDialog": ShowCustomFunctionsDialog(); return true;
                    case "HandleSimpleRename": HandleSimpleRename(); return true;
                    case "HandlePatternRename": HandlePatternRename(); return true;
                    case "HandleFileComparison": HandleFileComparison(); return true;
                    case "HandleFileSplitOrJoin": HandleFileSplitOrJoin(); return true;
                    case "HandleLaunchConfigurationProgram": HandleLaunchConfigurationProgram(); return true;
                    case "ReloadConfiguration": ReloadConfiguration(); return true;
                    case "ShowVersion": ShowVersionInfo(); return true;
                    case "ShowHistoryDialog": ShowHistoryDialog(); return true;
                    case "ShowFileInfoForCursor": ShowFileInfoForCursor(); return true;
                    case "HandleArchiveExtraction": HandleArchiveExtraction(); return true;
                    case "ViewFile": ViewFile(); return true;
                    case "SaveLog": SaveTaskLog(); return true;
                    case "ViewFileAsText": ViewFileAsText(); return true;
                    case "ViewFileAsHex": ViewFileAsHex(); return true;
                    case "MarkAll":
                        var allPane = GetActivePane();
                        foreach (var entry in allPane.Entries)
                            entry.IsMarked = true;
                        RefreshPanes();
                        _logger.LogDebug($"Marked all {allPane.Entries.Count} items");
                        return true;
                    case "ClearMarks":
                        var clearPane = GetActivePane();
                        foreach (var entry in clearPane.Entries)
                            entry.IsMarked = false;
                        RefreshPanes();
                        _logger.LogDebug("Marks cleared");
                        return true;
                    case "RefreshAndClearMarks":
                        var refreshClearPane = GetActivePane();
                        // Force invalidate cache to ensure fresh load
                        _directoryCache.Invalidate(refreshClearPane.CurrentPath);
                        LoadPaneDirectory(refreshClearPane); // Implicitly clears marks as objects are recreated
                        SetStatus("Refreshed and marks cleared");
                        return true;
                    case "RefreshNoClearMarks":
                        var refreshKeepPane = GetActivePane();
                        // Capture existing marks
                        var existingMarks = refreshKeepPane.Entries
                            .Where(e => e.IsMarked)
                            .Select(e => e.Name)
                            .ToList();
                        
                        // Force invalidate cache to ensure fresh load
                        _directoryCache.Invalidate(refreshKeepPane.CurrentPath);
                        LoadPaneDirectory(refreshKeepPane, null, existingMarks);
                        SetStatus("Refreshed (keeping marks)");
                        return true;
                    case "SyncPanes":
                        var activePane = GetActivePane();
                        var inactivePane = GetInactivePane();
                        inactivePane.CurrentPath = activePane.CurrentPath;
                        LoadPaneDirectory(inactivePane);
                        RefreshPanes();
                        _logger.LogDebug($"Synced panes to: {activePane.CurrentPath}");
                        return true;
                    case "SwapPanes":
                        SwapPanes();
                        return true;
                    case "ExitApplication": Application.RequestStop(); return true;
                    case "ExitApplicationAndChangeDirectory":
                        if (!string.IsNullOrEmpty(ChangeDirectoryOutputFile))
                        {
                            try
                            {
                                var activePanePath = GetActivePane().CurrentPath;
                                System.IO.File.WriteAllText(ChangeDirectoryOutputFile, activePanePath);
                                _logger.LogInformation($"Saved active pane path to {ChangeDirectoryOutputFile}: {activePanePath}");
                            }
                            catch (Exception ex)
                            {
                                _logger.LogError(ex, $"Failed to write exit directory to {ChangeDirectoryOutputFile}");
                            }
                        }
                        Application.RequestStop(); 
                        return true;
                    case "NewTab": NewTab(); return true;
                    case "CloseTab": CloseTab(); return true;
                    case "NextTab": NextTab(); return true;
                    case "PreviousTab": PreviousTab(); return true;
                    case "ShowTabSelector": ShowTabSelector(); return true;

                    // Task Management
                    case "ToggleTaskPanel":
                        _logger.LogDebug("Action: ToggleTaskPanel");
                        if (_taskStatusView != null)
                        {
                            _taskStatusView.IsExpanded = !_taskStatusView.IsExpanded;
                            _taskPanelExpanded = _taskStatusView.IsExpanded;
                            
                            // If expanding for the first time or from 1-line mode
                            if (_taskStatusView.IsExpanded && _taskPanelHeight <= 1)
                            {
                                _taskPanelHeight = DefaultTaskPanelHeight;
                            }
                            UpdateLayout();
                        }
                        return true;

                    case "ResizeTaskPanelUp":
                        _logger.LogDebug($"Action: ResizeTaskPanelUp (Current Height: {_taskPanelHeight})");
                        if (_taskStatusView != null && _taskStatusView.IsExpanded)
                        {
                            _taskPanelHeight++; 
                            UpdateLayout();
                        }
                        return true;

                    case "ResizeTaskPanelDown":
                        _logger.LogDebug($"Action: ResizeTaskPanelDown (Current Height: {_taskPanelHeight})");
                        if (_taskStatusView != null && _taskStatusView.IsExpanded)
                        {
                            _taskPanelHeight = Math.Max(1, _taskPanelHeight - 1);
                            UpdateLayout();
                        }
                        return true;

                    case "ScrollTaskPanelUp":
                        _logger.LogDebug("Action: ScrollTaskPanelUp");
                        _taskStatusView?.ScrollUp();
                        return true;

                    case "ScrollTaskPanelDown":
                        _logger.LogDebug("Action: ScrollTaskPanelDown");
                        _taskStatusView?.ScrollDown();
                        return true;

                    case "ShowJobManager":
                        Application.Run(new JobManagerDialog(_jobManager, _config));
                        return true;

                    default:
                        // Check if action matches a custom function name
                        var customFunction = _customFunctionManager.GetFunctions()
                            .FirstOrDefault(f => f.Name.Equals(actionName, StringComparison.OrdinalIgnoreCase));
                        
                        if (customFunction != null)
                        {
                            _logger.LogDebug($"Executing custom function: {actionName}");
                            var cfActivePane = GetActivePane();
                            var cfInactivePane = GetInactivePane();
                            
                            bool success = _customFunctionManager.ExecuteFunction(
                                customFunction,
                                cfActivePane,
                                cfInactivePane,
                                _leftState,
                                _rightState
                            );
                            
                            if (success)
                            {
                                SetStatus($"Executed: {customFunction.Name}");
                                // Refresh panes in case files changed
                                LoadPaneDirectory(cfActivePane);
                                LoadPaneDirectory(cfInactivePane);
                                RefreshPanes();
                            }
                            else
                            {
                                SetStatus($"Failed: {customFunction.Name}");
                            }
                            
                            return true;
                        }
                        
                        _logger.LogWarning($"Unknown action: {actionName}");
                        return false;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error executing action: {Action}", actionName);
                MessageBox.ErrorQuery("Error", $"Error executing action {actionName}: {ex.Message}", "OK");
                return false;
            }
        }

        /// <summary>
        /// Executes a built-in action with an argument
        /// </summary>
        private bool ExecuteActionWithArg(string actionName, string arg)
        {
            try
            {
                switch (actionName)
                {
                    case "JumpToPath":
                        JumpToPath(arg);
                        return true;
                    case "ExecuteFile":
                        ExecuteFile(arg);
                        return true;
                    case "ExecuteFileWithEditor":
                        ExecuteFileWithEditor(arg);
                        return true;
                    default:
                        _logger.LogWarning("Action {Action} does not support arguments or is not implemented", actionName);
                        return false;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error executing action with arg: {Action}", actionName);
                return false;
            }
        }
        /// <summary>
        /// Loads directory contents for a pane asynchronously
        /// </summary>
        private void LoadPaneDirectory(PaneState pane, string? focusTarget = null, IEnumerable<string>? preserveMarks = null)
        {
            _ = LoadPaneDirectoryAsync(pane, focusTarget, preserveMarks);
        }

        /// <summary>
        /// Saves the task status log to disk
        /// </summary>
        private void SaveTaskLog()
        {
            if (_taskStatusView != null)
            {
                bool success = _taskStatusView.SaveLog();
                if (success)
                    SetStatus("Log saved successfully");
                else
                    SetStatus("Failed to save log");
            }
        }

        private async Task LoadPaneDirectoryAsync(PaneState pane, string? focusTarget = null, IEnumerable<string>? preserveMarks = null)
        {
            CancellationTokenSource? cts = null;
            try
            {
                // Save state of PREVIOUS path
                if (_lastLoadedPaths.TryGetValue(pane, out var lastPath))
                {
                    // Only save if we are actually changing paths or refreshing
                    _navigationStateCache[lastPath] = (pane.CursorPosition, pane.ScrollOffset);
                }
                _lastLoadedPaths[pane] = pane.CurrentPath;

                // Handle Virtual Folders (Archives)
                if (pane.IsInVirtualFolder && !string.IsNullOrEmpty(pane.VirtualFolderArchivePath))
                {
                    _logger.LogDebug($"Reloading virtual folder: {pane.VirtualFolderArchivePath}");
                    
                    // Show loading state
                    pane.Entries = new List<FileEntry>();
                    RefreshPanes();

                    var archiveEntries = await _archiveManager.ListArchiveContentsAsync(pane.VirtualFolderArchivePath);
                    
                    Application.MainLoop.Invoke(() => 
                    {
                        pane.Entries = archiveEntries;
                        pane.DirectoryCount = pane.Entries.Count(e => e.IsDirectory);
                        pane.FileCount = pane.Entries.Count(e => !e.IsDirectory);
                        RestoreCursor(pane, focusTarget);
                        RefreshPanes();
                    });
                    return;
                }

                // Check Cache (only if mask is default, to avoid caching filtered subsets)
                bool useCache = string.IsNullOrEmpty(pane.FileMask) || pane.FileMask == "*";
                if (useCache && _directoryCache.TryGet(pane.CurrentPath, out var cachedEntries) && cachedEntries != null)
                {
                    _logger.LogDebug($"Cache hit for: {pane.CurrentPath}");
                    
                    // Cancel any running load
                    if (_loadingCts.TryGetValue(pane, out var existingCts))
                    {
                        existingCts.Cancel();
                        existingCts.Dispose();
                        _loadingCts.Remove(pane);
                    }
                    
                    // Sort (Cache stores raw unsorted usually, or we can store sorted. Let's sort to be safe)
                    var processedEntries = SortEngine.Sort(cachedEntries, pane.SortMode);
                    
                    pane.Entries = processedEntries;
                    RestoreCursor(pane, focusTarget);
                    
                    pane.DirectoryCount = pane.Entries.Count(e => e.IsDirectory);
                    pane.FileCount = pane.Entries.Count(e => !e.IsDirectory);

                    // Sync timestamp to prevent redundant auto-refresh
                    try {
                        if (Directory.Exists(pane.CurrentPath)) {
                            _lastDirectoryWriteTimes[pane] = Directory.GetLastWriteTime(pane.CurrentPath);
                        }
                    } catch {}
                    
                    RefreshPanes();
                    Application.MainLoop.Invoke(() => UpdateStatusBar());
                    return;
                }

                // Cache Miss - Proceed with Async Load
                if (_loadingCts.TryGetValue(pane, out var existingCts2))
                {
                    existingCts2.Cancel();
                    existingCts2.Dispose();
                }
                
                cts = new CancellationTokenSource();
                _loadingCts[pane] = cts;
                var token = cts.Token;
                
                Application.MainLoop.Invoke(() => UpdateStatusBar());

                _logger.LogDebug($"Loading directory async: {pane.CurrentPath}");

                _historyManager.Add(pane == _leftState, pane.CurrentPath);

                // Sync timestamp to prevent redundant auto-refresh during load
                try {
                    if (Directory.Exists(pane.CurrentPath)) {
                        _lastDirectoryWriteTimes[pane] = Directory.GetLastWriteTime(pane.CurrentPath);
                    }
                } catch {}

                var newEntries = new List<FileEntry>();
                var rawEntriesForCache = new List<FileEntry>(); // Capture raw for cache if needed
                var batch = new List<FileEntry>();
                var lastUpdateTime = DateTime.UtcNow;
                
                bool isCancelled = false;

                await Task.Run(async () => 
                {
                    await foreach (var item in _fileSystemProvider.EnumerateDirectoryAsync(pane.CurrentPath, token))
                    {
                        if (token.IsCancellationRequested) 
                        {
                            isCancelled = true;
                            break;
                        }

                        var entry = item.ToFileEntry();
                        batch.Add(entry);
                        if (useCache) rawEntriesForCache.Add(entry);

                        if (batch.Count >= 100 || (DateTime.UtcNow - lastUpdateTime).TotalMilliseconds > 100)
                        {
                            var batchCopy = new List<FileEntry>(batch);
                            batch.Clear();
                            lastUpdateTime = DateTime.UtcNow;
                            
                            // Filter batch for UI
                            if (!useCache)
                            {
                                batchCopy = _fileSystemProvider.ApplyFileMask(batchCopy, pane.FileMask);
                            }

                            if (batchCopy.Count > 0)
                            {
                                Application.MainLoop.Invoke(() => 
                                {
                                    if (token.IsCancellationRequested) return;
                                    
                                    newEntries.AddRange(batchCopy);
                                    pane.Entries = SortEngine.Sort(new List<FileEntry>(newEntries), pane.SortMode);
                                    
                                    // Only restore cursor on first batch? 
                                    // Or every time? Every time keeps it valid as list grows.
                                    RestoreCursor(pane, focusTarget);
                                    
                                    pane.DirectoryCount = pane.Entries.Count(e => e.IsDirectory);
                                    pane.FileCount = pane.Entries.Count(e => !e.IsDirectory);
                                    
                                    RefreshPanes();
                                });
                            }
                        }
                    }
                }, token);

                if (isCancelled) return;

                Application.MainLoop.Invoke(() => 
                {
                    if (batch.Count > 0)
                    {
                        if (!useCache)
                        {
                            batch = _fileSystemProvider.ApplyFileMask(batch, pane.FileMask);
                        }
                        newEntries.AddRange(batch);
                    }
                    
                    // Restore marks if requested
                    if (preserveMarks != null)
                    {
                        var marksSet = new HashSet<string>(preserveMarks);
                        foreach (var entry in newEntries)
                        {
                            if (marksSet.Contains(entry.Name))
                            {
                                entry.IsMarked = true;
                            }
                        }
                    }

                    pane.Entries = SortEngine.Sort(newEntries, pane.SortMode);
                    RestoreCursor(pane, focusTarget);
                    
                    pane.DirectoryCount = pane.Entries.Count(e => e.IsDirectory);
                    pane.FileCount = pane.Entries.Count(e => !e.IsDirectory);
                    
                    if (useCache)
                    {
                        _directoryCache.Add(pane.CurrentPath, rawEntriesForCache);
                    }
                    
                    RefreshPanes();
                    _logger.LogDebug($"Async load complete: {newEntries.Count} entries");
                });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Failed to load directory async: {pane.CurrentPath}");
                Application.MainLoop.Invoke(() => SetStatus($"Error loading directory: {ex.Message}"));
            }
            finally
            {
                if (cts != null && _loadingCts.TryGetValue(pane, out var currentCts) && currentCts == cts)
                {
                    _loadingCts.Remove(pane);
                    cts.Dispose();
                }
                Application.MainLoop.Invoke(() => UpdateStatusBar());
            }
        }

        private void RestoreCursor(PaneState pane, string? focusTarget)
        {
            // Priority 1: Explicit target (e.g. from Parent navigation)
            if (!string.IsNullOrEmpty(focusTarget))
            {
                for (int i = 0; i < pane.Entries.Count; i++)
                {
                    if (string.Equals(pane.Entries[i].Name, focusTarget, StringComparison.OrdinalIgnoreCase))
                    {
                        pane.CursorPosition = i;
                        // Center scroll roughly
                        int visibleHeight = 20; // Approx
                        if (i > visibleHeight / 2)
                        {
                            pane.ScrollOffset = Math.Max(0, i - (visibleHeight / 2));
                        }
                        else
                        {
                            pane.ScrollOffset = 0;
                        }
                        return;
                    }
                }
            }

            // Priority 2: History/State Cache
            if (_navigationStateCache.TryGetValue(pane.CurrentPath, out var state))
            {
                pane.CursorPosition = Math.Max(0, Math.Min(state.cursor, pane.Entries.Count - 1));
                pane.ScrollOffset = Math.Max(0, state.scroll);
                // Ensure scroll is valid
                if (pane.Entries.Count > 0 && pane.ScrollOffset >= pane.Entries.Count)
                {
                    pane.ScrollOffset = Math.Max(0, pane.Entries.Count - 10);
                }
                return;
            }

            // Priority 3: Default
            pane.CursorPosition = 0;
            pane.ScrollOffset = 0;
        }
        
        /// <summary>
        /// Runs the application main loop
        /// </summary>
        public void Run()
        {
            try
            {
                if (_app == null || _mainWindow == null)
                {
                    throw new InvalidOperationException("MainController must be initialized before running");
                }
                
                _logger.LogDebug("Starting application main loop");
                
                // Force a layout update before running
                UpdateLayout();
                
                // Set up periodic file list refresh timer (if enabled)
                if (_config.Display.FileListRefreshIntervalMs > 0)
                {
                    _logger.LogInformation($"File list auto-refresh enabled: {_config.Display.FileListRefreshIntervalMs}ms");
                    _app.MainLoop.AddTimeout(
                        TimeSpan.FromMilliseconds(_config.Display.FileListRefreshIntervalMs),
                        (mainLoop) =>
                        {
                            // Only refresh if we're in normal mode (not in dialogs/viewers)
                            if (_currentMode == UiMode.Normal)
                            {
                                // Priority 1: Check for directory-level changes (Create/Delete)
                                CheckAndRefreshFileList();

                                // Priority 2: Smart "Soft Check" for visible files (Size/Date changes)
                                if (_config.Display.SmartRefreshEnabled)
                                {
                                    PerformSmartRefresh(_leftPane, _leftState);
                                    PerformSmartRefresh(_rightPane, _rightState);
                                }
                            }
                            return true; // Continue timer
                        });
                }
                else
                {
                    _logger.LogInformation("File list auto-refresh disabled");
                }
                
                _app.Run(_mainWindow);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error in application main loop");
                throw;
            }
            finally
            {
                Shutdown();
            }
        }
        
        /// <summary>
        /// Gets the path and focus target to save for a pane in the session state.
        /// If in a virtual folder, returns the parent path and archive name.
        /// </summary>
        private (string path, string? target) GetSessionPaneState(PaneState pane)
        {
            if (pane.IsInVirtualFolder && !string.IsNullOrEmpty(pane.VirtualFolderParentPath))
            {
                string? archiveName = null;
                if (!string.IsNullOrEmpty(pane.VirtualFolderArchivePath))
                {
                    archiveName = Path.GetFileName(pane.VirtualFolderArchivePath);
                }
                return (pane.VirtualFolderParentPath, archiveName);
            }
            return (pane.CurrentPath, pane.GetCurrentEntry()?.Name);
        }

        /// <summary>
        /// Shuts down the application and saves state
        /// </summary>
        private void Shutdown()
        {
            if (_disposed) return;
            
            try
            {
                _logger.LogInformation("Shutting down application");
                
                // Save task logs
                if (_config.Display.SaveLogOnExit)
                {
                    _taskStatusView?.SaveLog();
                }

                // Save session state
                if (_config.SaveSessionState)
                {
                    var leftSession = GetSessionPaneState(_leftState);
                    var rightSession = GetSessionPaneState(_rightState);

                    var sessionState = new SessionState
                    {
                        LeftPath = leftSession.path,
                        RightPath = rightSession.path,
                        LeftFocusTarget = leftSession.target,
                        RightFocusTarget = rightSession.target,
                        LeftMask = _leftState.FileMask,
                        RightMask = _rightState.FileMask,
                        LeftSort = _leftState.SortMode,
                        RightSort = _rightState.SortMode,
                        LeftDisplayMode = _leftState.DisplayMode,
                        RightDisplayMode = _rightState.DisplayMode,
                        LeftPaneActive = _leftPaneActive,
                        LeftHistory = _historyManager.LeftHistory.ToList(),
                        RightHistory = _historyManager.RightHistory.ToList(),
                        ActiveTabIndex = _activeTabIndex,
                        TaskPaneHeight = _taskPanelHeight,
                        TaskPaneExpanded = _taskStatusView?.IsExpanded ?? false
                    };

                    // Save all tabs
                    foreach (var tab in _tabs)
                    {
                        var tLeft = GetSessionPaneState(tab.LeftState);
                        var tRight = GetSessionPaneState(tab.RightState);

                        sessionState.Tabs.Add(new TabSessionState
                        {
                            LeftPath = tLeft.path,
                            RightPath = tRight.path,
                            LeftFocusTarget = tLeft.target,
                            RightFocusTarget = tRight.target,
                            LeftMask = tab.LeftState.FileMask,
                            RightMask = tab.RightState.FileMask,
                            LeftSort = tab.LeftState.SortMode,
                            RightSort = tab.RightState.SortMode,
                            LeftDisplayMode = tab.LeftState.DisplayMode,
                            RightDisplayMode = tab.RightState.DisplayMode,
                            LeftPaneActive = tab.IsLeftPaneActive,
                            LeftHistory = tab.History.LeftHistory.ToList(),
                            RightHistory = tab.History.RightHistory.ToList()
                        });
                    }

                    _configProvider.SaveSessionState(sessionState);
                    _logger.LogInformation($"Session state saved ({_tabs.Count} tabs)");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error saving state during shutdown");
                // Continue with disposal even if save fails
            }
            finally
            {
                // Dispose IApplication instance (replaces Application.Shutdown())
                try
                {
                    _app?.Dispose();
                    _logger.LogDebug("IApplication disposed successfully");
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Error disposing IApplication");
                }
                
                // Dispose main window
                try
                {
                    _mainWindow?.Dispose();
                    _logger.LogDebug("Main window disposed successfully");
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Error disposing main window");
                }
                
                _disposed = true;
                _disposed = true;
            }
        }
        
        /// <summary>
        /// Creates a new tab
        /// </summary>
        private void NewTab()
        {
            try 
            {
                var newTab = new TabSession(_config);
                
                // Copy current path to new tab
                newTab.LeftState.CurrentPath = _leftState.CurrentPath;
                newTab.RightState.CurrentPath = _rightState.CurrentPath;
                newTab.LeftState.SortMode = _leftState.SortMode;
                newTab.RightState.SortMode = _rightState.SortMode;
                newTab.LeftState.FileMask = _leftState.FileMask;
                newTab.RightState.FileMask = _rightState.FileMask;
                newTab.IsLeftPaneActive = _leftPaneActive;
                
                _tabs.Add(newTab);
                _activeTabIndex = _tabs.Count - 1;
                
                // Load directories for the new tab
                // Note: _historyManager now points to the new tab's history
                LoadPaneDirectory(newTab.LeftState);
                LoadPaneDirectory(newTab.RightState);
                
                RefreshPanes();
                _logger.LogDebug($"New tab created ({_tabs.Count})");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error creating new tab");
                _logger.LogDebug("Error creating tab");
            }
        }

        /// <summary>
        /// Closes the current tab
        /// </summary>
        private void CloseTab()
        {
            if (_tabs.Count <= 1)
            {
                _logger.LogDebug("Cannot close last tab");
                return;
            }
            
            _tabs.RemoveAt(_activeTabIndex);
            if (_activeTabIndex >= _tabs.Count)
            {
                _activeTabIndex = _tabs.Count - 1;
            }
            
            // Reload directories for the new active tab
            LoadPaneDirectory(_leftState);
            LoadPaneDirectory(_rightState);
            
            RefreshPanes();
            _logger.LogDebug($"Tab closed ({_tabs.Count} remaining)");
        }

        /// <summary>
        /// Switches to the next tab
        /// </summary>
        private void NextTab()
        {
            if (_tabs.Count <= 1) return;
            _activeTabIndex = (_activeTabIndex + 1) % _tabs.Count;
            
            // Reload directories for the new active tab
            LoadPaneDirectory(_leftState, CurrentTab.LeftFocusTarget);
            LoadPaneDirectory(_rightState, CurrentTab.RightFocusTarget);
            
            // Clear targets after use
            CurrentTab.LeftFocusTarget = null;
            CurrentTab.RightFocusTarget = null;
            
            RefreshPanes();
        }

        /// <summary>
        /// Switches to the previous tab
        /// </summary>
        private void PreviousTab()
        {
            if (_tabs.Count <= 1) return;
            _activeTabIndex = (_activeTabIndex - 1 + _tabs.Count) % _tabs.Count;
            
            // Reload directories for the new active tab
            LoadPaneDirectory(_leftState, CurrentTab.LeftFocusTarget);
            LoadPaneDirectory(_rightState, CurrentTab.RightFocusTarget);
            
            // Clear targets after use
            CurrentTab.LeftFocusTarget = null;
            CurrentTab.RightFocusTarget = null;
            
            RefreshPanes();
        }

        /// <summary>
        /// Shows a dialog to select and manage tabs
        /// </summary>
        public void ShowTabSelector()
        {
            int truncLen = _config.Display.TabNameTruncationLength > 0 ? _config.Display.TabNameTruncationLength : 8;
            string spinner = _taskStatusView?.CurrentSpinnerFrame ?? "";

            var items = _tabs.Select((t, i) => new TabSelectorDialog.TabItem
            {
                OriginalIndex = i,
                DisplayName = GetTabDisplayName(i, t, truncLen, spinner),
                LeftPath = t.LeftState.CurrentPath,
                RightPath = t.RightState.CurrentPath,
                IsActive = i == _activeTabIndex
            }).ToList();

            var dialog = new TabSelectorDialog(items, _searchEngine, _config, (idx) => 
            {
                if (_tabs.Count <= 1) return false;
                CloseTabAtIndex(idx);
                return true;
            });

            Application.Run(dialog);

            if (dialog.IsJumped && dialog.SelectedTabIndex >= 0 && dialog.SelectedTabIndex < _tabs.Count)
            {
                _activeTabIndex = dialog.SelectedTabIndex;
                // Reload directories for the new active tab
                LoadPaneDirectory(_leftState, CurrentTab.LeftFocusTarget);
                LoadPaneDirectory(_rightState, CurrentTab.RightFocusTarget);
                
                // Clear targets
                CurrentTab.LeftFocusTarget = null;
                CurrentTab.RightFocusTarget = null;
                
                RefreshPanes();
            }
        }

        private void CloseTabAtIndex(int index)
        {
            if (_tabs.Count <= 1 || index < 0 || index >= _tabs.Count) return;

            _tabs.RemoveAt(index);
            if (_activeTabIndex >= _tabs.Count)
            {
                _activeTabIndex = _tabs.Count - 1;
            }
            else if (_activeTabIndex > index)
            {
                // If we closed a tab before the active one, shift index
                _activeTabIndex--;
            }

            // If the active tab changed, reload
            LoadPaneDirectory(_leftState);
            LoadPaneDirectory(_rightState);
            RefreshPanes();
        }

        private string GetTabDisplayName(int index, TabSession tab, int truncLen, string spinner)
        {
            // Get names for both panes
            string leftName = Path.GetFileName(tab.LeftState.CurrentPath);
            if (string.IsNullOrEmpty(leftName)) leftName = tab.LeftState.CurrentPath;
            
            string rightName = Path.GetFileName(tab.RightState.CurrentPath);
            if (string.IsNullOrEmpty(rightName)) rightName = tab.RightState.CurrentPath;

            // Truncate to maximum width and use single-char ellipsis
            leftName = CharacterWidthHelper.TruncateToWidth(leftName, truncLen, "\u2026");
            rightName = CharacterWidthHelper.TruncateToWidth(rightName, truncLen, "\u2026");

            // Indicators for which pane is active within the tab
            string leftInd = tab.IsLeftPaneActive ? "*" : " ";
            string rightInd = tab.IsLeftPaneActive ? " " : "*";

            // Format the core tab information
            string tabCore = $"{index + 1}:{leftName}{leftInd}|{rightName}{rightInd}";

            // Get busy spinners for background jobs in this tab
            int busyCount = _jobManager.GetActiveJobCount(index);
            string busySpinners = "";
            if (busyCount > 0 && !string.IsNullOrEmpty(spinner))
            {
                busySpinners = new string(spinner[0], busyCount);
            }

            return $"{tabCore}{busySpinners}";
        }

        /// <summary>
        /// Updates the tab bar display
        /// </summary>
        private void UpdateTabBar()
        {
            if (_tabBar == null) return;
            
            int truncLen = _config.Display.TabNameTruncationLength > 0 ? _config.Display.TabNameTruncationLength : 8;
            string spinner = _taskStatusView?.CurrentSpinnerFrame ?? "";

            var names = new List<string>();
            for (int i = 0; i < _tabs.Count; i++)
            {
                names.Add(GetTabDisplayName(i, _tabs[i], truncLen, spinner));
            }
            
            _tabBar.UpdateTabs(names, _activeTabIndex);
        }
        
        /// <summary>
        /// Refreshes both panes to reflect current state
        /// </summary>
        public void RefreshPanes()
        {
            RefreshPane(_leftPane, _leftState, _leftPaneActive);
            RefreshPane(_rightPane, _rightState, !_leftPaneActive);
            
            // Update window title with paths (top border)
            UpdateWindowTitle();
            UpdateTabBar();
            
            // Update status bar with drive usage (only if not in search mode)
            if (_currentMode != UiMode.Search)
            {
                UpdateStatusBar();
            }
            
            // Update bottom border with current filename
            UpdateBottomBorder();
            
            // Move cursor off-screen to prevent it from appearing on the pane view
            // This handles cursor artifacts from dialogs and viewers
            if (Application.Driver != null)
            {
                Application.Driver.Move(0, Application.Driver.Rows);
            }
        }
        
        /// <summary>
        /// Lightweight display update without reloading data (for resize events)
        /// </summary>
        private void UpdateDisplay()
        {
            // Update dynamic layout first
            UpdateLayout();

            // Just redraw the panes without reloading file data
            _leftPane?.SetNeedsDisplay();
            _rightPane?.SetNeedsDisplay();
            
            // Update window title and status
            UpdateWindowTitle();
            UpdateStatusBar();
            UpdateBottomBorder();
        }
        
        /// <summary>
        /// Checks if file list has changed and refreshes if needed (for timer-based refresh)
        /// </summary>
        private void CheckAndRefreshFileList()
        {
            try
            {
                bool leftChanged = HasDirectoryTimestampChanged(_leftState);
                bool rightChanged = HasDirectoryTimestampChanged(_rightState);
                
                if (leftChanged || rightChanged)
                {
                    if (leftChanged) LoadPaneDirectory(_leftState);
                    if (rightChanged) LoadPaneDirectory(_rightState);
                    
                    Application.MainLoop.Invoke(() => RefreshPanes());
                }
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Error checking file list changes");
            }
        }

        /// <summary>
        /// Performs a "Soft Check" on visible files to update size/date without full reload
        /// </summary>
        private void PerformSmartRefresh(PaneView? pane, PaneState state)
        {
            if (pane == null || state == null || state.Entries == null || state.IsInVirtualFolder) return;

            bool needsRedraw = false;
            try 
            {
                var visibleEntries = pane.GetVisibleEntries().ToList();
                foreach (var entry in visibleEntries)
                {
                    if (entry.IsDirectory || entry.Name == "..") continue; // Skip directories (expensive/noisy)

                    var path = Path.Combine(state.CurrentPath, entry.Name);
                    if (!File.Exists(path)) continue;

                    var info = new FileInfo(path);
                    // No need to call .Refresh() on new FileInfo instance
                    
                    if (info.Length != entry.Size || info.LastWriteTime != entry.LastModified)
                    {
                        entry.Size = info.Length;
                        entry.LastModified = info.LastWriteTime;
                        needsRedraw = true;
                    }
                }

                if (needsRedraw)
                {
                    pane.SetNeedsDisplay();
                }
            }
            catch 
            {
                // Ignore errors (e.g. file locked/deleted during check)
            }
        }
        
        /// <summary>
        /// Checks if a directory timestamp has changed (fast O(1) check)
        /// </summary>
        private bool HasDirectoryTimestampChanged(PaneState pane)
        {
            try
            {
                if (string.IsNullOrEmpty(pane.CurrentPath) || !Directory.Exists(pane.CurrentPath) || pane.IsInVirtualFolder)
                {
                    return false;
                }
                
                var currentTimestamp = Directory.GetLastWriteTime(pane.CurrentPath);
                
                if (!_lastDirectoryWriteTimes.TryGetValue(pane, out var lastTimestamp))
                {
                    _lastDirectoryWriteTimes[pane] = currentTimestamp;
                    return false;
                }

                if (currentTimestamp != lastTimestamp)
                {
                    _lastDirectoryWriteTimes[pane] = currentTimestamp;
                    return true;
                }
                
                return false;
            }
            catch
            {
                return false;
            }
        }
        
        /// <summary>
        /// Swaps the paths of left and right panes
        /// </summary>
        public void SwapPanes()
        {
            try
            {
                // Store the current paths
                string leftPath = _leftState.CurrentPath;
                string rightPath = _rightState.CurrentPath;
                
                // Swap the paths
                _leftState.CurrentPath = rightPath;
                _rightState.CurrentPath = leftPath;
                
                // Reload both panes with their new paths
                LoadPaneDirectory(_leftState);
                LoadPaneDirectory(_rightState);
                
                // Refresh display
                RefreshPanes();
                
                _logger.LogDebug("Swapped panes: Left='{LeftPath}', Right='{RightPath}'", rightPath, leftPath);
                _logger.LogDebug($"Swapped panes");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error swapping panes: {Message}", ex.Message);
                SetStatus($"Error swapping panes: {ex.Message}");
            }
        }

        
        /// <summary>
        /// Refreshes a single pane display
        /// </summary>
        private void RefreshPane(PaneView? pane, PaneState state, bool isActive)
        {
            if (pane == null) return;
            
            // Update pane state and active status
            pane.State = state;
            pane.IsActive = isActive;
            
            // Trigger redraw
            pane.SetNeedsDisplay();
        }
        
        /// <summary>
        /// Switches focus between left and right panes
        /// </summary>
        public void SwitchPane()
        {
            _leftPaneActive = !_leftPaneActive;
            
            if (_leftPaneActive && _leftPane != null)
            {
                _leftPane.SetFocus();
            }
            else if (!_leftPaneActive && _rightPane != null)
            {
                _rightPane.SetFocus();
            }
            
            RefreshPanes();
            _logger.LogDebug($"Switched to {(_leftPaneActive ? "left" : "right")} pane");
        }
        
        /// <summary>
        /// Sets the status bar message (redirects to TaskStatusView log)
        /// </summary>
        public void SetStatus(string message)
        {
            if (_taskStatusView != null)
            {
                _taskStatusView.AddLog(message);
                _logger.LogDebug($"Status: {message}");
            }
        }
        
        /// <summary>
        /// Sets the message area text (redirects to TaskStatusView log)
        /// </summary>
        public void SetMessage(string message)
        {
            SetStatus(message);
        }
        
        /// <summary>
        /// Gets the currently active pane state
        /// </summary>
        public PaneState GetActivePane()
        {
            return _leftPaneActive ? _leftState : _rightState;
        }
        
        /// <summary>
        /// Gets the inactive pane state
        /// </summary>
        public PaneState GetInactivePane()
        {
            return _leftPaneActive ? _rightState : _leftState;
        }
        
        /// <summary>
        /// Gets the current UI mode
        /// </summary>
        public UiMode GetCurrentMode()
        {
            return _currentMode;
        }
        
        /// <summary>
        /// Sets the current UI mode
        /// </summary>
        public void SetMode(UiMode mode)
        {
            _currentMode = mode;
            _logger.LogDebug($"UI mode changed to: {mode}");
        }
        
        /// <summary>
        /// Navigates to a specific directory
        /// </summary>
        public void NavigateToDirectory(string path, string? focusTarget = null)
        {
            var activePane = GetActivePane();
            
            // Clear virtual folder state when navigating to a real directory
            activePane.IsInVirtualFolder = false;
            activePane.VirtualFolderArchivePath = null;
            activePane.VirtualFolderParentPath = null;
            
            activePane.CurrentPath = path;
            LoadPaneDirectory(activePane, focusTarget);
            RefreshPanes();
            _logger.LogDebug($"Navigated to: {path}");
        }
        
        /// <summary>
        /// Opens an archive file as a virtual folder
        /// </summary>
        private void OpenArchiveAsVirtualFolder(string archivePath)
        {
            _ = OpenArchiveAsVirtualFolderAsync(archivePath);
        }

        private async Task OpenArchiveAsVirtualFolderAsync(string archivePath)
        {
            var activePane = GetActivePane();
            CancellationTokenSource? cts = null;
            
            try
            {
                if (_loadingCts.TryGetValue(activePane, out var existingCts))
                {
                    existingCts.Cancel();
                    existingCts.Dispose();
                }
                
                cts = new CancellationTokenSource();
                _loadingCts[activePane] = cts;
                
                Application.MainLoop.Invoke(() => UpdateStatusBar());

                _logger.LogDebug($"Opening archive async: {archivePath}");
                
                // Store the parent directory path before entering virtual folder
                activePane.VirtualFolderParentPath = activePane.CurrentPath;
                activePane.VirtualFolderArchivePath = archivePath;
                activePane.IsInVirtualFolder = true;
                
                // Set the current path to indicate we're in an archive
                activePane.CurrentPath = $"[{Path.GetFileName(archivePath)}]";
                
                // Set entries to empty to indicate loading
                activePane.Entries = new List<FileEntry>();
                activePane.CursorPosition = 0;
                activePane.ScrollOffset = 0;
                
                RefreshPanes();
                
                // Get archive contents async
                var archiveEntries = await _archiveManager.ListArchiveContentsAsync(archivePath, cts.Token);
                
                if (cts.Token.IsCancellationRequested) return;

                Application.MainLoop.Invoke(() => 
                {
                    activePane.Entries = archiveEntries;
                    activePane.DirectoryCount = activePane.Entries.Count(e => e.IsDirectory);
                    activePane.FileCount = activePane.Entries.Count(e => !e.IsDirectory);
                    
                    RefreshPanes();
                    
                    _logger.LogDebug($"Viewing archive: {Path.GetFileName(archivePath)} ({archiveEntries.Count} entries)");
                });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Failed to open archive: {archivePath}");
                Application.MainLoop.Invoke(() => 
                {
                    SetStatus($"Error opening archive: {ex.Message}");
                    // Revert navigation if we failed to load
                    if (activePane.IsInVirtualFolder && activePane.VirtualFolderParentPath != null)
                    {
                        activePane.CurrentPath = activePane.VirtualFolderParentPath;
                        activePane.IsInVirtualFolder = false;
                        activePane.VirtualFolderArchivePath = null;
                        activePane.VirtualFolderParentPath = null;
                        LoadPaneDirectory(activePane);
                    }
                });
            }
            finally
            {
                if (cts != null && _loadingCts.TryGetValue(activePane, out var currentCts) && currentCts == cts)
                {
                    _loadingCts.Remove(activePane);
                    cts.Dispose();
                }
                Application.MainLoop.Invoke(() => UpdateStatusBar());
            }
        }
        
        /// <summary>
        /// Handles O key press - extracts archive to current directory
        /// </summary>
        public void HandleArchiveExtraction()
        {
            var activePane = GetActivePane();
            var currentEntry = activePane.GetCurrentEntry();
            
            if (currentEntry == null)
            {
                return;
            }
            
            // Check if this is an archive file
            if (!_archiveManager.IsArchive(currentEntry.FullPath))
            {
                SetStatus("Not an archive file");
                return;
            }
            
            try
            {
                _logger.LogDebug($"Extracting archive: {currentEntry.FullPath}");
                
                // Show confirmation dialog
                var confirmed = ShowConfirmationDialog(
                    "Extract Archive",
                    $"Extract '{currentEntry.Name}' to current directory?");
                
                if (!confirmed)
                {
                    SetStatus("Extraction cancelled");
                    return;
                }
                
                // Execute extraction with progress dialog
                var cancellationTokenSource = new CancellationTokenSource();
                var progressDialog = new OperationProgressDialog("Extracting Archive", cancellationTokenSource);
                progressDialog.Status = "Extracting...";
                
                // Execute extraction asynchronously
                Task.Run(async () =>
                {
                    try
                    {
                        var result = await _archiveManager.ExtractAsync(
                            currentEntry.FullPath,
                            activePane.CurrentPath,
                            cancellationTokenSource.Token);
                        
                        Application.MainLoop.Invoke(() =>
                        {
                            Application.RequestStop();
                            
                            if (result.Success)
                            {
                                SetStatus($"Extracted {result.FilesProcessed} file(s) from {currentEntry.Name}");
                                
                                // Refresh the pane to show extracted files
                                LoadPaneDirectory(activePane);
                                RefreshPanes();
                            }
                            else
                            {
                                SetStatus($"Extraction failed: {result.Message}");
                                
                                if (result.Errors.Count > 0)
                                {
                                    var errorMsg = string.Join("\n", result.Errors.Take(5));
                                    if (result.Errors.Count > 5)
                                    {
                                        errorMsg += $"\n... and {result.Errors.Count - 5} more errors";
                                    }
                                    ShowMessageDialog("Extraction Errors", errorMsg);
                                }
                            }
                        });
                    }
                    catch (Exception ex)
                    {
                        Application.MainLoop.Invoke(() =>
                        {
                            Application.RequestStop();
                            SetStatus($"Extraction failed: {ex.Message}");
                            _logger.LogError(ex, "Archive extraction failed");
                        });
                    }
                });
                
                // Show the progress dialog
                Application.Run(progressDialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error extracting archive: {currentEntry.FullPath}");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Navigates to the parent directory in the active pane
        /// Exits virtual folder if currently browsing an archive
        /// </summary>
        public void NavigateToParent()
        {
            var activePane = GetActivePane();
            
            // Check if we're in a virtual folder (archive)
            if (activePane.IsInVirtualFolder && activePane.VirtualFolderParentPath != null)
            {
                // Exit the virtual folder and return to the parent directory
                _logger.LogDebug($"Exiting virtual folder, returning to: {activePane.VirtualFolderParentPath}");
                
                string parentPath = activePane.VirtualFolderParentPath;
                string? archiveName = null;
                
                if (!string.IsNullOrEmpty(activePane.VirtualFolderArchivePath))
                {
                    archiveName = Path.GetFileName(activePane.VirtualFolderArchivePath);
                }
                
                // Clear virtual folder state
                activePane.IsInVirtualFolder = false;
                activePane.VirtualFolderArchivePath = null;
                activePane.VirtualFolderParentPath = null;
                
                // Navigate back to the parent directory
                activePane.CurrentPath = parentPath;
                LoadPaneDirectory(activePane, archiveName);
                RefreshPanes();
                
                _logger.LogDebug($"Exited archive");
            }
            else
            {
                // Normal directory navigation
                var parentPath = Directory.GetParent(activePane.CurrentPath)?.FullName;
                
                if (parentPath != null)
                {
                    // Remember the current folder name to position cursor on it
                    string currentFolderName = Path.GetFileName(activePane.CurrentPath);
                    
                    NavigateToDirectory(parentPath, currentFolderName);
                }
                else
                {
                    SetStatus("Already at root directory");
                }
            }
        }
        
        /// <summary>
        /// Positions the cursor on a specific entry by name
        /// </summary>
        private void PositionCursorOnEntry(PaneState pane, string entryName)
        {
            try
            {
                // Find the index of the entry with the matching name
                int index = pane.Entries.FindIndex(e => 
                    string.Equals(e.Name, entryName, StringComparison.OrdinalIgnoreCase));
                
                if (index >= 0)
                {
                    pane.CursorPosition = index;
                    _logger.LogDebug($"Positioned cursor on entry: {entryName} at index {index}");
                }
                else
                {
                    _logger.LogDebug($"Entry not found: {entryName}, cursor remains at position {pane.CursorPosition}");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error positioning cursor on entry: {entryName}");
            }
        }
        
        /// <summary>
        /// Navigates to the root directory of the current drive
        /// </summary>
        public void NavigateToRoot()
        {
            var activePane = GetActivePane();
            var rootPath = Path.GetPathRoot(activePane.CurrentPath);
            
            if (!string.IsNullOrEmpty(rootPath))
            {
                NavigateToDirectory(rootPath);
            }
        }
        
        /// <summary>
        /// Navigates to a registered folder
        /// </summary>
        public void NavigateToRegisteredFolder(RegisteredFolder folder)
        {
            if (folder == null)
            {
                _logger.LogWarning("Attempted to navigate to null registered folder");
                SetStatus("Error: Invalid registered folder");
                return;
            }
            
            if (string.IsNullOrEmpty(folder.Path))
            {
                _logger.LogWarning($"Registered folder '{folder.Name}' has no path");
                SetStatus($"Error: Registered folder '{folder.Name}' has no path");
                return;
            }
            
            // Expand environment variables in the folder path
            string expandedPath = ExpandEnvironmentVariables(folder.Path);

            if (!Directory.Exists(expandedPath))
            {
                _logger.LogWarning($"Registered folder path does not exist: {expandedPath}");
                SetStatus($"Error: Path does not exist: {expandedPath}");
                return;
            }

            _logger.LogDebug($"Navigating to registered folder: {folder.Name} (original: {folder.Path}, expanded: {expandedPath})");
            NavigateToDirectory(expandedPath);
            SetStatus($"Navigated to: {folder.Name}");
        }

        /// <summary>
        /// Expands environment variables in a path string.
        /// Supports formats: $VAR, ${VAR}, $env:VAR, %VAR%
        /// </summary>
        /// <param name="path">The path string that may contain environment variables</param>
        /// <returns>The path with environment variables expanded</returns>
        private string ExpandEnvironmentVariables(string path)
        {
            if (string.IsNullOrEmpty(path))
                return path;

            string originalPath = path;
            string result = path;

            _logger.LogDebug($"Expanding environment variables in path: {path}");

            // Handle %VAR% format
            result = ExpandPercentFormat(result);

            // Handle $VAR format
            result = ExpandDollarFormat(result);

            // Handle ${VAR} format
            result = ExpandDollarBraceFormat(result);

            // Handle $env:VAR format (PowerShell style)
            result = ExpandDollarEnvFormat(result);

            if (originalPath != result)
            {
                _logger.LogDebug($"Environment variable expansion: '{originalPath}' -> '{result}'");
            }
            else
            {
                _logger.LogDebug($"No environment variables expanded in path: {originalPath}");
            }

            return result;
        }

        /// <summary>
        /// Expands %VAR% format environment variables
        /// </summary>
        private string ExpandPercentFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            // Use a more specific regex to match %VAR% format properly
            var regex = new System.Text.RegularExpressions.Regex(@"%([A-Za-z_][A-Za-z0-9_]*)%");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);

                if (varValue != null)
                {
                    _logger.LogDebug($"Expanded environment variable: %{varName}% -> {varValue}");
                    return varValue;
                }
                else
                {
                    _logger.LogDebug($"Environment variable not found: %{varName}% (keeping original)");
                    return match.Value; // Return original %VAR% if not found
                }
            });
        }

        /// <summary>
        /// Expands $VAR format environment variables
        /// </summary>
        private string ExpandDollarFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            var regex = new System.Text.RegularExpressions.Regex(@"\$([A-Za-z_][A-Za-z0-9_]*)");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);

                if (varValue != null)
                {
                    _logger.LogDebug($"Expanded environment variable: ${varName} -> {varValue}");
                    return varValue;
                }
                else
                {
                    _logger.LogDebug($"Environment variable not found: ${varName} (keeping original)");
                    return match.Value; // Return original $VAR if not found
                }
            });
        }

        /// <summary>
        /// Expands ${VAR} format environment variables
        /// </summary>
        private string ExpandDollarBraceFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            var regex = new System.Text.RegularExpressions.Regex(@"\$\{([^}]+)\}");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);

                if (varValue != null)
                {
                    _logger.LogDebug($"Expanded environment variable: ${{{varName}}} -> {varValue}");
                    return varValue;
                }
                else
                {
                    _logger.LogDebug($"Environment variable not found: ${{{varName}}} (keeping original)");
                    return match.Value; // Return original ${{VAR}} if not found
                }
            });
        }

        /// <summary>
        /// Expands $env:VAR format environment variables (PowerShell style)
        /// </summary>
        private string ExpandDollarEnvFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            var regex = new System.Text.RegularExpressions.Regex(@"\$env:([A-Za-z_][A-Za-z0-9_]*)");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);

                if (varValue != null)
                {
                    _logger.LogDebug($"Expanded environment variable: $env:{varName} -> {varValue}");
                    return varValue;
                }
                else
                {
                    _logger.LogDebug($"Environment variable not found: $env:{varName} (keeping original)");
                    return match.Value; // Return original $env:VAR if not found
                }
            });
        }

        /// <summary>
        /// Shows registered folders dialog (I key)
        /// </summary>
        public void ShowRegisteredFolderDialog()
        {
            try
            {
                // Get registered folders from configuration
                var registeredFolders = _listProvider.GetJumpList();
                
                if (registeredFolders.Count == 0)
                {
                    SetStatus("No registered folders. Press Shift+B to register current directory.");
                    return;
                }
                
                // Create selection dialog
                var dialog = new RegisteredFolderDialog(
                    registeredFolders,
                    _searchEngine,
                    _config,
                    (selectedFolder) => NavigateToRegisteredFolder(selectedFolder),
                    (folderToDelete) => DeleteRegisteredFolder(folderToDelete),
                    _logger);
                
                // Show dialog
                Application.Run(dialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing registered folder dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows custom functions dialog and executes selected function
        /// </summary>
        public void ShowCustomFunctionsDialog()
        {
            try
            {
                var functions = _customFunctionManager.GetFunctions();
                
                if (functions.Count == 0)
                {
                    SetStatus("No custom functions defined. Create custom_functions.json in %APPDATA%\\TWF\\");
                    return;
                }
                
                // Show custom function selection dialog
                var dialog = new TWF.UI.CustomFunctionDialog(functions);
                Application.Run(dialog);
                
                // Execute selected function
                if (dialog.SelectedFunction != null)
                {
                    var activePane = GetActivePane();
                    var inactivePane = GetInactivePane();
                    
                    bool success = _customFunctionManager.ExecuteFunction(
                        dialog.SelectedFunction,
                        activePane,
                        inactivePane,
                        _leftState,
                        _rightState
                    );
                    
                    if (success)
                    {
                        SetStatus($"Executed: {dialog.SelectedFunction.Name}");
                        
                        // Only reload and refresh if it wasn't a piped action that navigated elsewhere
                        // (Actions like JumpToPath already handle their own reloading and selection)
                        if (string.IsNullOrEmpty(dialog.SelectedFunction.PipeToAction))
                        {
                            // Refresh panes in case files changed
                            LoadPaneDirectory(activePane);
                            LoadPaneDirectory(inactivePane);
                            RefreshPanes();
                        }
                    }
                    else
                    {
                        SetStatus($"Failed to execute: {dialog.SelectedFunction.Name}");
                    }
                }
                else
                {
                    SetStatus("Cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing custom functions dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Registers the current directory as a registered folder
        /// Handles Shift+B key press
        /// </summary>
        public void RegisterCurrentDirectory()
        {
            try
            {
                var activePane = GetActivePane();
                var currentPath = activePane.CurrentPath;
                
                // Don't allow registering virtual folders
                if (activePane.IsInVirtualFolder)
                {
                    SetStatus("Cannot register virtual folders (archives)");
                    return;
                }
                
                // Determine target path: selected directory or current path
                var selectedEntry = activePane.GetCurrentEntry();
                string targetPath = currentPath;
                string defaultName = Path.GetFileName(currentPath) ?? "Folder";

                if (selectedEntry != null && selectedEntry.IsDirectory && selectedEntry.Name != "..")
                {
                    targetPath = selectedEntry.FullPath;
                    defaultName = selectedEntry.Name;
                }
                
                var dialog = new RegisterFolderDialog(defaultName, targetPath);
                Application.Run(dialog);
                
                // Process the input if OK was pressed
                if (dialog.IsOk)
                {
                    string folderName = dialog.FolderName;
                    
                    if (!string.IsNullOrWhiteSpace(folderName))
                    {
                        // Check if this path is already registered
                        if (_config.RegisteredFolders.Any(f => f.Path.Equals(targetPath, StringComparison.OrdinalIgnoreCase)))
                        {
                            SetStatus("This folder is already registered");
                            return;
                        }
                        
                        // Create new registered folder
                        var newFolder = new RegisteredFolder
                        {
                            Name = folderName,
                            Path = targetPath // Use the determined target path
                        };
                        
                        _config.RegisteredFolders.Add(newFolder);
                        
                        // Save configuration
                        _configProvider.SaveConfiguration(_config);
                        
                        SetStatus($"Registered folder: {folderName}");
                        _logger.LogInformation($"Registered folder: {folderName} -> {targetPath}");
                    }
                    else
                    {
                        SetStatus("Invalid folder name");
                    }
                }
                else
                {
                    SetStatus("Cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error registering folder");
                SetStatus($"Error: {ex.Message}");
            }
        }

        
        /// <summary>
        /// Moves marked files to a selected registered folder
        /// Handles Shift+M key press
        /// </summary>
        public void MoveToRegisteredFolder()
        {
            try
            {
                var activePane = GetActivePane();
                
                // Get files to move (marked files or current file)
                var filesToMove = activePane.GetMarkedEntries();
                if (filesToMove.Count == 0)
                {
                    var currentEntry = activePane.GetCurrentEntry();
                    if (currentEntry != null)
                    {
                        filesToMove.Add(currentEntry);
                    }
                }
                
                if (filesToMove.Count == 0)
                {
                    SetStatus("No files to move");
                    return;
                }
                
                // Get registered folders
                var registeredFolders = _listProvider.GetJumpList();
                
                if (registeredFolders.Count == 0)
                {
                    SetStatus("No registered folders. Press Shift+B to register a folder.");
                    return;
                }
                
                // Create selection dialog
                var dialog = new Dialog("Move to Registered Folder", 70, 20);
                
                var label = new Label($"Select destination for {filesToMove.Count} file(s):")
                {
                    X = 1,
                    Y = 1,
                    Width = Dim.Fill(1)
                };
                dialog.Add(label);
                
                // Create list view with registered folders
                var folderList = new ListView()
                {
                    X = 1,
                    Y = 2,
                    Width = Dim.Fill(1),
                    Height = Dim.Fill(3),
                    AllowsMarking = false
                };
                
                var displayItems = registeredFolders.Select(f => $"{f.Name} - {f.Path}").ToList();
                folderList.SetSource(displayItems);
                dialog.Add(folderList);
                
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
                
                bool okPressed = false;
                
                okButton.Clicked += () =>
                {
                    okPressed = true;
                    Application.RequestStop();
                };
                
                cancelButton.Clicked += () =>
                {
                    Application.RequestStop();
                };
                
                dialog.Add(okButton);
                dialog.Add(cancelButton);
                
                // Set focus to the list
                folderList.SetFocus();
                
                // Show dialog
                Application.Run(dialog);
                
                // Process the selection
                if (okPressed && folderList.SelectedItem >= 0 && folderList.SelectedItem < registeredFolders.Count)
                {
                    var selectedFolder = registeredFolders[folderList.SelectedItem];
                    
                    if (!Directory.Exists(selectedFolder.Path))
                    {
                        SetStatus($"Error: Destination path does not exist: {selectedFolder.Path}");
                        return;
                    }
                    
                    // Perform move operation
                    PerformMoveOperation(filesToMove, selectedFolder.Path);
                }
                else
                {
                    SetStatus("Move cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error moving to registered folder");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Deletes a registered folder from configuration
        /// </summary>
        private void DeleteRegisteredFolder(RegisteredFolder folder)
        {
            try
            {
                // Find and remove the folder
                var folderToRemove = _config.RegisteredFolders.FirstOrDefault(f => 
                    f.Name == folder.Name && f.Path == folder.Path);
                
                if (folderToRemove != null)
                {
                    _config.RegisteredFolders.Remove(folderToRemove);
                    
                    // Save configuration
                    _configProvider.SaveConfiguration(_config);
                    
                    SetStatus($"Deleted registered folder: {folder.Name}");
                    _logger.LogInformation($"Deleted registered folder: {folder.Name}");
                }
                else
                {
                    SetStatus("Folder not found in configuration");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error deleting registered folder");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Performs the actual move operation for files
        /// </summary>
        private async void PerformMoveOperation(List<FileEntry> files, string destination)
        {
            try
            {
                SetStatus($"Moving {files.Count} file(s)...");
                
                var result = await _fileOps.MoveAsync(files, destination, CancellationToken.None);
                
                if (result.Success)
                {
                    SetStatus($"Moved {result.FilesProcessed} file(s) successfully");
                    
                    // Refresh both panes
                    LoadPaneDirectory(_leftState);
                    LoadPaneDirectory(_rightState);
                    RefreshPanes();
                }
                else
                {
                    SetStatus($"Move failed: {result.Message}");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error performing move operation");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Moves the cursor up in the active pane
        /// </summary>
        public void MoveCursorUp()
        {
            var activePane = GetActivePane();
            if (activePane.CursorPosition > 0)
            {
                activePane.CursorPosition--;
                //RefreshPanes();
                UpdateDisplay();
            }
        }
        
        /// <summary>
        /// Moves the cursor down in the active pane
        /// </summary>
        public void MoveCursorDown()
        {
            var activePane = GetActivePane();
            if (activePane.CursorPosition < activePane.Entries.Count - 1)
            {
                activePane.CursorPosition++;
                //RefreshPanes();
                UpdateDisplay();
            }
        }
        
        /// <summary>
        /// Moves the cursor to the first entry in the active pane
        /// </summary>
        public void MoveCursorToFirst()
        {
            var activePane = GetActivePane();
            if (activePane.Entries.Count > 0)
            {
                activePane.CursorPosition = 0;
                //RefreshPanes();
                UpdateDisplay();
            }
        }
        
        /// <summary>
        /// Moves the cursor to the last entry in the active pane
        /// </summary>
        public void MoveCursorToLast()
        {
            var activePane = GetActivePane();
            if (activePane.Entries.Count > 0)
            {
                activePane.CursorPosition = activePane.Entries.Count - 1;
                //RefreshPanes();
                UpdateDisplay();
            }
        }
        
        /// <summary>
        /// Handles Enter key press - navigates into directories, opens archives, or executes files
        /// </summary>
        public void HandleEnterKey()
        {
            var activePane = GetActivePane();
            var currentEntry = activePane.GetCurrentEntry();
            
            if (currentEntry == null)
            {
                return;
            }
            
            // Check if this is an archive file
            if (!currentEntry.IsDirectory && _archiveManager.IsArchive(currentEntry.FullPath))
            {
                // Open archive as virtual folder
                OpenArchiveAsVirtualFolder(currentEntry.FullPath);
            }
            else if (currentEntry.IsDirectory)
            {
                // Navigate into directory
                NavigateToDirectory(currentEntry.FullPath);
            }
            else if (IsTextFile(currentEntry.FullPath))
            {
                // Open text file in text viewer
                OpenTextFile(currentEntry.FullPath);
            }
            else if (IsImageFile(currentEntry.FullPath))
            {
                // Open image file in image viewer
                OpenImageFile(currentEntry.FullPath);
            }
            else
            {
                // Execute file with default handler or association
                ExecuteFile(currentEntry.FullPath, ExecutionMode.Default);
            }
        }
        
        /// <summary>
        /// Handles Shift+Enter key press - opens file with editor or extracts archive
        /// </summary>
        public void HandleShiftEnter()
        {
            var activePane = GetActivePane();
            var currentEntry = activePane.GetCurrentEntry();
            
            if (currentEntry == null)
            {
                return;
            }
            
            // Check if this is an archive file
            if (!currentEntry.IsDirectory && _archiveManager.IsArchive(currentEntry.FullPath))
            {
                // Extract archive
                HandleArchiveExtraction();
            }
            else if (!currentEntry.IsDirectory)
            {
                // Open file with text editor
                ExecuteFile(currentEntry.FullPath, ExecutionMode.Editor);
            }
            else
            {
                SetStatus("Cannot open directory with editor");
            }
        }
        
        /// <summary>
        /// Handles Ctrl+Enter key press - opens file with Explorer association
        /// </summary>
        public void HandleCtrlEnter()
        {
            var activePane = GetActivePane();
            var currentEntry = activePane.GetCurrentEntry();
            
            if (currentEntry == null)
            {
                return;
            }
            
            if (!currentEntry.IsDirectory)
            {
                // Open file with Windows Explorer association
                ExecuteFile(currentEntry.FullPath, ExecutionMode.ExplorerAssociation);
            }
            else
            {
                SetStatus("Cannot execute directory");
            }
        }
        
        /// <summary>
        /// Executes a file with the specified execution mode
        /// </summary>
        private void ExecuteFile(string filePath, ExecutionMode mode)
        {
            try
            {
                _logger.LogDebug($"Executing file: {filePath} with mode: {mode}");
                
                var result = _fileOps.ExecuteFile(filePath, _config, mode);
                
                if (result.Success)
                {
                    SetStatus(result.Message);
                }
                else
                {
                    SetStatus($"Execution failed: {result.Message}");
                    _logger.LogError($"File execution failed: {result.Message}");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error executing file: {filePath}");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Determines if a file should be opened in the text viewer
        /// </summary>
        private bool IsTextFile(string filePath)
        {
            var textExtensions = new[]
            {
                ".txt", ".log", ".md", ".cs", ".json", ".xml", ".html", ".htm",
                ".css", ".js", ".ts", ".py", ".java", ".cpp", ".c", ".h",
                ".ini", ".cfg", ".conf", ".config", ".yaml", ".yml",
                ".sh", ".bat", ".cmd", ".ps1", ".sql", ".csv"
            };
            
            var extension = Path.GetExtension(filePath).ToLowerInvariant();
            return textExtensions.Contains(extension);
        }
        
        /// <summary>
        /// Opens a text file in the text viewer
        /// </summary>
        private void OpenTextFile(string filePath)
        {
            try
            {
                _logger.LogDebug($"Opening text file: {filePath}");
                
                // Open the file in the viewer manager
                _viewerManager.OpenTextViewer(filePath);
                
                // Get the text viewer instance
                var textViewer = _viewerManager.CurrentTextViewer;
                if (textViewer == null)
                {
                    SetStatus("Error: Failed to open text viewer");
                    return;
                }
                
                // Change UI mode to TextViewer
                SetMode(UiMode.TextViewer);
                
                // Create and show the text viewer window
                var tvLogger = LoggingConfiguration.GetLogger<UI.TextViewerWindow>();
                var viewerWindow = new UI.TextViewerWindow(textViewer, _keyBindings, _searchEngine, _config, tvLogger);
                Application.Run(viewerWindow);
                
                // After viewer closes, return to normal mode
                SetMode(UiMode.Normal);
                _viewerManager.CloseCurrentViewer();
                
                // Refresh panes
                RefreshPanes();
                
                _logger.LogDebug("Text viewer closed");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error opening text file: {filePath}");
                SetStatus($"Error opening file: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Determines if a file should be opened in the image viewer
        /// </summary>
        private bool IsImageFile(string filePath)
        {
            var imageExtensions = _config.Viewer.SupportedImageExtensions;
            
            var extension = Path.GetExtension(filePath).ToLowerInvariant();
            return imageExtensions.Contains(extension);
        }
        
        /// <summary>
        /// Opens an image file in the image viewer
        /// </summary>
        private void OpenImageFile(string filePath)
        {
            try
            {
                _logger.LogDebug($"Opening image file: {filePath}");
                
                // Open the file in the viewer manager
                _viewerManager.OpenImageViewer(filePath);
                
                // Get the image viewer instance
                var imageViewer = _viewerManager.CurrentImageViewer;
                if (imageViewer == null)
                {
                    SetStatus("Error: Failed to open image viewer");
                    return;
                }
                
                // Change UI mode to ImageViewer
                SetMode(UiMode.ImageViewer);
                
                // Create and show the image viewer window
                var viewerWindow = new UI.ImageViewerWindow(imageViewer, _keyBindings, _config);
                Application.Run(viewerWindow);
                
                // After viewer closes, return to normal mode
                SetMode(UiMode.Normal);
                _viewerManager.CloseCurrentViewer();
                
                // Refresh panes
                RefreshPanes();
                
                _logger.LogDebug("Image viewer closed");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error opening image file: {filePath}");
                SetStatus($"Error opening file: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Switches the display mode for the active pane
        /// Preserves cursor position and scroll offset
        /// </summary>
        public void SwitchDisplayMode(DisplayMode mode)
        {
            var activePane = GetActivePane();
            
            // Store current cursor position and scroll offset
            int previousCursorPosition = activePane.CursorPosition;
            int previousScrollOffset = activePane.ScrollOffset;
            
            // Change display mode
            activePane.DisplayMode = mode;
            
            // Preserve cursor position and scroll offset
            activePane.CursorPosition = previousCursorPosition;
            activePane.ScrollOffset = previousScrollOffset;
            
            // Refresh display
            RefreshPanes();
            
            // Update status to show current display mode
            string modeName = mode switch
            {
                DisplayMode.NameOnly => "Name Only",
                DisplayMode.Details => "Details",
                DisplayMode.Thumbnail => "Thumbnail",
                DisplayMode.Icon => "Icon",
                DisplayMode.OneColumn => "1 Column",
                DisplayMode.TwoColumns => "2 Columns",
                DisplayMode.ThreeColumns => "3 Columns",
                DisplayMode.FourColumns => "4 Columns",
                DisplayMode.FiveColumns => "5 Columns",
                DisplayMode.SixColumns => "6 Columns",
                DisplayMode.SevenColumns => "7 Columns",
                DisplayMode.EightColumns => "8 Columns",
                _ => "Unknown"
            };
            
            _logger.LogDebug($"Display mode: {modeName}");
            _logger.LogDebug($"Display mode changed to: {mode}");
        }
        
        /// <summary>
        /// Handles number key press (1-8) for display mode switching
        /// </summary>
        public void HandleNumberKey(int number)
        {
            if (number < 1 || number > 8)
            {
                _logger.LogWarning($"Invalid display mode number: {number}");
                return;
            }
            
            DisplayMode mode = number switch
            {
                1 => DisplayMode.OneColumn,
                2 => DisplayMode.TwoColumns,
                3 => DisplayMode.ThreeColumns,
                4 => DisplayMode.FourColumns,
                5 => DisplayMode.FiveColumns,
                6 => DisplayMode.SixColumns,
                7 => DisplayMode.SevenColumns,
                8 => DisplayMode.EightColumns,
                _ => DisplayMode.Details
            };
            
            SwitchDisplayMode(mode);
        }
        
        /// <summary>
        /// Toggles mark on current entry and moves cursor down
        /// Handles Space key press
        /// </summary>
        public void ToggleMarkAndMoveDown()
        {
            var activePane = GetActivePane();
            
            if (activePane.Entries.Count == 0)
            {
                return;
            }
            
            // Toggle mark at current cursor position
            _markingEngine.ToggleMark(activePane, activePane.CursorPosition);
            
            // Move cursor down
            MoveCursorDown();
            
            // Refresh display
            RefreshPanes();
            
            // Update status
            int markedCount = activePane.Entries.Count(e => e.IsMarked);
            _logger.LogTrace($"Marked: {markedCount} file(s)");

            _logger.LogDebug($"Toggled mark at index {activePane.CursorPosition}, marked count: {markedCount}");
        }        
        /// <summary>
        /// Toggles mark on current entry and moves cursor up
        /// Handles Shift+Space key press
        /// </summary>
        public void ToggleMarkAndMoveUp()
        {
            var activePane = GetActivePane();
            
            if (activePane.Entries.Count == 0)
            {
                return;
            }
            
            // Toggle mark at current cursor position
            _markingEngine.ToggleMark(activePane, activePane.CursorPosition);
            
            // Move cursor up
            MoveCursorUp();
            
            // Refresh display
            RefreshPanes();
            
            // Update status
            int markedCount = activePane.Entries.Count(e => e.IsMarked);
            _logger.LogTrace($"Marked: {markedCount} file(s)");

            _logger.LogDebug($"Toggled mark at index {activePane.CursorPosition}, marked count: {markedCount}");
        }
        
        /// <summary>
        /// Marks range from last marked entry to current cursor position
        /// Handles Ctrl+Space key press
        /// </summary>
        public void MarkRange()
        {
            var activePane = GetActivePane();
            
            if (activePane.Entries.Count == 0)
            {
                return;
            }
            
            // Find the last marked index (or use 0 if none marked)
            int lastMarkedIndex = activePane.Entries.FindLastIndex(e => e.IsMarked);
            if (lastMarkedIndex < 0) lastMarkedIndex = 0;
            
            // Mark range from last marked to current cursor
            _markingEngine.MarkRange(activePane, lastMarkedIndex, activePane.CursorPosition);
            
            // Refresh display
            RefreshPanes();
            
                        // Update status
            
                        int markedCount = activePane.Entries.Count(e => e.IsMarked);
            
                        _logger.LogDebug($"Marked range: {markedCount} file(s)");
            
            
            
                        _logger.LogDebug($"Marked range from {lastMarkedIndex} to {activePane.CursorPosition}");
            
                    }
        
        /// <summary>
        /// Inverts all marks in the active pane
        /// Handles Home or backtick key press
        /// </summary>
        public void InvertMarks()
        {
            var activePane = GetActivePane();
            
            if (activePane.Entries.Count == 0)
            {
                return;
            }
            
            // Store count before inversion
            int beforeCount = activePane.Entries.Count(e => e.IsMarked);
            
            // Invert all marks
            _markingEngine.InvertMarks(activePane);
            
            // Refresh display
            RefreshPanes();
            
            // Update status
            int afterCount = activePane.Entries.Count(e => e.IsMarked);
            _logger.LogDebug($"Inverted marks: {beforeCount} -> {afterCount} file(s)");

            _logger.LogDebug($"Inverted marks: {beforeCount} -> {afterCount}");
        }
        
        /// <summary>
        /// Shows wildcard marking dialog and applies pattern to mark files
        /// Handles @ key press
        /// Supports wildcard patterns with exclusions (colon prefix)
        /// Supports regex patterns with m/ syntax
        /// </summary>
        public void ShowWildcardMarkingDialog()
        {
            var activePane = GetActivePane();
            
            if (activePane.Entries.Count == 0)
            {
                SetStatus("No files to mark");
                return;
            }
            
            try
            {
                var dialog = new WildcardMarkingDialog(_config);
                
                // Show dialog
                Application.Run(dialog);
                
                // Process the pattern if OK was pressed
                if (dialog.IsOk)
                {
                    string pattern = dialog.Pattern;
                    
                    if (!string.IsNullOrWhiteSpace(pattern))
                    {
                        ApplyWildcardPattern(pattern);
                    }
                    else
                    {
                        SetStatus("No pattern entered");
                    }
                }
                else
                {
                    _logger.LogTrace("Wildcard marking cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing wildcard marking dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Applies a wildcard or regex pattern to mark files
        /// Supports multiple patterns separated by spaces
        /// Patterns starting with colon (:) are exclusion patterns
        /// Patterns starting with m/ are treated as regex patterns
        /// </summary>
        private void ApplyWildcardPattern(string pattern)
        {
            var activePane = GetActivePane();
            
            try
            {
                // Clear existing marks before applying pattern
                _markingEngine.ClearMarks(activePane);
                
                // Check if this is a regex pattern
                if (pattern.TrimStart().StartsWith("m/"))
                {
                    // Remove m/ prefix and apply regex marking
                    string regexPattern = pattern.TrimStart().Substring(2);
                    _markingEngine.MarkByRegex(activePane, regexPattern);
                    
                                        int markedCount = activePane.Entries.Count(e => e.IsMarked);
                    
                                        _logger.LogDebug($"Regex pattern applied: {markedCount} file(s) marked");
                    
                                        _logger.LogDebug($"Applied regex pattern '{regexPattern}': {markedCount} files marked");
                    
                                    }
                    
                                    else
                    
                                    {
                    
                                        // Apply wildcard pattern
                    
                                        _markingEngine.MarkByWildcard(activePane, pattern);
                    
                    
                    
                                        int markedCount = activePane.Entries.Count(e => e.IsMarked);
                    
                                        _logger.LogDebug($"Pattern applied: {markedCount} file(s) marked");
                    
                                        _logger.LogDebug($"Applied wildcard pattern '{pattern}': {markedCount} files marked");
                    
                                    }
                
                // Refresh display to show marked files
                RefreshPanes();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error applying pattern: {pattern}");
                SetStatus($"Error applying pattern: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles C key press - initiates copy operation
        /// Copies marked files or current file to opposite pane
        /// </summary>
        public void HandleCopyOperation()
        {
            var activePane = GetActivePane();
            var inactivePane = GetInactivePane();
            
            // Get files to copy (marked files or current file)
            var filesToCopy = activePane.GetMarkedEntries();
            if (filesToCopy.Count == 0)
            {
                var currentEntry = activePane.GetCurrentEntry();
                if (currentEntry != null)
                {
                    filesToCopy = new List<FileEntry> { currentEntry };
                }
            }
            
            if (filesToCopy.Count == 0)
            {
                SetStatus("No files to copy");
                return;
            }

            // Check if we are inside an archive
            if (activePane.IsInVirtualFolder && !string.IsNullOrEmpty(activePane.VirtualFolderArchivePath))
            {
                HandleArchiveCopyOut(activePane, inactivePane, filesToCopy);
                return;
            }
            
            // Execute copy operation via JobManager
            int tabId = _activeTabIndex;
            string tabName = Path.GetFileName(activePane.CurrentPath);
            
            _jobManager.StartJob(
                $"Copy to {Path.GetFileName(inactivePane.CurrentPath)}",
                $"Copying {filesToCopy.Count} items",
                tabId,
                tabName,
                async (job, token, progress) => 
                {
                    // Adapter for progress reporting
                    var progressAdapter = new Progress<ProgressEventArgs>(e => 
                    {
                        double filePercent = e.CurrentFileTotalBytes > 0 ? (double)e.CurrentFileBytesProcessed / e.CurrentFileTotalBytes * 100 : 0;
                        string stats = $"Count: {e.CurrentFileIndex}/{e.TotalFiles}";
                        if (e.TotalBytes > 0)
                        {
                            stats += $" - Overall: {e.PercentComplete:F0}% - File: {filePercent:F0}%";
                        }

                        if (e.Status == FileOperationStatus.Completed)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Copied {e.CurrentFile} [OK]");
                        }
                        else if (e.Status == FileOperationStatus.Failed)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Failed to copy {e.CurrentFile} [FAIL]");
                        }
                        else if (e.Status == FileOperationStatus.Skipped)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Skipped {e.CurrentFile} [WARN]");
                        }

                        progress.Report(new JobProgress 
                        { 
                            Percent = e.PercentComplete, 
                            Message = stats,
                            CurrentOperationDetail = e.CurrentFile
                        });
                    });

                    var result = await _fileOps.CopyAsync(filesToCopy, inactivePane.CurrentPath, token, HandleCollision, progressAdapter);
                    
                    if (!result.Success && result.Message != "Operation cancelled by user")
                    {
                        // Errors are logged by JobManager if exception is thrown, or we can handle result here
                         throw new Exception(result.Message);
                    }
                });
                
            SetStatus("Copy operation started in background");
        }
        
        /// <summary>
        /// Handles M key press - initiates move operation
        /// Moves marked files or current file to opposite pane
        /// </summary>
        public void HandleMoveOperation()
        {
            var activePane = GetActivePane();
            var inactivePane = GetInactivePane();
            
            // Get files to move (marked files or current file)
            var filesToMove = activePane.GetMarkedEntries();
            if (filesToMove.Count == 0)
            {
                var currentEntry = activePane.GetCurrentEntry();
                if (currentEntry != null)
                {
                    filesToMove = new List<FileEntry> { currentEntry };
                }
            }
            
            if (filesToMove.Count == 0)
            {
                SetStatus("No files to move");
                return;
            }
            
            // Execute move operation via JobManager
            int tabId = _activeTabIndex;
            string tabName = Path.GetFileName(activePane.CurrentPath);
            
            _jobManager.StartJob(
                $"Move to {Path.GetFileName(inactivePane.CurrentPath)}",
                $"Moving {filesToMove.Count} items",
                tabId,
                tabName,
                async (job, token, progress) => 
                {
                    // Adapter for progress reporting
                    var progressAdapter = new Progress<ProgressEventArgs>(e => 
                    {
                        double filePercent = e.CurrentFileTotalBytes > 0 ? (double)e.CurrentFileBytesProcessed / e.CurrentFileTotalBytes * 100 : 0;
                        string stats = $"Count: {e.CurrentFileIndex}/{e.TotalFiles}";
                        if (e.TotalBytes > 0)
                        {
                            stats += $" - Overall: {e.PercentComplete:F0}% - File: {filePercent:F0}%";
                        }

                        if (e.Status == FileOperationStatus.Completed)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Moved {e.CurrentFile} [OK]");
                        }
                        else if (e.Status == FileOperationStatus.Failed)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Failed to move {e.CurrentFile} [FAIL]");
                        }
                        else if (e.Status == FileOperationStatus.Skipped)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Skipped {e.CurrentFile} [WARN]");
                        }

                        progress.Report(new JobProgress 
                        { 
                            Percent = e.PercentComplete, 
                            Message = stats,
                            CurrentOperationDetail = e.CurrentFile
                        });
                    });

                    var result = await _fileOps.MoveAsync(filesToMove, inactivePane.CurrentPath, token, HandleCollision, progressAdapter);
                    
                    if (!result.Success && result.Message != "Operation cancelled by user")
                    {
                         throw new Exception(result.Message);
                    }
                });
                
            SetStatus("Move operation started in background");
        }
        
        /// <summary>
        /// Handles D key press - initiates delete operation with confirmation
        /// Deletes marked files or current file after user confirmation
        /// </summary>
        public void HandleDeleteOperation()
        {
            var activePane = GetActivePane();
            
            // Get files to delete (marked files or current file)
            var filesToDelete = activePane.GetMarkedEntries();
            if (filesToDelete.Count == 0)
            {
                var currentEntry = activePane.GetCurrentEntry();
                if (currentEntry != null)
                {
                    filesToDelete = new List<FileEntry> { currentEntry };
                }
            }
            
            if (filesToDelete.Count == 0)
            {
                SetStatus("No files to delete");
                return;
            }
            
            // Show confirmation dialog
            var fileList = string.Join(", \n", filesToDelete.Take(3).Select(f => f.IsDirectory ? f.Name + "/" : f.Name));
            if (filesToDelete.Count > 3)
            {
                fileList += $"\n and {filesToDelete.Count - 3} more";
            }

            // Count files and directories separately for proper message
            int fileCount = filesToDelete.Count(e => !e.IsDirectory);
            int dirCount = filesToDelete.Count(e => e.IsDirectory);

            string deleteMessage;
            if (fileCount > 0 && dirCount > 0)
            {
                // Mixed deletion: show both directories and files
                string dirText = dirCount == 1 ? "directory" : "directories";
                string fileText = fileCount == 1 ? "file" : "files";
                deleteMessage = $"{dirCount} {dirText}, {fileCount} {fileText}";
            }
            else if (dirCount > 0)
            {
                // Only directories
                string dirText = dirCount == 1 ? "directory" : "directories";
                deleteMessage = $"{dirCount} {dirText}";
            }
            else
            {
                // Only files
                string fileText = fileCount == 1 ? "file" : "files";
                deleteMessage = $"{fileCount} {fileText}";
            }

            var confirmed = ShowConfirmationDialog(
                "Delete Confirmation",
                $"Delete {deleteMessage}?\n{fileList}");

            if (!confirmed)
            {
                SetStatus("Delete cancelled");
                return;
            }

            // Check if we are inside an archive
            if (activePane.IsInVirtualFolder && !string.IsNullOrEmpty(activePane.VirtualFolderArchivePath))
            {
                HandleArchiveDelete(activePane, filesToDelete);
                return;
            }

            // Execute delete operation via JobManager
            int tabId = _activeTabIndex;
            string tabName = Path.GetFileName(activePane.CurrentPath);

            _jobManager.StartJob(
                $"Delete {deleteMessage}",
                $"Deleting from {tabName}",
                tabId,
                tabName,
                async (job, token, progress) => 
                {
                    var progressAdapter = new Progress<ProgressEventArgs>(e => 
                    {
                        string stats = $"Deleting ({e.CurrentFileIndex}/{e.TotalFiles}) - {e.PercentComplete:F0}%";
                        
                        if (e.Status == FileOperationStatus.Completed)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Deleted {e.CurrentFile} [OK]");
                        }
                        else if (e.Status == FileOperationStatus.Failed)
                        {
                            _taskStatusView?.AddLog($"#{job.ShortId}: Failed to delete {e.CurrentFile} [FAIL]");
                        }

                        progress.Report(new JobProgress 
                        { 
                            Percent = e.PercentComplete, 
                            Message = stats,
                            CurrentOperationDetail = e.CurrentFile 
                        });
                    });

                    var result = await _fileOps.DeleteAsync(filesToDelete, token, progressAdapter);
                    
                    if (!result.Success && result.Message != "Operation cancelled by user")
                    {
                         throw new Exception(result.Message);
                    }
                });
                
            SetStatus("Delete operation started in background");
        }
        
        /// <summary>
        /// Handles R key press - simple rename of current file
        /// Shows input dialog with current filename, allows editing
        /// </summary>
        public void HandleSimpleRename()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null)
                {
                    SetStatus("No file selected");
                    return;
                }
                
                var dialog = new SimpleRenameDialog(currentEntry.Name);
                Application.Run(dialog);
                
                if (dialog.IsOk)
                {
                    string newName = dialog.NewName;
                    if (!string.IsNullOrWhiteSpace(newName) && newName != currentEntry.Name)
                    {
                        try
                        {
                            string oldPath = currentEntry.FullPath;
                            string newPath = Path.Combine(Path.GetDirectoryName(oldPath) ?? "", newName);
                            
                            if (Path.Exists(newPath))
                            {
                                SetStatus($"Already exists: {newName}");
                            }
                            else
                            {
                                if (currentEntry.IsDirectory)
                                    Directory.Move(oldPath, newPath);
                                else
                                    File.Move(oldPath, newPath);
                                LoadPaneDirectory(activePane);
                                RefreshPanes();
                                SetStatus($"Renamed to: {newName}");
                                _logger.LogInformation($"Renamed {currentEntry.Name} to {newName}");
                            }
                        }
                        catch (Exception ex)
                        {
                            SetStatus($"Rename failed: {ex.Message}");
                            _logger.LogError(ex, "Error renaming file");
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error in simple rename");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles Shift+R key press - initiates pattern-based rename operation
        /// Shows dialog for pattern input, previews new names, and renames on confirmation
        /// </summary>
        public void HandlePatternRename()
        {
            var activePane = GetActivePane();
            
            // Get files to rename (marked files or current file)
            var filesToRename = activePane.GetMarkedEntries();
            if (filesToRename.Count == 0)
            {
                var currentEntry = activePane.GetCurrentEntry();
                if (currentEntry != null)
                {
                    filesToRename = new List<FileEntry> { currentEntry };
                }
            }
            
            if (filesToRename.Count == 0)
            {
                SetStatus("No files to rename");
                return;
            }
            
            var dialog = new PatternRenameDialog();
            Application.Run(dialog);
            
            if (!dialog.IsOk || string.IsNullOrEmpty(dialog.Pattern))
            {
                SetStatus("Rename cancelled");
                return;
            }
            
            string pattern = dialog.Pattern;
            string replacement = dialog.Replacement;

            // Execute rename operation with progress dialog
            ExecuteFileOperationWithProgress(
                "Rename",
                filesToRename,
                string.Empty, // No destination for rename
                (files, dest, token) => _fileOps.RenameAsync(files, pattern, replacement));
        }
        
        /// <summary>
        /// Applies rename pattern for preview purposes (same logic as FileOperations.ApplyRenamePattern)
        /// </summary>
        private string ApplyRenamePatternPreview(string filename, string pattern, string replacement)
        {
            // Support regex patterns with s/ syntax
            if (pattern.StartsWith("s/"))
            {
                var parts = pattern.Split('/');
                if (parts.Length >= 3)
                {
                    var searchPattern = parts[1];
                    var replacePattern = parts[2];
                    return System.Text.RegularExpressions.Regex.Replace(filename, searchPattern, replacePattern);
                }
            }

            // Support transliteration with tr/ syntax
            if (pattern.StartsWith("tr/"))
            {
                var parts = pattern.Split('/');
                if (parts.Length >= 3)
                {
                    var fromChars = parts[1];
                    var toChars = parts[2];
                    var result = filename.ToCharArray();
                    for (int i = 0; i < result.Length; i++)
                    {
                        int index = fromChars.IndexOf(result[i]);
                        if (index >= 0 && index < toChars.Length)
                        {
                            result[i] = toChars[index];
                        }
                    }
                    return new string(result);
                }
            }

            // Simple string replacement
            return filename.Replace(pattern, replacement);
        }
        
        /// <summary>
        /// Handles W key press - initiates file comparison operation
        /// Shows dialog to select comparison criteria, then marks matching files
        /// </summary>
        public void HandleFileComparison()
        {
            try
            {
                var leftPane = _leftState;
                var rightPane = _rightState;
                
                // Check if both panes have files
                if (leftPane.Entries.Count == 0 && rightPane.Entries.Count == 0)
                {
                    SetStatus("Both panes are empty");
                    return;
                }
                
                var dialog = new FileComparisonDialog();
                Application.Run(dialog);
                
                if (!dialog.IsOk)
                {
                    SetStatus("Comparison cancelled");
                    return;
                }
                
                // Execute comparison
                var result = _fileOps.CompareFiles(leftPane, rightPane, dialog.Criteria, dialog.TimestampTolerance);

                
                if (result.Success)
                {
                    // Refresh panes to show marked files
                    RefreshPanes();
                    SetStatus(result.Message);
                    _logger.LogInformation($"File comparison completed: {result.Message}");
                }
                else
                {
                    SetStatus($"Comparison failed: {result.Message}");
                    _logger.LogError($"File comparison failed: {result.Message}");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error during file comparison");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows a confirmation dialog
        /// </summary>

        
        /// <summary>
        /// Shows a confirmation dialog and returns user's choice
        /// </summary>
        private bool ShowConfirmationDialog(string title, string message)
        {
            var helpFg = ColorHelper.ParseConfigColor(_config.Display.DialogHelpForegroundColor, Color.BrightYellow);
            var helpBg = ColorHelper.ParseConfigColor(_config.Display.DialogHelpBackgroundColor, Color.Blue);
            
            var dialog = new ConfirmationDialog(title, message, "[Enter] Continue [Esc] Cancel", helpFg, helpBg);
            Application.Run(dialog);
            
            return dialog.Confirmed;
        }
        
        /// <summary>
        /// Executes a file operation with progress dialog and cancellation support
        /// </summary>
        private void ExecuteFileOperationWithProgress(
            string operationName,
            List<FileEntry> files,
            string destination,
            Func<List<FileEntry>, string, CancellationToken, Task<OperationResult>> operation)
        {
            var cancellationTokenSource = new CancellationTokenSource();
            var progressDialog = new OperationProgressDialog($"{operationName} Progress", cancellationTokenSource);
            
            // Subscribe to progress events
            EventHandler<ProgressEventArgs>? progressHandler = (sender, e) =>
            {
                Application.MainLoop.Invoke(() =>
                {
                    progressDialog.UpdateProgress(e.CurrentFile, e.CurrentFileIndex, e.TotalFiles, e.PercentComplete, e.BytesProcessed, e.TotalBytes);
                });
            };
            
            _fileOps.ProgressChanged += progressHandler;
            
            // Execute operation asynchronously
            Task.Run(async () =>
            {
                try
                {
                    var result = await operation(files, destination, cancellationTokenSource.Token);
                    
                    Application.MainLoop.Invoke(() =>
                    {
                        _fileOps.ProgressChanged -= progressHandler;
                        Application.RequestStop();
                        
                        // Show result message
                        SetStatus(result.Message);
                        
                        // Refresh both panes
                        LoadPaneDirectory(_leftState);
                        LoadPaneDirectory(_rightState);
                        RefreshPanes();
                    });
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, $"Error during {operationName} operation");
                    Application.MainLoop.Invoke(() =>
                    {
                        _fileOps.ProgressChanged -= progressHandler;
                        Application.RequestStop();
                        SetStatus($"{operationName} failed: {ex.Message}");
                    });
                }
            });
            
            // Show progress dialog (blocking until RequestStop)
            Application.Run(progressDialog);
        }

        /// <summary>
        /// Handles file collision by showing a dialog to the user
        /// </summary>
        private Task<FileCollisionResult> HandleCollision(string destPath)
        {
            var tcs = new TaskCompletionSource<FileCollisionResult>();
            
            Application.MainLoop.Invoke(() =>
            {
                var dialog = new FileCollisionDialog(Path.GetFileName(destPath));
                Application.Run(dialog);
                tcs.SetResult(dialog.Result);
            });
            
            return tcs.Task;
        }
        
        /// <summary>
        /// Shows a message dialog
        /// </summary>
        private void ShowMessageDialog(string title, string message)
        {
            Application.Run(new MessageDialog(title, message));
        }
        
        /// <summary>
        /// Handles J key press - shows directory creation dialog
        /// Creates a new directory in the active pane's current path
        /// Positions cursor on the newly created directory
        /// Handles Escape for cancellation
        /// </summary>
        public void HandleCreateDirectory()
        {
            var activePane = GetActivePane();
            
            try
            {
                var dialog = new CreateDirectoryDialog();
                Application.Run(dialog);
                
                // Process the directory name if OK was pressed
                if (dialog.IsOk)
                {
                    string directoryName = dialog.DirectoryName;
                    
                    if (!string.IsNullOrWhiteSpace(directoryName))
                    {
                        CreateDirectory(directoryName);
                    }
                    else
                    {
                        SetStatus("No directory name entered");
                    }
                }
                else
                {
                    SetStatus("Directory creation cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing directory creation dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Creates a new directory in the active pane's current path
        /// and positions the cursor on the newly created directory
        /// </summary>
        private void CreateDirectory(string directoryName)
        {
            var activePane = GetActivePane();
            
            try
            {
                // Create the directory using FileOperations
                var result = _fileOps.CreateDirectory(activePane.CurrentPath, directoryName);
                
                if (result.Success)
                {
                    // Reload the directory to show the new folder
                    LoadPaneDirectory(activePane);
                    
                    // Find the newly created directory and position cursor on it
                    var newDirPath = Path.Combine(activePane.CurrentPath, directoryName);
                    var newDirIndex = activePane.Entries.FindIndex(e => 
                        e.FullPath.Equals(newDirPath, StringComparison.OrdinalIgnoreCase));
                    
                    if (newDirIndex >= 0)
                    {
                        activePane.CursorPosition = newDirIndex;
                    }
                    
                    // Refresh display
                    RefreshPanes();
                    
                    SetStatus($"Directory created: {directoryName}");
                    _logger.LogInformation($"Directory created: {newDirPath}");
                }
                else
                {
                    SetStatus($"Failed to create directory: {result.Message}");
                    _logger.LogWarning($"Failed to create directory '{directoryName}': {result.Message}");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error creating directory: {directoryName}");
                SetStatus($"Error creating directory: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles E key press - shows dialog to create and edit a new file
        /// Creates a 0-byte file in the active pane's current path
        /// Opens the file with the associated program based on extension
        /// Positions cursor on the newly created file
        /// Handles Escape for cancellation
        /// </summary>
        public void HandleEditNewFile()
        {
            var activePane = GetActivePane();
            
            try
            {
                var dialog = new CreateNewFileDialog();
                Application.Run(dialog);
                
                // Process the file name if OK was pressed
                if (dialog.IsOk)
                {
                    string fileName = dialog.FileName;
                    
                    if (!string.IsNullOrWhiteSpace(fileName))
                    {
                        CreateAndEditNewFile(fileName);
                    }
                    else
                    {
                        SetStatus("No file name entered");
                    }
                }
                else
                {
                    SetStatus("File creation cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing new file creation dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Creates a new 0-byte file in the active pane's current path,
        /// opens it with the associated program, and positions the cursor on it
        /// </summary>
        private void CreateAndEditNewFile(string fileName)
        {
            var activePane = GetActivePane();
            
            try
            {
                // Validate file name
                var invalidChars = Path.GetInvalidFileNameChars();
                if (fileName.IndexOfAny(invalidChars) >= 0)
                {
                    SetStatus("Invalid file name: contains invalid characters");
                    _logger.LogWarning($"Invalid file name attempted: {fileName}");
                    return;
                }
                
                // Build full path
                var fullPath = Path.Combine(activePane.CurrentPath, fileName);
                
                // Check if file already exists
                if (File.Exists(fullPath))
                {
                    SetStatus($"File already exists: {fileName}");
                    _logger.LogWarning($"File already exists: {fullPath}");
                    return;
                }
                
                // Create 0-byte file
                File.Create(fullPath).Dispose();
                _logger.LogInformation($"Created new file: {fullPath}");
                
                // Reload the directory to show the new file
                LoadPaneDirectory(activePane);
                
                // Find the newly created file and position cursor on it
                var newFileIndex = activePane.Entries.FindIndex(e => 
                    e.FullPath.Equals(fullPath, StringComparison.OrdinalIgnoreCase));
                
                if (newFileIndex >= 0)
                {
                    activePane.CursorPosition = newFileIndex;
                }
                
                // Refresh display
                RefreshPanes();
                
                // Open the file with associated program
                OpenFileWithAssociatedProgram(fullPath);
                
                SetStatus($"Created and opened: {fileName}");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error creating new file: {fileName}");
                SetStatus($"Error creating file: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Opens a file with the associated program from ExtensionAssociations config
        /// Falls back to default system association if no specific association is configured
        /// </summary>
        private void OpenFileWithAssociatedProgram(string filePath)
        {
            try
            {
                var extension = Path.GetExtension(filePath).ToLowerInvariant();
                
                string? programPath = null;
                
                // Look up associated program from configuration
                if (!string.IsNullOrEmpty(extension) && 
                    _config.ExtensionAssociations.TryGetValue(extension, out var associatedProgram))
                {
                    programPath = associatedProgram;
                    _logger.LogDebug($"Found associated program for {extension}: {programPath}");
                }
                
                // Launch the file
                if (!string.IsNullOrEmpty(programPath))
                {
                    // Launch with specific program from config
                    var processStartInfo = new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = programPath,
                        Arguments = $"\"{filePath}\"",
                        UseShellExecute = true,
                        CreateNoWindow = false
                    };
                    
                    System.Diagnostics.Process.Start(processStartInfo);
                    _logger.LogInformation($"Opened {filePath} with {programPath}");
                }
                else
                {
                    // Use default system association
                    var processStartInfo = new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = filePath,
                        UseShellExecute = true,
                        CreateNoWindow = false
                    };
                    
                    System.Diagnostics.Process.Start(processStartInfo);
                    _logger.LogInformation($"Opened {filePath} with default system association");
                }
            }
            catch (System.ComponentModel.Win32Exception ex)
            {
                // Program not found or no association
                _logger.LogWarning(ex, $"Could not open file with associated program: {filePath}");
                SetStatus($"Warning: Could not open file - {ex.Message}");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error opening file: {filePath}");
                SetStatus($"Error opening file: {ex.Message}");
            }
        }
        

        /// <summary>
        /// Handles S key press - cycles through sort modes
        /// Updates pane display with new sort order
        /// Shows current sort mode in status area
        /// </summary>
        public void CycleSortMode()
        {
            var activePane = GetActivePane();
            
            try
            {
                // Define the sort mode cycle order
                var sortModes = new[]
                {
                    SortMode.NameAscending,
                    SortMode.NameDescending,
                    SortMode.ExtensionAscending,
                    SortMode.ExtensionDescending,
                    SortMode.SizeAscending,
                    SortMode.SizeDescending,
                    SortMode.DateAscending,
                    SortMode.DateDescending,
                    SortMode.Unsorted
                };
                
                // Find current mode index
                int currentIndex = Array.IndexOf(sortModes, activePane.SortMode);
                
                // Move to next mode (wrap around to beginning if at end)
                int nextIndex = (currentIndex + 1) % sortModes.Length;
                SortMode newSortMode = sortModes[nextIndex];
                
                // Update the pane's sort mode
                activePane.SortMode = newSortMode;
                
                // Re-sort the entries
                activePane.Entries = SortEngine.Sort(activePane.Entries, newSortMode);
                
                // Refresh the display
                RefreshPanes();
                
                // Get friendly name for status display
                string sortModeName = GetSortModeName(newSortMode);
                string paneTitle = _leftPaneActive ? "[Left Pane]" : "[Right Pane]";
                _taskStatusView?.AddLog($"{paneTitle} Sort order set to: {sortModeName}");
                
                _logger.LogDebug($"Sort mode: {sortModeName}");
                _logger.LogDebug($"Sort mode changed to: {newSortMode}");
            }            catch (Exception ex)
            {
                _logger.LogError(ex, "Error cycling sort mode");
                SetStatus($"Error changing sort mode: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows a dialog to select sort order explicitly
        /// </summary>
        private void ShowSortDialog()
        {
            var activePane = GetActivePane();
            string paneTitle = _leftPaneActive ? "Left Pane" : "Right Pane";
            
            var dialog = new SortDialog(activePane.SortMode, paneTitle);
            Application.Run(dialog);
            
            if (dialog.IsOk)
            {
                activePane.SortMode = dialog.SelectedMode;
                
                // Re-sort
                activePane.Entries = SortEngine.Sort(activePane.Entries, activePane.SortMode);
                RefreshPanes();
                
                string sortModeName = GetSortModeName(activePane.SortMode);
                _taskStatusView?.AddLog($"[{paneTitle}] Sort order set to: {sortModeName}");
            }
        }

        /// <summary>
        /// Gets a friendly display name for a sort mode
        /// </summary>
        private string GetSortModeName(SortMode mode)
        {
            return mode switch
            {
                SortMode.Unsorted => "Unsorted",
                SortMode.NameAscending => "Name (A-Z)",
                SortMode.NameDescending => "Name (Z-A)",
                SortMode.ExtensionAscending => "Extension (A-Z)",
                SortMode.ExtensionDescending => "Extension (Z-A)",
                SortMode.SizeAscending => "Size (Small-Large)",
                SortMode.SizeDescending => "Size (Large-Small)",
                SortMode.DateAscending => "Date (Old-New)",
                SortMode.DateDescending => "Date (New-Old)",
                _ => "Unknown"
            };
        }
        
        /// <summary>
        /// Handles colon key press - shows file mask input dialog
        /// Parses multiple masks with spaces
        /// Supports exclusion patterns with colon prefix
        /// Applies mask to filter file list
        /// </summary>
        public void ShowFileMaskDialog()
        {
            var activePane = GetActivePane();
            
            try
            {
                var dialog = new FileMaskDialog(activePane.FileMask, _config);
                
                // Show dialog
                Application.Run(dialog);
                
                // Process the mask if OK was pressed
                if (dialog.IsOk)
                {
                    string mask = dialog.Mask;
                    
                    if (string.IsNullOrWhiteSpace(mask))
                    {
                        mask = "*";
                    }
                    
                    if (activePane.FileMask != mask)
                    {
                        activePane.FileMask = mask;
                        LoadPaneDirectory(activePane);
                        RefreshPanes();
                        _logger.LogInformation($"File mask changed to: {mask}");
                    }
                }
                else
                {
                    _logger.LogTrace("File mask dialog cancelled");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing file mask dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Applies a file mask filter to the active pane
        /// Supports multiple patterns separated by spaces
        /// Patterns starting with colon (:) are exclusion patterns
        /// </summary>
        private void ApplyFileMask(string mask)
        {
            var activePane = GetActivePane();
            
            try
            {
                // Update the pane's file mask
                activePane.FileMask = mask;
                
                // Reload the directory with the new mask applied
                LoadPaneDirectory(activePane);
                
                // Refresh display
                RefreshPanes();
                
                // Update status
                int fileCount = activePane.Entries.Count(e => !e.IsDirectory);
                int dirCount = activePane.Entries.Count(e => e.IsDirectory);
                
                SetStatus($"File mask '{mask}' applied: {fileCount} file(s), {dirCount} dir(s)");
                _logger.LogDebug($"Applied file mask '{mask}': {fileCount} files, {dirCount} directories");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error applying file mask: {mask}");

                SetStatus($"Error applying file mask: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Jumps to the specified path
        /// </summary>
        public void JumpToPath(string path)
        {
            try
            {
                 // Basic validation
                 if (string.IsNullOrWhiteSpace(path)) return;
                 
                 // Expand environment variables just in case
                 path = Environment.ExpandEnvironmentVariables(path);
                 
                 if (Directory.Exists(path))
                 {
                     var pane = GetActivePane();
                     pane.CurrentPath = path;
                     LoadPaneDirectory(pane);
                     
                     // Set focus to the active pane view
                     var view = _leftPaneActive ? _leftPane : _rightPane;
                     view?.SetFocus();
                 }
                 else if (File.Exists(path))
                 {
                      string? dir = Path.GetDirectoryName(path);
                     if (Directory.Exists(dir))
                     {
                        var pane = GetActivePane();
                        pane.CurrentPath = dir;
                        LoadPaneDirectory(pane);
                        
                        // Find the file in the entries and set cursor position
                        string fileName = Path.GetFileName(path);
                        int fileIndex = pane.Entries.FindIndex(e => e.Name.Equals(fileName, StringComparison.OrdinalIgnoreCase));
                        if (fileIndex != -1)
                        {
                            pane.CursorPosition = fileIndex;
                        }

                        // Set focus to the active pane view
                        var view = _leftPaneActive ? _leftPane : _rightPane;
                        view?.SetFocus();
                        RefreshPanes();
                     }
                 }
                 else
                 {
                     SetStatus($"Path not found: {path}");
                 }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error jumping to path: {Path}", path);
                SetStatus($"Error jumping to path: {ex.Message}");
            }
        }

        /// <summary>
        /// Shows path input dialog to jump to a specific path (J key)
        /// </summary>
        public void JumpToPath()
        {
            try
            {
                var activePane = GetActivePane();
                var dialog = new JumpToPathDialog(activePane.CurrentPath);
                
                Application.Run(dialog);
                
                if (dialog.IsOk && !string.IsNullOrWhiteSpace(dialog.Path))
                {
                    string newPath = dialog.Path;
                    
                    // Run verification in background to prevent freezing UI
                    Task.Run(async () =>
                    {
                        Application.MainLoop.Invoke(() => SetStatus($"Verifying path: {newPath}..."));
                        
                        try { newPath = Environment.ExpandEnvironmentVariables(newPath); } catch {}

                        if (await _pathValidator.IsPathAccessibleAsync(newPath))
                        {
                            Application.MainLoop.Invoke(() => 
                            {
                                NavigateToDirectory(newPath);
                            });
                        }
                        else
                        {
                            Application.MainLoop.Invoke(() => SetStatus($"Path not found or unreachable: {newPath}"));
                        }
                    });
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error in jump to path");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows drive change dialog (L key)
        /// </summary>
        public void ShowDriveChangeDialog()
        {
            try
            {
                var drives = _listProvider.GetDriveList();
                string paneTitle = _leftPaneActive ? "Left Pane" : "Right Pane";

                // Create drive selection dialog
                var dialog = new DriveDialog(
                    drives,
                    _historyManager,
                    _searchEngine,
                    _config,
                    (path) => 
                    {
                        NavigateToDirectory(path);
                        _logger.LogDebug($"Changed drive to {path}");
                    },
                    _logger,
                    paneTitle);
                
                Application.Run(dialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing drive change dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows file size summary (H key)
        /// </summary>
        /// <summary>
        /// Shows version and environment information in the task log
        /// </summary>
        public void ShowVersionInfo()
        {
            if (_taskStatusView == null) return;

            // Application name, version and build
            _taskStatusView.AddLog(VersionHelper.GetVersionInfo());
            
            // Runtime and OS info
            _taskStatusView.AddLog(VersionHelper.GetRuntimeInfo());
            
            // Configuration file path
            _taskStatusView.AddLog($"Config: {_configProvider.GetConfigFilePath()}");
            
            // Status of key features
            string migemoStatus = _searchEngine.IsMigemoAvailable ? "OK" : "Unavailable";
            _taskStatusView.AddLog($"LogLevel: {_config.LogLevel} | Migemo: {migemoStatus}");
            
            // Supported archive formats
            var archiveExts = _archiveManager.GetSupportedArchiveExtensions()
                .Select(e => e.TrimStart('.').ToUpper())
                .Distinct();
            var archives = string.Join(", ", archiveExts);
            _taskStatusView.AddLog($"Archive Support: {archives}");
        }

        /// <summary>
        /// Shows help dialog with dynamic key bindings and search (F1 key)
        /// </summary>
        public void ShowHelp()
        {
            try
            {
                _logger.LogDebug("ShowHelp called");
                
                if (_helpManager == null)
                {
                    _helpManager = new HelpManager(_keyBindings, _configProvider.ConfigDirectory, _config.Display.HelpLanguage, LoggingConfiguration.GetLogger<HelpManager>());
                }

                var helpView = new HelpView(_helpManager, _searchEngine, _currentMode);
                Application.Run(helpView);
                
                // Refresh display after dialog closes
                RefreshPanes();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing help");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        public void ShowFileInfoForCursor()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null)
                {
                    SetStatus("No file selected");
                    return;
                }
                
                if (currentEntry.IsDirectory)
                {
                    SetStatus($"[DIR] {currentEntry.Name} - Modified: {currentEntry.LastModified:yyyy-MM-dd HH:mm:ss}");
                    _taskStatusView?.AddLog($"Calculating size of '{currentEntry.Name}'...");

                    int tabId = _activeTabIndex;
                    string tabName = Path.GetFileName(activePane.CurrentPath);
                    string targetPath = currentEntry.FullPath;
                    string targetName = currentEntry.Name;
                    int interval = _config.Display.FileListRefreshIntervalMs > 0 ? _config.Display.FileListRefreshIntervalMs : 500;

                    _jobManager.StartJob(
                        $"Size: {targetName}",
                        "Calculating...",
                        tabId,
                        tabName,
                        async (job, token, progress) => 
                        {
                            var adapter = new Progress<ProgressEventArgs>(e => 
                            {
                                string msg = $"{e.FilesProcessed:N0} files ({FormatFileSize(e.BytesProcessed)})";
                                progress.Report(new JobProgress { Message = msg, Percent = -1 });
                            });

                            var (size, files, dirs) = await _fileOps.CalculateDirectorySizeAsync(targetPath, token, adapter, interval);
                            
                            if (!token.IsCancellationRequested)
                            {
                                string sizeStr = FormatFileSize(size);
                                Application.MainLoop.Invoke(() => 
                                {
                                    _taskStatusView?.AddLog($"[Size] {targetName}: {sizeStr} ({files:N0} files, {dirs:N0} folders)");
                                });
                            }
                        });
                }
                else
                {
                    SetStatus($"{currentEntry.Name} - Size: {FormatFileSize(currentEntry.Size)} - Modified: {currentEntry.LastModified:yyyy-MM-dd HH:mm:ss}");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing file info");
                SetStatus($"Error: {ex.Message}");
            }
        }

        private void HandleArchiveCopyOut(PaneState activePane, PaneState inactivePane, List<FileEntry> filesToCopy)
        {
            int tabId = _activeTabIndex;
            string tabName = Path.GetFileName(activePane.CurrentPath);
            string archivePath = activePane.VirtualFolderArchivePath!;
            string destination = inactivePane.CurrentPath;

            _logger.LogDebug("HandleArchiveCopyOut: archive={ArchivePath}, dest={Destination}, count={Count}", archivePath, destination, filesToCopy.Count);

            var entryNames = filesToCopy.Select(f => Path.GetRelativePath(archivePath, f.FullPath)).ToList();
            _logger.LogDebug("Relative entry names identified: {EntryNames}", string.Join(", ", entryNames));

            _jobManager.StartJob(
                $"Extract from {Path.GetFileName(archivePath)}",
                $"Extracting {filesToCopy.Count} items",
                tabId,
                tabName,
                async (job, token, progress) => 
                {
                    var result = await _archiveManager.ExtractEntriesAsync(archivePath, entryNames, destination, token);
                    
                    if (!result.Success && result.Message != "Operation cancelled by user")
                    {
                         _logger.LogError("Archive extraction failed: {Message}", result.Message);
                         throw new Exception(result.Message);
                    }

                    _logger.LogInformation("Archive extraction successful: {Processed} items", result.FilesProcessed);
                    Application.MainLoop.Invoke(() => LoadPaneDirectory(inactivePane));
                });
                
            SetStatus("Extraction started in background");
        }

        private void HandleArchiveDelete(PaneState activePane, List<FileEntry> filesToDelete)
        {
            int tabId = _activeTabIndex;
            string tabName = Path.GetFileName(activePane.CurrentPath);
            string archivePath = activePane.VirtualFolderArchivePath!;

            _logger.LogDebug("HandleArchiveDelete: archive={ArchivePath}, count={Count}", archivePath, filesToDelete.Count);

            var entryNames = filesToDelete.Select(f => Path.GetRelativePath(archivePath, f.FullPath)).ToList();
            _logger.LogDebug("Relative entry names identified for deletion: {EntryNames}", string.Join(", ", entryNames));

            _jobManager.StartJob(
                $"Delete from {Path.GetFileName(archivePath)}",
                $"Deleting {filesToDelete.Count} items",
                tabId,
                tabName,
                async (job, token, progress) => 
                {
                    var result = await _archiveManager.DeleteEntriesAsync(archivePath, entryNames, token);
                    
                    if (!result.Success && result.Message != "Operation cancelled by user")
                    {
                         _logger.LogError("Archive deletion failed: {Message}", result.Message);
                         throw new Exception(result.Message);
                    }

                    _logger.LogInformation("Archive deletion successful: {Processed} items", result.FilesProcessed);
                    Application.MainLoop.Invoke(() => LoadPaneDirectory(activePane));
                });
                
            SetStatus("Archive deletion started in background");
        }
        
        /// <summary>
        /// Views file under cursor, automatically selecting appropriate viewer
        /// </summary>
        public void ViewFile()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null || currentEntry.IsDirectory)
                {
                    SetStatus("No file selected");
                    return;
                }
                
                string extension = Path.GetExtension(currentEntry.FullPath).ToLower();
                
                // Check if it's an image
                if (_config.Viewer.SupportedImageExtensions.Contains(extension))
                {
                    _viewerManager.OpenImageViewer(currentEntry.FullPath);
                    _currentMode = UiMode.ImageViewer;
                    
                    var imageViewer = _viewerManager.CurrentImageViewer;
                    if (imageViewer != null)
                    {
                        var viewerWindow = new TWF.UI.ImageViewerWindow(imageViewer, _keyBindings, _config);
                        Application.Run(viewerWindow);
                        
                        _currentMode = UiMode.Normal;
                        _viewerManager.CloseCurrentViewer();
                        
                        try
                        {
                            Console.CursorVisible = false;
                        }
                        catch {}
                        
                        RefreshPanes();
                        _logger.LogDebug("Image viewer closed");
                    }
                    return;
                }
                
                // Check if it's a text file
                if (_config.Viewer.SupportedTextExtensions.Contains(extension))
                {
                    ViewFileAsText();
                    return;
                }
                
                // Default to hex viewer for unknown types
                ViewFileAsHex();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error viewing file");
                SetStatus($"Error: {ex.Message}");
                _currentMode = UiMode.Normal;
            }
        }

        /// <summary>
        /// Views file under cursor as text (V key)
        /// </summary>
        public void ViewFileAsText()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null || currentEntry.IsDirectory)
                {
                    SetStatus("No file selected");
                    return;
                }
                
                // Open text viewer
                _viewerManager.OpenTextViewer(currentEntry.FullPath, Encoding.UTF8);
                _currentMode = UiMode.TextViewer;
                
                // Create and show the text viewer window
                var textViewer = _viewerManager.CurrentTextViewer;
                if (textViewer != null)
                {
                    var viewerWindow = new TWF.UI.TextViewerWindow(textViewer, _keyBindings, _searchEngine, _config);
                    Application.Run(viewerWindow);
                    
                    // After viewer closes, return to normal mode
                    _currentMode = UiMode.Normal;
                    _viewerManager.CloseCurrentViewer();
                    
                    // Hide cursor after closing viewer
                    try
                    {
                        Console.CursorVisible = false;
                    }
                    catch
                    {
                        // Ignore errors hiding cursor
                    }
                    
                    // Refresh panes
                    RefreshPanes();
                    
                    _logger.LogDebug("Text viewer closed");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error viewing file as text");
                SetStatus($"Error: {ex.Message}");
                _currentMode = UiMode.Normal;
            }
        }

        /// <summary>
        /// Views file under cursor as hex (binary view)
        /// </summary>
        public void ViewFileAsHex()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null || currentEntry.IsDirectory)
                {
                    SetStatus("No file selected");
                    return;
                }
                
                // Open text viewer
                _viewerManager.OpenTextViewer(currentEntry.FullPath, Encoding.UTF8);
                _currentMode = UiMode.TextViewer;
                
                // Create and show the text viewer window in hex mode
                var textViewer = _viewerManager.CurrentTextViewer;
                if (textViewer != null)
                {
                    var viewerWindow = new TWF.UI.TextViewerWindow(textViewer, _keyBindings, _searchEngine, _config, startInHexMode: true);
                    Application.Run(viewerWindow);
                    
                    // After viewer closes, return to normal mode
                    _currentMode = UiMode.Normal;
                    _viewerManager.CloseCurrentViewer();
                    
                    // Hide cursor after closing viewer
                    try
                    {
                        Console.CursorVisible = false;
                    }
                    catch
                    {
                        // Ignore errors hiding cursor
                    }
                    
                    // Refresh panes
                    RefreshPanes();
                    
                    _logger.LogDebug("Hex viewer closed");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error viewing file as hex");
                SetStatus($"Error: {ex.Message}");
                _currentMode = UiMode.Normal;
            }
        }
        

        /// <summary>
        /// Handles P key press - shows compression dialog and compresses files
        /// Compresses marked files or cursor file to opposite pane
        /// Shows compression ratio after completion
        /// </summary>
        public void HandleCompressionOperation()
        {
            var activePane = GetActivePane();
            var inactivePane = GetInactivePane();
            
            // Get files to compress (marked files or current file)
            var filesToCompress = activePane.GetMarkedEntries();
            if (filesToCompress.Count == 0)
            {
                var currentEntry = activePane.GetCurrentEntry();
                if (currentEntry != null)
                {
                    filesToCompress = new List<FileEntry> { currentEntry };
                }
            }
            
            if (filesToCompress.Count == 0)
            {
                SetStatus("No files to compress");
                return;
            }
            
            try
            {
                // Show compression dialog to select format and archive name
                var (archiveFormat, archiveName, confirmed) = ShowCompressionDialog(filesToCompress);
                
                if (!confirmed)
                {
                    SetStatus("Compression cancelled");
                    return;
                }
                
                // Determine the full archive path in the opposite pane
                var archivePath = Path.Combine(inactivePane.CurrentPath, archiveName);
                
                // Calculate original size for compression ratio
                long originalSize = filesToCompress.Sum(f => f.IsDirectory ? 0 : f.Size);
                
                // Execute compression with progress dialog
                ExecuteCompressionWithProgress(filesToCompress, archivePath, archiveFormat, originalSize);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error in compression operation");
                SetStatus($"Compression error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows a dialog to select archive format and enter archive name
        /// Returns the selected format, archive name, and whether the user confirmed
        /// </summary>
        private (ArchiveFormat format, string archiveName, bool confirmed) ShowCompressionDialog(List<FileEntry> filesToCompress)
        {
            var defaultName = filesToCompress.Count == 1 
                ? Path.GetFileNameWithoutExtension(filesToCompress[0].Name) 
                : "archive";

            var supportedFormats = _archiveManager.GetSupportedFormats();
            var dialog = new CompressionOptionsDialog(filesToCompress.Count, defaultName, supportedFormats);
            Application.Run(dialog);

            if (dialog.IsOk)
            {
                string archiveName = dialog.ArchiveName;
                var extension = GetArchiveExtension(dialog.SelectedFormat);
                if (!archiveName.EndsWith(extension, StringComparison.OrdinalIgnoreCase))
                {
                    archiveName += extension;
                }
                return (dialog.SelectedFormat, archiveName, true);
            }

            return (ArchiveFormat.ZIP, string.Empty, false);
        }
        
        /// <summary>
        /// Gets the file extension for an archive format
        /// </summary>
        private string GetArchiveExtension(ArchiveFormat format)
        {
            return format switch
            {
                ArchiveFormat.ZIP => ".zip",
                ArchiveFormat.TAR => ".tar",
                ArchiveFormat.TGZ => ".tar.gz",
                ArchiveFormat.SevenZip => ".7z",
                ArchiveFormat.RAR => ".rar",
                ArchiveFormat.LZH => ".lzh",
                ArchiveFormat.CAB => ".cab",
                ArchiveFormat.BZ2 => ".bz2",
                ArchiveFormat.XZ => ".xz",
                ArchiveFormat.LZMA => ".lzma",
                _ => ".zip"
            };
        }
        
        /// <summary>
        /// Executes compression operation with progress dialog
        /// </summary>
        private void ExecuteCompressionWithProgress(
            List<FileEntry> files,
            string archivePath,
            ArchiveFormat format,
            long originalSize)
        {
            var cancellationTokenSource = new CancellationTokenSource();
            var progressDialog = new OperationProgressDialog("Compression Progress", cancellationTokenSource);
            progressDialog.Status = "Compressing...";
            
            // Execute compression asynchronously
            Task.Run(async () =>
            {
                try
                {
                    var result = await _archiveManager.CompressAsync(
                        files,
                        archivePath,
                        format,
                        cancellationTokenSource.Token);
                    
                    Application.MainLoop.Invoke(() =>
                    {
                        Application.RequestStop();
                        
                        if (result.Success)
                        {
                            // Calculate compression ratio
                            long compressedSize = 0;
                            if (File.Exists(archivePath))
                            {
                                compressedSize = new FileInfo(archivePath).Length;
                            }
                            
                            double compressionRatio = originalSize > 0 
                                ? (1.0 - (double)compressedSize / originalSize) * 100 
                                : 0;
                            
                            SetStatus($"Compressed {result.FilesProcessed} file(s) - Ratio: {compressionRatio:F1}%");
                            
                            // Invalidate cache for destination folder to ensure fresh metadata (fixes 0-byte bug)
                            string? destDir = Path.GetDirectoryName(archivePath);
                            if (!string.IsNullOrEmpty(destDir))
                            {
                                _directoryCache.Invalidate(destDir);
                            }

                            // Refresh both panes to show the new archive
                            LoadPaneDirectory(_leftState);
                            LoadPaneDirectory(_rightState);
                            RefreshPanes();
                        }
                        else
                        {
                            SetStatus($"Compression failed: {result.Message}");
                            
                            if (result.Errors.Count > 0)
                            {
                                var errorMsg = string.Join("\n", result.Errors.Take(5));
                                if (result.Errors.Count > 5)
                                {
                                    errorMsg += $"\n... and {result.Errors.Count - 5} more errors";
                                }
                                ShowMessageDialog("Compression Errors", errorMsg);
                            }
                        }
                    });
                }
                catch (Exception ex)
                {
                    Application.MainLoop.Invoke(() =>
                    {
                        Application.RequestStop();
                        SetStatus($"Compression failed: {ex.Message}");
                        _logger.LogError(ex, "Compression operation failed");
                    });
                }
            });
            
            // Show the progress dialog
            Application.Run(progressDialog);
        }
        
        /// <summary>
        /// Handles F key press - enters incremental search mode
        /// Updates cursor position as characters are typed
        /// Supports Migemo if available
        /// </summary>
        public void EnterSearchMode()
        {
            try
            {
                var activePane = GetActivePane();
                
                if (activePane.Entries.Count == 0)
                {
                    SetStatus("No files to search");
                    return;
                }
                
                                // Enter search mode
                                _currentMode = UiMode.Search;
                                _searchPattern = string.Empty;
                                _searchStatus = string.Empty;
                                _searchStartIndex = activePane.CursorPosition;
                
                                // Update status to show search mode
                                _logger.LogDebug($"Enter Search: StartIndex={_searchStartIndex}");
                                RefreshPanes();
                            }            catch (Exception ex)
            {
                _logger.LogError(ex, "Error entering search mode");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles character input during search mode
        /// Updates cursor position to first matching file
        /// </summary>
        public void HandleSearchInput(char character)
        {
            try
            {
                var activePane = GetActivePane();
                
                // Add character to search pattern
                _searchPattern += character;
                
                // Find first match from current position
                bool useMigemo = _searchEngine != null && _searchPattern.Length > 0;
                int matchIndex = _searchEngine?.FindNext(
                    activePane.Entries,
                    _searchPattern,
                    _searchStartIndex - 1, // Start from before current position to include it
                    useMigemo: useMigemo) ?? -1;
                
                                if (matchIndex >= 0)
                                {
                                    // Move cursor to match
                                    activePane.CursorPosition = matchIndex;
                                    _searchStatus = "(found)";
                                    RefreshPanes();
                                }
                                else
                                {
                                    // No match found
                                    _searchStatus = "(not found)";
                                    RefreshPanes();
                                }
                
                                _logger.LogDebug($"Search pattern: '{_searchPattern}', match: {matchIndex}"); 
                            }            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling search input");
                SetStatus($"Search error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles backspace during search mode
        /// Removes last character from search pattern
        /// </summary>
        public void HandleSearchBackspace()
        {
            try
            {
                if (_searchPattern.Length > 0)
                {
                    // Remove last character
                    _searchPattern = _searchPattern.Substring(0, _searchPattern.Length - 1);
                    
                    if (_searchPattern.Length > 0)
                    {
                        // Find match with updated pattern
                        var activePane = GetActivePane();
                        bool useMigemo = _searchEngine != null && _searchPattern.Length > 0;
                        int matchIndex = _searchEngine?.FindNext(
                            activePane.Entries,
                            _searchPattern,
                            _searchStartIndex - 1,
                            useMigemo: useMigemo) ?? -1;
                        
                        if (matchIndex >= 0)
                        {
                            activePane.CursorPosition = matchIndex;
                            _searchStatus = "(found)";
                            RefreshPanes();
                        }
                        else
                        {
                            _searchStatus = "(not found)";
                            RefreshPanes();
                        }
                    }
                    else
                    {
                        // Pattern is now empty, return to start position
                        var activePane = GetActivePane();
                        activePane.CursorPosition = _searchStartIndex;
                        _searchStatus = string.Empty;
                        RefreshPanes();
                    }
                }
                
                _logger.LogDebug($"Search pattern after backspace: '{_searchPattern}'");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling search backspace");
                SetStatus($"Search error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles Space key during search mode
        /// Toggles mark on current file and finds next match
        /// </summary>
        public void HandleSearchMarkAndNext()
        {
            try
            {
                var activePane = GetActivePane();
                
                if (activePane.Entries.Count == 0 || string.IsNullOrEmpty(_searchPattern))
                {
                    return;
                }
                
                // Toggle mark on current entry
                _markingEngine.ToggleMark(activePane, activePane.CursorPosition);
                
                // Find next match
                bool useMigemo = _searchEngine != null && _searchPattern.Length > 0;
                int nextMatch = _searchEngine?.FindNext(
                    activePane.Entries,
                    _searchPattern,
                    activePane.CursorPosition,
                    useMigemo: useMigemo) ?? -1;
                
                                if (nextMatch >= 0)
                                {
                                    activePane.CursorPosition = nextMatch;
                                    _searchStatus = "(marked, next found)";
                                }
                                else
                                {
                                    _searchStatus = "(marked, no more matches)";
                                }
                                RefreshPanes();
                            }            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling search mark and next");
                SetStatus($"Search error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles arrow down key during search mode
        /// Finds next match
        /// </summary>
        public void HandleSearchNext()
        {
            try
            {
                var activePane = GetActivePane();
                
                if (activePane.Entries.Count == 0 || string.IsNullOrEmpty(_searchPattern))
                {
                    return;
                }
                
                // Find next match
                bool useMigemo = _searchEngine != null && _searchPattern.Length > 0;
                int nextMatch = _searchEngine?.FindNext(
                    activePane.Entries,
                    _searchPattern,
                    activePane.CursorPosition,
                    useMigemo: useMigemo) ?? -1;
                
                                if (nextMatch >= 0)
                                {
                                    activePane.CursorPosition = nextMatch;
                                    _searchStatus = "(next found)";
                                }
                                else
                                {
                                    _searchStatus = "(no more matches)";
                                }
                                RefreshPanes();
                            }            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling search next");
                SetStatus($"Search error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Handles arrow up key during search mode
        /// Finds previous match
        /// </summary>
        public void HandleSearchPrevious()
        {
            try
            {
                var activePane = GetActivePane();
                
                if (activePane.Entries.Count == 0 || string.IsNullOrEmpty(_searchPattern))
                {
                    return;
                }
                
                // Find previous match
                bool useMigemo = _searchEngine != null && _searchPattern.Length > 0;
                int prevMatch = _searchEngine?.FindPrevious(
                    activePane.Entries,
                    _searchPattern,
                    activePane.CursorPosition,
                    useMigemo: useMigemo) ?? -1;
                
                                if (prevMatch >= 0)
                                {
                                    activePane.CursorPosition = prevMatch;
                                    _searchStatus = "(previous found)";
                                }
                                else
                                {
                                    _searchStatus = "(no previous matches)";
                                }
                                RefreshPanes();
                
                                _logger.LogDebug($"Search previous: {prevMatch}");
                            }            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling search previous");
                SetStatus($"Search error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Exits search mode
        /// Handles Enter or Escape key press during search
        /// </summary>
        public void ExitSearchMode()
        {
            try
            {
                // Exit search mode
                _currentMode = UiMode.Normal;
                _searchPattern = string.Empty;
                _searchStartIndex = 0;
                
                // Update status
                var activePane = GetActivePane();
                int markedCount = activePane.Entries.Count(e => e.IsMarked);

                _logger.LogDebug($"Search exited - {markedCount} file(s) marked");
                
                RefreshPanes();
                _logger.LogDebug("Exited search mode");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error exiting search mode");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Gets whether the application is currently in search mode
        /// </summary>
        public bool IsInSearchMode()
        {
            return _currentMode == UiMode.Search;
        }
        
        /// <summary>
        /// Handles Shift+W key press - determines whether to split or join files
        /// </summary>
        public void HandleFileSplitOrJoin()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null)
                {
                    SetStatus("No file selected");
                    return;
                }
                
                // Check if this is a split file part (e.g., filename.001, filename.002)
                if (IsSplitFilePart(currentEntry.FullPath))
                {
                    // This is a split file part - offer to join
                    HandleFileJoin();
                }
                else if (!currentEntry.IsDirectory)
                {
                    // This is a regular file - offer to split
                    HandleFileSplit();
                }
                else
                {
                    SetStatus("Cannot split or join directories");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error handling file split/join");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Checks if a file is a split file part (e.g., filename.001, filename.002)
        /// </summary>
        private bool IsSplitFilePart(string filePath)
        {
            var fileName = Path.GetFileName(filePath);
            var extension = Path.GetExtension(fileName);
            
            // Check if extension is .001, .002, etc. (3 digits)
            if (extension.Length == 4 && extension[0] == '.')
            {
                return extension.Substring(1).All(char.IsDigit);
            }
            
            return false;
        }
        
        /// <summary>
        /// Handles file split operation
        /// Shows dialog to get split size and calls FileOperations.SplitAsync
        /// </summary>
        public void HandleFileSplit()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null || currentEntry.IsDirectory)
                {
                    SetStatus("No file selected for splitting");
                    return;
                }
                
                _logger.LogDebug($"Splitting file: {currentEntry.FullPath}");
                
                // Show split dialog to get part size
                var (partSize, outputDirectory, confirmed) = ShowFileSplitDialog(currentEntry);
                
                if (!confirmed)
                {
                    SetStatus("Split cancelled");
                    return;
                }
                
                // Execute split operation with progress dialog
                var cancellationTokenSource = new CancellationTokenSource();
                var progressDialog = new OperationProgressDialog("Splitting File", cancellationTokenSource);
                progressDialog.Status = "Splitting file...";
                
                // Subscribe to progress events
                EventHandler<ProgressEventArgs>? progressHandler = (sender, args) =>
                {
                    Application.MainLoop.Invoke(() =>
                    {
                        progressDialog.UpdateProgress(args.CurrentFile, args.CurrentFileIndex, args.TotalFiles, args.PercentComplete);
                    });
                };
                
                _fileOps.ProgressChanged += progressHandler;
                
                // Execute split asynchronously
                Task.Run(async () =>
                {
                    try
                    {
                        var result = await _fileOps.SplitAsync(
                            currentEntry.FullPath,
                            partSize,
                            outputDirectory,
                            cancellationTokenSource.Token);
                        
                        Application.MainLoop.Invoke(() =>
                        {
                            _fileOps.ProgressChanged -= progressHandler;
                            Application.RequestStop();
                            
                            if (result.Success)
                            {
                                SetStatus($"Split into {result.FilesProcessed} part(s) successfully");
                                
                                // Refresh the pane to show split files
                                LoadPaneDirectory(activePane);
                                RefreshPanes();
                            }
                            else
                            {
                                SetStatus($"Split failed: {result.Message}");
                                
                                if (result.Errors.Count > 0)
                                {
                                    var errorMsg = string.Join("\n", result.Errors.Take(5));
                                    if (result.Errors.Count > 5)
                                    {
                                        errorMsg += $"\n... and {result.Errors.Count - 5} more errors";
                                    }
                                    ShowMessageDialog("Split Errors", errorMsg);
                                }
                            }
                        });
                    }
                    catch (Exception ex)
                    {
                        Application.MainLoop.Invoke(() =>
                        {
                            _fileOps.ProgressChanged -= progressHandler;
                            Application.RequestStop();
                            SetStatus($"Split failed: {ex.Message}");
                            _logger.LogError(ex, "File split failed");
                        });
                    }
                });
                
                // Show the progress dialog
                Application.Run(progressDialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error splitting file");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows a dialog for file split configuration
        /// Returns (partSize, outputDirectory, confirmed)
        /// </summary>
        private (long partSize, string outputDirectory, bool confirmed) ShowFileSplitDialog(FileEntry file)
        {
            var dialog = new FileSplitOptionsDialog(file.Name, file.Size, GetInactivePane().CurrentPath);
            Application.Run(dialog);

            if (dialog.IsOk)
            {
                return (dialog.PartSize, dialog.OutputDirectory, true);
            }

            return (0, string.Empty, false);
        }
        
        /// <summary>
        /// Handles file join operation
        /// Shows dialog to select part files and calls FileOperations.JoinAsync
        /// </summary>
        public void HandleFileJoin()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                
                if (currentEntry == null)
                {
                    SetStatus("No file selected for joining");
                    return;
                }
                
                _logger.LogDebug($"Joining split files starting with: {currentEntry.FullPath}");
                
                // Find all related split file parts
                var partFiles = FindSplitFileParts(currentEntry.FullPath);
                
                if (partFiles.Count == 0)
                {
                    SetStatus("No split file parts found");
                    return;
                }
                
                // Show join dialog to get output file name
                var (outputFile, confirmed) = ShowFileJoinDialog(currentEntry, partFiles);
                
                if (!confirmed)
                {
                    SetStatus("Join cancelled");
                    return;
                }
                
                // Execute join operation with progress dialog
                var cancellationTokenSource = new CancellationTokenSource();
                var progressDialog = new OperationProgressDialog("Joining Files", cancellationTokenSource);
                progressDialog.Status = "Joining file parts...";
                
                // Subscribe to progress events
                EventHandler<ProgressEventArgs>? progressHandler = (sender, args) =>
                {
                    Application.MainLoop.Invoke(() =>
                    {
                        progressDialog.UpdateProgress(args.CurrentFile, args.CurrentFileIndex, args.TotalFiles, args.PercentComplete);
                    });
                };
                
                _fileOps.ProgressChanged += progressHandler;
                
                // Execute join asynchronously
                Task.Run(async () =>
                {
                    try
                    {
                        var result = await _fileOps.JoinAsync(
                            partFiles,
                            outputFile,
                            cancellationTokenSource.Token);
                        
                        Application.MainLoop.Invoke(() =>
                        {
                            _fileOps.ProgressChanged -= progressHandler;
                            Application.RequestStop();
                            
                            if (result.Success)
                            {
                                SetStatus($"Joined {result.FilesProcessed} part(s) successfully");
                                
                                // Refresh the pane to show joined file
                                LoadPaneDirectory(activePane);
                                RefreshPanes();
                            }
                            else
                            {
                                SetStatus($"Join failed: {result.Message}");
                                
                                if (result.Errors.Count > 0)
                                {
                                    var errorMsg = string.Join("\n", result.Errors.Take(5));
                                    if (result.Errors.Count > 5)
                                    {
                                        errorMsg += $"\n... and {result.Errors.Count - 5} more errors";
                                    }
                                    ShowMessageDialog("Join Errors", errorMsg);
                                }
                            }
                        });
                    }
                    catch (Exception ex)
                    {
                        Application.MainLoop.Invoke(() =>
                        {
                            _fileOps.ProgressChanged -= progressHandler;
                            Application.RequestStop();
                            SetStatus($"Join failed: {ex.Message}");
                            _logger.LogError(ex, "File join failed");
                        });
                    }
                });
                
                // Show the progress dialog
                Application.Run(progressDialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error joining files");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Finds all split file parts related to a given split file
        /// </summary>
        private List<string> FindSplitFileParts(string splitFilePath)
        {
            var parts = new List<string>();
            
            try
            {
                var directory = Path.GetDirectoryName(splitFilePath);
                if (string.IsNullOrEmpty(directory))
                {
                    return parts;
                }
                
                var fileName = Path.GetFileName(splitFilePath);
                var extension = Path.GetExtension(fileName);
                
                // Get the base name without the .001, .002, etc. extension
                var baseName = fileName.Substring(0, fileName.Length - extension.Length);
                
                // Find all files matching the pattern
                var allFiles = Directory.GetFiles(directory, $"{baseName}.*");
                
                foreach (var file in allFiles)
                {
                    if (IsSplitFilePart(file))
                    {
                        parts.Add(file);
                    }
                }
                
                // Sort by file name to ensure correct order
                parts.Sort();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error finding split file parts for: {splitFilePath}");
            }
            
            return parts;
        }
        
        /// <summary>
        /// Shows a dialog to select join output file
        /// Returns (outputFile, confirmed)
        /// </summary>
        private (string outputFile, bool confirmed) ShowFileJoinDialog(FileEntry file, List<string> partFiles)
        {
            var fileName = Path.GetFileName(file.FullPath);
            var extension = Path.GetExtension(fileName);
            var baseName = fileName.Substring(0, fileName.Length - extension.Length);
            var directory = GetInactivePane().CurrentPath;
            string defaultOutputFile = Path.Combine(directory, baseName);

            var dialog = new FileJoinOptionsDialog(partFiles.Count, partFiles.Select(Path.GetFileName).ToList()!, defaultOutputFile);
            Application.Run(dialog);

            if (dialog.IsOk)
            {
                if (string.IsNullOrWhiteSpace(dialog.OutputFile))
                {
                    SetStatus("Invalid output file name");
                    return (string.Empty, false);
                }
                return (dialog.OutputFile, true);
            }

            return (string.Empty, false);
        }
        
        /// <summary>
        /// Handles backtick (`) key press - shows context menu dialog
        /// Displays menu items based on current selection and executes selected operation
        /// </summary>
        public void HandleContextMenu()
        {
            try
            {
                var activePane = GetActivePane();
                var currentEntry = activePane.GetCurrentEntry();
                bool hasMarkedFiles = activePane.Entries.Any(e => e.IsMarked);
                
                _logger.LogDebug($"Opening context menu for entry: {currentEntry?.Name ?? "(none)"}, marked files: {hasMarkedFiles}");
                
                // Get context menu items from ListProvider
                var menuItems = _listProvider.GetContextMenu(currentEntry, hasMarkedFiles);
                
                if (menuItems.Count == 0)
                {
                    SetStatus("No menu items available");
                    return;
                }
                
                var dialog = new ContextMenuDialog(menuItems);
                Application.Run(dialog);
                
                if (dialog.SelectedItem != null)
                {
                    _logger.LogInformation($"Context menu operation selected: {dialog.SelectedItem.Label}");
                    ExecuteAction(dialog.SelectedItem.Action);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing context menu");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Executes the action associated with a context menu item
        /// Routes to existing handler methods based on action name
        /// </summary>
        private void ExecuteContextMenuAction(string action)
        {
            try
            {
                _logger.LogDebug($"Executing context menu action: {action}");
                
                switch (action)
                {
                    case "Navigate":
                        HandleEnterKey();
                        break;
                        
                    case "Execute":
                        HandleEnterKey();
                        break;
                        
                    case "OpenEditor":
                        HandleShiftEnter();
                        break;
                        
                    case "Copy":
                        HandleCopyOperation();
                        break;
                        
                    case "Move":
                        HandleMoveOperation();
                        break;
                        
                    case "Delete":
                        HandleDeleteOperation();
                        break;
                        
                    case "Rename":
                        HandlePatternRename();
                        break;
                        
                    case "ClearMarks":
                        var activePane = GetActivePane();
                        _markingEngine.ClearMarks(activePane);
                        RefreshPanes();
                        SetStatus("Marks cleared");
                        break;
                        
                    case "BrowseArchive":
                        HandleEnterKey();
                        break;
                        
                    case "ExtractArchive":
                        HandleArchiveExtraction();
                        break;
                        
                    case "Compress":
                        HandleCompressionOperation();
                        break;
                        
                    case "ViewText":
                        HandleEnterKey();
                        break;
                        
                    case "ViewImage":
                        HandleEnterKey();
                        break;
                        
                    case "Compare":
                        HandleFileComparison();
                        break;
                        
                    case "Split":
                        HandleFileSplitOrJoin();
                        break;
                        
                    case "Join":
                        HandleFileSplitOrJoin();
                        break;
                        
                    case "Refresh":
                        LoadPaneDirectory(GetActivePane());
                        RefreshPanes();
                        SetStatus("Refreshed");
                        break;
                        
                    case "CreateDirectory":
                        HandleCreateDirectory();
                        break;
                        
                    case "Properties":
                        ShowFileProperties();
                        break;
                        
                    default:
                        SetStatus($"Unknown action: {action}");
                        _logger.LogWarning($"Unknown context menu action: {action}");
                        break;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error executing context menu action: {action}");
                SetStatus($"Error executing action: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Shows file properties dialog
        /// </summary>
        private void ShowFileProperties()
        {
            var activePane = GetActivePane();
            var currentEntry = activePane.GetCurrentEntry();
            
            if (currentEntry == null)
            {
                SetStatus("No file selected");
                return;
            }
            
            try
            {
                var properties = new StringBuilder();
                properties.AppendLine($"Name: {currentEntry.Name}");
                properties.AppendLine($"Path: {currentEntry.FullPath}");
                properties.AppendLine($"Type: {(currentEntry.IsDirectory ? "Directory" : "File")}");
                
                if (!currentEntry.IsDirectory)
                {
                    properties.AppendLine($"Size: {FormatFileSize(currentEntry.Size)} ({currentEntry.Size:N0} bytes)");
                    properties.AppendLine($"Extension: {currentEntry.Extension}");
                }
                
                properties.AppendLine($"Modified: {currentEntry.LastModified:yyyy-MM-dd HH:mm:ss}");
                properties.AppendLine($"Attributes: {currentEntry.Attributes}");
                
                if (currentEntry.IsArchive)
                {
                    properties.AppendLine("Archive: Yes");
                }
                
                var dialog = new FilePropertiesDialog(properties.ToString());
                Application.Run(dialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing file properties");
                SetStatus($"Error: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Formats a file size in bytes to a human-readable string
        /// </summary>
        private string FormatFileSize(long bytes)
        {
            string[] sizes = { "B", "KB", "MB", "GB", "TB" };
            double len = bytes;
            int order = 0;
            
            while (len >= 1024 && order < sizes.Length - 1)
            {
                order++;
                len = len / 1024;
            }
            
            return $"{len:0.##} {sizes[order]}";
        }
        
        /// <summary>
        /// Launches the external configuration program to edit the configuration file
        /// Handles Y key press
        /// </summary>
        public void HandleLaunchConfigurationProgram()
        {
            try
            {
                _logger.LogDebug("Launching configuration program");
                
                // Use cached configuration to get the program path
                var programPath = _config.ConfigurationProgramPath;
                
                // Get the configuration file path
                var configFilePath = _configProvider.GetConfigFilePath();
                
                if (!File.Exists(configFilePath))
                {
                    SetStatus($"Configuration file not found: {configFilePath}");
                    _logger.LogWarning($"Configuration file not found: {configFilePath}");
                    return;
                }

                // Use EditorLauncher to handle the external process safely with TUI suspension
                var launcher = new EditorLauncher();
                
                // Pass programPath as preferredEditor. If it's null/empty, launcher uses default (VISUAL/EDITOR/notepad/vim)
                int exitCode = launcher.LaunchEditorAndWait(configFilePath, programPath);
                
                if (exitCode != 0)
                {
                     _logger.LogWarning("Editor exited with code {ExitCode}", exitCode);
                }
                
                // Ask user if they want to reload configuration
                var result = MessageBox.Query("Configuration", 
                    $"Launched editor for configuration.\n\nDo you want to reload configuration now?", 
                    "Yes", "No");
                    
                if (result == 0) // Yes
                {
                    ReloadConfiguration();
                }
                else
                {
                    // Even if not reloading, we should refresh the screen
                    Application.Refresh();
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error launching configuration program");
                SetStatus($"Error launching configuration program: {ex.Message}");
            }
        }

        /// <summary>
        /// Reloads configuration from disk and updates UI
        /// </summary>
        public void ReloadConfiguration()
        {
            try
            {
                _logger.LogInformation("Reloading configuration...");

                // 1. Force reload configuration and update cache
                _config = _configProvider.ReloadConfiguration();

                // Change log level dynamically
                LoggingConfiguration.ChangeLogLevel(_config.LogLevel);

                // 2. Apply configuration
                ApplyConfiguration(_config);

                // 3. Reload KeyBindings
                if (_config.KeyBindings != null && !string.IsNullOrEmpty(_config.KeyBindings.KeyBindingFile))
                {
                    try
                    {
                        string keyBindingPath = Path.Combine(_configProvider.GetConfigDirectory(), _config.KeyBindings.KeyBindingFile);
                        if (!File.Exists(keyBindingPath))
                        {
                            // If not in config dir, try current dir
                             if (File.Exists(_config.KeyBindings.KeyBindingFile))
                             {
                                 keyBindingPath = _config.KeyBindings.KeyBindingFile;
                             }
                        }
                        
                        _keyBindings.LoadBindings(keyBindingPath);
                        _logger.LogInformation("Reloaded key bindings from {Path}", keyBindingPath);
                    }
                    catch (Exception ex)
                    {
                        _logger.LogError(ex, "Error reloading key bindings");
                    }
                }
                
                // 4. Reload Custom Functions
                try
                {
                    string customFunctionsPath = Path.Combine(_configProvider.GetConfigDirectory(), "custom_functions.json");
                    _customFunctionManager.LoadFunctions(customFunctionsPath);
                    _logger.LogInformation("Reloaded custom functions from {Path}", customFunctionsPath);
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Error reloading custom functions");
                }

                // 4. Reload Custom Functions
                try
                {
                    string customFunctionsPath = Path.Combine(_configProvider.GetConfigDirectory(), "custom_functions.json");
                    _customFunctionManager.LoadFunctions(customFunctionsPath);
                    _logger.LogInformation("Reloaded custom functions from {Path}", customFunctionsPath);
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Error reloading custom functions");
                }

                // 5. Update dynamic components
                // Note: MaxSimultaneousJobs requires a restart because the Semaphore cannot be resized safely live.
                _jobManager.UpdateSettings(_config.Display.TaskPanelUpdateIntervalMs);

                SetStatus("Configuration reloaded. (Restart required for Migemo or Simultaneous Jobs limit)");
                _logger.LogInformation("Configuration reloaded successfully");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to reload configuration");
                SetStatus($"Error reloading configuration: {ex.Message}");
            }
        }

        /// <summary>
        /// Applies the configuration to the application components
        /// </summary>
        private void ApplyConfiguration(Configuration config)
        {
            // Configure display
            if (config.Display != null)
            {
                // Note: Font settings are applied at startup currently, so changing them requires restart
                // But we can update colors
                ApplyColorScheme(config.Display);
                
                // Update specific display settings
                if (_leftPane != null) _leftPane.Configuration = config;
                if (_rightPane != null) _rightPane.Configuration = config;
            }
            
            // Configure Archive settings
            /*
            if (config.Archive != null)
            {
                _archiveManager.Configure(config.Archive);
            }
            */
            
            // Refresh panes to reflect changes
            RefreshPanes();
        }
        
        /// <summary>
        /// Sets the display mode to Details (the default detailed view)
        /// </summary>
        private void SetDisplayModeDetailed()
        {
            SwitchDisplayMode(DisplayMode.Details);
        }

        /// <summary>
        /// Shows the history dialog for selecting previously visited directories
        /// </summary>
        public void ShowHistoryDialog()
        {
            try
            {
                var historyDialog = new HistoryDialog(
                    _historyManager, 
                    _searchEngine, 
                    _config, 
                    _leftPaneActive,
                    (path, samePane) => 
                    {
                        if (samePane) NavigateToDirectory(path);
                        else 
                        {
                            var inactive = GetInactivePane();
                            inactive.CurrentPath = path;
                            LoadPaneDirectory(inactive);
                            RefreshPanes();
                        }
                    },
                    _logger);
                Application.Run(historyDialog);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error showing history dialog");
                SetStatus($"Error: {ex.Message}");
            }
        }

        /// <summary>
        /// Executes a file using the system's default handler (for PipeToAction)
        /// </summary>
        private void ExecuteFile(string filePath)
        {
            try
            {
                _logger.LogDebug($"Executing file from PipeToAction: {filePath}");

                // Remove surrounding quotes if present (common when paths are quoted by macros)
                string cleanPath = filePath;
                if (cleanPath.StartsWith("\"") && cleanPath.EndsWith("\"") && cleanPath.Length > 1)
                {
                    cleanPath = cleanPath.Substring(1, cleanPath.Length - 2);
                }

                _logger.LogDebug($"Cleaned path: {cleanPath}");

                // Use the existing ExecuteFile functionality with Default execution mode
                ExecuteFile(cleanPath, ExecutionMode.Default);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error executing file from PipeToAction: {FilePath}", filePath);
                SetStatus($"Error executing file: {ex.Message}");
            }
        }

        /// <summary>
        /// Executes a file using the editor (for PipeToAction)
        /// </summary>
        private void ExecuteFileWithEditor(string filePath)
        {
            try
            {
                _logger.LogDebug($"Executing file with editor from PipeToAction: {filePath}");

                // Remove surrounding quotes if present (common when paths are quoted by macros)
                string cleanPath = filePath;
                if (cleanPath.StartsWith("\"") && cleanPath.EndsWith("\"") && cleanPath.Length > 1)
                {
                    cleanPath = cleanPath.Substring(1, cleanPath.Length - 2);
                }

                _logger.LogDebug($"Cleaned path: {cleanPath}");

                // Use the existing ExecuteFile functionality with Editor execution mode
                ExecuteFile(cleanPath, ExecutionMode.Editor);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error executing file with editor from PipeToAction: {FilePath}", filePath);
                SetStatus($"Error executing file with editor: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Disposes the MainController and all its resources
        /// </summary>
        public void Dispose()
        {
            Shutdown();
            GC.SuppressFinalize(this);
        }
    }

}

