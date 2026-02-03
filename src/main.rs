use std::io::{self, Write};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use log::warn;

use viberails::{
    JoinTeamArgs, Logging, LoginArgs, MenuAction, PROJECT_NAME, PROJECT_VERSION, Providers,
    codex_hook, get_menu_options, hook, install, is_authorized, join_team, list, login,
    poll_upgrade, show_configuration,
    tui::{MessageStyle, message_prompt, select_prompt, text_prompt},
    uninstall, uninstall_hooks, upgrade,
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

/// Wait for user to press any key before continuing.
/// Used to let users see output before returning to the menu.
fn wait_for_keypress() {
    print!("\nPress any key to continue...");
    let _ = io::stdout().flush();

    // Enable raw mode to capture single keypress
    if enable_raw_mode().is_ok() {
        loop {
            if let Ok(Event::Key(key)) = event::read()
                && key.kind == KeyEventKind::Press
            {
                break;
            }
        }
        let _ = disable_raw_mode();
    }
    println!();
}

/// Display the interactive menu and execute the selected action in a loop
fn show_menu() -> Result<()> {
    loop {
        let options = get_menu_options();
        let labels: Vec<&str> = options.iter().map(|o| o.label).collect();

        let selection_idx = select_prompt(
            "What would you like to do?",
            labels,
            Some("↑↓ navigate, Enter select, Esc cancel"),
        )
        .context("Failed to read menu selection")?;

        // Handle cancellation (Esc) - exit the loop
        let Some(idx) = selection_idx else {
            return Ok(());
        };

        // Get the action for the selected index
        let action = options.get(idx).map(|o| o.action);

        let result = match action {
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
                    continue; // User cancelled, show menu again
                };

                let args = JoinTeamArgs { url };
                join_team(&args).context("Unable to join team")?;
                install()
            }
            Some(MenuAction::InstallHooks) => {
                if !is_authorized() {
                    let _ = message_prompt(
                        " Not Logged In ",
                        "Please initialize or join a team first",
                        MessageStyle::Error,
                    );
                    continue;
                }
                let r = install();
                wait_for_keypress();
                r
            }
            Some(MenuAction::UninstallHooks) => {
                let r = uninstall_hooks();
                wait_for_keypress();
                r
            }
            Some(MenuAction::UninstallFully) => {
                let r = uninstall();
                wait_for_keypress();
                r
            }
            Some(MenuAction::ListHooks) => {
                list();
                wait_for_keypress();
                Ok(())
            }
            Some(MenuAction::ShowConfiguration) => {
                let r = show_configuration();
                wait_for_keypress();
                r
            }
            Some(MenuAction::Upgrade) => {
                let r = upgrade();
                wait_for_keypress();
                r
            }
            Some(MenuAction::Quit) | None => return Ok(()),
        };

        // If an action failed, propagate the error
        result?;
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
