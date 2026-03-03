using Microsoft.Extensions.Logging;
using TWF.Infrastructure;
using TWF.Models;

namespace TWF.Providers
{
    /// <summary>
    /// Provides access to the file system for directory and file operations
    /// </summary>
    public class FileSystemProvider
    {
        private readonly ILogger<FileSystemProvider> _logger;
        private static readonly HashSet<string> ArchiveExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".lzh", ".zip", ".tar", ".tgz", ".gz", ".cab", ".rar", ".7z", ".bz2", ".xz", ".lzma"
        };

        public FileSystemProvider()
        {
            _logger = LoggingConfiguration.GetLogger<FileSystemProvider>();
        }

        /// <summary>
        /// Enumerates directory contents asynchronously using a lightweight struct.
        /// Optimized for performance and memory usage.
        /// </summary>
        public async IAsyncEnumerable<FileSystemItem> EnumerateDirectoryAsync(string path, [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default)
        {
            await Task.Yield();
            if (string.IsNullOrWhiteSpace(path) || !Directory.Exists(path))
            {
                yield break;
            }

            var directoryInfo = new DirectoryInfo(path);

            // First yield directories
            IEnumerable<DirectoryInfo>? dirs = null;
            try
            {
                dirs = directoryInfo.EnumerateDirectories();
            }
            catch (UnauthorizedAccessException) { }
            catch (Exception ex) 
            {
                 _logger.LogError(ex, "Error enumerating directories in {Path}", path);
            }

            if (dirs != null)
            {
                foreach (var dir in dirs)
                {
                    if (cancellationToken.IsCancellationRequested) yield break;

                    FileSystemItem item;
                    try
                    {
                         item = new FileSystemItem(
                            dir.FullName,
                            dir.Name,
                            true,
                            0,
                            dir.LastWriteTime,
                            dir.Attributes
                        );
                    }
                    catch
                    {
                        continue;
                    }
                    yield return item;
                }
            }

            // Then yield files
            IEnumerable<FileInfo>? files = null;
            try
            {
                files = directoryInfo.EnumerateFiles();
            }
            catch (UnauthorizedAccessException) { }
            catch (Exception ex)
            {
                 _logger.LogError(ex, "Error enumerating files in {Path}", path);
            }

            if (files != null)
            {
                foreach (var file in files)
                {
                    if (cancellationToken.IsCancellationRequested) yield break;

                    FileSystemItem item;
                    try
                    {
                        item = new FileSystemItem(
                            file.FullName,
                            file.Name,
                            false,
                            file.Length,
                            file.LastWriteTime,
                            file.Attributes
                        );
                    }
                    catch
                    {
                        continue;
                    }
                    yield return item;
                }
            }
        }

        /// <summary>
        /// Lists all files and directories in the specified path
        /// </summary>
        /// <param name="path">The directory path to list</param>
        /// <returns>List of FileEntry objects, or empty list on error</returns>
        public List<FileEntry> ListDirectory(string path)
        {
            var entries = new List<FileEntry>();

            try
            {
                if (string.IsNullOrWhiteSpace(path))
                {
                    _logger.LogWarning("ListDirectory called with null or empty path");
                    return entries;
                }

                if (!Directory.Exists(path))
                {
                    _logger.LogError("Directory not found: {Path}", path);
                    return entries;
                }

                // Get directories
                try
                {
                    var directories = Directory.GetDirectories(path);
                    foreach (var dir in directories)
                    {
                        try
                        {
                            var entry = CreateFileEntryFromDirectory(dir);
                            entries.Add(entry);
                        }
                        catch (UnauthorizedAccessException ex)
                        {
                            _logger.LogWarning(ex, "Access denied to directory: {Directory}", dir);
                            // Add entry with limited information
                            entries.Add(CreateLimitedAccessEntry(dir, isDirectory: true));
                        }
                        catch (Exception ex)
                        {
                            _logger.LogError(ex, "Error reading directory: {Directory}", dir);
                        }
                    }
                }
                catch (UnauthorizedAccessException ex)
                {
                    _logger.LogError(ex, "Access denied when listing directories in: {Path}", path);
                }

                // Get files
                try
                {
                    var files = Directory.GetFiles(path);
                    foreach (var file in files)
                    {
                        try
                        {
                            var entry = CreateFileEntryFromFile(file);
                            entries.Add(entry);
                        }
                        catch (UnauthorizedAccessException ex)
                        {
                            _logger.LogWarning(ex, "Access denied to file: {File}", file);
                            // Add entry with limited information
                            entries.Add(CreateLimitedAccessEntry(file, isDirectory: false));
                        }
                        catch (Exception ex)
                        {
                            _logger.LogError(ex, "Error reading file: {File}", file);
                        }
                    }
                }
                catch (UnauthorizedAccessException ex)
                {
                    _logger.LogError(ex, "Access denied when listing files in: {Path}", path);
                }
            }
            catch (DirectoryNotFoundException ex)
            {
                _logger.LogError(ex, "Directory not found: {Path}", path);
            }
            catch (PathTooLongException ex)
            {
                _logger.LogError(ex, "Path too long: {Path}", path);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Unexpected error listing directory: {Path}", path);
            }

            return entries;
        }

        /// <summary>
        /// Gets file metadata for a specific file
        /// </summary>
        /// <param name="filePath">The file path</param>
        /// <returns>FileEntry with metadata, or null on error</returns>
        public FileEntry? GetFileMetadata(string filePath)
        {
            try
            {
                if (string.IsNullOrWhiteSpace(filePath))
                {
                    _logger.LogWarning("GetFileMetadata called with null or empty path");
                    return null;
                }

                if (Directory.Exists(filePath))
                {
                    return CreateFileEntryFromDirectory(filePath);
                }
                else if (File.Exists(filePath))
                {
                    return CreateFileEntryFromFile(filePath);
                }
                else
                {
                    _logger.LogError("File or directory not found: {Path}", filePath);
                    return null;
                }
            }
            catch (UnauthorizedAccessException ex)
            {
                _logger.LogError(ex, "Access denied: {Path}", filePath);
                return null;
            }
            catch (PathTooLongException ex)
            {
                _logger.LogError(ex, "Path too long: {Path}", filePath);
                return null;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting file metadata: {Path}", filePath);
                return null;
            }
        }

        /// <summary>
        /// Checks if a path exists and is accessible
        /// </summary>
        /// <param name="path">The path to check</param>
        /// <returns>True if path exists and is accessible, false otherwise</returns>
        public bool PathExists(string path)
        {
            try
            {
                return Directory.Exists(path) || File.Exists(path);
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Error checking path existence: {Path}", path);
                return false;
            }
        }

        /// <summary>
        /// Gets the parent directory path
        /// </summary>
        /// <param name="path">The current path</param>
        /// <returns>Parent directory path, or null if at root</returns>
        public string? GetParentDirectory(string path)
        {
            try
            {
                if (string.IsNullOrWhiteSpace(path))
                {
                    return null;
                }

                var directoryInfo = new DirectoryInfo(path);
                return directoryInfo.Parent?.FullName;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting parent directory: {Path}", path);
                return null;
            }
        }

        /// <summary>
        /// Creates a FileEntry from a directory path
        /// </summary>
        private FileEntry CreateFileEntryFromDirectory(string dirPath)
        {
            var dirInfo = new DirectoryInfo(dirPath);
            
            return new FileEntry
            {
                FullPath = dirInfo.FullName,
                Name = dirInfo.Name,
                Extension = string.Empty,
                Size = 0,
                LastModified = dirInfo.LastWriteTime,
                Attributes = dirInfo.Attributes,
                IsDirectory = true,
                IsArchive = false,
                IsVirtualFolder = false
            };
        }

        /// <summary>
        /// Creates a FileEntry from a file path
        /// </summary>
        private FileEntry CreateFileEntryFromFile(string filePath)
        {
            var fileInfo = new FileInfo(filePath);
            var extension = fileInfo.Extension;
            var isArchive = ArchiveExtensions.Contains(extension);

            return new FileEntry
            {
                FullPath = fileInfo.FullName,
                Name = fileInfo.Name,
                Extension = extension,
                Size = fileInfo.Length,
                LastModified = fileInfo.LastWriteTime,
                Attributes = fileInfo.Attributes,
                IsDirectory = false,
                IsArchive = isArchive,
                IsVirtualFolder = false
            };
        }

        /// <summary>
        /// Creates a limited FileEntry for files/directories with access restrictions
        /// </summary>
        private FileEntry CreateLimitedAccessEntry(string path, bool isDirectory)
        {
            var name = Path.GetFileName(path);
            
            return new FileEntry
            {
                FullPath = path,
                Name = name ?? path,
                Extension = isDirectory ? string.Empty : Path.GetExtension(path),
                Size = 0,
                LastModified = DateTime.MinValue,
                Attributes = FileAttributes.Normal,
                IsDirectory = isDirectory,
                IsArchive = false,
                IsVirtualFolder = false
            };
        }

        /// <summary>
        /// Gets directory entries (alias for ListDirectory for compatibility)
        /// </summary>
        public List<FileEntry> GetDirectoryEntries(string path)
        {
            return ListDirectory(path);
        }

        /// <summary>
        /// Applies a file mask filter to a list of entries
        /// </summary>
        /// <param name="entries">List of file entries to filter</param>
        /// <param name="mask">File mask pattern (e.g., "*.txt" or "*.txt *.doc")</param>
        /// <returns>Filtered list of entries</returns>
        public List<FileEntry> ApplyFileMask(List<FileEntry> entries, string mask)
        {
            if (string.IsNullOrWhiteSpace(mask) || mask == "*")
            {
                return entries;
            }

            try
            {
                // Split mask by spaces to support multiple patterns
                var patterns = mask.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                var inclusionPatterns = new List<string>();
                var exclusionPatterns = new List<string>();

                foreach (var pattern in patterns)
                {
                    if (pattern.StartsWith(":"))
                    {
                        // Exclusion pattern
                        exclusionPatterns.Add(pattern.Substring(1));
                    }
                    else
                    {
                        // Inclusion pattern
                        inclusionPatterns.Add(pattern);
                    }
                }

                var filtered = entries.Where(entry =>
                {
                    // Always include directories
                    if (entry.IsDirectory)
                    {
                        return true;
                    }

                    // Check inclusion patterns
                    bool included = inclusionPatterns.Count == 0 || 
                                  inclusionPatterns.Any(p => MatchesWildcard(entry.Name, p));

                    // Check exclusion patterns
                    bool excluded = exclusionPatterns.Any(p => MatchesWildcard(entry.Name, p));

                    return included && !excluded;
                }).ToList();

                return filtered;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error applying file mask: {Mask}", mask);
                return entries;
            }
        }

        /// <summary>
        /// Checks if a filename matches a pattern (wildcard or regex)
        /// Supports both traditional wildcards (*, ?) and regex patterns (enclosed in /pattern/)
        /// </summary>
        private bool MatchesWildcard(string filename, string pattern)
        {
            if (string.IsNullOrEmpty(pattern) || pattern == "*")
            {
                return true;
            }

            // Check if this is a regex pattern (enclosed in forward slashes)
            if (pattern.Length > 2 && pattern.StartsWith("/") && pattern.EndsWith("/"))
            {
                // Extract the regex pattern (without the surrounding slashes)
                string regexPattern = pattern.Substring(1, pattern.Length - 2);

                // Handle case-insensitive flag (/pattern/i)
                bool isCaseInsensitive = false;
                if (regexPattern.EndsWith("i") && regexPattern.Length > 1 && regexPattern[regexPattern.Length - 2] != '\\')
                {
                    regexPattern = regexPattern.Substring(0, regexPattern.Length - 1);
                    isCaseInsensitive = true;
                }

                try
                {
                    var options = isCaseInsensitive ?
                        System.Text.RegularExpressions.RegexOptions.IgnoreCase :
                        System.Text.RegularExpressions.RegexOptions.None;

                    return System.Text.RegularExpressions.Regex.IsMatch(
                        filename,
                        regexPattern,
                        options);
                }
                catch (ArgumentException ex)
                {
                    _logger.LogWarning(ex, "Invalid regex pattern: {Pattern}", regexPattern);
                    return false;
                }
            }
            else
            {
                // Handle as traditional wildcard pattern
                try
                {
                    // Convert wildcard pattern to regex
                    var regexPattern = "^" + System.Text.RegularExpressions.Regex.Escape(pattern)
                        .Replace("\\*", ".*")
                        .Replace("\\?", ".") + "$";

                    return System.Text.RegularExpressions.Regex.IsMatch(
                        filename,
                        regexPattern,
                        System.Text.RegularExpressions.RegexOptions.IgnoreCase);
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "Invalid wildcard pattern: {Pattern}", pattern);
                    return false;
                }
            }
        }
    }
}
