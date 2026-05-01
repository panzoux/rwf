mod app;
mod performance;
mod terminal;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use rwf_lib::AppState;
use terminal::TerminalManager;
use tracing::info;

/// Two-Pane File Manager
#[derive(Parser, Debug)]
#[command(name = "rwf")]
#[command(about = "A two-pane file manager for the terminal", long_about = None)]
struct Args {
    /// Enable directory change on exit (outputs final directory to stdout)
    #[arg(long)]
    cwd: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();
    
    // Get proper app data directory based on OS
    let log_dir = rwf_lib::logging::default_log_dir();
    
    // Initialize logging (this sets up the global tracing subscriber)
    // and correctly bridges with our in-memory LogManager
    rwf_lib::logging::init_logging(rwf_lib::logging::LogLevel::Information, &log_dir)?;

    info!("Two-Pane File Manager starting...");

    // Initialize terminal
    let mut terminal_manager = TerminalManager::new()?;
    info!("Terminal initialized");

    // Initialize application state with session restoration
    // Load configuration from file or use defaults
    let config_manager = rwf_lib::config::ConfigManager::new();
    let config = config_manager.load_config().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {:?}, using defaults", e);
        rwf_lib::config::AppConfig::default()
    });
    info!("Configuration loaded from {:?}", config_manager.config_path());
    
    let state = AppState::new_with_session(config);
    info!("Application state initialized with session restoration");

    // Create and run application
    let mut app = App::with_cwd_flag(state, args.cwd);
    app.run(terminal_manager.terminal_mut()).await?;

    // Restore terminal state
    terminal_manager.restore()?;
    info!("Terminal restored");
    
    // Output directory to stdout if -cwd flag was provided or Shift+Q was pressed
    if args.cwd || app.should_output_directory() {
        let exit_dir = app.get_exit_directory_public();
        println!("{}", exit_dir);
        info!("Output exit directory: {}", exit_dir);
    }
    
    Ok(())
}
