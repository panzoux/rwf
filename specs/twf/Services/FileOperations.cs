using System.Text.RegularExpressions;
using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    ///  Action to take when a file collision occurs
    /// </summary>
    public enum FileCollisionAction
    {
        Overwrite,
        OverwriteAll,
        Skip,
        SkipAll,
        Rename,
        Cancel
    }

    /// <summary>
    /// Result of a file collision resolution
    /// </summary>
    public class FileCollisionResult
    {
        public FileCollisionAction Action { get; set; }
        public string? NewName { get; set; }
    }

    /// <summary>
    /// Handles file system operations with progress reporting and cancellation support
    /// </summary>
    public class FileOperations
    {
        public event EventHandler<ProgressEventArgs>? ProgressChanged;
        public event EventHandler<ErrorEventArgs>? ErrorOccurred;

        /// <summary>
        /// Copies files to a destination directory with progress reporting
        /// </summary>
        public async Task<OperationResult> CopyAsync(
            List<FileEntry> sources,
            string destination,
            CancellationToken cancellationToken,
            Func<string, Task<FileCollisionResult>>? collisionHandler = null,
            IProgress<ProgressEventArgs>? progress = null)
        {
            // Implementation note: collisionHandler(destPath) -> Action
            
            var result = new OperationResult();
            var startTime = DateTime.Now;
            var errors = new List<string>();

            try
            {
                // Validate destination
                if (!Directory.Exists(destination))
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = $"Destination directory does not exist: {destination}"
                    };
                }

                // Calculate total stats
                long totalBytes = 0;
                int totalFiles = 0;
                
                foreach (var source in sources)
                {
                    if (source.IsDirectory)
                    {
                        var stats = GetDirectoryStats(source.FullPath);
                        totalBytes += stats.TotalBytes;
                        totalFiles += stats.TotalFiles;
                    }
                    else
                    {
                        totalBytes += source.Size;
                        totalFiles++;
                    }
                }

                long bytesProcessed = 0;
                int filesProcessedCount = 0;
                // Collision context to remember "All" actions
                FileCollisionAction? stickyAction = null;

                for (int i = 0; i < sources.Count; i++)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Message = "Operation cancelled by user";
                        break;
                    }

                    var source = sources[i];
                    
                    try
                    {
                        if (source.IsDirectory)
                        {
                            (bytesProcessed, filesProcessedCount, stickyAction) = await CopyDirectoryAsync(source.FullPath, destination, 
                                filesProcessedCount, totalFiles, bytesProcessed, totalBytes, cancellationToken, collisionHandler, stickyAction, progress);
                        }
                        else
                        {
                            (bytesProcessed, stickyAction) = await CopyFileAsync(source.FullPath, destination, 
                                filesProcessedCount, totalFiles, bytesProcessed, totalBytes, cancellationToken, collisionHandler, stickyAction, progress);
                            filesProcessedCount++;
                        }
                        result.FilesProcessed++;
                    }
                    catch (OperationCanceledException)
                    {
                         result.Message = "Operation cancelled by user";
                         break;
                    }
                    catch (Exception ex)
                    {
                        errors.Add($"{source.Name}: {ex.Message}");
                        result.FilesSkipped++;
                        OnErrorOccurred(new ErrorEventArgs(ex));
                    }
                }


                result.Success = result.FilesProcessed > 0 || (result.FilesSkipped > 0 && errors.Count == 0);
                result.Errors = errors;
                result.Duration = DateTime.Now - startTime;
                
                if (result.Message == null) // Don't overwrite cancel message
                {
                    result.Message = result.Success 
                        ? $"Copied {result.FilesProcessed} file(s) successfully"
                        : "Copy operation failed";
                }
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Copy operation failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Moves files to a destination directory with progress reporting
        /// </summary>
        public async Task<OperationResult> MoveAsync(
            List<FileEntry> sources,
            string destination,
            CancellationToken cancellationToken,
            Func<string, Task<FileCollisionResult>>? collisionHandler = null,
            IProgress<ProgressEventArgs>? progress = null)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;
            var errors = new List<string>();

            try
            {
                // Validate destination
                if (!Directory.Exists(destination))
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = $"Destination directory does not exist: {destination}"
                    };
                }

                // Calculate total stats
                long totalBytes = 0;
                int totalFiles = 0;
                
                foreach (var source in sources)
                {
                    if (source.IsDirectory)
                    {
                        var stats = GetDirectoryStats(source.FullPath);
                        totalBytes += stats.TotalBytes;
                        totalFiles += stats.TotalFiles;
                    }
                    else
                    {
                        totalBytes += source.Size;
                        totalFiles++;
                    }
                }

                long bytesProcessed = 0;
                int filesProcessedCount = 0;
                // Collision context to remember "All" actions
                FileCollisionAction? stickyAction = null;

                for (int i = 0; i < sources.Count; i++)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Message = "Operation cancelled by user";
                        break;
                    }

                    var source = sources[i];
                    
                    try
                    {
                        var destPath = Path.Combine(destination, source.Name);
                        bool isCollision = source.IsDirectory ? Directory.Exists(destPath) : File.Exists(destPath);
                        
                        // Handle collision
                        if (isCollision)
                        {
                            FileCollisionResult? collisionResult = null;
                            if (stickyAction == FileCollisionAction.OverwriteAll)
                                collisionResult = new FileCollisionResult { Action = FileCollisionAction.Overwrite };
                            else if (stickyAction == FileCollisionAction.SkipAll)
                                collisionResult = new FileCollisionResult { Action = FileCollisionAction.Skip };
                            else if (collisionHandler != null)
                            {
                                collisionResult = await collisionHandler(destPath);
                                if (collisionResult.Action == FileCollisionAction.OverwriteAll || collisionResult.Action == FileCollisionAction.SkipAll)
                                    stickyAction = collisionResult.Action;
                            }

                            if (collisionResult != null)
                            {
                                if (collisionResult.Action == FileCollisionAction.Cancel)
                                {
                                    result.Message = "Operation cancelled by user";
                                    break;
                                }
                                else if (collisionResult.Action == FileCollisionAction.Skip || collisionResult.Action == FileCollisionAction.SkipAll)
                                {
                                    // Skip needs to count files that were skipped
                                    if (source.IsDirectory)
                                    {
                                        var stats = GetDirectoryStats(source.FullPath);
                                        result.FilesSkipped += stats.TotalFiles;
                                    }
                                    else
                                    {
                                        result.FilesSkipped++;
                                    }
                                    continue; 
                                }
                                else if (collisionResult.Action == FileCollisionAction.Rename && !string.IsNullOrEmpty(collisionResult.NewName))
                                {
                                    destPath = Path.Combine(destination, collisionResult.NewName);
                                }
                                else if (collisionResult.Action == FileCollisionAction.Overwrite || collisionResult.Action == FileCollisionAction.OverwriteAll)
                                {
                                    if (source.IsDirectory)
                                    {
                                        Directory.Delete(destPath, true);
                                    }
                                    else
                                    {
                                        File.Delete(destPath);
                                    }
                                }
                            }
                            else
                            {
                                // Default behavior without handler: fail
                                throw new IOException($"Destination already exists: {destPath}");
                            }
                        }
                        
                        // Check for cross-volume move
                        bool isCrossVolume = false;
                        try 
                        {
                            var rootSource = Path.GetPathRoot(source.FullPath);
                            var rootDest = Path.GetPathRoot(destPath);
                            if (rootSource != null && rootDest != null)
                            {
                                isCrossVolume = !string.Equals(rootSource, rootDest, StringComparison.OrdinalIgnoreCase);
                            }
                        }
                        catch { /* Ignore */ }

                        if (source.IsDirectory)
                        {
                            if (isCrossVolume)
                            {
                                (bytesProcessed, filesProcessedCount, stickyAction) = await CopyDirectoryAsync(source.FullPath, destination, 
                                    filesProcessedCount, totalFiles, bytesProcessed, totalBytes, cancellationToken, collisionHandler, stickyAction, progress);
                                Directory.Delete(source.FullPath, true);
                            }
                            else
                            {
                                Directory.Move(source.FullPath, destPath);
                                var stats = GetDirectoryStats(destPath);
                                bytesProcessed += stats.TotalBytes;
                                filesProcessedCount += stats.TotalFiles;
                            }
                        }
                        else
                        {
                            if (isCrossVolume)
                            {
                                (bytesProcessed, stickyAction) = await CopyFileAsync(source.FullPath, destination, 
                                    filesProcessedCount, totalFiles, bytesProcessed, totalBytes, cancellationToken, collisionHandler, stickyAction, progress);
                                File.Delete(source.FullPath);
                                filesProcessedCount++;
                            }
                            else
                            {
                                File.Move(source.FullPath, destPath);
                                bytesProcessed += source.Size;
                                filesProcessedCount++;
                            }
                        }

                        var progressData = new ProgressEventArgs
                        {
                            CurrentFile = source.Name,
                            CurrentFileIndex = filesProcessedCount,
                            TotalFiles = totalFiles,
                            BytesProcessed = bytesProcessed,
                            TotalBytes = totalBytes,
                            PercentComplete = (double)bytesProcessed / totalBytes * 100
                        };

                        OnProgressChanged(progressData);
                        progress?.Report(progressData);

                        result.FilesProcessed++;
                    }
                    catch (Exception ex)
                    {
                        errors.Add($"{source.Name}: {ex.Message}");
                        result.FilesSkipped++;
                        OnErrorOccurred(new ErrorEventArgs(ex));
                    }
                }

                result.Success = result.FilesProcessed > 0 || (result.FilesSkipped > 0 && errors.Count == 0);
                result.Errors = errors;
                result.Duration = DateTime.Now - startTime;
                
                if (result.Message == null)
                {
                    result.Message = result.Success 
                        ? $"Moved {result.FilesProcessed} file(s) successfully"
                        : "Move operation failed";
                }
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Move operation failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Deletes files with confirmation
        /// </summary>
        public async Task<OperationResult> DeleteAsync(
            List<FileEntry> entries,
            CancellationToken cancellationToken,
            IProgress<ProgressEventArgs>? progress = null)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;
            var errors = new List<string>();

            try
            {
                for (int i = 0; i < entries.Count; i++)
                {
                    if (cancellationToken.IsCancellationRequested)
                    {
                        result.Message = "Operation cancelled by user";
                        break;
                    }

                    var entry = entries[i];
                    
                    try
                    {
                        if (entry.IsDirectory)
                        {
                            Directory.Delete(entry.FullPath, recursive: true);
                        }
                        else
                        {
                            File.Delete(entry.FullPath);
                        }

                        var progressData = new ProgressEventArgs
                        {
                            CurrentFile = entry.Name,
                            CurrentFileIndex = i + 1,
                            TotalFiles = entries.Count,
                            PercentComplete = (double)(i + 1) / entries.Count * 100
                        };

                        OnProgressChanged(progressData);
                        progress?.Report(progressData);

                        result.FilesProcessed++;
                    }
                    catch (Exception ex)
                    {
                        errors.Add($"{entry.Name}: {ex.Message}");
                        result.FilesSkipped++;
                        OnErrorOccurred(new ErrorEventArgs(ex));
                    }

                    // Add small delay to allow async processing
                    await Task.Delay(1, cancellationToken);
                }

                result.Success = result.FilesProcessed > 0;
                result.Errors = errors;
                result.Duration = DateTime.Now - startTime;
                result.Message = result.Success 
                    ? $"Deleted {result.FilesProcessed} file(s) successfully"
                    : "Delete operation failed";
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Delete operation failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Creates a new directory
        /// </summary>
        public OperationResult CreateDirectory(string path, string name)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;

            try
            {
                var fullPath = Path.Combine(path, name);
                
                if (Directory.Exists(fullPath))
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = $"Directory already exists: {name}"
                    };
                }

                Directory.CreateDirectory(fullPath);
                
                result.Success = true;
                result.FilesProcessed = 1;
                result.Message = $"Created directory: {name}";
                result.Duration = DateTime.Now - startTime;
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Failed to create directory: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Renames files using pattern-based transformations
        /// </summary>
        public async Task<OperationResult> RenameAsync(
            List<FileEntry> entries,
            string pattern,
            string replacement)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;
            var errors = new List<string>();

            try
            {
                for (int i = 0; i < entries.Count; i++)
                {
                    var entry = entries[i];
                    
                    try
                    {
                        var newName = ApplyRenamePattern(entry.Name, pattern, replacement);
                        
                        if (newName == entry.Name)
                        {
                            result.FilesSkipped++;
                            continue;
                        }

                        var directory = Path.GetDirectoryName(entry.FullPath) ?? string.Empty;
                        var newPath = Path.Combine(directory, newName);

                        if (entry.IsDirectory)
                        {
                            Directory.Move(entry.FullPath, newPath);
                        }
                        else
                        {
                            File.Move(entry.FullPath, newPath);
                        }

                        OnProgressChanged(new ProgressEventArgs
                        {
                            CurrentFile = entry.Name,
                            CurrentFileIndex = i + 1,
                            TotalFiles = entries.Count,
                            PercentComplete = (double)(i + 1) / entries.Count * 100
                        });

                        result.FilesProcessed++;
                    }
                    catch (Exception ex)
                    {
                        errors.Add($"{entry.Name}: {ex.Message}");
                        result.FilesSkipped++;
                        OnErrorOccurred(new ErrorEventArgs(ex));
                    }

                    await Task.Delay(1);
                }

                result.Success = result.FilesProcessed > 0;
                result.Errors = errors;
                result.Duration = DateTime.Now - startTime;
                result.Message = result.Success 
                    ? $"Renamed {result.FilesProcessed} file(s) successfully"
                    : "Rename operation failed";
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Rename operation failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        private string ApplyRenamePattern(string filename, string pattern, string replacement)
        {
            // Support regex patterns with s/ syntax
            if (pattern.StartsWith("s/"))
            {
                var parts = pattern.Split('/');
                if (parts.Length >= 3)
                {
                    var searchPattern = parts[1];
                    var replacePattern = parts[2];
                    return Regex.Replace(filename, searchPattern, replacePattern);
                }
            }

            // Support transliteration with tr/ syntax
            if (pattern.StartsWith("tr/"))
            {
                var parts = pattern.Split('/');
                if (parts.Length >= 3)
                {
                    var fromChars = parts[1];
                    var toChars = parts[2];
                    var result = filename.ToCharArray();
                    for (int i = 0; i < result.Length; i++)
                    {
                        int index = fromChars.IndexOf(result[i]);
                        if (index >= 0 && index < toChars.Length)
                        {
                            result[i] = toChars[index];
                        }
                    }
                    return new string(result);
                }
            }

            // Simple string replacement
            return filename.Replace(pattern, replacement);
        }

        private async Task<(long bytesProcessed, FileCollisionAction? stickyAction)> CopyFileAsync(string sourcePath, string destDir, 
            int filesProcessedCount, int totalFiles,
            long bytesProcessed, long totalBytes, CancellationToken cancellationToken,
            Func<string, Task<FileCollisionResult>>? collisionHandler = null,
            FileCollisionAction? stickyAction = null,
            IProgress<ProgressEventArgs>? progress = null)
        {
            var fileName = Path.GetFileName(sourcePath);
            var destPath = Path.Combine(destDir, fileName);

            // Check for collision
            if (File.Exists(destPath))
            {
                FileCollisionResult? collisionResult = null;
                if (stickyAction == FileCollisionAction.OverwriteAll)
                    collisionResult = new FileCollisionResult { Action = FileCollisionAction.Overwrite };
                else if (stickyAction == FileCollisionAction.SkipAll)
                    collisionResult = new FileCollisionResult { Action = FileCollisionAction.Skip };
                else if (collisionHandler != null)
                {
                    collisionResult = await collisionHandler(destPath);
                    if (collisionResult.Action == FileCollisionAction.OverwriteAll || collisionResult.Action == FileCollisionAction.SkipAll)
                        stickyAction = collisionResult.Action;
                }

                if (collisionResult != null)
                {
                    if (collisionResult.Action == FileCollisionAction.Cancel)
                    {
                        throw new OperationCanceledException("Operation cancelled by user");
                    }
                    else if (collisionResult.Action == FileCollisionAction.Skip || collisionResult.Action == FileCollisionAction.SkipAll)
                    {
                        // Emit skipped event
                         var skipData = new ProgressEventArgs
                        {
                            Status = FileOperationStatus.Skipped,
                            CurrentFile = fileName,
                            CurrentFileIndex = filesProcessedCount + 1,
                            TotalFiles = totalFiles,
                            BytesProcessed = bytesProcessed,
                            TotalBytes = totalBytes,
                            PercentComplete = totalBytes > 0 ? (double)bytesProcessed / totalBytes * 100 : 0
                        };
                        OnProgressChanged(skipData);
                        progress?.Report(skipData);
                        
                        return (bytesProcessed, stickyAction);
                    }
                    else if (collisionResult.Action == FileCollisionAction.Rename && !string.IsNullOrEmpty(collisionResult.NewName))
                    {
                        destPath = Path.Combine(destDir, collisionResult.NewName);
                    }
                }
                else
                {
                    // Default behavior without handler: fail
                    throw new IOException($"Destination already exists: {destPath}");
                }
            }

            // Get source file info before copying
            var sourceInfo = new FileInfo(sourcePath);
            var sourceLastWriteTime = sourceInfo.LastWriteTimeUtc;
            var sourceCreationTime = sourceInfo.CreationTimeUtc;
            var sourceAttributes = sourceInfo.Attributes;
            long fileLength = sourceInfo.Length;

            // Emit Started event
            var startData = new ProgressEventArgs
            {
                Status = FileOperationStatus.Started,
                CurrentFile = fileName,
                CurrentFileIndex = filesProcessedCount + 1,
                TotalFiles = totalFiles,
                BytesProcessed = bytesProcessed,
                TotalBytes = totalBytes,
                PercentComplete = totalBytes > 0 ? (double)bytesProcessed / totalBytes * 100 : 0,
                CurrentFileBytesProcessed = 0,
                CurrentFileTotalBytes = fileLength
            };
            OnProgressChanged(startData);
            progress?.Report(startData);
            
            // Copy file content
            // Use 1MB buffer for better performance on various media
            const int bufferSize = 1024 * 1024; 
            var fileOptions = FileOptions.Asynchronous | FileOptions.SequentialScan;

            using (var sourceStream = new FileStream(sourcePath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, fileOptions))
            using (var destStream = new FileStream(destPath, FileMode.Create, FileAccess.Write, FileShare.None, bufferSize, fileOptions))
            {
                var buffer = new byte[bufferSize];
                int bytesRead;
                long currentBytesProcessed = bytesProcessed;
                long fileBytesProcessed = 0;

                while ((bytesRead = await sourceStream.ReadAsync(buffer, 0, buffer.Length, cancellationToken)) > 0)
                {
                    await destStream.WriteAsync(buffer, 0, bytesRead, cancellationToken);
                    currentBytesProcessed += bytesRead;
                    fileBytesProcessed += bytesRead;

                    var progressData = new ProgressEventArgs
                    {
                        Status = FileOperationStatus.Processing,
                        CurrentFile = fileName,
                        CurrentFileIndex = filesProcessedCount + 1,
                        TotalFiles = totalFiles,
                        BytesProcessed = currentBytesProcessed,
                        TotalBytes = totalBytes,
                        PercentComplete = totalBytes > 0 ? (double)currentBytesProcessed / totalBytes * 100 : 0,
                        CurrentFileBytesProcessed = fileBytesProcessed,
                        CurrentFileTotalBytes = fileLength
                    };
                    
                    OnProgressChanged(progressData);
                    progress?.Report(progressData);
                }

                bytesProcessed = currentBytesProcessed;
            }

            // Preserve file attributes and timestamps (after streams are closed)
            // Set attributes first (but remove ReadOnly to allow timestamp changes)
            var attrs = sourceAttributes & ~FileAttributes.ReadOnly;
            if (attrs != FileAttributes.Normal)
            {
                File.SetAttributes(destPath, attrs);
            }
            // Then set timestamps
            File.SetLastWriteTimeUtc(destPath, sourceLastWriteTime);
            File.SetCreationTimeUtc(destPath, sourceCreationTime);

            // Emit Completed event
            var completedData = new ProgressEventArgs
            {
                Status = FileOperationStatus.Completed,
                CurrentFile = fileName,
                CurrentFileIndex = filesProcessedCount + 1,
                TotalFiles = totalFiles,
                BytesProcessed = bytesProcessed,
                TotalBytes = totalBytes,
                PercentComplete = totalBytes > 0 ? (double)bytesProcessed / totalBytes * 100 : 0,
                CurrentFileBytesProcessed = fileLength,
                CurrentFileTotalBytes = fileLength
            };
            OnProgressChanged(completedData);
            progress?.Report(completedData);

            return (bytesProcessed, stickyAction);
        }

        private async Task<(long bytesProcessed, int filesProcessed, FileCollisionAction? stickyAction)> CopyDirectoryAsync(string sourceDir, string destParentDir, 
            int filesProcessedCount, int totalFiles,
            long bytesProcessed, long totalBytes, CancellationToken cancellationToken,
            Func<string, Task<FileCollisionResult>>? collisionHandler = null,
            FileCollisionAction? stickyAction = null,
            IProgress<ProgressEventArgs>? progress = null)
        {
            var dirName = Path.GetFileName(sourceDir);
            var destDir = Path.Combine(destParentDir, dirName);

            // Handle directory collision
            if (Directory.Exists(destDir))
            {
                FileCollisionResult? collisionResult = null;
                if (stickyAction == FileCollisionAction.OverwriteAll)
                    collisionResult = new FileCollisionResult { Action = FileCollisionAction.Overwrite };
                else if (stickyAction == FileCollisionAction.SkipAll)
                    collisionResult = new FileCollisionResult { Action = FileCollisionAction.Skip };
                else if (collisionHandler != null)
                {
                    collisionResult = await collisionHandler(destDir);
                    if (collisionResult.Action == FileCollisionAction.OverwriteAll || collisionResult.Action == FileCollisionAction.SkipAll)
                        stickyAction = collisionResult.Action;
                }

                if (collisionResult != null)
                {
                    if (collisionResult.Action == FileCollisionAction.Cancel)
                    {
                        throw new OperationCanceledException("Operation cancelled by user");
                    }
                    else if (collisionResult.Action == FileCollisionAction.Skip || collisionResult.Action == FileCollisionAction.SkipAll)
                    {
                        return (bytesProcessed, filesProcessedCount, stickyAction);
                    }
                    else if (collisionResult.Action == FileCollisionAction.Rename && !string.IsNullOrEmpty(collisionResult.NewName))
                    {
                        destDir = Path.Combine(destParentDir, collisionResult.NewName);
                    }
                    else if (collisionResult.Action == FileCollisionAction.Overwrite || collisionResult.Action == FileCollisionAction.OverwriteAll)
                    {
                        // Overwrite directory by deleting existing one first
                        Directory.Delete(destDir, true);
                    }
                }
                else
                {
                    throw new IOException($"Destination already exists: {destDir}");
                }
            }

            Directory.CreateDirectory(destDir);

            long currentBytesProcessed = bytesProcessed;
            int currentFilesProcessed = filesProcessedCount;

            // Copy all files
            foreach (var file in Directory.GetFiles(sourceDir))
            {
                if (cancellationToken.IsCancellationRequested)
                    break;

                (currentBytesProcessed, stickyAction) = await CopyFileAsync(file, destDir, currentFilesProcessed, totalFiles, 
                    currentBytesProcessed, totalBytes, cancellationToken, collisionHandler, stickyAction, progress);
                currentFilesProcessed++;
            }

            // Recursively copy subdirectories
            foreach (var subDir in Directory.GetDirectories(sourceDir))
            {
                if (cancellationToken.IsCancellationRequested)
                    break;

                (currentBytesProcessed, currentFilesProcessed, stickyAction) = await CopyDirectoryAsync(subDir, destDir, currentFilesProcessed, totalFiles, 
                    currentBytesProcessed, totalBytes, cancellationToken, collisionHandler, stickyAction, progress);
            }

            return (currentBytesProcessed, currentFilesProcessed, stickyAction);
        }

        private (long TotalBytes, int TotalFiles) GetDirectoryStats(string path)
        {
            if (!Directory.Exists(path))
                return (0, 0);

            long totalBytes = 0;
            int totalFiles = 0;
            
            try
            {
                // Get all files recursively
                var files = Directory.GetFiles(path, "*", SearchOption.AllDirectories);
                totalFiles = files.Length;
                
                foreach (var file in files)
                {
                    totalBytes += new FileInfo(file).Length;
                }
            }
            catch
            {
                // Ignore access denied errors
            }

            return (totalBytes, totalFiles);
        }

        /// <summary>
        /// Splits a file into multiple parts
        /// </summary>
        public async Task<OperationResult> SplitAsync(
            string sourceFile,
            long partSize,
            string outputDirectory,
            CancellationToken cancellationToken)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;
            var errors = new List<string>();

            try
            {
                // Validate inputs
                if (!File.Exists(sourceFile))
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = $"Source file does not exist: {sourceFile}"
                    };
                }

                if (partSize <= 0)
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = "Part size must be greater than zero"
                    };
                }

                if (!Directory.Exists(outputDirectory))
                {
                    Directory.CreateDirectory(outputDirectory);
                }

                var fileInfo = new FileInfo(sourceFile);
                var totalBytes = fileInfo.Length;
                var fileName = Path.GetFileName(sourceFile);
                var baseName = Path.GetFileNameWithoutExtension(sourceFile);
                var extension = Path.GetExtension(sourceFile);

                // Calculate number of parts
                var partCount = (int)Math.Ceiling((double)totalBytes / partSize);
                long bytesProcessed = 0;
                int partNumber = 1;
                const int bufferSize = 1024 * 1024;

                using (var sourceStream = new FileStream(sourceFile, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, true))
                {
                    while (bytesProcessed < totalBytes)
                    {
                        if (cancellationToken.IsCancellationRequested)
                        {
                            result.Message = "Operation cancelled by user";
                            break;
                        }

                        // Generate part file name: filename.001, filename.002, etc.
                        var partFileName = $"{baseName}{extension}.{partNumber:D3}";
                        var partFilePath = Path.Combine(outputDirectory, partFileName);

                        var bytesToRead = Math.Min(partSize, totalBytes - bytesProcessed);
                        
                        using (var partStream = new FileStream(partFilePath, FileMode.Create, FileAccess.Write, FileShare.None, bufferSize, true))
                        {
                            var buffer = new byte[bufferSize];
                            long partBytesWritten = 0;

                            while (partBytesWritten < bytesToRead)
                            {
                                var chunkSize = (int)Math.Min(buffer.Length, bytesToRead - partBytesWritten);
                                var bytesRead = await sourceStream.ReadAsync(buffer, 0, chunkSize, cancellationToken);
                                
                                if (bytesRead == 0)
                                    break;

                                await partStream.WriteAsync(buffer, 0, bytesRead, cancellationToken);
                                partBytesWritten += bytesRead;
                                bytesProcessed += bytesRead;

                                OnProgressChanged(new ProgressEventArgs
                                {
                                    CurrentFile = partFileName,
                                    CurrentFileIndex = partNumber,
                                    TotalFiles = partCount,
                                    BytesProcessed = bytesProcessed,
                                    TotalBytes = totalBytes,
                                    PercentComplete = (double)bytesProcessed / totalBytes * 100
                                });
                            }
                        }

                        result.FilesProcessed++;
                        partNumber++;
                    }
                }

                result.Success = result.FilesProcessed > 0;
                result.Duration = DateTime.Now - startTime;
                result.Message = result.Success 
                    ? $"Split file into {result.FilesProcessed} part(s) successfully"
                    : "Split operation failed";
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Split operation failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Joins multiple split file parts back into the original file
        /// </summary>
        public async Task<OperationResult> JoinAsync(
            List<string> partFiles,
            string outputFile,
            CancellationToken cancellationToken)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;
            var errors = new List<string>();

            try
            {
                // Validate inputs
                if (partFiles == null || partFiles.Count == 0)
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = "No part files specified"
                    };
                }

                // Validate all part files exist
                foreach (var partFile in partFiles)
                {
                    if (!File.Exists(partFile))
                    {
                        return new OperationResult
                        {
                            Success = false,
                            Message = $"Part file does not exist: {partFile}"
                        };
                    }
                }

                // Sort part files to ensure correct order
                var sortedParts = partFiles.OrderBy(f => f).ToList();

                // Calculate total size
                long totalBytes = sortedParts.Sum(f => new FileInfo(f).Length);
                long bytesProcessed = 0;

                // Ensure output directory exists
                var outputDir = Path.GetDirectoryName(outputFile);
                if (!string.IsNullOrEmpty(outputDir) && !Directory.Exists(outputDir))
                {
                    Directory.CreateDirectory(outputDir);
                }

                const int bufferSize = 1024 * 1024;
                using (var outputStream = new FileStream(outputFile, FileMode.Create, FileAccess.Write, FileShare.None, bufferSize, true))
                {
                    for (int i = 0; i < sortedParts.Count; i++)
                    {
                        if (cancellationToken.IsCancellationRequested)
                        {
                            result.Message = "Operation cancelled by user";
                            break;
                        }

                        var partFile = sortedParts[i];
                        var partFileName = Path.GetFileName(partFile);

                        using (var partStream = new FileStream(partFile, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, true))
                        {
                            var buffer = new byte[bufferSize];
                            int bytesRead;

                            while ((bytesRead = await partStream.ReadAsync(buffer, 0, buffer.Length, cancellationToken)) > 0)
                            {
                                await outputStream.WriteAsync(buffer, 0, bytesRead, cancellationToken);
                                bytesProcessed += bytesRead;

                                OnProgressChanged(new ProgressEventArgs
                                {
                                    CurrentFile = partFileName,
                                    CurrentFileIndex = i + 1,
                                    TotalFiles = sortedParts.Count,
                                    BytesProcessed = bytesProcessed,
                                    TotalBytes = totalBytes,
                                    PercentComplete = (double)bytesProcessed / totalBytes * 100
                                });
                            }
                        }

                        result.FilesProcessed++;
                    }
                }

                result.Success = result.FilesProcessed > 0;
                result.Duration = DateTime.Now - startTime;
                result.Message = result.Success 
                    ? $"Joined {result.FilesProcessed} part(s) into {Path.GetFileName(outputFile)} successfully"
                    : "Join operation failed";
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Join operation failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Compares files between two panes and marks matching files
        /// </summary>
        public OperationResult CompareFiles(
            PaneState leftPane,
            PaneState rightPane,
            ComparisonCriteria criteria,
            TimeSpan? timestampTolerance = null)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;

            try
            {
                // Clear existing marks
                foreach (var entry in leftPane.Entries) entry.IsMarked = false;
                foreach (var entry in rightPane.Entries) entry.IsMarked = false;

                // Build comparison sets based on criteria
                switch (criteria)
                {
                    case ComparisonCriteria.Size:
                        CompareBySize(leftPane, rightPane);
                        break;

                    case ComparisonCriteria.Timestamp:
                        var tolerance = timestampTolerance ?? TimeSpan.FromSeconds(2);
                        CompareByTimestamp(leftPane, rightPane, tolerance);
                        break;

                    case ComparisonCriteria.Name:
                        CompareByName(leftPane, rightPane);
                        break;

                    default:
                        return new OperationResult
                        {
                            Success = false,
                            Message = $"Unknown comparison criteria: {criteria}"
                        };
                }

                var totalMarked = leftPane.Entries.Count(e => e.IsMarked) + rightPane.Entries.Count(e => e.IsMarked);
                result.Success = true;
                result.FilesProcessed = totalMarked;
                result.Duration = DateTime.Now - startTime;
                result.Message = $"Marked {totalMarked} matching file(s)";
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Comparison failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Compares files by size and marks files with identical sizes
        /// </summary>
        private void CompareBySize(PaneState leftPane, PaneState rightPane)
        {
            // Build a dictionary of sizes from right pane
            var rightSizes = new Dictionary<long, List<int>>();
            for (int i = 0; i < rightPane.Entries.Count; i++)
            {
                var entry = rightPane.Entries[i];
                if (!entry.IsDirectory)
                {
                    if (!rightSizes.ContainsKey(entry.Size))
                    {
                        rightSizes[entry.Size] = new List<int>();
                    }
                    rightSizes[entry.Size].Add(i);
                }
            }

            // Mark files in left pane that have matching sizes in right pane
            for (int i = 0; i < leftPane.Entries.Count; i++)
            {
                var entry = leftPane.Entries[i];
                if (!entry.IsDirectory && rightSizes.ContainsKey(entry.Size))
                {
                    leftPane.Entries[i].IsMarked = true;
                    
                    // Also mark the matching files in right pane
                    foreach (var rightIndex in rightSizes[entry.Size])
                    {
                        rightPane.Entries[rightIndex].IsMarked = true;
                    }
                }
            }
        }

        /// <summary>
        /// Compares files by timestamp with tolerance and marks files with matching timestamps
        /// </summary>
        private void CompareByTimestamp(PaneState leftPane, PaneState rightPane, TimeSpan tolerance)
        {
            // Build a list of timestamps from right pane
            var rightTimestamps = new List<(int index, DateTime timestamp)>();
            for (int i = 0; i < rightPane.Entries.Count; i++)
            {
                var entry = rightPane.Entries[i];
                if (!entry.IsDirectory)
                {
                    rightTimestamps.Add((i, entry.LastModified));
                }
            }

            // Mark files in left pane that have matching timestamps in right pane
            for (int i = 0; i < leftPane.Entries.Count; i++)
            {
                var entry = leftPane.Entries[i];
                if (!entry.IsDirectory)
                {
                    foreach (var (rightIndex, rightTimestamp) in rightTimestamps)
                    {
                        var timeDiff = Math.Abs((entry.LastModified - rightTimestamp).TotalSeconds);
                        if (timeDiff <= tolerance.TotalSeconds)
                        {
                            leftPane.Entries[i].IsMarked = true;
                            rightPane.Entries[rightIndex].IsMarked = true;
                            break; // Only mark once per left file
                        }
                    }
                }
            }
        }

        /// <summary>
        /// Compares files by name and marks files with identical names
        /// </summary>
        private void CompareByName(PaneState leftPane, PaneState rightPane)
        {
            // Build a dictionary of names from right pane
            var rightNames = new Dictionary<string, List<int>>(StringComparer.OrdinalIgnoreCase);
            for (int i = 0; i < rightPane.Entries.Count; i++)
            {
                var entry = rightPane.Entries[i];
                if (!entry.IsDirectory)
                {
                    if (!rightNames.ContainsKey(entry.Name))
                    {
                        rightNames[entry.Name] = new List<int>();
                    }
                    rightNames[entry.Name].Add(i);
                }
            }

            // Mark files in left pane that have matching names in right pane
            for (int i = 0; i < leftPane.Entries.Count; i++)
            {
                var entry = leftPane.Entries[i];
                if (!entry.IsDirectory && rightNames.ContainsKey(entry.Name))
                {
                    leftPane.Entries[i].IsMarked = true;
                    
                    // Also mark the matching files in right pane
                    foreach (var rightIndex in rightNames[entry.Name])
                    {
                        rightPane.Entries[rightIndex].IsMarked = true;
                    }
                }
            }
        }

        /// <summary>
        /// Calculates directory size recursively with cancellation and progress
        /// </summary>
        public async Task<(long size, int fileCount, int dirCount)> CalculateDirectorySizeAsync(
            string path,
            CancellationToken cancellationToken,
            IProgress<ProgressEventArgs>? progress = null,
            int reportIntervalMs = 500)
        {
            long size = 0;
            int files = 0;
            int dirs = 0;

            await Task.Run(() =>
            {
                var stack = new Stack<string>();
                stack.Push(path);

                var stopwatch = System.Diagnostics.Stopwatch.StartNew();

                while (stack.Count > 0)
                {
                    if (cancellationToken.IsCancellationRequested) break;

                    var currentDir = stack.Pop();
                    dirs++;

                    // List files
                    try
                    {
                        foreach (var file in Directory.EnumerateFiles(currentDir))
                        {
                            if (cancellationToken.IsCancellationRequested) break;
                            try
                            {
                                var info = new FileInfo(file);
                                size += info.Length;
                                files++;
                            }
                            catch { }
                        }
                    }
                    catch { }

                    // List subdirectories
                    try
                    {
                        foreach (var subDir in Directory.EnumerateDirectories(currentDir))
                        {
                            if (cancellationToken.IsCancellationRequested) break;
                            stack.Push(subDir);
                        }
                    }
                    catch { }

                    // Time-based progress reporting based on config
                    if (stopwatch.ElapsedMilliseconds >= reportIntervalMs)
                    {
                        progress?.Report(new ProgressEventArgs 
                        { 
                            CurrentFile = currentDir,
                            FilesProcessed = files,
                            BytesProcessed = size
                        });
                        stopwatch.Restart();
                    }
                }
            }, cancellationToken);

            return (size, files, Math.Max(0, dirs - 1));
        }

        /// <summary>
        /// Executes a file with its associated program or default handler
        /// </summary>
        public OperationResult ExecuteFile(string filePath, Configuration config, ExecutionMode mode = ExecutionMode.Default)
        {
            var result = new OperationResult();
            var startTime = DateTime.Now;

            try
            {
                if (!File.Exists(filePath))
                {
                    return new OperationResult
                    {
                        Success = false,
                        Message = $"File does not exist: {filePath}"
                    };
                }

                var extension = Path.GetExtension(filePath).ToLowerInvariant();
                var fileName = Path.GetFileName(filePath);

                switch (mode)
                {
                    case ExecutionMode.Default:
                        // Check if file is executable
                        if (IsExecutableFile(filePath))
                        {
                            result = ExecuteExecutableFile(filePath);
                        }
                        // Check for extension association
                        else if (config.ExtensionAssociations.TryGetValue(extension, out var associatedProgram))
                        {
                            result = ExecuteWithAssociatedProgram(filePath, associatedProgram);
                        }
                        // Fall back to shell execute (Windows default handler)
                        else
                        {
                            result = ExecuteWithShellExecute(filePath);
                        }
                        break;

                    case ExecutionMode.Editor:
                        // Open with configured text editor
                        result = ExecuteWithEditor(filePath, config.TextEditorPath);
                        break;

                    case ExecutionMode.ExplorerAssociation:
                        // Use Windows Explorer's file association
                        result = ExecuteWithShellExecute(filePath);
                        break;

                    default:
                        return new OperationResult
                        {
                            Success = false,
                            Message = $"Unknown execution mode: {mode}"
                        };
                }

                result.Duration = DateTime.Now - startTime;
            }
            catch (Exception ex)
            {
                result.Success = false;
                result.Message = $"Execution failed: {ex.Message}";
                result.Errors.Add(ex.Message);
            }

            return result;
        }

        /// <summary>
        /// Determines if a file is executable (has .exe, .bat, .cmd, .com extension)
        /// </summary>
        private bool IsExecutableFile(string filePath)
        {
            var executableExtensions = new[] { ".exe", ".bat", ".cmd", ".com", ".ps1" };
            var extension = Path.GetExtension(filePath).ToLowerInvariant();
            return executableExtensions.Contains(extension);
        }

        /// <summary>
        /// Executes an executable file directly
        /// </summary>
        private OperationResult ExecuteExecutableFile(string filePath)
        {
            try
            {
                var startInfo = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = filePath,
                    WorkingDirectory = Path.GetDirectoryName(filePath) ?? string.Empty,
                    UseShellExecute = true
                };

                System.Diagnostics.Process.Start(startInfo);

                return new OperationResult
                {
                    Success = true,
                    Message = $"Executed: {Path.GetFileName(filePath)}",
                    FilesProcessed = 1
                };
            }
            catch (Exception ex)
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Failed to execute: {ex.Message}",
                    Errors = new List<string> { ex.Message }
                };
            }
        }

        /// <summary>
        /// Executes a file with an associated program
        /// </summary>
        private OperationResult ExecuteWithAssociatedProgram(string filePath, string programPath)
        {
            try
            {
                var startInfo = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = programPath,
                    Arguments = $"\"{filePath}\"",
                    UseShellExecute = true
                };

                System.Diagnostics.Process.Start(startInfo);

                return new OperationResult
                {
                    Success = true,
                    Message = $"Opened {Path.GetFileName(filePath)} with {Path.GetFileName(programPath)}",
                    FilesProcessed = 1
                };
            }
            catch (Exception ex)
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Failed to open with associated program: {ex.Message}",
                    Errors = new List<string> { ex.Message }
                };
            }
        }

        /// <summary>
        /// Executes a file using Windows shell execute (default handler)
        /// </summary>
        private OperationResult ExecuteWithShellExecute(string filePath)
        {
            try
            {
                var startInfo = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = filePath,
                    UseShellExecute = true,
                    Verb = "open"
                };

                System.Diagnostics.Process.Start(startInfo);

                return new OperationResult
                {
                    Success = true,
                    Message = $"Opened: {Path.GetFileName(filePath)}",
                    FilesProcessed = 1
                };
            }
            catch (Exception ex)
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Failed to open file: {ex.Message}",
                    Errors = new List<string> { ex.Message }
                };
            }
        }

        /// <summary>
        /// Opens a file with the configured text editor
        /// </summary>
        private OperationResult ExecuteWithEditor(string filePath, string editorPath)
        {
            try
            {
                var launcher = new EditorLauncher();
                // We use editorPath as preferred editor. 
                // EditorLauncher handles the logic of ignoring "notepad.exe" on Linux/Mac if needed,
                // or we can rely on its smart defaults if editorPath is empty.
                
                int exitCode = launcher.LaunchEditorAndWait(filePath, editorPath);

                if (exitCode == 0)
                {
                    return new OperationResult
                    {
                        Success = true,
                        Message = $"Opened {Path.GetFileName(filePath)} in editor",
                        FilesProcessed = 1
                    };
                }
                else
                {
                    return new OperationResult
                    {
                        Success = false, // Or true if we consider "ran but failed" as success? usually non-zero is error.
                        Message = $"Editor exited with code {exitCode}",
                        FilesProcessed = 0
                    };
                }
            }
            catch (Exception ex)
            {
                return new OperationResult
                {
                    Success = false,
                    Message = $"Failed to open in editor: {ex.Message}",
                    Errors = new List<string> { ex.Message }
                };
            }
        }

        protected virtual void OnProgressChanged(ProgressEventArgs e)
        {
            ProgressChanged?.Invoke(this, e);
        }

        protected virtual void OnErrorOccurred(ErrorEventArgs e)
        {
            ErrorOccurred?.Invoke(this, e);
        }
    }

    /// <summary>
    /// Execution mode for file operations
    /// </summary>
    public enum ExecutionMode
    {
        Default,              // Execute with default handler or association
        Editor,               // Open with text editor (Shift+Enter)
        ExplorerAssociation   // Use Windows Explorer association (Ctrl+Enter)
    }
}
