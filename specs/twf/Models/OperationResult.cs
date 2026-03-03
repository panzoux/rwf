namespace TWF.Models
{
    /// <summary>
    /// Represents the result of a file operation
    /// </summary>
    public class OperationResult
    {
        public bool Success { get; set; }
        public string Message { get; set; } = string.Empty;
        public int FilesProcessed { get; set; }
        public int FilesSkipped { get; set; }
        public List<string> Errors { get; set; } = new List<string>();
        public TimeSpan Duration { get; set; }
    }

    public enum FileOperationStatus
    {
        Queued,
        Started,
        Processing,
        Completed,
        Failed,
        Skipped
    }

    /// <summary>
    /// Event arguments for operation progress reporting
    /// </summary>
    public class ProgressEventArgs : EventArgs
    {
        public FileOperationStatus Status { get; set; } = FileOperationStatus.Processing;
        public string CurrentFile { get; set; } = string.Empty;
        public int CurrentFileIndex { get; set; }
        public int TotalFiles { get; set; }
        public int FilesProcessed { get; set; }
        public long BytesProcessed { get; set; }
        public long TotalBytes { get; set; }
        public double PercentComplete { get; set; }
        
        // For individual file progress
        public long CurrentFileBytesProcessed { get; set; }
        public long CurrentFileTotalBytes { get; set; }
    }
}
