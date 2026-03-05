mod app;
mod performance;
mod terminal;
mod ui;

use anyhow::Result;
use app::App;
use rwf_lib::AppState;
use terminal::TerminalManager;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber to write to file instead of stdout
    // This prevents logs from interfering with the TUI
    
    // Get proper app data directory based on OS
    // Windows: %APPDATA%\rwf\logs\session.log
    // Linux/macOS: ~/.config/rwf/logs/session.log
    let log_dir = if cfg!(target_os = "windows") {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("rwf")
            .join("logs")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("rwf")
            .join("logs")
    };
    
    // Create logs directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;
    
    let log_file_path = log_dir.join("session.log");
    let mut log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)?;
    
    // Write session start marker to confirm file is writable
    use std::io::Write;
    writeln!(log_file, "\n=== Log session started at {} ===", chrono::Local::now())?;
    
    // Try to get local timezone offset, fallback to UTC if it fails
    let timer = time::UtcOffset::current_local_offset()
        .ok()
        .map(|offset| {
            tracing_subscriber::fmt::time::OffsetTime::new(
                offset,
                time::format_description::well_known::Rfc3339,
            )
        });
    
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::DEBUG.into()),
        )
        .with_writer(std::sync::Arc::new(log_file))
        .with_ansi(false); // Disable ANSI colors in log file
    
    // Apply timer if we got local offset, otherwise use default (UTC)
    if let Some(timer) = timer {
        subscriber.with_timer(timer).init();
    } else {
        subscriber.init();
    }

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
    let mut app = App::new(state);
    app.run(terminal_manager.terminal_mut()).await?;

    // Restore terminal state
    terminal_manager.restore()?;
    info!("Terminal restored");
    
    Ok(())
}
