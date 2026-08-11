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

    /// Print the embedded English action description file (action_descriptions.en.json) to stdout
    /// and exit. Use this as a template for creating custom translation files.
    #[arg(long)]
    export_function_list: bool,

    /// Export all default config files to DIR (skips files that already exist).
    #[arg(long, value_name = "DIR")]
    export_config_files: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Export embedded action descriptions and exit (no UI needed)
    if args.export_function_list {
        print!(
            "{}",
            include_str!("../../rwf-lib/resources/action_descriptions.en.json")
        );
        return Ok(());
    }

    // Export default config files and exit
    if let Some(dir) = &args.export_config_files {
        std::fs::create_dir_all(dir)?;
        export_default_configs(dir)?;
        return Ok(());
    }

    // Get proper app data directory based on OS
    let log_dir = rwf_lib::logging::default_log_dir();

    // Initialize logging (this sets up the global tracing subscriber)
    // and correctly bridges with our in-memory LogManager
    rwf_lib::logging::init_logging(rwf_lib::logging::LogLevel::Information, &log_dir)?;

    info!("Two-Pane File Manager starting...");

    // Phase 7.15 stages 1-2: start a diagnostic session for the whole run when
    // RWF_DIAGNOSTICS is set. The interactive F12 toggle arrives in stage 5;
    // this makes event capture usable (and the §1.5 wake-timing hypothesis
    // testable) before any UI exists for it.
    if std::env::var("RWF_DIAGNOSTICS").is_ok_and(|v| v != "0" && !v.is_empty()) {
        let root = rwf_lib::diagnostics::default_diagnostics_dir();
        match rwf_lib::diagnostics::start_session(root, "env") {
            Some(paths) => info!("Diagnostic session recording to {:?}", paths.dir),
            None => info!("Diagnostic session could not be started"),
        }
    }

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
            info!("Keybindings loaded from {:?}", kb_path);
            (kb, rwf_lib::config::ConfigLoadResult::ok(kb_path))
        }
        Err(e) => {
            let result = if kb_exists {
                tracing::warn!(
                    "Failed to parse {:?}, using built-in defaults: {:?}",
                    kb_path,
                    e
                );
                rwf_lib::config::ConfigLoadResult::error(kb_path, e.to_string())
            } else {
                tracing::info!(
                    "Keybindings file not found at {:?}, using built-in defaults",
                    kb_path
                );
                rwf_lib::config::ConfigLoadResult::default_fallback(kb_path, "built-in defaults")
            };
            (rwf_lib::KeyBindings::default(), result)
        }
    };
    state.config.key_bindings = key_bindings.clone();
    info!("Key bindings ready");

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

    // Phase 7.15 stages 1-2: finalise an env-triggered session. Stage 5 replaces
    // the placeholder report with the user's description from the exit prompt.
    if let Some(paths) = rwf_lib::diagnostics::stop_session(None) {
        println!("Diagnostic session written to {}", paths.dir.display());
        println!("It contains file paths and screen contents — review before sharing.");
    }

    // Output directory to stdout if -cwd flag was provided or Shift+Q was pressed
    if args.cwd || app.should_output_directory() {
        let exit_dir = app.get_exit_directory_public();
        println!("{}", exit_dir);
        info!("Output exit directory: {}", exit_dir);
    }

    Ok(())
}

fn write_if_absent(dir: &std::path::Path, name: &str, content: &str) -> Result<()> {
    let path = dir.join(name);
    if path.exists() {
        println!("skipped (exists): {}", path.display());
        return Ok(());
    }
    std::fs::write(&path, content)?;
    println!("written:          {}", path.display());
    Ok(())
}

fn export_default_configs(dir: &std::path::Path) -> Result<()> {
    write_if_absent(
        dir,
        "keybindings.json",
        include_str!("../../rwf-lib/resources/default_keybindings.json"),
    )?;
    write_if_absent(
        dir,
        "custom_functions.json",
        rwf_lib::DEFAULT_CUSTOM_FUNCTIONS,
    )?;
    write_if_absent(dir, "menu_config.json", rwf_lib::DEFAULT_MENU_CONFIG)?;
    write_if_absent(dir, "file_type_map.json", rwf_lib::DEFAULT_FILE_TYPE_MAP)?;
    write_if_absent(
        dir,
        "extension_associations.json",
        rwf_lib::DEFAULT_EXTENSION_ASSOCIATIONS,
    )?;
    write_if_absent(
        dir,
        "action_descriptions.en.json",
        include_str!("../../rwf-lib/resources/action_descriptions.en.json"),
    )?;
    let config_json = serde_json::to_string_pretty(&rwf_lib::config::AppConfig::default())?;
    write_if_absent(dir, "config.json", &config_json)?;
    Ok(())
}
