using Terminal.Gui;
using TWF.Models;
using TWF.Services;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for selecting archive format and name
    /// </summary>
    public class CompressionOptionsDialog : Dialog
    {
        private ListView _formatListView;
        private TextField _nameField;
        private List<ArchiveFormat> _supportedFormats;
        public ArchiveFormat SelectedFormat { get; private set; } = ArchiveFormat.ZIP;
        public string ArchiveName => _nameField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public CompressionOptionsDialog(int fileCount, string defaultName, List<ArchiveFormat> supportedFormats) : base("Compress Files", 70, 16)
        {
            _supportedFormats = supportedFormats;
            var infoLabel = new Label($"Compressing {fileCount} file(s)")
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1)
            };
            Add(infoLabel);

            var formatLabel = new Label("Select archive format:")
            {
                X = 1,
                Y = 3,
                Width = Dim.Fill(1)
            };
            Add(formatLabel);

            var formatOptions = _supportedFormats.Select(f => f.ToString().ToUpper()).ToList();
            _formatListView = new ListView(formatOptions)
            {
                X = 1,
                Y = 4,
                Width = Dim.Fill(1),
                Height = Math.Min(5, formatOptions.Count),
                AllowsMarking = false,
                CanFocus = true
            };
            _formatListView.SelectedItem = 0;
            Add(_formatListView);

            var nameLabel = new Label("Archive name:")
            {
                X = 1,
                Y = 10,
                Width = Dim.Fill(1)
            };
            Add(nameLabel);

            _nameField = new TextField(defaultName)
            {
                X = 1,
                Y = 11,
                Width = Dim.Fill(1)
            };
            Add(_nameField);

            var okButton = new Button("OK")
            {
                X = Pos.Center() - 10,
                Y = 13,
                IsDefault = true
            };
            okButton.Clicked += () =>
            {
                if (_formatListView.SelectedItem >= 0 && _formatListView.SelectedItem < _supportedFormats.Count)
                {
                    SelectedFormat = _supportedFormats[_formatListView.SelectedItem];
                    IsOk = true;
                }
                Application.RequestStop();
            };

            var cancelButton = new Button("Cancel")
            {
                X = Pos.Center() + 2,
                Y = 13
            };
            cancelButton.Clicked += () => { IsOk = false; Application.RequestStop(); };

            AddButton(okButton);
            AddButton(cancelButton);

            _nameField.SetFocus();
        }
    }

    /// <summary>
    /// Dialog for selecting file split parameters
    /// </summary>
    public class FileSplitOptionsDialog : Dialog
    {
        private TextField _sizeField;
        private TextField _dirField;
        public long PartSize { get; private set; }
        public string OutputDirectory => _dirField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public FileSplitOptionsDialog(string fileName, long fileSize, string initialOutputDir) : base("Split File", 70, 16)
        {
            var fileInfo = new Label($"File: {fileName} ({FormatSize(fileSize)})")
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1),
                ColorScheme = new ColorScheme() { Normal = Application.Driver.MakeAttribute(Color.BrightCyan, Color.Black) }
            };
            Add(fileInfo);

            var sizeLabel = new Label("Part size (bytes):") { X = 1, Y = 3 };
            Add(sizeLabel);

            _sizeField = new TextField("1048576") { X = 25, Y = 3, Width = 20 };
            Add(_sizeField);

            var btnY = 5;
            var s1 = new Button("1 MB") { X = 1, Y = btnY }; s1.Clicked += () => _sizeField.Text = (1024 * 1024).ToString(); Add(s1);
            var s10 = new Button("10 MB") { X = 10, Y = btnY }; s10.Clicked += () => _sizeField.Text = (10 * 1024 * 1024).ToString(); Add(s10);
            var s100 = new Button("100 MB") { X = 20, Y = btnY }; s100.Clicked += () => _sizeField.Text = (100 * 1024 * 1024).ToString(); Add(s100);
            var s1G = new Button("1 GB") { X = 32, Y = btnY }; s1G.Clicked += () => _sizeField.Text = (1024L * 1024 * 1024).ToString(); Add(s1G);

            Add(new Label("Output directory:") { X = 1, Y = 7 });
            _dirField = new TextField(initialOutputDir) { X = 1, Y = 8, Width = Dim.Fill(1) };
            Add(_dirField);

            var okButton = new Button("OK") { X = Pos.Center() - 10, Y = 11, IsDefault = true };
            okButton.Clicked += () =>
            {
                if (long.TryParse(_sizeField.Text.ToString(), out long sz) && sz > 0)
                {
                    PartSize = sz;
                    IsOk = true;
                    Application.RequestStop();
                }
            };

            var cancelButton = new Button("Cancel") { X = Pos.Center() + 2, Y = 11 };
            cancelButton.Clicked += () => { IsOk = false; Application.RequestStop(); };

            AddButton(okButton);
            AddButton(cancelButton);
        }

        private string FormatSize(long size)
        {
            if (size < 1024) return $"{size} B";
            if (size < 1024 * 1024) return $"{size / 1024.0:F1} KB";
            return $"{size / (1024.0 * 1024.0):F1} MB";
        }
    }

    /// <summary>
    /// Dialog for selecting file join parameters
    /// </summary>
    public class FileJoinOptionsDialog : Dialog
    {
        private TextField _outputField;
        public string OutputFile => _outputField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public FileJoinOptionsDialog(int partCount, List<string> partNames, string initialOutputFile) : base("Join Split Files", 70, 18)
        {
            Add(new Label($"Found {partCount} part file(s) to join:") 
            {
                X = 1, Y = 1, Width = Dim.Fill(1),
                ColorScheme = new ColorScheme() { Normal = Application.Driver.MakeAttribute(Color.BrightCyan, Color.Black) }
            });

            Add(new Label(string.Join("\n", partNames.Take(5))) { X = 3, Y = 2, Width = Dim.Fill(1), Height = 5 });
            if (partCount > 5) Add(new Label($"... and {partCount - 5} more") { X = 3, Y = 7 });

            Add(new Label("Output file:") { X = 1, Y = 9 });
            _outputField = new TextField(initialOutputFile) { X = 1, Y = 10, Width = Dim.Fill(1) };
            Add(_outputField);

            var okButton = new Button("OK") { X = Pos.Center() - 10, Y = 13, IsDefault = true };
            okButton.Clicked += () => { IsOk = true; Application.RequestStop(); };

            var cancelButton = new Button("Cancel") { X = Pos.Center() + 2, Y = 13 };
            cancelButton.Clicked += () => { IsOk = false; Application.RequestStop(); };

            AddButton(okButton);
            AddButton(cancelButton);
        }
    }

    /// <summary>
    /// Dialog for selecting file comparison criteria
    /// </summary>
    public class FileComparisonDialog : Dialog
    {
        private RadioGroup _radioGroup;
        private TextField _toleranceField;
        public ComparisonCriteria Criteria { get; private set; }
        public TimeSpan? TimestampTolerance { get; private set; }
        public bool IsOk { get; private set; }

        public FileComparisonDialog() : base("File Comparison", 70, 18)
        {
            Add(new Label("Select comparison criteria to mark matching files in both panes:") { X = 1, Y = 1, Width = Dim.Fill(1) });
            Add(new Label("Comparison Criteria:") { X = 1, Y = 3 });

            _radioGroup = new RadioGroup(new NStack.ustring[] { "Size", "Timestamp", "Name" }) { X = 3, Y = 4, SelectedItem = 0 };
            Add(_radioGroup);

            var tolLabel = new Label("Timestamp Tolerance (seconds):") { X = 1, Y = 8, Visible = false };
            _toleranceField = new TextField("2") { X = 35, Y = 8, Width = 10, Visible = false };
            Add(tolLabel); Add(_toleranceField);

            var descLabel = new Label("") { X = 1, Y = 11, Width = Dim.Fill(1), Height = 3, ColorScheme = new ColorScheme { Normal = Application.Driver.MakeAttribute(Color.BrightYellow, Color.Black) } };
            Add(descLabel);

            _radioGroup.SelectedItemChanged += (args) =>
            {
                bool isTs = args.SelectedItem == 1;
                tolLabel.Visible = _toleranceField.Visible = isTs;
                descLabel.Text = args.SelectedItem switch {
                    0 => "Marks files with identical file sizes in both panes.",
                    1 => "Marks files with matching last modified timestamps\n(within the specified tolerance).",
                    2 => "Marks files with identical names in both panes\n(case-insensitive comparison).",
                    _ => ""
                };
            };

            var okButton = new Button("OK") { X = Pos.Center() - 10, Y = Pos.AnchorEnd(2), IsDefault = true };
            okButton.Clicked += () =>
            {
                Criteria = _radioGroup.SelectedItem switch { 0 => ComparisonCriteria.Size, 1 => ComparisonCriteria.Timestamp, 2 => ComparisonCriteria.Name, _ => ComparisonCriteria.Size };
                if (Criteria == ComparisonCriteria.Timestamp && double.TryParse(_toleranceField.Text.ToString(), out double sec)) TimestampTolerance = TimeSpan.FromSeconds(sec);
                IsOk = true; Application.RequestStop();
            };

            var cancelButton = new Button("Cancel") { X = Pos.Center() + 2, Y = Pos.AnchorEnd(2) };
            cancelButton.Clicked += () => { IsOk = false; Application.RequestStop(); };

            AddButton(okButton); AddButton(cancelButton);
            _radioGroup.SelectedItem = 0; // Trigger update
        }
    }

    /// <summary>
    /// Dialog for handling file collisions during operations
    /// </summary>
    public class FileCollisionDialog : Dialog
    {
        public FileCollisionResult Result { get; private set; } = new FileCollisionResult { Action = FileCollisionAction.Cancel };

        public FileCollisionDialog(string filename) : base("File Exists", 60, 11)
        {
            Add(new Label($"File already exists:\n{filename}") { X = 1, Y = 1, Width = Dim.Fill(1), Height = 2 });

            Add(new Label("Ctrl+Enter to 'Overwrite All' or 'Skip All'")
            {
                X = 1, Y = 4, Width = Dim.Fill(1),
                ColorScheme = new ColorScheme { Normal = Application.Driver.MakeAttribute(Color.BrightYellow, Color.Blue) }
            });

            var overwriteBtn = new Button("Overwrite") { X = 1, Y = Pos.AnchorEnd(1) };
            var skipBtn = new Button("Skip") { X = Pos.Right(overwriteBtn) + 1, Y = Pos.AnchorEnd(1) };
            var renameBtn = new Button("Rename") { X = Pos.Right(skipBtn) + 1, Y = Pos.AnchorEnd(1) };
            var cancelBtn = new Button("Cancel") { X = Pos.Right(renameBtn) + 1, Y = Pos.AnchorEnd(1) };

            bool isShiftHeld = false;
            this.KeyPress += (e) =>
            {
                var key = e.KeyEvent.Key;
                var cleanKey = key & ~Key.ShiftMask & ~Key.AltMask & ~Key.CtrlMask;
                bool shift = (key & Key.ShiftMask) != 0;
                bool alt = (key & Key.AltMask) != 0;
                bool ctrl = (key & Key.CtrlMask) != 0;

                if (alt && shift && cleanKey == Key.O) { Result = new FileCollisionResult { Action = FileCollisionAction.OverwriteAll }; Application.RequestStop(); e.Handled = true; return; }
                if (alt && shift && cleanKey == Key.S) { Result = new FileCollisionResult { Action = FileCollisionAction.SkipAll }; Application.RequestStop(); e.Handled = true; return; }

                if (ctrl && cleanKey == Key.Enter)
                {
                    if (overwriteBtn.HasFocus) { Result = new FileCollisionResult { Action = FileCollisionAction.OverwriteAll }; Application.RequestStop(); e.Handled = true; return; }
                    if (skipBtn.HasFocus) { Result = new FileCollisionResult { Action = FileCollisionAction.SkipAll }; Application.RequestStop(); e.Handled = true; return; }
                }

                if (alt && cleanKey == Key.O) { Result = new FileCollisionResult { Action = FileCollisionAction.Overwrite }; Application.RequestStop(); e.Handled = true; return; }
                if (alt && cleanKey == Key.S) { Result = new FileCollisionResult { Action = FileCollisionAction.Skip }; Application.RequestStop(); e.Handled = true; return; }
                
                isShiftHeld = shift;
            };

            overwriteBtn.Clicked += () => { Result = new FileCollisionResult { Action = isShiftHeld ? FileCollisionAction.OverwriteAll : FileCollisionAction.Overwrite }; Application.RequestStop(); };
            skipBtn.Clicked += () => { Result = new FileCollisionResult { Action = isShiftHeld ? FileCollisionAction.SkipAll : FileCollisionAction.Skip }; Application.RequestStop(); };
            renameBtn.Clicked += () =>
            {
                var inputDialog = new RenameConflictDialog(filename);
                Application.Run(inputDialog);
                if (inputDialog.IsOk) { Result = new FileCollisionResult { Action = FileCollisionAction.Rename, NewName = inputDialog.NewName }; Application.RequestStop(); }
            };
            cancelBtn.Clicked += () => { Result = new FileCollisionResult { Action = FileCollisionAction.Cancel }; Application.RequestStop(); };

            AddButton(overwriteBtn); AddButton(skipBtn); AddButton(renameBtn); AddButton(cancelBtn);
            renameBtn.IsDefault = true;
        }
    }

    /// <summary>
    /// Small sub-dialog for entering a new name during a collision
    /// </summary>
    public class RenameConflictDialog : Dialog
    {
        private TextField _nameField;
        public string NewName => _nameField.Text.ToString() ?? string.Empty;
        public bool IsOk { get; private set; }

        public RenameConflictDialog(string filename) : base("Rename File", 60, 7)
        {
            Add(new Label("New name:") { X = 1, Y = 1 });
            _nameField = new TextField(filename) { X = 1, Y = 2, Width = Dim.Fill(1) };
            Add(_nameField);

            var okBtn = new Button("OK") { IsDefault = true };
            var cancelBtn = new Button("Cancel");

            okBtn.Clicked += () => { IsOk = true; Application.RequestStop(); };
            cancelBtn.Clicked += () => { IsOk = false; Application.RequestStop(); };

            AddButton(okBtn); AddButton(cancelBtn);
        }
    }
}
