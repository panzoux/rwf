namespace TWF.Models
{
    /// <summary>
    /// Represents a bookmarked directory path for quick access
    /// </summary>
    public class RegisteredFolder
    {
        public string Name { get; set; } = string.Empty;
        public string Path { get; set; } = string.Empty;
        public int SortOrder { get; set; }
    }
}
