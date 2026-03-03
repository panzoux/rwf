namespace TWF.Models
{
    /// <summary>
    /// Represents the key binding configuration loaded from JSON
    /// </summary>
    public class KeyBindingConfig
    {
        public string Version { get; set; } = "1.0";
        public string Description { get; set; } = string.Empty;
        public Dictionary<string, string> Bindings { get; set; } = new Dictionary<string, string>();
        public Dictionary<string, string>? TextViewerBindings { get; set; }
        public Dictionary<string, string>? ImageViewerBindings { get; set; }
    }
}
