using System.Runtime.InteropServices;
using Microsoft.Extensions.Logging;
using TWF.Models;
using TWF.Infrastructure;

namespace TWF.Services
{
    /// <summary>
    /// Handles incremental search with optional Migemo support for Japanese text
    /// </summary>
    public class SearchEngine
    {
        private readonly IMigemoProvider? _migemoProvider;
        private readonly ILogger<SearchEngine>? _logger;

        public SearchEngine(IMigemoProvider? migemoProvider = null)
        {
            _migemoProvider = migemoProvider;
            _logger = LoggingConfiguration.GetLogger<SearchEngine>();
            _logger?.LogInformation("SearchEngine initialized with Migemo provider: {HasProvider}", migemoProvider != null);
            if (migemoProvider != null)
            {
                _logger?.LogInformation("Migemo available: {IsAvailable}", migemoProvider.IsAvailable);
            }
        }

        /// <summary>
        /// Finds all entries matching the search pattern
        /// </summary>
        /// <param name="entries">List of file entries to search</param>
        /// <param name="searchPattern">Pattern to search for</param>
        /// <param name="useMigemo">Whether to use Migemo expansion if available</param>
        /// <returns>List of indices of matching entries</returns>
        public List<int> FindMatches(List<FileEntry> entries, string searchPattern, bool useMigemo)
        {
            if (entries == null || entries.Count == 0 || string.IsNullOrWhiteSpace(searchPattern))
            {
                return new List<int>();
            }

            var matches = new List<int>();
            string effectivePattern = searchPattern;

            // Use Migemo expansion if requested and available
            if (useMigemo && _migemoProvider?.IsAvailable == true)
            {
                effectivePattern = _migemoProvider.ExpandPattern(searchPattern);
            }

            for (int i = 0; i < entries.Count; i++)
            {
                if (MatchesPattern(entries[i].Name, effectivePattern, useMigemo))
                {
                    matches.Add(i);
                }
            }

            return matches;
        }

        /// <summary>
        /// Finds the next entry matching the search pattern starting from the current index
        /// </summary>
        /// <param name="entries">List of file entries to search</param>
        /// <param name="searchPattern">Pattern to search for</param>
        /// <param name="currentIndex">Current cursor position</param>
        /// <param name="useMigemo">Whether to use Migemo expansion if available</param>
        /// <returns>Index of next match, or -1 if no match found</returns>
        public int FindNext(List<FileEntry> entries, string searchPattern, int currentIndex, bool useMigemo)
        {
            if (entries == null || entries.Count == 0 || string.IsNullOrWhiteSpace(searchPattern))
            {
                return -1;
            }

            string effectivePattern = searchPattern;

            // Use Migemo expansion if requested and available
            if (useMigemo && _migemoProvider?.IsAvailable == true)
            {
                effectivePattern = _migemoProvider.ExpandPattern(searchPattern);
                _logger?.LogDebug("Migemo: Expanded '{OriginalPattern}' to '{ExpandedPattern}'", searchPattern, effectivePattern);
            }
            else
            {
                _logger?.LogDebug("Migemo: Not used (useMigemo={UseMigemo}, available={Available})", useMigemo, _migemoProvider?.IsAvailable);
            }

            // Search from current index + 1 to end
            for (int i = currentIndex + 1; i < entries.Count; i++)
            {
                if (MatchesPattern(entries[i].Name, effectivePattern, useMigemo))
                {
                    return i;
                }
            }

            // Wrap around: search from beginning to current index
            for (int i = 0; i <= currentIndex; i++)
            {
                if (MatchesPattern(entries[i].Name, effectivePattern, useMigemo))
                {
                    return i;
                }
            }

            return -1;
        }

        /// <summary>
        /// Finds the previous entry matching the search pattern starting from the current index
        /// </summary>
        /// <param name="entries">List of file entries to search</param>
        /// <param name="searchPattern">Pattern to search for</param>
        /// <param name="currentIndex">Current cursor position</param>
        /// <param name="useMigemo">Whether to use Migemo expansion if available</param>
        /// <returns>Index of previous match, or -1 if no match found</returns>
        public int FindPrevious(List<FileEntry> entries, string searchPattern, int currentIndex, bool useMigemo)
        {
            if (entries == null || entries.Count == 0 || string.IsNullOrWhiteSpace(searchPattern))
            {
                return -1;
            }

            string effectivePattern = searchPattern;

            // Use Migemo expansion if requested and available
            if (useMigemo && _migemoProvider?.IsAvailable == true)
            {
                effectivePattern = _migemoProvider.ExpandPattern(searchPattern);
            }

            // Search backwards from current index - 1 to beginning
            for (int i = currentIndex - 1; i >= 0; i--)
            {
                if (MatchesPattern(entries[i].Name, effectivePattern, useMigemo))
                {
                    return i;
                }
            }

            // Wrap around: search from end to current index
            for (int i = entries.Count - 1; i >= currentIndex; i--)
            {
                if (MatchesPattern(entries[i].Name, effectivePattern, useMigemo))
                {
                    return i;
                }
            }

            return -1;
        }

        /// <summary>
        /// Expands a pattern using Migemo if available
        /// </summary>
        public string ExpandPattern(string pattern)
        {
            if (_migemoProvider?.IsAvailable == true)
            {
                return _migemoProvider.ExpandPattern(pattern);
            }
            return pattern;
        }

        /// <summary>
        /// Checks if a text matches the search pattern
        /// </summary>
        public bool IsMatch(string text, string pattern, bool useMigemo = true)
        {
            if (string.IsNullOrEmpty(text) || string.IsNullOrEmpty(pattern)) return false;
            string effectivePattern = useMigemo ? ExpandPattern(pattern) : pattern;
            return MatchesPattern(text, effectivePattern, useMigemo);
        }

        /// <summary>
        /// Gets whether Migemo is available
        /// </summary>
        public bool IsMigemoAvailable => _migemoProvider?.IsAvailable == true;

        /// <summary>
        /// Checks if a filename matches the search pattern
        /// </summary>
        private bool MatchesPattern(string filename, string pattern, bool isMigemoPattern)
        {
            if (string.IsNullOrEmpty(filename) || string.IsNullOrEmpty(pattern))
            {
                return false;
            }

            // If it's a Migemo pattern, it's already a regex pattern
            if (isMigemoPattern && _migemoProvider?.IsAvailable == true)
            {
                try
                {
                    //_logger?.LogDebug("Migemo: Using regex match for '{Filename}' against pattern '{Pattern}'", filename, pattern);
                    return System.Text.RegularExpressions.Regex.IsMatch(
                        filename,
                        pattern,
                        System.Text.RegularExpressions.RegexOptions.IgnoreCase);
                }
                catch (Exception ex)
                {
                    _logger?.LogWarning(ex, "Migemo: Regex match failed, falling back to substring match");
                    // If regex fails, fall back to simple matching
                    return filename.StartsWith(pattern, StringComparison.OrdinalIgnoreCase);
                }
            }

            // Standard incremental search: check if filename contains pattern (substring match)
            _logger?.LogDebug("Standard search: Substring match for '{Filename}' contains '{Pattern}'", filename, pattern);
            return filename.Contains(pattern, StringComparison.OrdinalIgnoreCase);
        }
    }

    /// <summary>
    /// Interface for Migemo provider that converts romaji to Japanese text patterns
    /// </summary>
    public interface IMigemoProvider
    {
        /// <summary>
        /// Gets whether Migemo is available (DLL and dictionary files exist)
        /// </summary>
        bool IsAvailable { get; }

        /// <summary>
        /// Expands a romaji pattern to a regex pattern matching hiragana/katakana/kanji
        /// </summary>
        /// <param name="romajiPattern">Romaji input pattern</param>
        /// <returns>Regex pattern that matches Japanese text equivalents</returns>
        string ExpandPattern(string romajiPattern);
    }

    /// <summary>
    /// Migemo provider implementation that loads cmigemo library for Japanese text search
    /// Cross-platform: Works with migemo.dll (Windows), libmigemo.so (Linux), libmigemo.dylib (macOS)
    /// </summary>
    public class MigemoProvider : IMigemoProvider, IDisposable
    {
        private const string LibraryName = "migemo";
        private IntPtr _migemoHandle = IntPtr.Zero;
        private bool _isDisposed = false;
        private bool? _isAvailable;

        // P/Invoke declarations for cmigemo library
        // .NET automatically maps "migemo" to platform-specific library:
        // - Windows: migemo.dll
        // - Linux: libmigemo.so
        // - macOS: libmigemo.dylib
        [DllImport(LibraryName, CharSet = CharSet.Ansi, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr migemo_open(string dict_path);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void migemo_close(IntPtr handle);

        [DllImport(LibraryName, CharSet = CharSet.Ansi, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr migemo_query(IntPtr handle, string query);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void migemo_release(IntPtr handle, IntPtr result);

        public bool IsAvailable
        {
            get
            {
                if (_isAvailable.HasValue)
                {
                    return _isAvailable.Value;
                }

                _isAvailable = _migemoHandle != IntPtr.Zero;
                return _isAvailable.Value;
            }
        }

        /// <summary>
        /// Initializes Migemo provider with optional custom library and dictionary paths
        /// </summary>
        /// <param name="dictPath">Path to dictionary directory</param>
        public MigemoProvider(string? dictPath = null)
        {
            try
            {
                // Determine dictionary path with the following priority:
                // 1. Configured path (from config file, if provided and exists)
                // 2. User profile directory
                // 3. Executable's path
                // 4. Common system paths for Linux/macOS
                string effectiveDictPath = "dict"; // default fallback

                var logger = LoggingConfiguration.GetLogger<MigemoProvider>();
                logger?.LogDebug("MigemoProvider: Searching dictionary path: DictPath='{DictPath}'", dictPath);

                // First, try configured path (if provided and exists)
                if (!string.IsNullOrEmpty(dictPath))
                {
                    string configDictPath = dictPath;
                    if (!Path.IsPathRooted(configDictPath))
                    {
                        string appDir = AppDomain.CurrentDomain.BaseDirectory;
                        configDictPath = Path.Combine(appDir, configDictPath);
                    }

                    if (Directory.Exists(configDictPath))
                    {
                        effectiveDictPath = configDictPath;
                    }
                    else
                    {
                        logger?.LogDebug("MigemoProvider: Configured dictionary path '{DictPath}' does not exist", configDictPath);
                    }
                }

                // If configured path doesn't exist, try user profile directory
                if (!Directory.Exists(effectiveDictPath))
                {
                    /*
                    string userProfileDictPath = Path.Combine(
                        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                        ".twf"
                    ;
                    */

                    string userProfileDictPath = Path.Combine(
                        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                        "TWF","dict"
                    );

                    if (Directory.Exists(userProfileDictPath))
                    {
                        effectiveDictPath = userProfileDictPath;
                    }
                    else
                    {
                        logger?.LogDebug("MigemoProvider: Dictionary not found at userProfileDictPath '{userProfileDictPath}'", userProfileDictPath);
                    }
                }

                // If user profile doesn't exist, try executable's directory
                if (!Directory.Exists(effectiveDictPath))
                {
                    string appDir = AppDomain.CurrentDomain.BaseDirectory;
                    string appDictPath = Path.Combine(appDir, "dict");
                    if (Directory.Exists(appDictPath))
                    {
                        effectiveDictPath = appDictPath;
                    }
                    else
                    {
                        logger?.LogDebug("MigemoProvider: Dictionary not found at appDictPath '{appDictPath}'", appDictPath);

                        // If all else fails, try common Linux/macOS system paths
                        var systemPaths = new[]
                        {
                            "/usr/share/cmigemo/utf-8",
                            "/usr/local/share/migemo/utf-8",
                            "/opt/homebrew/share/migemo/utf-8"
                        };

                        foreach (var sysPath in systemPaths)
                        {
                            if (Directory.Exists(sysPath))
                            {
                                effectiveDictPath = sysPath;
                                break;
                            }
                            else
                            {
                                logger?.LogDebug("MigemoProvider: Dictionary not found at sysPath '{sysPath}'", sysPath);
                            }
                        }
                    }
                }

                // Check for utf-8 subdirectory (common in cmigemo distributions)
                string utf8DictPath = Path.Combine(effectiveDictPath, "utf-8");
                if (Directory.Exists(utf8DictPath))
                {
                    effectiveDictPath = utf8DictPath;
                }

                // Look for migemo-dict file
                string dictFile = Path.Combine(effectiveDictPath, "migemo-dict");
                if (!File.Exists(dictFile))
                {
                    //var logger = LoggingConfiguration.GetLogger<MigemoProvider>();
                    logger?.LogWarning("MigemoProvider: Dictionary file not found at '{DictFile}'", dictFile);
                    _isAvailable = false;
                    return;
                }

                // Initialize Migemo
                var initLogger = LoggingConfiguration.GetLogger<MigemoProvider>();
                initLogger?.LogInformation("MigemoProvider: Initializing with dictionary path '{DictPath}'", effectiveDictPath);
                initLogger?.LogInformation("MigemoProvider: Dictionary file exists at '{DictFile}'", dictFile);
                
                _migemoHandle = migemo_open(dictFile);
                
                initLogger?.LogInformation("MigemoProvider: migemo_open returned handle: {Handle}", _migemoHandle);
                bool isHandleValid = _migemoHandle != IntPtr.Zero;
                _isAvailable = isHandleValid;
                
                if (!isHandleValid)
                {
                    initLogger?.LogError("MigemoProvider: migemo_open failed - returned null handle");
                }
                else
                {
                    initLogger?.LogInformation("MigemoProvider: Successfully initialized Migemo");
                }
            }
            catch
            {
                _isAvailable = false;
                _migemoHandle = IntPtr.Zero;
            }
        }

        /// <summary>
        /// Expands a romaji pattern to a regex pattern matching Japanese text
        /// </summary>
        /// <param name="romajiPattern">Romaji input (e.g., "nihon")</param>
        /// <returns>Regex pattern (e.g., "(nihon|にほん|ニホン|日本)")</returns>
        public string ExpandPattern(string romajiPattern)
        {
            var logger = LoggingConfiguration.GetLogger<MigemoProvider>();
            
            if (!IsAvailable || string.IsNullOrWhiteSpace(romajiPattern))
            {
                logger?.LogDebug("MigemoProvider: Cannot expand - IsAvailable={IsAvailable}, pattern empty={IsEmpty}", IsAvailable, string.IsNullOrWhiteSpace(romajiPattern));
                return romajiPattern;
            }

            if (_migemoHandle == IntPtr.Zero)
            {
                logger?.LogWarning("MigemoProvider: Migemo handle is null/zero");
                return romajiPattern;
            }

            try
            {
                logger?.LogDebug("MigemoProvider: Calling migemo_query with pattern '{Pattern}'", romajiPattern);
                
                // Call migemo_query to expand the pattern
                IntPtr resultPtr = migemo_query(_migemoHandle, romajiPattern);
                
                logger?.LogDebug("MigemoProvider: migemo_query returned pointer: {Pointer}", resultPtr);
                
                if (resultPtr == IntPtr.Zero)
                {
                    logger?.LogWarning("MigemoProvider: migemo_query returned null pointer");
                    return romajiPattern;
                }

                // Convert result to string (UTF-8 encoding)
                string? expandedPattern;
                
                // Use Marshal.PtrToStringUTF8 if available (.NET 7+), otherwise use Ansi
                #if NET7_0_OR_GREATER
                expandedPattern = Marshal.PtrToStringUTF8(resultPtr);
                logger?.LogDebug("MigemoProvider: Used PtrToStringUTF8");
                #else
                expandedPattern = Marshal.PtrToStringAnsi(resultPtr);
                logger?.LogDebug("MigemoProvider: Used PtrToStringAnsi");
                #endif

                logger?.LogDebug("MigemoProvider: Expanded pattern result: '{Result}'", expandedPattern);

                // Release the result memory
                migemo_release(_migemoHandle, resultPtr);

                return expandedPattern ?? romajiPattern;
            }
            catch (Exception ex)
            {
                logger?.LogError(ex, "MigemoProvider: Exception in ExpandPattern");
                // If anything fails, fall back to original pattern
                return romajiPattern;
            }
        }

        /// <summary>
        /// Disposes of Migemo resources
        /// </summary>
        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing)
        {
            if (!_isDisposed)
            {
                if (_migemoHandle != IntPtr.Zero)
                {
                    try
                    {
                        migemo_close(_migemoHandle);
                    }
                    catch
                    {
                        // Ignore errors during cleanup
                    }
                    _migemoHandle = IntPtr.Zero;
                }

                _isDisposed = true;
            }
        }

        ~MigemoProvider()
        {
            Dispose(false);
        }
    }
}
