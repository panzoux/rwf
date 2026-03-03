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
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Two-Pane File Manager starting...");

    // Initialize terminal
    let mut terminal_manager = TerminalManager::new()?;
    info!("Terminal initialized");

    // Initialize application state with session restoration
    let config = rwf_lib::state::AppConfig::default();
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
