namespace TWF.Models
{
    /// <summary>
    /// Represents a key binding action
    /// </summary>
    public class ActionBinding
    {
        /// <summary>
        /// The type of action (Function, KeyRedirect, Command)
        /// </summary>
        public ActionType Type { get; set; }

        /// <summary>
        /// The target of the action (function name, key code, or command string)
        /// </summary>
        public string Target { get; set; } = string.Empty;

        /// <summary>
        /// Optional parameters for the action
        /// </summary>
        public Dictionary<string, string> Parameters { get; set; } = new Dictionary<string, string>();
    }
}
