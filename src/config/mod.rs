mod loader;
pub use loader::{Config, JoinTeamArgs, LcOrg, join_team, show_configuration};

#[cfg(test)]
mod loader_tests;
