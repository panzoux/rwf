namespace TWF.Models
{
    /// <summary>
    /// Represents the state of a single tab
    /// </summary>
    public class TabSessionState
    {
        public string LeftPath { get; set; } = string.Empty;
        public string RightPath { get; set; } = string.Empty;
        public string? LeftFocusTarget { get; set; }
        public string? RightFocusTarget { get; set; }
        public string LeftMask { get; set; } = "*";
        public string RightMask { get; set; } = "*";
        public SortMode LeftSort { get; set; } = SortMode.NameAscending;
        public SortMode RightSort { get; set; } = SortMode.NameAscending;
        public DisplayMode LeftDisplayMode { get; set; } = DisplayMode.Details;
        public DisplayMode RightDisplayMode { get; set; } = DisplayMode.Details;
        public bool LeftPaneActive { get; set; } = true;
        public List<string> LeftHistory { get; set; } = new List<string>();
        public List<string> RightHistory { get; set; } = new List<string>();
    }

    /// <summary>
    /// Represents the session state for persisting pane states between application runs
    /// </summary>
    public class SessionState
    {
        // For backward compatibility (stores the active tab's state)
        public string LeftPath { get; set; } = string.Empty;
        public string RightPath { get; set; } = string.Empty;
        public string? LeftFocusTarget { get; set; }
        public string? RightFocusTarget { get; set; }
        public string LeftMask { get; set; } = "*";
        public string RightMask { get; set; } = "*";
        public SortMode LeftSort { get; set; } = SortMode.NameAscending;
        public SortMode RightSort { get; set; } = SortMode.NameAscending;
        public DisplayMode LeftDisplayMode { get; set; } = DisplayMode.Details;
        public DisplayMode RightDisplayMode { get; set; } = DisplayMode.Details;
        public bool LeftPaneActive { get; set; } = true;
        public List<string> LeftHistory { get; set; } = new List<string>();
        public List<string> RightHistory { get; set; } = new List<string>();

        // New property to store all tabs
        public List<TabSessionState> Tabs { get; set; } = new List<TabSessionState>();
        public int ActiveTabIndex { get; set; } = 0;

        // Task Pane State
        public int TaskPaneHeight { get; set; } = 5;
        public bool TaskPaneExpanded { get; set; } = false;
    }
}
