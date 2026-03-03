namespace TWF.Models
{
    /// <summary>
    /// A lightweight, memory-efficient representation of a file system entry.
    /// Optimized for low memory usage when handling millions of files.
    /// </summary>
    public readonly struct FileSystemItem
    {
        public string Name { get; }
        public string FullPath { get; }
        public bool IsDirectory { get; }
        public long Size { get; }
        public DateTime LastModified { get; }
        public FileAttributes Attributes { get; }
        public string Extension { get; }

        public FileSystemItem(string fullPath, string name, bool isDirectory, long size, DateTime lastModified, FileAttributes attributes)
        {
            FullPath = fullPath;
            Name = name;
            IsDirectory = isDirectory;
            Size = size;
            LastModified = lastModified;
            Attributes = attributes;
            Extension = isDirectory ? string.Empty : Path.GetExtension(name);
        }
        
        /// <summary>
        /// Converts to the legacy FileEntry class for compatibility.
        /// </summary>
        public FileEntry ToFileEntry()
        {
            return new FileEntry
            {
                FullPath = FullPath,
                Name = Name,
                Extension = Extension,
                Size = Size,
                LastModified = LastModified,
                Attributes = Attributes,
                IsDirectory = IsDirectory,
                IsArchive = false, // Archive detection should be done separately/lazily
                IsVirtualFolder = false
            };
        }
    }
}
