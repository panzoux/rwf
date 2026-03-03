using Terminal.Gui;
using TWF.Models;

namespace TWF.UI
{
    /// <summary>
    /// Dialog for selecting and executing custom functions
    /// </summary>
    public class CustomFunctionDialog : Dialog
    {
        private readonly List<CustomFunction> _functions;
        private ListView _functionList = null!;
        private Label _descriptionLabel = null!;
        private CustomFunction? _selectedFunction;

        /// <summary>
        /// Gets the selected custom function, or null if cancelled
        /// </summary>
        public CustomFunction? SelectedFunction => _selectedFunction;

        public CustomFunctionDialog(List<CustomFunction> functions) : base("Custom Functions", 70, 20)
        {
            _functions = functions ?? throw new ArgumentNullException(nameof(functions));
            InitializeComponents();
        }

        private void InitializeComponents()
        {
            // Title label
            var titleLabel = new Label("> Select a custom function to execute:")
            {
                X = 1,
                Y = 0
            };
            Add(titleLabel);

            // Function list
            _functionList = new ListView()
            {
                X = 1,
                Y = 1,
                Width = Dim.Fill(1),
                Height = Dim.Fill(5),
                AllowsMarking = false
            };

            var functionNames = _functions.Select(f => f.Name).ToList();
            _functionList.SetSource(functionNames);

            _functionList.SelectedItemChanged += (args) =>
            {
                UpdateDescription();
            };

            Add(_functionList);

            /*
            // Description label
            var descLabel = new Label("Description:")
            {
                X = 1,
                Y = Pos.AnchorEnd(4)
            };
            Add(descLabel);
            */

            _descriptionLabel = new Label("")
            {
                X = 1,
                Y = Pos.AnchorEnd(3),
                Width = Dim.Fill(1),
                Height = 1
            };
            Add(_descriptionLabel);

            // Buttons
            var executeButton = new Button("Execute", is_default: true);
            executeButton.Clicked += () =>
            {
                if (_functionList.SelectedItem >= 0 && _functionList.SelectedItem < _functions.Count)
                {
                    _selectedFunction = _functions[_functionList.SelectedItem];
                    Application.RequestStop();
                }
            };

            var cancelButton = new Button("Cancel");
            cancelButton.Clicked += () =>
            {
                _selectedFunction = null;
                Application.RequestStop();
            };

            AddButton(executeButton);
            AddButton(cancelButton);

            // Show initial description
            UpdateDescription();
        }

        private void UpdateDescription()
        {
            if (_functionList.SelectedItem >= 0 && _functionList.SelectedItem < _functions.Count)
            {
                var function = _functions[_functionList.SelectedItem];
                _descriptionLabel.Text = $"Description: {function.Description}\nCommand: {function.Command}";
            }
            else
            {
                _descriptionLabel.Text = "";
            }
        }
    }
}
