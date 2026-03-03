using System.Text;
using Microsoft.Extensions.Logging;
using TWF.Controllers;
using TWF.Services;
using TWF.Providers;
using TWF.Infrastructure;

namespace TWF
{
    /// <summary>
    /// Entry point for TWF (Two-pane Window Filer) application
    /// </summary>
    class Program
    {
        /// <summary>
        /// Reads initial logging settings from the config file without initializing the full configuration system
        /// </summary>
        private static (string LogLevel, int MaxLogFiles) ReadInitialConfig()
        {
            string logLevel = "Information";
            int maxLogFiles = 5;

            try
            {
                var configDirectory = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                    "TWF");

                var configPath = Path.Combine(configDirectory, "config.json");

                if (File.Exists(configPath))
                {
                    var json = File.ReadAllText(configPath);
                    using var doc = System.Text.Json.JsonDocument.Parse(json);
                    
                    if (doc.RootElement.TryGetProperty("LogLevel", out var logLevelElement))
                    {
                        logLevel = logLevelElement.GetString() ?? "Information";
                    }

                    if (doc.RootElement.TryGetProperty("Display", out var displayElement) &&
                        displayElement.TryGetProperty("MaxLogFiles", out var maxFilesElement))
                    {
                        maxLogFiles = maxFilesElement.GetInt32();
                    }
                }
            }
            catch { }

            return (logLevel, maxLogFiles);
        }

        static void Main(string[] args)
        {
            // Parse arguments
            string? changeDirectoryOutputFile = null;
            for (int i = 0; i < args.Length; i++)
            {
                if (args[i] == "-cwd" && i + 1 < args.Length)
                {
                    changeDirectoryOutputFile = args[i + 1];
                    i++;
                }
                else if (args[i].StartsWith("--cwd-file="))
                {
                    changeDirectoryOutputFile = args[i].Substring("--cwd-file=".Length);
                }
            }

            // Register encoding provider for Japanese and other code pages
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

            // Read initial config from file without initializing full logging system
            var initialConfig = ReadInitialConfig();

            // Set up logging infrastructure with settings from config
            LoggingConfiguration.Initialize(initialConfig.LogLevel, initialConfig.MaxLogFiles);
            var logger = LoggingConfiguration.GetLogger<Program>();

            // Now load the full configuration
            var configProvider = new ConfigurationProvider();
            var config = configProvider.LoadConfiguration();

            try
            {
                logger.LogInformation("Starting TWF - Two-pane Window Filer");

                // Create all dependencies (using parameterless constructors)
                var fileSystemProvider = new FileSystemProvider();
                var listProvider = new ListProvider(configProvider, LoggingConfiguration.GetLogger<ListProvider>());
                
                var sortEngine = new SortEngine();
                var markingEngine = new MarkingEngine();
                
                // Initialize Migemo if enabled and available
                IMigemoProvider? migemoProvider = null;
                if (config.Migemo.Enabled)
                {
                    migemoProvider = new MigemoProvider(
                        config.Migemo.DictPath
                    );
                    if (migemoProvider.IsAvailable)
                    {
                        logger.LogInformation("Migemo search enabled");
                    }
                    else
                    {
                        logger.LogInformation("Migemo not available (library or dictionaries not found)");
                    }
                }
                
                var searchEngine = new SearchEngine(migemoProvider);
                
                var archiveManager = new ArchiveManager();
                var fileOps = new FileOperations();
                var viewerManager = new ViewerManager(searchEngine);
                var historyManager = new HistoryManager(config);
                
                var keyBindings = new KeyBindingManager();
                
                var macroExpander = new MacroExpander();
                var customFunctionManager = new CustomFunctionManager(macroExpander, configProvider, LoggingConfiguration.GetLogger<CustomFunctionManager>());
                
                // Create JobManager with concurrency and throttle settings from config (min 100ms throttle)
                int updateInterval = Math.Max(100, config.Display.TaskPanelUpdateIntervalMs);
                var jobManager = new JobManager(
                    LoggingConfiguration.GetLogger<JobManager>(),
                    config.Display.MaxSimultaneousJobs,
                    updateInterval
                );
                
                // Create MenuManager with config directory path
                var menuManager = new MenuManager(
                    configProvider.GetConfigDirectory(),
                    LoggingConfiguration.GetLogger<MenuManager>()
                );

                // Create and initialize MainController
                var controller = new MainController(
                    keyBindings,
                    fileOps,
                    markingEngine,
                    sortEngine,
                    searchEngine,
                    archiveManager,
                    viewerManager,
                    configProvider,
                    fileSystemProvider,
                    listProvider,
                    customFunctionManager,
                    menuManager,
                    historyManager,
                    jobManager,
                    LoggingConfiguration.GetLogger<MainController>()
                );

                try
                {
                    if (!string.IsNullOrEmpty(changeDirectoryOutputFile))
                    {
                        controller.ChangeDirectoryOutputFile = changeDirectoryOutputFile;
                    }

                    controller.Initialize();
                    controller.Run();

                    logger.LogInformation("TWF application exited normally");
                }
                finally
                {
                    // Ensure proper cleanup on all exit paths
                    controller.Dispose();
                }
            }
            catch (Exception ex)
            {
                logger.LogCritical(ex, "Fatal error in TWF application");
                Console.WriteLine($"Fatal error: {ex.Message}");
                Console.WriteLine("Press any key to exit...");
                Console.ReadKey();
                Environment.Exit(1);
            }
        }
    }
}
