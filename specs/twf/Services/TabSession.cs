using TWF.Models;

namespace TWF.Services
{
    public class TabSession
    {
        public PaneState LeftState { get; set; }
        public PaneState RightState { get; set; }
        public string? LeftFocusTarget { get; set; }
        public string? RightFocusTarget { get; set; }
        public bool IsLeftPaneActive { get; set; } = true;
        public HistoryManager History { get; private set; }

        public TabSession(Configuration config)
        {
            LeftState = new PaneState();
            RightState = new PaneState();
            History = new HistoryManager(config);
        }

        public TabSession(HistoryManager historyManager)
        {
            LeftState = new PaneState();
            RightState = new PaneState();
            History = historyManager;
        }
    }
}
