namespace TWF.Models
{
    /// <summary>
    /// Maintains the state of a single file pane
    /// </summary>
    public class PaneState
    {
        public string CurrentPath { get; set; } = string.Empty;
        public List<FileEntry> Entries { get; set; } = new List<FileEntry>();
        // MarkedIndices removed in favor of FileEntry.IsMarked
        public int CursorPosition { get; set; }
        public int ScrollOffset { get; set; }
        public string FileMask { get; set; } = "*";
        public SortMode SortMode { get; set; } = SortMode.NameAscending;
        public DisplayMode DisplayMode { get; set; } = DisplayMode.Details;
        
        // Virtual folder support for archives
        public bool IsInVirtualFolder { get; set; } = false;
        public string? VirtualFolderArchivePath { get; set; } = null;
        public string? VirtualFolderParentPath { get; set; } = null;

        // Directory and file counts
        public int DirectoryCount { get; set; } = 0;
        public int FileCount { get; set; } = 0;

        /// <summary>
        /// Loads directory contents into the pane
        /// </summary>
        public void LoadDirectory(string path)
        {
            CurrentPath = path;
            // Implementation will be added when FileSystemProvider is implemented
        }

        /// <summary>
        /// Applies a file mask filter to the entries
        /// </summary>
        public void ApplyFileMask(string mask)
        {
            FileMask = mask;
            // Implementation will be added when filtering logic is implemented
        }

        /// <summary>
        /// Applies sorting to the entries
        /// </summary>
        public void ApplySort(SortMode mode)
        {
            SortMode = mode;
            // Implementation will be added when SortEngine is implemented
        }

        /// <summary>
        /// Gets the file entry at the current cursor position
        /// </summary>
        public FileEntry? GetCurrentEntry()
        {
            if (CursorPosition >= 0 && CursorPosition < Entries.Count)
                return Entries[CursorPosition];
            return null;
        }

        /// <summary>
        /// Gets all marked file entries
        /// </summary>
        public List<FileEntry> GetMarkedEntries()
        {
            return Entries.Where(e => e.IsMarked).ToList();
        }
    }
}
