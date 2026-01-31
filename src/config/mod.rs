mod loader;
pub use loader::{
    Config, ConfigureArgs, JoinTeamArgs, LcOrg, configure, join_team, show_configuration,
};

#[cfg(test)]
mod loader_tests;
