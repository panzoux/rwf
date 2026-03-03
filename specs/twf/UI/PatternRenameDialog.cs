using Terminal.Gui;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for pattern-based batch renaming
    /// </summary>
    public class PatternRenameDialog : Dialog
    {
        private TextField _patternField;
        private TextField _replacementField;
        public string Pattern => _patternField.Text.ToString() ?? string.Empty;
        public string Replacement => _replacementField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public PatternRenameDialog() : base("Pattern Rename", 60, 10)
        {
            var label1 = new Label("Search pattern (e.g. s/old/new/ or .txt):")
            {
                X = 1,
                Y = 1
            };
            Add(label1);

            _patternField = new TextField("")
            {
                X = 1,
                Y = 2,
                Width = Dim.Fill(1)
            };
            Add(_patternField);

            var label2 = new Label("Replacement (leave empty if using s/ or tr/):")
            {
                X = 1,
                Y = 3
            };
            Add(label2);

            _replacementField = new TextField("")
            {
                X = 1,
                Y = 4,
                Width = Dim.Fill(1)
            };
            Add(_replacementField);

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

            _patternField.SetFocus();
        }
    }
}
