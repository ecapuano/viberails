use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use log::warn;

use viberails::{
    JoinTeamArgs, Logging, LoginArgs, MenuAction, PROJECT_NAME, PROJECT_VERSION, Providers,
    codex_hook, get_menu_options, hook, install, join_team, list, login, poll_upgrade,
    show_configuration,
    tui::{select_prompt, text_prompt},
    uninstall, upgrade,
};

#[derive(Parser)]
#[command(version =  PROJECT_VERSION, about, long_about = None)]
pub struct UserArgs {
    #[command(subcommand)]
    command: Option<Command>,

    /// Verbose
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize team configuration via OAuth
    #[command(visible_alias = "init")]
    InitTeam(LoginArgs),

    /// Join an existing team using a team URL
    #[command(visible_alias = "join")]
    JoinTeam(JoinTeamArgs),

    /// Show Config
    #[command(visible_alias = "show-config")]
    ShowConfiguration,

    /// Install hooks
    Install,
    /// Uninstall hooks
    Uninstall,

    /// List Hooks
    #[command(visible_alias = "ls")]
    List,

    /// Upgrade
    Upgrade,

    // Provider callback commands (internal - used by hooks)
    /// Claude Code callback
    #[command(visible_alias = "cc", hide = true)]
    ClaudeCallback,

    /// Cursor callback
    #[command(hide = true)]
    CursorCallback,

    /// Gemini CLI callback
    #[command(hide = true)]
    GeminiCallback,

    /// `OpenAI` Codex callback
    #[command(hide = true)]
    CodexCallback {
        /// JSON payload from Codex (passed as command line argument)
        payload: String,
    },

    /// `OpenCode` callback
    #[command(hide = true)]
    OpencodeCallback,

    /// `OpenClaw` callback
    #[command(hide = true)]
    OpenclawCallback,
}

fn init_logging(verbose: bool) -> Result<()> {
    if verbose {
        Logging::new().start()
    } else {
        let file_name = format!("{PROJECT_NAME}.log");
        Logging::new().with_file(file_name).start()
    }
}

/// Display the interactive menu and execute the selected action
fn show_menu() -> Result<()> {
    let options = get_menu_options();
    let labels: Vec<&str> = options.iter().map(|o| o.label).collect();

    let selection_idx = select_prompt(
        "What would you like to do?",
        labels,
        Some("↑↓ navigate, Enter select, Esc cancel"),
    )
    .context("Failed to read menu selection")?;

    // Handle cancellation
    let Some(idx) = selection_idx else {
        return Ok(());
    };

    // Get the action for the selected index
    let action = options.get(idx).map(|o| o.action);

    match action {
        Some(MenuAction::InitializeTeam) => {
            let args = LoginArgs {
                no_browser: false,
                existing_org: None,
            };
            login(&args).context("Login Failure")?;
            install()
        }
        Some(MenuAction::JoinTeam) => {
            let url = text_prompt::<fn(&str) -> viberails::tui::ValidationResult>(
                "Enter the team URL:",
                Some("Enter to submit, Esc to cancel"),
                None,
            )
            .context("Failed to read team URL")?;

            let Some(url) = url else {
                return Ok(());
            };

            let args = JoinTeamArgs { url };
            join_team(&args).context("Unable to join team")?;
            install()
        }
        Some(MenuAction::InstallHooks) => install(),
        Some(MenuAction::UninstallHooks) => uninstall(),
        Some(MenuAction::ListHooks) => {
            list();
            Ok(())
        }
        Some(MenuAction::ShowConfiguration) => show_configuration(),
        Some(MenuAction::Upgrade) => upgrade(),
        None => Ok(()),
    }
}

fn main() -> Result<()> {
    let args = UserArgs::parse();

    init_logging(args.verbose)?;

    let ret = match args.command {
        None => show_menu(),
        Some(Command::Install) => install(),
        Some(Command::Uninstall) => uninstall(),
        Some(Command::List) => {
            list();
            Ok(())
        }
        Some(Command::ShowConfiguration) => show_configuration(),
        Some(Command::InitTeam(args)) => login(&args),
        Some(Command::JoinTeam(args)) => join_team(&args),
        Some(Command::Upgrade) => upgrade(),

        // Provider callbacks
        Some(Command::ClaudeCallback) => hook(Providers::ClaudeCode),
        Some(Command::CursorCallback) => hook(Providers::Cursor),
        Some(Command::GeminiCallback) => hook(Providers::GeminiCli),
        Some(Command::CodexCallback { payload }) => codex_hook(&payload),
        Some(Command::OpencodeCallback) => hook(Providers::OpenCode),
        Some(Command::OpenclawCallback) => hook(Providers::OpenClaw),
    };

    //
    // This'll try to upgrade every x hours on exit
    //
    if let Err(e) = poll_upgrade() {
        warn!("upgrade failure: {e}");
    }

    ret
}
