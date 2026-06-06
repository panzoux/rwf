mod app;
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
    let config_path = config_manager.config_path().to_path_buf();
    let (config, config_result) = match config_manager.load_config() {
        Ok(c) => {
            info!("Configuration loaded from {:?}", config_path);
            let result = rwf_lib::config::ConfigLoadResult::ok(config_path);
            (c, result)
        }
        Err(e) => {
            let is_not_found = matches!(&e, rwf_lib::config::ConfigError::IoError(io) if io.kind() == std::io::ErrorKind::NotFound);
            tracing::warn!("Failed to load config: {:?}, using defaults", e);
            let result = if is_not_found {
                rwf_lib::config::ConfigLoadResult::skipped(config_path, "file not found")
            } else {
                rwf_lib::config::ConfigLoadResult::error(config_path, format!("{:?}", e))
            };
            (rwf_lib::config::AppConfig::default(), result)
        }
    };

    let mut state = AppState::new_with_session(config);
    info!("Application state initialized with session restoration");

    // Load key bindings from keybindings.json (merges over defaults; falls back entirely on parse error)
    let kb_path = config_manager.keybindings_path().to_path_buf();
    let kb_exists = kb_path.exists();
    let (key_bindings, kb_result) = match rwf_lib::input::KeyBindings::load_from_file(&kb_path) {
        Ok(kb) => {
            info!("Key bindings loaded from {:?}", kb_path);
            (kb, rwf_lib::config::ConfigLoadResult::ok(kb_path))
        }
        Err(e) => {
            let result = if kb_exists {
                tracing::warn!("Failed to parse keybindings.json, using defaults: {:?}", e);
                rwf_lib::config::ConfigLoadResult::error(kb_path, e.to_string())
            } else {
                tracing::info!("No keybindings.json found, using defaults");
                rwf_lib::config::ConfigLoadResult::skipped(kb_path, "file not found")
            };
            (rwf_lib::KeyBindings::default(), result)
        }
    };
    info!("Key bindings loaded");

    // Prepend config.json and keybindings.json results so the order is:
    // config, keybindings, extension_associations, custom_functions, context_menu
    state.config_load_results.insert(0, kb_result);
    state.config_load_results.insert(0, config_result);

    // Create and run application
    let mut app = App::with_state_and_keybindings(state, args.cwd, key_bindings);
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
