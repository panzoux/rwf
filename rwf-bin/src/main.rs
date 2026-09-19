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

    /// For this run, let a user config file replace its built-in defaults instead
    /// of layering over them (same as "UseBuiltInDefaults": false).
    #[arg(long, conflicts_with = "no_user_config")]
    no_default_config: bool,

    /// Ignore the user config directory and run on the built-in defaults only
    /// (for reproducing bugs against a known configuration).
    #[arg(long)]
    no_user_config: bool,
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(run());
    // Not a plain drop: dropping a runtime waits for every blocking task, and a
    // listing of an unreachable SMB share sits in the OS for ~20 s. Quit already
    // gave in-flight jobs `QUIT_GRACE` to finish; anything left is abandoned.
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    result
}

async fn run() -> Result<()> {
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

    // Phase 7.26: must be set before the first config load (config.json below,
    // then every list-type file in AppState::new, and again on each reload).
    rwf_lib::config_layers::set_cli_layering(if args.no_user_config {
        rwf_lib::config_layers::CliLayering::NoUserConfig
    } else if args.no_default_config {
        rwf_lib::config_layers::CliLayering::NoDefaultConfig
    } else {
        rwf_lib::config_layers::CliLayering::FromConfig
    });

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
    let (config, config_result) = rwf_lib::config_layers::load_app_config(&config_manager);
    let config = config.unwrap_or_default();
    info!("Configuration: {:?}", config_result.status);

    let mut state = AppState::new_with_session(config);
    info!("Application state initialized with session restoration");

    // Phase 7.15: `RWF_DIAGNOSTICS=1` records the whole run, for cases where the
    // problem happens before you can reach a keybinding (startup, first paint).
    // `F12` is the normal way in. Placed after config load so it honours
    // Diagnostics.Enabled and Diagnostics.OutputDirectory; App::run picks the
    // running session up and writes the start snapshot and effective config.
    if state.config.diagnostics.enabled
        && std::env::var("RWF_DIAGNOSTICS").is_ok_and(|v| v != "0" && !v.is_empty())
    {
        let configured = state.config.diagnostics.output_directory.clone();
        let root = if configured.is_empty() {
            rwf_lib::diagnostics::default_diagnostics_dir()
        } else {
            std::path::PathBuf::from(state.registered_folders.expand_env_vars(&configured))
        };
        match rwf_lib::diagnostics::start_session(root, "env") {
            Some(paths) => info!("Diagnostic session recording to {:?}", paths.dir),
            None => info!("Diagnostic session could not be started"),
        }
    }

    // Load key bindings: the built-ins with keybindings.json layered over them (or
    // replacing them, per the layering mode); a parse error keeps the built-ins.
    let (key_bindings, kb_result) = rwf_lib::config_layers::load_keybindings(
        config_manager.keybindings_path(),
        state.config_layers.layering,
    );
    info!("Keybindings: {:?}", kb_result.status);
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
    //
    // MUST be stderr, never stdout. Stdout is a data channel here: the shell
    // integration documented in docs/USER_GUIDE.md captures it with
    // `local output=$(rwf -cwd)` and feeds the result straight to `cd`, so any
    // extra stdout line silently breaks directory-on-exit whenever a diagnostic
    // session is running. stderr is not captured by that substitution and is
    // still shown to the user once the TUI has been torn down.
    // `submit_report`: quit with the report prompt still open — recording
    // already stopped and the bundle is on disk with a placeholder report.
    if let Some(paths) = rwf_lib::diagnostics::stop_session(None)
        .or_else(|| rwf_lib::diagnostics::submit_report(None))
    {
        eprintln!("Diagnostic session written to {}", paths.dir.display());
        eprintln!("It contains file paths and screen contents — review before sharing.");
    }

    // Output directory to stdout if -cwd flag was provided or Shift+Q was pressed
    if args.cwd || app.should_output_directory() {
        let exit_dir = app.get_exit_directory_public();
        println!("{}", exit_dir);
        info!("Output exit directory: {}", exit_dir);
    }

    Ok(())
}

/// Printed after `--export-config-files`. file_type_map.json and
/// extension_associations.json are bare JSON arrays with no room for a comment,
/// so this is where their users learn it (the other files carry it in `_comment`).
const LAYERING_NOTE: &str = "
These files are layered over rwf's built-in defaults: an entry you keep overrides
the built-in of the same name/key, and an entry you delete falls back to the
built-in instead of disappearing. To remove a built-in, keep its entry and add
\"Disabled\": true (custom_functions.json, extension_associations.json,
file_type_map.json) or bind the key to null (keybindings.json).
Set \"UseBuiltInDefaults\": false in config.json to make these files replace the
built-ins instead.";

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
    write_if_absent(dir, "keybindings.json", rwf_lib::DEFAULT_KEYBINDINGS)?;
    write_if_absent(
        dir,
        "custom_functions.json",
        rwf_lib::DEFAULT_CUSTOM_FUNCTIONS,
    )?;
    write_if_absent(dir, "menu_config.json", rwf_lib::DEFAULT_MENU_CONFIG)?;
    write_if_absent(dir, "menu_clip.json", rwf_lib::DEFAULT_MENU_CLIP)?;
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
    write_if_absent(dir, "config.json", rwf_lib::DEFAULT_CONFIG)?;
    println!("{LAYERING_NOTE}");
    Ok(())
}
