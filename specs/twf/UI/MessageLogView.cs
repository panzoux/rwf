using Terminal.Gui;
using System.Text;

namespace TWF.UI
{
    /// <summary>
    /// A read-only text view that acts as a message log
    /// </summary>
    public class MessageLogView : TextView
    {
        private const int MaxLogHistory = 1000;
        private readonly List<string> _messages = new List<string>();

        public MessageLogView()
        {
            ReadOnly = true;
            ColorScheme = new ColorScheme
            {
                Normal = Application.Driver.MakeAttribute(Color.White, Color.Black),
                Focus = Application.Driver.MakeAttribute(Color.White, Color.Black)
            };
        }

        /// <summary>
        /// Adds a message to the log
        /// </summary>
        public void AddMessage(string message)
        {
            var timestamp = DateTime.Now.ToString("HH:mm:ss");
            var formattedMessage = $"[{timestamp}] {message}";
            
            _messages.Add(formattedMessage);
            
            // Trim history if needed
            if (_messages.Count > MaxLogHistory)
            {
                _messages.RemoveAt(0);
            }

            // Update text
            // For efficient updates, we might want to just append, but TextView handles Text property setting reasonably well
            // We reverse the list for display if we want newest on top? 
            // Standard logs usually have newest at bottom.
            // But for a single line view (collapsed), we want the newest.
            
            // Let's store full text.
            Text = string.Join(Environment.NewLine, _messages);
            
            // Auto-scroll to bottom
            MoveEnd();
        }

        /// <summary>
        /// Gets the last message added
        /// </summary>
        public string GetLastMessage()
        {
            if (_messages.Count > 0)
            {
                return _messages.Last();
            }
            return string.Empty;
        }

        /// <summary>
        /// Clears the log
        /// </summary>
        public void ClearLog()
        {
            _messages.Clear();
            Text = string.Empty;
        }
    }
}
