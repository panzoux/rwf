using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    /// Handles sorting of file lists according to various criteria
    /// </summary>
    public class SortEngine
    {
        /// <summary>
        /// Sorts a list of file entries according to the specified sort mode
        /// </summary>
        /// <param name="entries">The list of file entries to sort</param>
        /// <param name="mode">The sort mode to apply</param>
        /// <returns>A new sorted list of file entries</returns>
        public static List<FileEntry> Sort(List<FileEntry> entries, SortMode mode)
        {
            if (entries == null || entries.Count == 0)
            {
                return new List<FileEntry>();
            }

            // Create a copy to avoid modifying the original list
            var sorted = new List<FileEntry>(entries);

            switch (mode)
            {
                case SortMode.Unsorted:
                    // Return as-is
                    break;

                case SortMode.NameAscending:
                    sorted = SortByName(sorted, ascending: true);
                    break;

                case SortMode.NameDescending:
                    sorted = SortByName(sorted, ascending: false);
                    break;

                case SortMode.ExtensionAscending:
                    sorted = SortByExtension(sorted, ascending: true);
                    break;

                case SortMode.ExtensionDescending:
                    sorted = SortByExtension(sorted, ascending: false);
                    break;

                case SortMode.SizeAscending:
                    sorted = SortBySize(sorted, ascending: true);
                    break;

                case SortMode.SizeDescending:
                    sorted = SortBySize(sorted, ascending: false);
                    break;

                case SortMode.DateAscending:
                    sorted = SortByDate(sorted, ascending: true);
                    break;

                case SortMode.DateDescending:
                    sorted = SortByDate(sorted, ascending: false);
                    break;

                default:
                    break;
            }

            return sorted;
        }

        /// <summary>
        /// Sorts by name with directories appearing before files
        /// </summary>
        private static List<FileEntry> SortByName(List<FileEntry> entries, bool ascending)
        {
            if (ascending)
            {
                return entries
                    .OrderBy(e => !e.IsDirectory) // Directories first
                    .ThenBy(e => e.Name, StringComparer.OrdinalIgnoreCase)
                    .ToList();
            }
            else
            {
                return entries
                    .OrderBy(e => !e.IsDirectory) // Directories first
                    .ThenByDescending(e => e.Name, StringComparer.OrdinalIgnoreCase)
                    .ToList();
            }
        }

        /// <summary>
        /// Sorts by extension
        /// </summary>
        private static List<FileEntry> SortByExtension(List<FileEntry> entries, bool ascending)
        {
            if (ascending)
            {
                return entries
                    .OrderBy(e => e.Extension, StringComparer.OrdinalIgnoreCase)
                    .ThenBy(e => e.Name, StringComparer.Ordinal)
                    .ToList();
            }
            else
            {
                return entries
                    .OrderByDescending(e => e.Extension, StringComparer.OrdinalIgnoreCase)
                    .ThenBy(e => e.Name, StringComparer.Ordinal)
                    .ToList();
            }
        }

        /// <summary>
        /// Sorts by file size
        /// </summary>
        private static List<FileEntry> SortBySize(List<FileEntry> entries, bool ascending)
        {
            if (ascending)
            {
                return entries
                    .OrderBy(e => e.Size)
                    .ThenBy(e => e.Name, StringComparer.Ordinal)
                    .ToList();
            }
            else
            {
                return entries
                    .OrderByDescending(e => e.Size)
                    .ThenBy(e => e.Name, StringComparer.Ordinal)
                    .ToList();
            }
        }

        /// <summary>
        /// Sorts by last modified date
        /// </summary>
        private static List<FileEntry> SortByDate(List<FileEntry> entries, bool ascending)
        {
            if (ascending)
            {
                return entries
                    .OrderBy(e => e.LastModified)
                    .ThenBy(e => e.Name, StringComparer.Ordinal)
                    .ToList();
            }
            else
            {
                return entries
                    .OrderByDescending(e => e.LastModified)
                    .ThenBy(e => e.Name, StringComparer.Ordinal)
                    .ToList();
            }
        }
    }
}
