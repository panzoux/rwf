namespace TWF.Services
{
    using System.IO.Compression;
    using TWF.Models;
    using Microsoft.Extensions.Logging;
    using TWF.Infrastructure;

    /// <summary>
    /// Archive provider for ZIP format using System.IO.Compression
    /// </summary>
    public class ZipArchiveProvider : IArchiveProvider
    {
        private readonly ILogger<ZipArchiveProvider> _logger;

        public ZipArchiveProvider()
        {
            _logger = LoggingConfiguration.GetLogger<ZipArchiveProvider>();
        }

        public string[] SupportedExtensions => new[] { ".zip" };

        public List<FileEntry> List(string archivePath)
        {
            var entries = new List<FileEntry>();

            try
            {
                using var archive = ZipFile.OpenRead(archivePath);
                foreach (var entry in archive.Entries)
                {
                    // Skip directory entries (they end with /)
                    if (entry.FullName.EndsWith("/"))
                        continue;

                    entries.Add(new FileEntry
                    {
                        FullPath = Path.Combine(archivePath, entry.FullName),
                        Name = entry.Name,
                        Extension = Path.GetExtension(entry.Name),
                        Size = entry.Length,
                        LastModified = entry.LastWriteTime.DateTime,
                        Attributes = FileAttributes.Normal,
                        IsDirectory = false,
                        IsArchive = false,
                        IsVirtualFolder = true
                    });
                }
            }
            catch (Exception ex)
            {
                throw new InvalidOperationException($"Failed to list archive contents: {ex.Message}", ex);
            }

            return entries;
        }

        public async Task<OperationResult> Extract(string archivePath, string destination, CancellationToken cancellationToken)
        {
            var startTime = DateTime.Now;
            var result = new OperationResult { Success = true };

            try
            {
                // Ensure destination directory exists
                Directory.CreateDirectory(destination);

                using var archive = ZipFile.OpenRead(archivePath);
                var totalEntries = archive.Entries.Count;
                var processedEntries = 0;

                foreach (var entry in archive.Entries)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Success = false;
                        result.Message = "Extraction cancelled by user";
                        break;
                    }

                    // Skip directory entries
                    if (entry.FullName.EndsWith("/"))
                        continue;

                    var destinationPath = Path.Combine(destination, entry.FullName);
                    
                    // Ensure the directory exists
                    var directoryPath = Path.GetDirectoryName(destinationPath);
                    if (!string.IsNullOrEmpty(directoryPath))
                    {
                        Directory.CreateDirectory(directoryPath);
                    }

                    // Extract the file
                    await Task.Run(() => entry.ExtractToFile(destinationPath, overwrite: true), cancellationToken);
                    
                    processedEntries++;
                }

                result.FilesProcessed = processedEntries;
                result.Message = $"Extracted {processedEntries} files from archive";
            }
            catch (OperationCanceledException)
            {
                result.Success = false;
                result.Message = "Extraction cancelled by user";
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Extraction failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            result.Duration = DateTime.Now - startTime;
            return result;
        }

        public async Task<OperationResult> ExtractEntries(string archivePath, List<string> entryNames, string destination, CancellationToken cancellationToken)
        {
            var startTime = DateTime.Now;
            var result = new OperationResult { Success = true };

            try
            {
                _logger.LogDebug("ExtractEntries started: archive={ArchivePath}, destination={Destination}, entryCount={Count}", archivePath, destination, entryNames.Count);
                Directory.CreateDirectory(destination);

                using var archive = ZipFile.OpenRead(archivePath);
                var processedEntries = 0;
                
                // Normalize targets: forward slashes and no trailing slash
                var targets = entryNames.Select(n => n.Replace('\\', '/').TrimEnd('/')).ToList();
                _logger.LogDebug("Normalized targets: {Targets}", string.Join(", ", targets));

                foreach (var entry in archive.Entries)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Success = false;
                        result.Message = "Operation cancelled by user";
                        _logger.LogInformation("Extraction cancelled by user");
                        break;
                    }

                    // Normalize entry name for comparison (no trailing slash)
                    string normalizedEntryName = entry.FullName.Replace('\\', '/').TrimEnd('/');

                    // Find which target matches this entry
                    string? matchingTarget = targets.FirstOrDefault(t => 
                        normalizedEntryName.Equals(t, StringComparison.OrdinalIgnoreCase) || 
                        normalizedEntryName.StartsWith(t + "/", StringComparison.OrdinalIgnoreCase));

                    if (matchingTarget == null || entry.FullName.EndsWith("/")) continue;

                    // Calculate destination path relative to the target's parent
                    // e.g. If target is "Tests/a.txt", we strip "Tests/" to get "a.txt"
                    string relativePath;
                    int lastSlash = matchingTarget.LastIndexOf('/');
                    if (lastSlash >= 0)
                    {
                        string prefixToStrip = matchingTarget.Substring(0, lastSlash + 1);
                        relativePath = entry.FullName.Substring(prefixToStrip.Length);
                    }
                    else
                    {
                        relativePath = entry.FullName;
                    }

                    var destinationPath = Path.Combine(destination, relativePath);
                    var directoryPath = Path.GetDirectoryName(destinationPath);
                    if (!string.IsNullOrEmpty(directoryPath))
                    {
                        Directory.CreateDirectory(directoryPath);
                    }

                    _logger.LogDebug("Extracting entry: {EntryName} -> {DestPath} (Stripped relative: {Relative})", 
                        entry.FullName, destinationPath, relativePath);
                    
                    await Task.Run(() => entry.ExtractToFile(destinationPath, overwrite: true), cancellationToken);
                    processedEntries++;
                }

                result.FilesProcessed = processedEntries;
                result.Message = $"Extracted {processedEntries} items from archive";
                _logger.LogInformation("Extraction completed: {Message}", result.Message);
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Extraction failed: {ex.Message}";
                result.Errors.Add(ex.Message);
                _logger.LogError(ex, "Extraction failed for {ArchivePath}", archivePath);
            }

            result.Duration = DateTime.Now - startTime;
            return result;
        }

        public async Task<OperationResult> DeleteEntries(string archivePath, List<string> entryNames, CancellationToken cancellationToken)
        {
            var startTime = DateTime.Now;
            var result = new OperationResult { Success = true };

            try
            {
                _logger.LogDebug("DeleteEntries started: archive={ArchivePath}, entryCount={Count}", archivePath, entryNames.Count);
                using var archive = ZipFile.Open(archivePath, ZipArchiveMode.Update);
                var processedEntries = 0;
                
                // Normalize targets: forward slashes and no trailing slash
                var targets = entryNames.Select(n => n.Replace('\\', '/').TrimEnd('/')).ToList();
                _logger.LogDebug("Normalized targets: {Targets}", string.Join(", ", targets));

                var toDelete = new List<ZipArchiveEntry>();
                foreach (var entry in archive.Entries)
                {
                    // Normalize entry name for comparison (no trailing slash)
                    string normalizedEntryName = entry.FullName.Replace('\\', '/').TrimEnd('/');

                    bool matches = targets.Any(t => 
                        normalizedEntryName.Equals(t, StringComparison.OrdinalIgnoreCase) || 
                        normalizedEntryName.StartsWith(t + "/", StringComparison.OrdinalIgnoreCase));
                    
                    if (matches)
                    {
                        toDelete.Add(entry);
                    }
                }

                _logger.LogDebug("Identified {Count} entries to delete in archive", toDelete.Count);

                foreach (var entry in toDelete)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Success = false;
                        result.Message = "Operation cancelled by user";
                        _logger.LogInformation("Deletion cancelled by user");
                        break;
                    }

                    _logger.LogDebug("Deleting entry: {EntryName}", entry.FullName);
                    await Task.Run(() => entry.Delete(), cancellationToken);
                    processedEntries++;
                }

                result.FilesProcessed = processedEntries;
                result.Message = $"Deleted {processedEntries} items from archive";
                _logger.LogInformation("Deletion completed: {Message}", result.Message);
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Deletion failed: {ex.Message}";
                result.Errors.Add(ex.Message);
                _logger.LogError(ex, "Deletion failed for {ArchivePath}", archivePath);
            }

            result.Duration = DateTime.Now - startTime;
            return result;
        }

        public async Task<OperationResult> Compress(List<string> sources, string archivePath, CancellationToken cancellationToken)
        {
            var startTime = DateTime.Now;
            var result = new OperationResult { Success = true };

            try
            {
                // Delete existing archive if it exists
                if (File.Exists(archivePath))
                {
                    File.Delete(archivePath);
                }

                using var archive = ZipFile.Open(archivePath, ZipArchiveMode.Create);
                var processedFiles = 0;

                foreach (var sourcePath in sources)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Success = false;
                        result.Message = "Compression cancelled by user";
                        break;
                    }

                    if (File.Exists(sourcePath))
                    {
                        // Add file to archive
                        var entryName = Path.GetFileName(sourcePath);
                        await Task.Run(() => archive.CreateEntryFromFile(sourcePath, entryName, CompressionLevel.Optimal), cancellationToken);
                        processedFiles++;
                    }
                    else if (Directory.Exists(sourcePath))
                    {
                        // Add directory recursively
                        var dirInfo = new DirectoryInfo(sourcePath);
                        var files = dirInfo.GetFiles("*", SearchOption.AllDirectories);
                        
                        foreach (var file in files)
                        {
                            if (cancellationToken.IsCancellationRequested)
                            {
                                result.Success = false;
                                result.Message = "Compression cancelled by user";
                                break;
                            }

                            // Calculate relative path for entry name
                            var relativePath = Path.GetRelativePath(Path.GetDirectoryName(sourcePath) ?? "", file.FullName);
                            await Task.Run(() => archive.CreateEntryFromFile(file.FullName, relativePath, CompressionLevel.Optimal), cancellationToken);
                            processedFiles++;
                        }
                    }
                }

                result.FilesProcessed = processedFiles;
                result.Message = $"Compressed {processedFiles} files into archive";
            }
            catch (OperationCanceledException)
            {
                result.Success = false;
                result.Message = "Compression cancelled by user";
                
                // Clean up partial archive
                if (File.Exists(archivePath))
                {
                    try { File.Delete(archivePath); } catch { }
                }
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Compression failed: {ex.Message}";
                result.Errors.Add(ex.Message);
                
                // Clean up partial archive
                if (File.Exists(archivePath))
                {
                    try { File.Delete(archivePath); } catch { }
                }
            }

            result.Duration = DateTime.Now - startTime;
            return result;
        }
    }
}
