mod cloud;
mod common;
mod config;
mod default;
mod hooks;
mod logging;
mod oauth;
mod providers;
mod upgrade;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use inquire::{Select, Text};
use log::warn;

use crate::{
    common::{PROJECT_NAME, PROJECT_VERSION},
    config::{JoinTeamArgs, join_team, show_configuration},
    hooks::{hook, install, list, uninstall},
    logging::Logging,
    oauth::{LoginArgs, login::login},
    providers::Providers,
    upgrade::{poll_upgrade, upgrade},
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
    CodexCallback,

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

/// Menu option for the interactive menu
struct MenuOption {
    label: &'static str,
    action: MenuAction,
}

/// Actions that can be performed from the interactive menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    InitializeTeam,
    JoinTeam,
    InstallHooks,
    UninstallHooks,
    ListHooks,
    ShowConfiguration,
    Upgrade,
}

/// Returns all available menu options
fn get_menu_options() -> Vec<MenuOption> {
    vec![
        MenuOption {
            label: "Initialize Team",
            action: MenuAction::InitializeTeam,
        },
        MenuOption {
            label: "Join Team",
            action: MenuAction::JoinTeam,
        },
        MenuOption {
            label: "Install Hooks",
            action: MenuAction::InstallHooks,
        },
        MenuOption {
            label: "Uninstall Hooks",
            action: MenuAction::UninstallHooks,
        },
        MenuOption {
            label: "List Hooks",
            action: MenuAction::ListHooks,
        },
        MenuOption {
            label: "Show Configuration",
            action: MenuAction::ShowConfiguration,
        },
        MenuOption {
            label: "Upgrade",
            action: MenuAction::Upgrade,
        },
    ]
}

/// Display the interactive menu and execute the selected action
fn show_menu() -> Result<()> {
    let options = get_menu_options();
    let labels: Vec<&str> = options.iter().map(|o| o.label).collect();

    let selection = Select::new("What would you like to do?", labels)
        .with_help_message("Use ↑↓ to navigate, Enter to select")
        .prompt()
        .context("Failed to read menu selection")?;

    // Find the matching action
    let action = options
        .into_iter()
        .find(|o| o.label == selection)
        .map(|o| o.action);

    match action {
        Some(MenuAction::InitializeTeam) => {
            let args = LoginArgs {
                no_browser: false,
                existing_org: None,
            };
            login(&args)
        }
        Some(MenuAction::JoinTeam) => {
            let url = Text::new("Enter the team URL:")
                .prompt()
                .context("Failed to read team URL")?;
            let args = JoinTeamArgs { url };
            join_team(&args)
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
        Some(Command::CodexCallback) => hook(Providers::Codex),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_options_count() {
        let options = get_menu_options();
        // Should have 7 menu options (user-facing only, not hidden callbacks)
        assert_eq!(options.len(), 7);
    }

    #[test]
    fn test_menu_options_labels_are_unique() {
        let options = get_menu_options();
        let labels: Vec<_> = options.iter().map(|o| o.label).collect();

        let mut unique_labels = labels.clone();
        unique_labels.sort();
        unique_labels.dedup();
        assert_eq!(labels.len(), unique_labels.len());
    }

    #[test]
    fn test_menu_options_labels_not_empty() {
        let options = get_menu_options();
        for option in &options {
            assert!(!option.label.is_empty());
        }
    }

    #[test]
    fn test_menu_options_initialize_team_is_first() {
        let options = get_menu_options();
        // Initialize Team should be first (most common action for new users)
        assert_eq!(options[0].action, MenuAction::InitializeTeam);
    }

    #[test]
    fn test_menu_options_contains_all_actions() {
        let options = get_menu_options();
        let actions: Vec<_> = options.iter().map(|o| o.action).collect();

        assert!(actions.contains(&MenuAction::InitializeTeam));
        assert!(actions.contains(&MenuAction::JoinTeam));
        assert!(actions.contains(&MenuAction::InstallHooks));
        assert!(actions.contains(&MenuAction::UninstallHooks));
        assert!(actions.contains(&MenuAction::ListHooks));
        assert!(actions.contains(&MenuAction::ShowConfiguration));
        assert!(actions.contains(&MenuAction::Upgrade));
    }

    #[test]
    fn test_menu_lookup_finds_initialize_team() {
        let options = get_menu_options();
        let label = options
            .iter()
            .find(|o| o.action == MenuAction::InitializeTeam)
            .map(|o| o.label)
            .unwrap();

        let found = options
            .into_iter()
            .find(|o| o.label == label)
            .map(|o| o.action);

        assert_eq!(found, Some(MenuAction::InitializeTeam));
    }

    #[test]
    fn test_menu_lookup_finds_join_team() {
        let options = get_menu_options();
        let label = options
            .iter()
            .find(|o| o.action == MenuAction::JoinTeam)
            .map(|o| o.label)
            .unwrap();

        let found = options
            .into_iter()
            .find(|o| o.label == label)
            .map(|o| o.action);

        assert_eq!(found, Some(MenuAction::JoinTeam));
    }

    #[test]
    fn test_menu_lookup_unknown_label_returns_none() {
        let options = get_menu_options();

        let found = options
            .into_iter()
            .find(|o| o.label == "Unknown Action")
            .map(|o| o.action);

        assert_eq!(found, None);
    }
}
