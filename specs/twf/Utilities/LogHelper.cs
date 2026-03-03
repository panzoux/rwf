using System;
using System.IO;
using System.Linq;

namespace TWF.Utilities
{
    /// <summary>
    /// Utility for log file rotation and retention management
    /// </summary>
    public static class LogHelper
    {
        private const long MaxLogSize = 10 * 1024 * 1024; // 10MB

        /// <summary>
        /// Rotates the log file if it exceeds the size limit and cleans up old compressed/rotated logs
        /// </summary>
        /// <param name="logPath">Path to the active log file</param>
        /// <param name="maxFiles">Maximum number of rotated log files to keep (0 = keep all)</param>
        public static void RotateAndCleanup(string logPath, int maxFiles)
        {
            try
            {
                if (!File.Exists(logPath)) return;

                var fileInfo = new FileInfo(logPath);
                if (fileInfo.Length >= MaxLogSize)
                {
                    string dir = Path.GetDirectoryName(logPath) ?? string.Empty;
                    string name = Path.GetFileNameWithoutExtension(logPath);
                    string ext = Path.GetExtension(logPath);
                    string timestamp = DateTime.Now.ToString("yyyyMMdd_HHmmss");
                    string backupPath = Path.Combine(dir, $"{name}_{timestamp}{ext}");

                    File.Move(logPath, backupPath);
                }

                if (maxFiles > 0)
                {
                    CleanupOldLogs(logPath, maxFiles);
                }
            }
            catch
            {
                // Silently fail to avoid crashing the app due to logging issues
            }
        }

        /// <summary>
        /// Deletes old rotated logs, keeping only the specified number of most recent ones
        /// </summary>
        private static void CleanupOldLogs(string logPath, int maxFiles)
        {
            try
            {
                string dir = Path.GetDirectoryName(logPath) ?? string.Empty;
                if (!Directory.Exists(dir)) return;

                string name = Path.GetFileNameWithoutExtension(logPath);
                string ext = Path.GetExtension(logPath);
                string pattern = $"{name}_*{ext}";

                var oldLogs = Directory.GetFiles(dir, pattern)
                    .Select(f => new FileInfo(f))
                    .OrderByDescending(f => f.LastWriteTime)
                    .ToList();

                if (oldLogs.Count > maxFiles)
                {
                    var logsToDelete = oldLogs.Skip(maxFiles);
                    foreach (var log in logsToDelete)
                    {
                        try { log.Delete(); } catch { }
                    }
                }
            }
            catch
            {
                // Silently fail
            }
        }
    }
}
