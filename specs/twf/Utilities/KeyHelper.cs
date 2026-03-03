using System;
using System.Collections.Generic;
using Terminal.Gui;

namespace TWF.Utilities
{
    /// <summary>
    /// Helper class for handling Terminal.Gui key events and conversions
    /// </summary>
    public static class KeyHelper
    {
        /// <summary>
        /// Converts a Terminal.Gui Key to a string representation for key binding lookup
        /// </summary>
        public static string ConvertKeyToString(Key key)
        {
            var parts = new List<string>();
            bool hasShift = false;
            
            // Check for modifiers
            if ((key & Key.ShiftMask) == Key.ShiftMask)
            {
                hasShift = true;
                parts.Add("Shift");
            }
            if ((key & Key.CtrlMask) == Key.CtrlMask)
                parts.Add("Ctrl");
            if ((key & Key.AltMask) == Key.AltMask)
                parts.Add("Alt");
            
            // Get the base key (remove modifiers)
            Key baseKey = key & ~(Key.ShiftMask | Key.CtrlMask | Key.AltMask);
            
            // Check if this is a lowercase letter
            bool isLowercaseLetter = baseKey >= (Key)'a' && baseKey <= (Key)'z';
            
            // Convert base key to string
            string keyName = baseKey switch
            {
                Key.Enter => "Enter",
                Key.Backspace => "Backspace",
                Key.Tab => "Tab",
                Key.Home => "Home",
                Key.End => "End",
                Key.PageUp => "PageUp",
                Key.PageDown => "PageDown",
                Key.CursorUp => "Up",
                Key.CursorDown => "Down",
                Key.CursorLeft => "Left",
                Key.CursorRight => "Right",
                Key.Space => "Space",
                (Key)27 => "Escape",
                Key.F1 => "F1",
                Key.F2 => "F2",
                Key.F3 => "F3",
                Key.F4 => "F4",
                Key.F5 => "F5",
                Key.F6 => "F6",
                Key.F7 => "F7",
                Key.F8 => "F8",
                Key.F9 => "F9",
                Key.F10 => "F10",
                Key.D1 => "1",
                Key.D2 => "2",
                Key.D3 => "3",
                Key.D4 => "4",
                Key.D5 => "5",
                Key.D6 => "6",
                Key.D7 => "7",
                Key.D8 => "8",
                Key.D9 => "9",
                Key.D0 => "0",
                // Special characters
                (Key)'$' => "$",
                (Key)'@' => "@",
                (Key)':' => ":",
                (Key)'`' => "`",
                (Key)'~' => "~",
                (Key)'!' => "!",
                (Key)'#' => "#",
                (Key)'%' => "%",
                (Key)'^' => "^",
                (Key)'&' => "&",
                (Key)'*' => "*",
                (Key)'(' => "(",
                (Key)')' => ")",
                (Key)'-' => "-",
                (Key)'_' => "_",
                (Key)'=' => "=",
                (Key)'+' => "+",
                (Key)'[' => "[",
                (Key)']' => "]",
                (Key)'{' => "{",
                (Key)'}' => "}",
                (Key)'\\' => "\\",
                (Key)'|' => "|",
                (Key)';' => ";",
                (Key)'\'' => "'",
                (Key)'"' => "\"",
                (Key)',' => ",",
                (Key)'.' => ".",
                (Key)'<' => "<",
                (Key)'>' => ">",
                (Key)'/' => "/",
                (Key)'?' => "?",
                _ => baseKey >= Key.A && baseKey <= Key.Z ? ((char)baseKey).ToString() :
                     baseKey >= (Key)'a' && baseKey <= (Key)'z' ? ((char)baseKey).ToString().ToUpper() :
                     ((char)baseKey).ToString()
            };
            
            // Handle uppercase letters as Shift+Letter
            // Terminal.Gui sends uppercase letters WITHOUT ShiftMask when Shift is pressed
            if (baseKey >= Key.A && baseKey <= Key.Z && !hasShift && parts.Count == 0 && !isLowercaseLetter)
            {
                // This is an uppercase letter without explicit Shift modifier
                // Add Shift to the parts
                parts.Insert(0, "Shift");
            }
            
            parts.Add(keyName);
            
            return string.Join("+", parts);
        }
    }
}
