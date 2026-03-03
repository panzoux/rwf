namespace TWF.Models
{
    /// <summary>
    /// Represents cached drive statistics to avoid blocking I/O
    /// </summary>
    public readonly struct DriveStats
    {
        public bool IsReady { get; }
        public string VolumeLabel { get; }
        public long TotalSize { get; }
        public long AvailableFreeSpace { get; }
        public string DriveFormat { get; }
        public bool IsNetwork { get; }
        public bool IsLoading { get; }

        public DriveStats(bool isReady, string volumeLabel, long totalSize, long availableFreeSpace, string driveFormat, bool isNetwork, bool isLoading = false)
        {
            IsReady = isReady;
            VolumeLabel = volumeLabel;
            TotalSize = totalSize;
            AvailableFreeSpace = availableFreeSpace;
            DriveFormat = driveFormat;
            IsNetwork = isNetwork;
            IsLoading = isLoading;
        }

        public static DriveStats Loading => new DriveStats(false, "Loading...", 0, 0, "", false, true);
        public static DriveStats Offline => new DriveStats(false, "Offline/Unreachable", 0, 0, "", false, false);
    }
}
