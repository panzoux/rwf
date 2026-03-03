using System;
using System.IO;
using System.Threading.Tasks;

namespace TWF.Services
{
    /// <summary>
    /// Validates filesystem paths asynchronously with timeout protection.
    /// Prevents UI freezes when checking network shares.
    /// </summary>
    public class PathValidator
    {
        /// <summary>
        /// Checks if a directory exists and is accessible.
        /// </summary>
        public async Task<bool> IsPathAccessibleAsync(string path, int timeoutMs = 2000)
        {
            if (string.IsNullOrWhiteSpace(path)) return false;

            var task = Task.Run(() => 
            {
                try
                {
                    return Directory.Exists(path);
                }
                catch
                {
                    return false;
                }
            });

            if (await Task.WhenAny(task, Task.Delay(timeoutMs)) == task)
            {
                return await task;
            }
            
            return false; // Timeout
        }
    }
}
