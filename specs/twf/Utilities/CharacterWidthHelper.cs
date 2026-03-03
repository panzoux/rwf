using System;

namespace TWF.Utilities
{
    /// <summary>
    /// Helper class for calculating display width of characters, handling CJK double-width characters
    /// </summary>
    public static class CharacterWidthHelper
    {
        /// <summary>
        /// Configurable width for CJK characters (default: 2)
        /// Set to 0 to disable CJK width calculation and use standard string length
        /// </summary>
        public static int CJKCharacterWidth { get; set; } = 2;
        
        /// <summary>
        /// Default ellipsis string used for truncation
        /// </summary>
        public static string DefaultEllipsis { get; set; } = "...";

        /// <summary>
        /// Gets the display width of a single character
        /// </summary>
        /// <param name="c">Character to measure</param>
        /// <returns>Display width: 0 for zero-width, 1 for single-width, configured width for CJK</returns>
        public static int GetCharWidth(char c)
        {
            // If CJK_CharacterWidth is 0, disable the feature and treat all characters as width 1
            if (CJKCharacterWidth == 0)
            {
                return 1;
            }

            // Check for zero-width characters first
            if (IsZeroWidthCharacter(c))
            {
                return 0;
            }

            // Check for CJK characters (configurable width)
            if (IsCJKCharacter(c))
            {
                return CJKCharacterWidth;
            }

            // Default to single-width for ASCII and most other characters
            return 1;
        }

        /// <summary>
        /// Checks if a character is a CJK (Chinese, Japanese, Korean) character
        /// </summary>
        /// <param name="c">Character to check</param>
        /// <returns>True if the character is CJK</returns>
        private static bool IsCJKCharacter(char c)
        {
            int code = c;

            // CJK Unified Ideographs
            if (code >= 0x4E00 && code <= 0x9FFF) return true;

            // CJK Extension A
            if (code >= 0x3400 && code <= 0x4DBF) return true;

            // Hiragana
            if (code >= 0x3040 && code <= 0x309F) return true;

            // Katakana
            if (code >= 0x30A0 && code <= 0x30FF) return true;

            // Katakana Phonetic Extensions
            if (code >= 0x31F0 && code <= 0x31FF) return true;

            // Hangul Syllables
            if (code >= 0xAC00 && code <= 0xD7AF) return true;

            // Hangul Jamo
            if (code >= 0x1100 && code <= 0x11FF) return true;

            // Fullwidth Forms (fullwidth ASCII variants)
            if (code >= 0xFF00 && code <= 0xFFEF) return true;

            // CJK Compatibility Ideographs
            if (code >= 0xF900 && code <= 0xFAFF) return true;

            // CJK Radicals Supplement
            if (code >= 0x2E80 && code <= 0x2EFF) return true;

            // CJK Symbols and Punctuation
            if (code >= 0x3000 && code <= 0x303F) return true;

            return false;
        }

        /// <summary>
        /// Checks if a character is zero-width (combining marks, zero-width joiners, etc.)
        /// </summary>
        /// <param name="c">Character to check</param>
        /// <returns>True if the character is zero-width</returns>
        private static bool IsZeroWidthCharacter(char c)
        {
            int code = c;

            // Combining Diacritical Marks
            if (code >= 0x0300 && code <= 0x036F) return true;

            // Combining Diacritical Marks Extended
            if (code >= 0x1AB0 && code <= 0x1AFF) return true;

            // Combining Diacritical Marks Supplement
            if (code >= 0x1DC0 && code <= 0x1DFF) return true;

            // Combining Half Marks
            if (code >= 0xFE20 && code <= 0xFE2F) return true;

            // Zero Width Space, Zero Width Non-Joiner, Zero Width Joiner
            if (code == 0x200B || code == 0x200C || code == 0x200D) return true;

            // Variation Selectors
            if (code >= 0xFE00 && code <= 0xFE0F) return true;

            return false;
        }

        /// <summary>
        /// Gets the display width of a string
        /// </summary>
        /// <param name="text">String to measure</param>
        /// <returns>Total display width</returns>
        public static int GetStringWidth(string? text)
        {
            if (string.IsNullOrEmpty(text))
            {
                return 0;
            }

            // If CJK_CharacterWidth is 0, disable the feature and use standard string length
            if (CJKCharacterWidth == 0)
            {
                return text.Length;
            }

            int width = 0;
            foreach (char c in text)
            {
                width += GetCharWidth(c);
            }

            return width;
        }

        /// <summary>
        /// Pads a string to a specific display width
        /// </summary>
        /// <param name="text">String to pad</param>
        /// <param name="targetWidth">Target display width</param>
        /// <param name="paddingChar">Character to use for padding (default: space)</param>
        /// <returns>Padded string</returns>
        public static string PadToWidth(string? text, int targetWidth, char paddingChar = ' ')
        {
            if (targetWidth < 0)
            {
                throw new ArgumentException("Target width cannot be negative", nameof(targetWidth));
            }

            if (string.IsNullOrEmpty(text))
            {
                return new string(paddingChar, targetWidth);
            }

            int currentWidth = GetStringWidth(text);
            
            if (currentWidth >= targetWidth)
            {
                return text;
            }

            int paddingNeeded = targetWidth - currentWidth;
            return text + new string(paddingChar, paddingNeeded);
        }

        /// <summary>
        /// Truncates a string to fit within a maximum display width
        /// </summary>
        /// <param name="text">String to truncate</param>
        /// <param name="maxWidth">Maximum display width</param>
        /// <param name="ellipsis">Ellipsis string to append when truncated (null to use DefaultEllipsis)</param>
        /// <returns>Truncated string</returns>
        public static string TruncateToWidth(string? text, int maxWidth, string? ellipsis = null)
        {
            if (maxWidth < 0)
            {
                throw new ArgumentException("Max width cannot be negative", nameof(maxWidth));
            }

            if (string.IsNullOrEmpty(text))
            {
                return string.Empty;
            }

            ellipsis ??= DefaultEllipsis;
            int currentWidth = 0;
            int ellipsisWidth = GetStringWidth(ellipsis);
            int targetWidth = maxWidth - ellipsisWidth;

            // If ellipsis itself is too wide, just truncate without ellipsis
            if (targetWidth < 0)
            {
                targetWidth = maxWidth;
                ellipsis = string.Empty;
            }

            string padding = string.Empty;
            for (int i = 0; i < text.Length; i++)
            {
                int charWidth = GetCharWidth(text[i]);
                
                if (currentWidth + charWidth > targetWidth)
                {
                    // padding for truncating at 2-char width boundaries
                    if (currentWidth < targetWidth) {
                        // must take targetWidth into account as well
                        padding = padding.PadRight((currentWidth + charWidth + targetWidth) % CJKCharacterWidth,' ');
                    }
                    // Need to truncate here
                    return text.Substring(0, i) + ellipsis + padding;
                }

                currentWidth += charWidth;
            }

            // String fits within max width
            return text;
        }

        /// <summary>
        /// Smartly truncates a string to fit within a maximum display width, preserving the end of the string (e.g. extension).
        /// </summary>
        /// <param name="text">String to truncate.</param>
        /// <param name="maxWidth">Maximum display width.</param>
        /// <param name="ellipsis">Ellipsis string to use (null to use DefaultEllipsis).</param>
        /// <returns>Truncated string.</returns>
        public static string SmartTruncate(string? text, int maxWidth, string? ellipsis = null)
        {
            if (string.IsNullOrEmpty(text)) return string.Empty;
            if (maxWidth < 0) throw new ArgumentException("Max width cannot be negative", nameof(maxWidth));

            ellipsis ??= DefaultEllipsis;
            int textWidth = GetStringWidth(text);
            if (textWidth <= maxWidth) return text;

            int ellipsisWidth = GetStringWidth(ellipsis);
            int availableWidth = maxWidth - ellipsisWidth;

            if (availableWidth < 2) 
            {
                // If practically no space, just truncate from start with ellipsis if possible
                return TruncateToWidth(text, maxWidth, ellipsis);
            }

            // Calculate split point: keep more of the start, some of the end
            // Strategy: Try to keep quarter at end, rest at start
            int widthForEnd = availableWidth / 3; 
            int widthForStart = availableWidth - widthForEnd;

            // Find index for start part
            int currentWidth = 0;
            int startIndex = 0;
            for (int i = 0; i < text.Length; i++)
            {
                int len = GetCharWidth(text[i]);
                if (currentWidth + len > widthForStart) break;
                currentWidth += len;
                startIndex++;
            }

            // Find index for end part (scan backwards)
            currentWidth = 0;
            int endIndex = text.Length;
            for (int i = text.Length - 1; i >= 0; i--)
            {
                int len = GetCharWidth(text[i]);
                if (currentWidth + len > widthForEnd) break;
                currentWidth += len;
                endIndex--;
            }

            if (startIndex >= endIndex) return TruncateToWidth(text, maxWidth, ellipsis);

            return text.Substring(0, startIndex) + ellipsis + text.Substring(endIndex);
        }
    }
}
