mod loader;
pub use loader::{Config, JoinTeamArgs, LcOrg, is_authorized, join_team, show_configuration};

#[cfg(test)]
mod loader_tests;
