namespace TWF.Models
{
    /// <summary>
    /// Represents a single help item with action mapping
    /// </summary>
    public class HelpItem
    {
        /// <summary>
        /// Group category (e.g., "Navigation", "File Operations")
        /// </summary>
        public string Category { get; set; } = string.Empty;

        /// <summary>
        /// Context/Mode (e.g., "Normal", "TextViewer", "ImageViewer")
        /// </summary>
        public string Context { get; set; } = "Normal";

        /// <summary>
        /// Internal action name (matches KeyBindingManager actions)
        /// </summary>
        public string Action { get; set; } = string.Empty;

        /// <summary>
        /// Description in the currently loaded language
        /// </summary>
        public string Description { get; set; } = string.Empty;

        /// <summary>
        /// Keys currently bound to this action (dynamically populated)
        /// </summary>
        public string BoundKeys { get; set; } = string.Empty;
    }
}
