using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    /// Caches directory contents to provide instant navigation for recently visited folders.
    /// Uses LRU (Least Recently Used) eviction policy.
    /// </summary>
    public class DirectoryCache
    {
        private class CacheEntry
        {
            public List<FileEntry> Entries { get; set; } = new List<FileEntry>();
            public DateTime DirectoryTimestamp { get; set; }
            public DateTime LastAccessTime { get; set; }
        }

        private readonly ConcurrentDictionary<string, CacheEntry> _cache = new();
        private readonly int _capacity;
        private readonly object _lock = new();

        public DirectoryCache(int capacity = 20)
        {
            _capacity = capacity;
        }

        /// <summary>
        /// Tries to retrieve cached entries for a path.
        /// Validates the cache against the directory's LastWriteTime.
        /// </summary>
        public bool TryGet(string path, out List<FileEntry>? entries)
        {
            entries = null;
            if (string.IsNullOrWhiteSpace(path)) return false;

            if (_cache.TryGetValue(path, out var entry))
            {
                try
                {
                    // Validate timestamp
                    if (Directory.Exists(path))
                    {
                        var currentTimestamp = Directory.GetLastWriteTime(path);
                        if (currentTimestamp == entry.DirectoryTimestamp)
                        {
                            entry.LastAccessTime = DateTime.UtcNow;
                            entries = new List<FileEntry>(entry.Entries); // Return copy
                            return true;
                        }
                        else
                        {
                            Invalidate(path);
                        }
                    }
                    else
                    {
                        Invalidate(path);
                    }
                }
                catch
                {
                    Invalidate(path);
                }
            }
            return false;
        }

        /// <summary>
        /// Adds entries to the cache.
        /// </summary>
        public void Add(string path, List<FileEntry> entries)
        {
            if (string.IsNullOrWhiteSpace(path)) return;

            try
            {
                if (!Directory.Exists(path)) return;

                var timestamp = Directory.GetLastWriteTime(path);
                var entry = new CacheEntry
                {
                    Entries = new List<FileEntry>(entries), // Store copy
                    DirectoryTimestamp = timestamp,
                    LastAccessTime = DateTime.UtcNow
                };

                lock (_lock)
                {
                    // LRU Eviction
                    if (_cache.Count >= _capacity && !_cache.ContainsKey(path))
                    {
                        string? oldestKey = null;
                        DateTime oldestTime = DateTime.MaxValue;

                        foreach (var kvp in _cache)
                        {
                            if (kvp.Value.LastAccessTime < oldestTime)
                            {
                                oldestTime = kvp.Value.LastAccessTime;
                                oldestKey = kvp.Key;
                            }
                        }

                        if (oldestKey != null)
                        {
                            _cache.TryRemove(oldestKey, out _);
                        }
                    }

                    _cache[path] = entry;
                }
            }
            catch
            {
                // Ignore errors
            }
        }

        public void Invalidate(string path)
        {
            _cache.TryRemove(path, out _);
        }

        public void Clear()
        {
            _cache.Clear();
        }
    }
}
