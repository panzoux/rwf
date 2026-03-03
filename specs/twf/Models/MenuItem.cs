namespace TWF.Models
{
    /// <summary>
    /// Represents a menu item in a context menu
    /// </summary>
    public class MenuItem
    {
        public string Label { get; set; } = string.Empty;
        public string Action { get; set; } = string.Empty;
        public bool IsEnabled { get; set; } = true;
        public bool IsSeparator { get; set; } = false;
        public string Shortcut { get; set; } = string.Empty;
    }
}
