namespace TWF.Models
{
    /// <summary>
    /// Represents information about a drive
    /// </summary>
    public class DriveInfo
    {
        public string DriveLetter { get; set; } = string.Empty;
        public string VolumeLabel { get; set; } = string.Empty;
        public DriveType DriveType { get; set; }
        public long TotalSize { get; set; }
        public long FreeSpace { get; set; }
    }
}
