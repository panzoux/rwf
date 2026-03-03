using Terminal.Gui;
using System.Threading;

namespace TWF.UI
{
    /// <summary>
    /// Reusable dialog for showing progress of background operations
    /// </summary>
    public class OperationProgressDialog : Dialog
    {
        private Label _statusLabel;
        private Label _fileLabel;
        private Label _progressLabel;
        private Label _bytesLabel;
        private CancellationTokenSource _cts;

        public string Status
        {
            get => _statusLabel.Text.ToString() ?? string.Empty;
            set => _statusLabel.Text = value;
        }

        public OperationProgressDialog(string title, CancellationTokenSource cts) : base(title, 70, 12)
        {
            _cts = cts;

            _statusLabel = new Label("Preparing...")
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1)
            };
            Add(_statusLabel);

            _fileLabel = new Label("")
            {
                X = 1,
                Y = 2,
                Width = Dim.Fill(1)
            };
            Add(_fileLabel);

            _progressLabel = new Label("0%")
            {
                X = 1,
                Y = 3,
                Width = Dim.Fill(1)
            };
            Add(_progressLabel);

            _bytesLabel = new Label("")
            {
                X = 1,
                Y = 4,
                Width = Dim.Fill(1)
            };
            Add(_bytesLabel);

            var cancelButton = new Button("Cancel (ESC)")
            {
                X = Pos.Center(),
                Y = Pos.AnchorEnd(2)
            };

            cancelButton.Clicked += () =>
            {
                _cts.Cancel();
                _statusLabel.Text = "Cancelling...";
            };

            AddButton(cancelButton);

            // Handle Escape key for cancellation
            this.KeyPress += (e) =>
            {
                if (e.KeyEvent.Key == (Key)27) // Escape
                {
                    _cts.Cancel();
                    _statusLabel.Text = "Cancelling...";
                    e.Handled = true;
                }
            };
        }

        /// <summary>
        /// Updates the progress information labels
        /// </summary>
        public void UpdateProgress(string currentFile, int currentIndex, int totalFiles, double percent, long bytesProcessed = -1, long totalBytes = -1)
        {
            const int maxFileWidth = 60;
            string truncatedFile = TWF.Utilities.CharacterWidthHelper.SmartTruncate(currentFile, maxFileWidth);
            _fileLabel.Text = $"File: {truncatedFile} ({currentIndex}/{totalFiles})";
            _progressLabel.Text = $"{percent:F1}%";

            if (totalBytes > 0 && bytesProcessed >= 0)
            {
                var mbProcessed = bytesProcessed / (1024.0 * 1024.0);
                var mbTotal = totalBytes / (1024.0 * 1024.0);
                _bytesLabel.Text = $"{mbProcessed:F2} MB / {mbTotal:F2} MB";
            }
            else if (bytesProcessed >= 0)
            {
                var mbProcessed = bytesProcessed / (1024.0 * 1024.0);
                _bytesLabel.Text = $"{mbProcessed:F2} MB";
            }
        }
    }
}
