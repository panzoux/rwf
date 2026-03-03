using System;
using System.Collections.Concurrent;
using System.IO;
using System.Threading.Tasks;
using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    /// Provides cached, asynchronous access to drive information to prevent UI blocking.
    /// </summary>
    public class DriveInfoService
    {
        private readonly ConcurrentDictionary<string, DriveStats> _cache = new();
        private readonly ConcurrentDictionary<string, DateTime> _lastUpdateTime = new();
        private readonly TimeSpan _cacheDuration = TimeSpan.FromSeconds(5);
        private readonly ConcurrentDictionary<string, Task> _activeUpdates = new();

        /// <summary>
        /// Gets the drive statistics for the specified path.
        /// Returns cached value immediately if available, or a "Loading" state while fetching in background.
        /// </summary>
        public DriveStats GetDriveStats(string path)
        {
            var root = GetPathRoot(path);
            if (string.IsNullOrEmpty(root)) return DriveStats.Offline;

            if (_cache.TryGetValue(root, out var stats))
            {
                // Check if stale
                if (_lastUpdateTime.TryGetValue(root, out var lastTime) && (DateTime.UtcNow - lastTime) > _cacheDuration)
                {
                    TriggerBackgroundUpdate(root);
                }
                return stats;
            }

            // Not in cache, trigger update and return Loading
            TriggerBackgroundUpdate(root);
            return DriveStats.Loading;
        }

        private string? GetPathRoot(string path)
        {
            try
            {
                return Path.GetPathRoot(path);
            }
            catch
            {
                return null;
            }
        }

        private void TriggerBackgroundUpdate(string root)
        {
            if (_activeUpdates.ContainsKey(root)) return;

            // Fire and forget
            Task.Run(async () =>
            {
                if (!_activeUpdates.TryAdd(root, Task.CompletedTask)) return;

                try
                {
                    // Timeout protection for network shares
                    var fetchTask = Task.Run(() => FetchDriveInfo(root));
                    var completedTask = await Task.WhenAny(fetchTask, Task.Delay(2000));

                    if (completedTask == fetchTask)
                    {
                        var stats = await fetchTask;
                        _cache[root] = stats;
                        _lastUpdateTime[root] = DateTime.UtcNow;
                    }
                    else
                    {
                        // Timeout
                        _cache[root] = DriveStats.Offline; // Mark as offline/slow temporarily
                    }
                }
                catch
                {
                    _cache[root] = DriveStats.Offline;
                }
                finally
                {
                    _activeUpdates.TryRemove(root, out _);
                }
            });
        }

        private DriveStats FetchDriveInfo(string root)
        {
            try
            {
                var driveInfo = new System.IO.DriveInfo(root);
                if (driveInfo.IsReady)
                {
                    return new DriveStats(
                        true,
                        driveInfo.VolumeLabel,
                        driveInfo.TotalSize,
                        driveInfo.AvailableFreeSpace,
                        driveInfo.DriveFormat,
                        driveInfo.DriveType == DriveType.Network
                    );
                }
                else
                {
                    return new DriveStats(
                        false, 
                        "Not Ready", 
                        0, 
                        0, 
                        "", 
                        driveInfo.DriveType == DriveType.Network
                    );
                }
            }
            catch
            {
                return DriveStats.Offline;
            }
        }
    }
}
