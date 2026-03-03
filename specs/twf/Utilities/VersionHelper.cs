using System;
using System.Reflection;
using System.Runtime.InteropServices;

namespace TWF.Utilities
{
    /// <summary>
    /// Helper class to retrieve application version and environment information
    /// </summary>
    public static class VersionHelper
    {
        /// <summary>
        /// Gets the application version information string
        /// </summary>
        public static string GetVersionInfo()
        {
            var assembly = Assembly.GetExecutingAssembly();
            var name = assembly.GetName().Name ?? "TWF";
            
            // Get InformationalVersion (this contains our yyyyMMdd.HHmm format from csproj)
            var infoVersion = assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
            
            if (string.IsNullOrEmpty(infoVersion))
            {
                var version = assembly.GetName().Version;
                return $"{name} Version {version}";
            }

            // Split 0.1.20260108.1430 into "0.1" and "20260108.1430"
            var parts = infoVersion.Split('.');
            if (parts.Length >= 4)
            {
                return $"{name} Version {parts[0]}.{parts[1]} build {parts[2]}.{parts[3]}";
            }

            return $"{name} Version {infoVersion}";
        }

        /// <summary>
        /// Gets information about the runtime and operating system
        /// </summary>
        public static string GetRuntimeInfo()
        {
            return $"Runtime: {RuntimeInformation.FrameworkDescription} on {RuntimeInformation.OSDescription}";
        }
    }
}
