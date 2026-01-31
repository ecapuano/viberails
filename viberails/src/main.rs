mod cloud;
mod common;
mod config;
mod default;
mod hooks;
mod logging;
mod oauth;
mod providers;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{
    common::PROJECT_NAME,
    config::{ConfigureArgs, JoinTeamArgs, configure, join_team, show_configuration},
    hooks::{hook, install, list, uninstall},
    logging::Logging,
    oauth::{LoginArgs, login::login},
    providers::Providers,
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct UserArgs {
    #[command(subcommand)]
    command: Command,

    /// Verbose
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize team configuration via OAuth
    InitTeam(LoginArgs),

    /// Join an existing team using a team URL
    JoinTeam(JoinTeamArgs),

    /// Configure
    #[command(visible_alias = "config")]
    Configure(Box<ConfigureArgs>),

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
    CodexCallback,

    /// `OpenCode` callback
    #[command(hide = true)]
    OpencodeCallback,

    /// Clawdbot/OpenClaw callback
    #[command(hide = true)]
    ClawdbotCallback,
}

fn init_logging(verbose: bool) -> Result<()> {
    if verbose {
        Logging::new().start()
    } else {
        let file_name = format!("{PROJECT_NAME}.log");
        Logging::new().with_file(file_name).start()
    }
}

fn main() -> Result<()> {
    let args = UserArgs::parse();

    init_logging(args.verbose)?;

    match args.command {
        Command::Install => install(),
        Command::Uninstall => uninstall(),
        Command::List => list(),
        Command::Configure(a) => configure(&a),
        Command::ShowConfiguration => show_configuration(),
        Command::InitTeam(args) => login(&args),
        Command::JoinTeam(args) => join_team(&args),

        // Provider callbacks
        Command::ClaudeCallback => hook(Providers::ClaudeCode),
        Command::CursorCallback => hook(Providers::Cursor),
        Command::GeminiCallback => hook(Providers::GeminiCli),
        Command::CodexCallback => hook(Providers::Codex),
        Command::OpencodeCallback => hook(Providers::OpenCode),
        Command::ClawdbotCallback => hook(Providers::Clawdbot),
    }
}
