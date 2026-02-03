mod cloud;
mod common;
mod config;
mod default;
mod hooks;
mod logging;
mod oauth;
mod providers;
pub mod tui;
mod upgrade;

pub use common::{PROJECT_NAME, PROJECT_VERSION};
pub use config::{JoinTeamArgs, join_team, show_configuration};
pub use hooks::{codex_hook, hook, install, list, uninstall};
pub use logging::Logging;
pub use oauth::{LoginArgs, login::login};
pub use providers::Providers;
pub use upgrade::{poll_upgrade, upgrade};

/// Menu option for the interactive menu
pub struct MenuOption {
    pub label: &'static str,
    pub action: MenuAction,
}

/// Actions that can be performed from the interactive menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    InitializeTeam,
    JoinTeam,
    InstallHooks,
    UninstallHooks,
    ListHooks,
    ShowConfiguration,
    Upgrade,
}

/// Returns all available menu options
#[must_use]
pub fn get_menu_options() -> Vec<MenuOption> {
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
