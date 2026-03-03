using Terminal.Gui;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for simple file renaming
    /// </summary>
    public class SimpleRenameDialog : Dialog
    {
        private TextField _nameField;
        public string NewName => _nameField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public SimpleRenameDialog(string currentName) : base("Rename", 60, 8)
        {
            var label = new Label("New name:")
            {
                X = 1,
                Y = 1
            };
            Add(label);

            _nameField = new TextField(currentName)
            {
                X = 1,
                Y = 2,
                Width = Dim.Fill(1)
            };
            Add(_nameField);

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

            _nameField.SetFocus();
            _nameField.CursorPosition = _nameField.Text.Length;
        }
    }
}
