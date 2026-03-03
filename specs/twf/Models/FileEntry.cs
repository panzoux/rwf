namespace TWF.Models
{
    /// <summary>
    /// Represents a file or directory entry with its metadata
    /// </summary>
    public class FileEntry
    {
        public string FullPath { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public string Extension { get; set; } = string.Empty;
        public long Size { get; set; }
        public DateTime LastModified { get; set; }
        public FileAttributes Attributes { get; set; }
        public bool IsDirectory { get; set; }
        public bool IsArchive { get; set; }
        public bool IsVirtualFolder { get; set; }
        /// <summary>
        /// Gets or sets whether the file entry is marked/selected
        /// </summary>
        public bool IsMarked { get; set; } = false;

        /// <summary>
        /// Formats the file entry for display based on the specified display mode
        /// </summary>
        public string FormatForDisplay(DisplayMode mode, int availableWidth = 80)
        {
            return mode switch
            {
                DisplayMode.NameOnly => Name,
                DisplayMode.Details => FormatDetailView(availableWidth),
                DisplayMode.OneColumn => Name,
                DisplayMode.TwoColumns => Name,
                DisplayMode.ThreeColumns => Name,
                DisplayMode.FourColumns => Name,
                DisplayMode.FiveColumns => Name,
                DisplayMode.SixColumns => Name,
                DisplayMode.SevenColumns => Name,
                DisplayMode.EightColumns => Name,
                DisplayMode.Thumbnail => Name,
                DisplayMode.Icon => Name,
                _ => Name
            };
        }

        private string FormatDetailView(int availableWidth)
        {
            // Fixed widths for size, date, and attributes
            const int sizeWidth = 12;
            const int dateWidth = 16;
            const int attrWidth = 4;
            const int spacing = 3; // spaces between columns
            
            // Calculate name width (remaining space)
            int nameWidth = Math.Max(20, availableWidth - sizeWidth - dateWidth - attrWidth - spacing);
            
            // Truncate name if longer than available width
            var displayName = TWF.Utilities.CharacterWidthHelper.SmartTruncate(Name, nameWidth);
            displayName = TWF.Utilities.CharacterWidthHelper.PadToWidth(displayName, nameWidth); // Pad to fixed width
            
            var sizeStr = IsDirectory ? "<DIR>" : FormatSize(Size);
            sizeStr = sizeStr.PadLeft(sizeWidth); // Pad to fixed width
            
            var dateStr = LastModified.ToString("yyyy-MM-dd HH:mm");
            dateStr = dateStr.PadLeft(dateWidth); // Pad to fixed width
            
            var attrStr = FormatAttributes();
            
            return $"{displayName} {sizeStr} {dateStr} {attrStr}";
        }

        private string FormatSize(long bytes)
        {
            if (bytes < 1024)
                return $"{bytes} B";
            if (bytes < 1024 * 1024)
                return $"{bytes / 1024.0:F1} KB";
            if (bytes < 1024 * 1024 * 1024)
                return $"{bytes / (1024.0 * 1024.0):F1} MB";
            return $"{bytes / (1024.0 * 1024.0 * 1024.0):F1} GB";
        }

        private string FormatAttributes()
        {
            var attrs = "";
            if (Attributes.HasFlag(FileAttributes.ReadOnly)) attrs += "R";
            if (Attributes.HasFlag(FileAttributes.Hidden)) attrs += "H";
            if (Attributes.HasFlag(FileAttributes.System)) attrs += "S";
            if (Attributes.HasFlag(FileAttributes.Archive)) attrs += "A";
            return attrs;
        }
    }
}
