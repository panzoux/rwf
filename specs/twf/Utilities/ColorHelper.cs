using Terminal.Gui;

namespace TWF.Utilities
{
    public static class ColorHelper
    {
        public static Color ParseConfigColor(string? name, Color defaultColor)
        {
            if (string.IsNullOrEmpty(name)) return defaultColor;
            return name.ToLower() switch
            {
                "black" => Color.Black,
                "blue" => Color.Blue,
                "green" => Color.Green,
                "cyan" => Color.Cyan,
                "red" => Color.Red,
                "magenta" => Color.Magenta,
                "brown" => Color.Brown,
                "gray" => Color.Gray,
                "darkgray" => Color.DarkGray,
                "brightblue" => Color.BrightBlue,
                "brightgreen" => Color.BrightGreen,
                "brightcyan" => Color.BrightCyan,
                "brightred" => Color.BrightRed,
                "brightmagenta" => Color.BrightMagenta,
                "yellow" => Color.Brown,
                "white" => Color.White,
                _ => Enum.TryParse<Color>(name, true, out var color) ? color : defaultColor
            };
        }
    }
}
