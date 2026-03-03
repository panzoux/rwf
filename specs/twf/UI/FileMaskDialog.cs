using Terminal.Gui;
using TWF.Models;
using TWF.Utilities;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for entering file mask filters
    /// </summary>
    public class FileMaskDialog : Dialog
    {
        private TextField _maskField;
        public string Mask => _maskField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public FileMaskDialog(string initialMask, Configuration config) : base("File Mask Filter", 60, 10)
        {
            var label = new Label("Enter file mask (* = any chars, ? = single char):")
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1)
            };
            Add(label);

            _maskField = new TextField(initialMask)
            {
                X = 1,
                Y = 2,
                Width = Dim.Fill(1)
            };
            Add(_maskField);

            var helpFg = ColorHelper.ParseConfigColor(config.Display.DialogHelpForegroundColor, Color.BrightYellow);
            var helpBg = ColorHelper.ParseConfigColor(config.Display.DialogHelpBackgroundColor, Color.Blue);

            var helpLabel1 = new Label("Multiple patterns: *.txt *.doc")
            {
                X = 1,
                Y = 3,
                Width = Dim.Fill(1),
                ColorScheme = new ColorScheme()
                {
                    Normal = Application.Driver.MakeAttribute(helpFg, helpBg)
                }
            };
            Add(helpLabel1);

            var helpLabel2 = new Label("Exclusion: :*.txt :temp*")
            {
                X = 1,
                Y = 4,
                Width = Dim.Fill(1),
                ColorScheme = new ColorScheme()
                {
                    Normal = Application.Driver.MakeAttribute(helpFg, helpBg)
                }
            };
            Add(helpLabel2);

            var helpLabel3 = new Label("Regexp: /.*\\.json$/ /TEST/i /Test/")
            {
                X = 1,
                Y = 5,
                Width = Dim.Fill(1),
                ColorScheme = new ColorScheme()
                {
                    Normal = Application.Driver.MakeAttribute(Color.BrightYellow, Color.Black)
                }
            };
            Add(helpLabel3);

            var okButton = new Button("OK")
            {
                X = Pos.Center() - 10,
                Y = 6,
                IsDefault = true
            };
            okButton.Clicked += () =>
            {
                IsOk = true;
                Application.RequestStop();
            };

            var cancelButton = new Button("Cancel")
            {
                X = Pos.Center() + 2,
                Y = 6
            };
            cancelButton.Clicked += () =>
            {
                IsOk = false;
                Application.RequestStop();
            };

            AddButton(okButton);
            AddButton(cancelButton);

            _maskField.SetFocus();
        }
    }
}