namespace TWF.Models
{
    /// <summary>
    /// Represents the current UI mode of the application
    /// </summary>
    public enum UiMode
    {
        Normal,
        InputField,
        Confirmation,
        TextViewer,
        ImageViewer,
        Menu,
        Search
    }

    /// <summary>
    /// Represents the display mode for file lists
    /// </summary>
    public enum DisplayMode
    {
        NameOnly,
        Details,
        Thumbnail,
        Icon,
        OneColumn,
        TwoColumns,
        ThreeColumns,
        FourColumns,
        FiveColumns,
        SixColumns,
        SevenColumns,
        EightColumns
    }

    /// <summary>
    /// Represents the sort mode for file lists
    /// </summary>
    public enum SortMode
    {
        Unsorted,
        NameAscending,
        NameDescending,
        ExtensionAscending,
        ExtensionDescending,
        SizeAscending,
        SizeDescending,
        DateAscending,
        DateDescending
    }

    /// <summary>
    /// Represents supported archive formats
    /// </summary>
    public enum ArchiveFormat
    {
        LZH,
        ZIP,
        TAR,
        TGZ,
        CAB,
        RAR,
        SevenZip,
        BZ2,
        XZ,
        LZMA
    }

    /// <summary>
    /// Represents view modes for image viewer
    /// </summary>
    public enum ViewMode
    {
        OriginalSize,
        FitToWindow,
        FitToScreen,
        FixedZoom
    }

    /// <summary>
    /// Represents the type of action for key bindings
    /// </summary>
    public enum ActionType
    {
        Function,      // Execute a built-in function
        KeyRedirect,   // Redirect to another key
        Command        // Execute a shell command
    }

    /// <summary>
    /// Represents the type of history list
    /// </summary>
    public enum HistoryType
    {
        DirectoryHistory,
        SearchHistory,
        CommandHistory
    }

    /// <summary>
    /// Represents criteria for file comparison
    /// </summary>
    public enum ComparisonCriteria
    {
        Size,
        Timestamp,
        Name
    }
}
