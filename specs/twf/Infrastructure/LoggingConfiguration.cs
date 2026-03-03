using Microsoft.Extensions.Logging;
using System;
using System.IO;

namespace TWF.Infrastructure
{
    /// <summary>
    /// Configures logging infrastructure for the application
    /// </summary>
    public static class LoggingConfiguration
    {
        private static ILoggerFactory? _loggerFactory;
        private static readonly object _lock = new object();
        private static LogLevel _minimumLogLevel = LogLevel.Information;

        /// <summary>
        /// Gets or sets the current minimum log level globally for all loggers
        /// </summary>
        public static LogLevel MinimumLogLevel 
        { 
            get => _minimumLogLevel; 
            private set => _minimumLogLevel = value; 
        }

        /// <summary>
        /// Initializes the logging infrastructure
        /// </summary>
        /// <param name="logLevel">Minimum log level</param>
        /// <param name="maxLogFiles">Maximum number of rotated log files to keep</param>
        public static void Initialize(string logLevel = "Information", int maxLogFiles = 5)
        {
            lock (_lock)
            {
                if (_loggerFactory != null)
                {
                    return;
                }

                _minimumLogLevel = ParseLogLevel(logLevel);

                _loggerFactory = LoggerFactory.Create(builder =>
                {
                    builder
                        .AddProvider(new FileLoggerProvider(maxLogFiles))
                        .SetMinimumLevel(LogLevel.Trace); 
                });
            }
        }

        /// <summary>
        /// Parses a log level string into a LogLevel enum
        /// </summary>
        private static LogLevel ParseLogLevel(string logLevel)
        {
            var parsedLevel = logLevel?.ToLowerInvariant() switch
            {
                "none" => LogLevel.None,
                "trace" => LogLevel.Trace,
                "debug" => LogLevel.Debug,
                "information" => LogLevel.Information,
                "warning" => LogLevel.Warning,
                "error" => LogLevel.Error,
                "critical" => LogLevel.Critical,
                _ => LogLevel.Information
            };

            return parsedLevel;
        }

        /// <summary>
        /// Gets a logger for the specified type
        /// </summary>
        public static ILogger<T> GetLogger<T>()
        {
            if (_loggerFactory == null)
            {
                Initialize();
            }

            return _loggerFactory!.CreateLogger<T>();
        }

        /// <summary>
        /// Gets a logger with the specified name
        /// </summary>
        public static ILogger GetLogger(string categoryName)
        {
            if (_loggerFactory == null)
            {
                Initialize();
            }

            return _loggerFactory!.CreateLogger(categoryName);
        }

        /// <summary>
        /// Changes the log level at runtime
        /// </summary>
        /// <param name="logLevel">New log level as string</param>
        public static void ChangeLogLevel(string logLevel)
        {
            lock (_lock)
            {
                var oldLogLevel = _minimumLogLevel;
                _minimumLogLevel = ParseLogLevel(logLevel);

                var logger = GetLogger("LoggingConfiguration");
                logger.LogInformation("Log level changed from {OldLevel} to {NewLevel}", oldLogLevel, _minimumLogLevel);
            }
        }

        /// <summary>
        /// Shuts down the logging infrastructure
        /// </summary>
        public static void Shutdown()
        {
            lock (_lock)
            {
                _loggerFactory?.Dispose();
                _loggerFactory = null;
            }
        }
    }

    /// <summary>
    /// Custom file logger provider
    /// </summary>
    internal class FileLoggerProvider : ILoggerProvider
    {
        private readonly string _logFilePath;
        private readonly object _lock = new object();

        public FileLoggerProvider(int maxLogFiles = 5)
        {
            var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            var logDirectory = Path.Combine(appData, "TWF", "logs");

            Directory.CreateDirectory(logDirectory);
            _logFilePath = Path.Combine(logDirectory, "twf_errors.log");

            // Migration: Move old log from TWF root to logs if it exists
            var oldLogPath = Path.Combine(appData, "TWF", "twf_errors.log");
            if (File.Exists(oldLogPath) && !File.Exists(_logFilePath))
            {
                try { File.Move(oldLogPath, _logFilePath); } catch { }
            }

            // Centralized rotation and cleanup
            TWF.Utilities.LogHelper.RotateAndCleanup(_logFilePath, maxLogFiles);
        }

        public ILogger CreateLogger(string categoryName)
        {
            return new FileLogger(categoryName, _logFilePath, _lock);
        }

        public void Dispose()
        {
            // Nothing to dispose
        }
    }

    /// <summary>
    /// Custom file logger
    /// </summary>
    internal class FileLogger : ILogger
    {
        private readonly string _categoryName;
        private readonly string _logFilePath;
        private readonly object _lock;

        public FileLogger(string categoryName, string logFilePath, object lockObject)
        {
            _categoryName = categoryName;
            _logFilePath = logFilePath;
            _lock = lockObject;

            // Log the creation of the logger
            try
            {
                var message = $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] [Information] [FileLoggerProvider] FileLogger: Created logger for category '{categoryName}'";
                lock (_lock)
                {
                    File.AppendAllText(_logFilePath, message + Environment.NewLine);
                }
            }
            catch
            {
                // Silently fail
            }
        }

        public IDisposable? BeginScope<TState>(TState state) where TState : notnull
        {
            return null;
        }

        public bool IsEnabled(LogLevel logLevel)
        {
            // Check the static global level live
            return logLevel >= LoggingConfiguration.MinimumLogLevel;
        }

        public void Log<TState>(
            LogLevel logLevel,
            EventId eventId,
            TState state,
            Exception? exception,
            Func<TState, Exception?, string> formatter)
        {
            if (!IsEnabled(logLevel))
            {
                return;
            }

            var message = formatter(state, exception);
            var logEntry = $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] [{logLevel}] [{_categoryName}] {message}";

            if (exception != null)
            {
                logEntry += Environment.NewLine + exception.ToString();
            }

            lock (_lock)
            {
                try
                {
                    File.AppendAllText(_logFilePath, logEntry + Environment.NewLine);
                }
                catch
                {
                    // Silently fail if we can't write to the log file
                }
            }
        }
    }
}
