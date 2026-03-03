using System.Text.RegularExpressions;

namespace TWF.Utilities
{
    /// <summary>
    /// Utility class for expanding environment variables in strings
    /// Supports formats: $VAR, ${VAR}, $env:VAR, %VAR%
    /// </summary>
    public static class EnvironmentVariableExpander
    {
        /// <summary>
        /// Expands environment variables in a path string.
        /// Supports formats: $VAR, ${VAR}, $env:VAR, %VAR%
        /// </summary>
        /// <param name="input">The input string that may contain environment variables</param>
        /// <returns>The string with environment variables expanded</returns>
        public static string ExpandEnvironmentVariables(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            string result = input;

            // Handle %VAR% format
            result = ExpandPercentFormat(result);

            // Handle $VAR format
            result = ExpandDollarFormat(result);

            // Handle ${VAR} format
            result = ExpandDollarBraceFormat(result);

            // Handle $env:VAR format (PowerShell style)
            result = ExpandDollarEnvFormat(result);

            return result;
        }

        /// <summary>
        /// Expands %VAR% format environment variables
        /// </summary>
        private static string ExpandPercentFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            // Use a more specific regex to match %VAR% format properly
            var regex = new Regex(@"%([A-Za-z_][A-Za-z0-9_]*)%");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);
                
                if (varValue != null)
                {
                    return varValue;
                }
                else
                {
                    return match.Value; // Return original %VAR% if not found
                }
            });
        }

        /// <summary>
        /// Expands $VAR format environment variables
        /// </summary>
        private static string ExpandDollarFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            var regex = new Regex(@"\$([A-Za-z_][A-Za-z0-9_]*)");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);
                
                if (varValue != null)
                {
                    return varValue;
                }
                else
                {
                    return match.Value; // Return original $VAR if not found
                }
            });
        }

        /// <summary>
        /// Expands ${VAR} format environment variables
        /// </summary>
        private static string ExpandDollarBraceFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            var regex = new Regex(@"\$\{([^}]+)\}");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);
                
                if (varValue != null)
                {
                    return varValue;
                }
                else
                {
                    return match.Value; // Return original ${VAR} if not found
                }
            });
        }

        /// <summary>
        /// Expands $env:VAR format environment variables (PowerShell style)
        /// </summary>
        private static string ExpandDollarEnvFormat(string input)
        {
            if (string.IsNullOrEmpty(input))
                return input;

            var regex = new Regex(@"\$env:([A-Za-z_][A-Za-z0-9_]*)");
            return regex.Replace(input, match =>
            {
                string varName = match.Groups[1].Value;
                string? varValue = Environment.GetEnvironmentVariable(varName);
                
                if (varValue != null)
                {
                    return varValue;
                }
                else
                {
                    return match.Value; // Return original $env:VAR if not found
                }
            });
        }
    }
}