use std::{env, fs, path::PathBuf, sync::OnceLock};

use anyhow::{Context, Result, anyhow};

pub const PROJECT_NAME: &str = env!("CARGO_PKG_NAME");
pub const PROJECT_VERSION: &str = env!("GIT_VERSION");
pub const PROJECT_VERSION_HASH: &str = env!("GIT_HASH");

/// Returns the User-Agent header for HTTP requests: "viberails/VERSION (OS; ARCH)"
pub fn user_agent() -> &'static str {
    static USER_AGENT: OnceLock<String> = OnceLock::new();
    USER_AGENT.get_or_init(|| {
        format!(
            "{}/{} ({}; {})",
            PROJECT_NAME,
            PROJECT_VERSION,
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })
}

#[cfg(windows)]
pub const EXECUTABLE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), ".exe");
#[cfg(not(windows))]
pub const EXECUTABLE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(windows)]
pub const EXECUTABLE_EXT: &str = ".exe";
#[cfg(not(windows))]
pub const EXECUTABLE_EXT: &str = "";

pub fn print_header() {
    println!("{PROJECT_NAME} {PROJECT_VERSION}");
}

pub fn display_authorize_help() {
    print_header();

    let exe = env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| PROJECT_NAME.to_string());

    println!();
    println!("  Not logged in.");
    println!();
    println!("  Run `{exe} init-team` to create a new team, or");
    println!("  Run `{exe} join-team <URL>` to join an existing team.");
    println!();
}

pub fn project_data_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().ok_or_else(|| anyhow!("Unable to determine data directory. Ensure XDG_DATA_HOME or HOME environment variable is set"))?;

    let project_data_dir = data_dir.join(PROJECT_NAME);

    //
    // create the rootdir for our data is not there already
    //
    if !project_data_dir.exists() {
        fs::create_dir_all(&project_data_dir)
            .with_context(|| format!("Unable to create {}", project_data_dir.display()))?;
    }

    Ok(project_data_dir)
}

pub fn project_config_dir() -> Result<PathBuf> {
    let data_dir = dirs::config_dir().ok_or_else(|| anyhow!("Unable to determine config directory. Ensure XDG_CONFIG_HOME or HOME environment variable is set"))?;

    let project_data_dir = data_dir.join(PROJECT_NAME);

    //
    // create the rootdir for our data is not there already
    //
    if !project_data_dir.exists() {
        fs::create_dir_all(&project_data_dir)
            .with_context(|| format!("Unable to create {}", project_data_dir.display()))?;
    }

    Ok(project_data_dir)
}
