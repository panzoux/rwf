using System;
using System.Collections.Generic;
using System.Linq;
using TWF.Models;

namespace TWF.Services
{
    public class HistoryManager
    {
        private readonly List<string> _leftHistory = new List<string>();
        private readonly List<string> _rightHistory = new List<string>();
        private readonly Configuration _configuration;

        public HistoryManager(Configuration configuration)
        {
            _configuration = configuration;
        }

        public IReadOnlyList<string> LeftHistory => _leftHistory;
        public IReadOnlyList<string> RightHistory => _rightHistory;

        public void SetHistory(bool isLeft, List<string> history)
        {
            var target = isLeft ? _leftHistory : _rightHistory;
            target.Clear();
            if (history != null)
            {
                target.AddRange(history.Where(p => !string.IsNullOrWhiteSpace(p)));
            }
            TrimHistory(isLeft);
        }

        public void Add(bool isLeft, string path)
        {
            if (string.IsNullOrWhiteSpace(path)) return;

            var target = isLeft ? _leftHistory : _rightHistory;
            
            // Move to top if exists, otherwise just insert at top
            target.Remove(path);
            target.Insert(0, path);

            TrimHistory(isLeft);
        }

        private void TrimHistory(bool isLeft)
        {
            var target = isLeft ? _leftHistory : _rightHistory;
            int maxItems = _configuration.MaxHistoryItems;
            if (maxItems > 0 && target.Count > maxItems)
            {
                target.RemoveRange(maxItems, target.Count - maxItems);
            }
        }

        public void Clear(bool isLeft)
        {
            var target = isLeft ? _leftHistory : _rightHistory;
            target.Clear();
        }
    }
}
