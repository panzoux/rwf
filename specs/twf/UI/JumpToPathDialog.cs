using Terminal.Gui;
using TWF.Models;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for jumping to a specific directory path
    /// </summary>
    public class JumpToPathDialog : Dialog
    {
        private TextField _pathField;
        public string Path => _pathField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public JumpToPathDialog(string initialPath) : base("Jump to Path", 70, 8)
        {
            var label = new Label("Enter path:")
            {
                X = 1,
                Y = 1
            };
            Add(label);

            _pathField = new TextField(initialPath)
            {
                X = 1,
                Y = 2,
                Width = Dim.Fill(1)
            };
            Add(_pathField);

            var okButton = new Button("OK", is_default: true);
            okButton.Clicked += () =>
            {
                IsOk = true;
                Application.RequestStop();
            };

            var cancelButton = new Button("Cancel");
            cancelButton.Clicked += () =>
            {
                IsOk = false;
                Application.RequestStop();
            };

            AddButton(okButton);
            AddButton(cancelButton);

            _pathField.SetFocus();
        }
    }
}
