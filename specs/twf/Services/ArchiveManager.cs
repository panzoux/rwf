namespace TWF.Services
{
    using TWF.Models;

    /// <summary>
    /// Manages archive operations and provider registration
    /// </summary>
    public class ArchiveManager
    {
        private readonly Dictionary<string, IArchiveProvider> _providers = new();

        public ArchiveManager()
        {
            // Register built-in ZIP provider
            RegisterProvider(new ZipArchiveProvider());
        }

        /// <summary>
        /// Registers an archive provider for its supported extensions
        /// </summary>
        public void RegisterProvider(IArchiveProvider provider)
        {
            foreach (var extension in provider.SupportedExtensions)
            {
                var normalizedExt = extension.ToLowerInvariant();
                if (!normalizedExt.StartsWith("."))
                {
                    normalizedExt = "." + normalizedExt;
                }
                _providers[normalizedExt] = provider;
            }
        }

        /// <summary>
        /// Checks if a file is a supported archive based on its extension
        /// </summary>
        public bool IsArchive(string path)
        {
            var extension = Path.GetExtension(path).ToLowerInvariant();
            return _providers.ContainsKey(extension);
        }

        /// <summary>
        /// Gets a list of all supported archive extensions
        /// </summary>
        public IEnumerable<string> GetSupportedArchiveExtensions()
        {
            return _providers.Keys.OrderBy(e => e);
        }

        /// <summary>
        /// Gets a list of all supported archive formats
        /// </summary>
        public List<ArchiveFormat> GetSupportedFormats()
        {
            var formats = new HashSet<ArchiveFormat>();
            foreach (var ext in _providers.Keys)
            {
                var format = ext.ToLowerInvariant() switch
                {
                    ".zip" => ArchiveFormat.ZIP,
                    ".tar" => ArchiveFormat.TAR,
                    ".tar.gz" => ArchiveFormat.TGZ,
                    ".tgz" => ArchiveFormat.TGZ,
                    ".7z" => ArchiveFormat.SevenZip,
                    ".rar" => ArchiveFormat.RAR,
                    ".lzh" => ArchiveFormat.LZH,
                    ".cab" => ArchiveFormat.CAB,
                    ".bz2" => ArchiveFormat.BZ2,
                    ".xz" => ArchiveFormat.XZ,
                    ".lzma" => ArchiveFormat.LZMA,
                    _ => (ArchiveFormat?)null
                };
                if (format.HasValue) formats.Add(format.Value);
            }
            return formats.OrderBy(f => f.ToString()).ToList();
        }

        /// <summary>
        /// Lists the contents of an archive file as a virtual folder
        /// </summary>
        public List<FileEntry> ListArchiveContents(string archivePath)
        {
            if (!File.Exists(archivePath))
            {
                throw new FileNotFoundException($"Archive file not found: {archivePath}");
            }

            var extension = Path.GetExtension(archivePath).ToLowerInvariant();
            
            if (!_providers.TryGetValue(extension, out var provider))
            {
                throw new NotSupportedException($"Archive format not supported: {extension}");
            }

            return provider.List(archivePath);
        }

        /// <summary>
        /// Lists the contents of an archive file asynchronously
        /// </summary>
        public Task<List<FileEntry>> ListArchiveContentsAsync(string archivePath, CancellationToken cancellationToken = default)
        {
            return Task.Run(() => 
            {
                cancellationToken.ThrowIfCancellationRequested();
                return ListArchiveContents(archivePath);
            }, cancellationToken);
        }

        /// <summary>
        /// Extracts specific entries from an archive file asynchronously
        /// </summary>
        public async Task<OperationResult> ExtractEntriesAsync(
            string archivePath, 
            List<string> entryNames, 
            string destination, 
            CancellationToken cancellationToken = default)
        {
            var extension = Path.GetExtension(archivePath).ToLowerInvariant();
            if (!_providers.TryGetValue(extension, out var provider))
            {
                return new OperationResult { Success = false, Message = "Unsupported archive format" };
            }

            return await provider.ExtractEntries(archivePath, entryNames, destination, cancellationToken);
        }

        /// <summary>
        /// Deletes specific entries from an archive file asynchronously
        /// </summary>
        public async Task<OperationResult> DeleteEntriesAsync(
            string archivePath, 
            List<string> entryNames, 
            CancellationToken cancellationToken = default)
        {
            var extension = Path.GetExtension(archivePath).ToLowerInvariant();
            if (!_providers.TryGetValue(extension, out var provider))
            {
                return new OperationResult { Success = false, Message = "Unsupported archive format" };
            }

            return await provider.DeleteEntries(archivePath, entryNames, cancellationToken);
        }

        /// <summary>
        /// Extracts an archive to a destination directory
        /// </summary>
        public async Task<OperationResult> ExtractAsync(
            string archivePath, 
            string destination, 
            CancellationToken cancellationToken = default)
        {
            if (!File.Exists(archivePath))
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Archive file not found: {archivePath}",
                    Errors = new List<string> { "File not found" }
                };
            }

            var extension = Path.GetExtension(archivePath).ToLowerInvariant();
            
            if (!_providers.TryGetValue(extension, out var provider))
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Archive format not supported: {extension}",
                    Errors = new List<string> { "Unsupported format" }
                };
            }

            return await provider.Extract(archivePath, destination, cancellationToken);
        }

        /// <summary>
        /// Compresses files into an archive
        /// </summary>
        public async Task<OperationResult> CompressAsync(
            List<FileEntry> sources, 
            string archivePath, 
            ArchiveFormat format, 
            CancellationToken cancellationToken = default)
        {
            // Determine extension from format
            var extension = format switch
            {
                ArchiveFormat.ZIP => ".zip",
                ArchiveFormat.TAR => ".tar",
                ArchiveFormat.TGZ => ".tar.gz",
                ArchiveFormat.SevenZip => ".7z",
                ArchiveFormat.RAR => ".rar",
                ArchiveFormat.LZH => ".lzh",
                ArchiveFormat.CAB => ".cab",
                ArchiveFormat.BZ2 => ".bz2",
                ArchiveFormat.XZ => ".xz",
                ArchiveFormat.LZMA => ".lzma",
                _ => ".zip"
            };

            if (!_providers.TryGetValue(extension, out var provider))
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Archive format not supported: {format}",
                    Errors = new List<string> { "Unsupported format" }
                };
            }

            // Convert FileEntry list to path list
            var sourcePaths = sources.Select(e => e.FullPath).ToList();

            return await provider.Compress(sourcePaths, archivePath, cancellationToken);
        }
    }
}
