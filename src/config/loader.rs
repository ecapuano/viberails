use std::{fs, io::Write, path::Path};

use crate::tui::{ConfigEntry, ConfigView};
use anyhow::{Context, Result};
use bon::Builder;
use log::info;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    PROJECT_NAME,
    common::project_config_dir,
    hooks::{binary_location, install_binary},
};

use colored::Colorize;

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(clap::Args)]
pub struct ConfigureArgs {
    /// Accept command on cloud failure
    #[arg(long)]
    fail_open: Option<bool>,

    /// Send tool use events to cloud for authorization
    #[arg(long)]
    audit_tool_use: Option<bool>,

    /// Send prompt/chat events to cloud for audit logging
    #[arg(long)]
    audit_prompts: Option<bool>,
}

#[derive(clap::Args)]
pub struct JoinTeamArgs {
    /// Team URL to join (obtained from init-team on another machine)
    pub url: String,
}

#[derive(Serialize, Deserialize, Builder)]
#[allow(clippy::struct_excessive_bools)]
pub struct UserConfig {
    pub fail_open: bool,
    /// Send tool use events to cloud for authorization (default: true)
    #[serde(default = "default_true")]
    #[builder(default = true)]
    pub audit_tool_use: bool,
    /// Send prompt/chat events to cloud for audit logging (default: true)
    #[serde(default = "default_true")]
    #[builder(default = true)]
    pub audit_prompts: bool,
    /// Enable debug mode for verbose logging (default: false)
    #[serde(default)]
    #[builder(default = false)]
    pub debug: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Default, Serialize, Deserialize)]
pub struct LcOrg {
    pub oid: String,
    pub name: String,
    pub url: String,
}

impl LcOrg {
    #[must_use]
    pub fn authorized(&self) -> bool {
        !self.url.is_empty() && !self.oid.is_empty()
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            fail_open: true,
            audit_tool_use: true,
            audit_prompts: true,
            debug: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub user: UserConfig,
    pub install_id: String,
    pub org: LcOrg,
}

impl Config {
    pub(crate) fn load_existing(config_file: &Path) -> Result<Self> {
        let config_string = fs::read_to_string(config_file)
            .with_context(|| format!("Unable to read {}", config_file.display()))?;

        let config: Config = serde_json::from_str(&config_string)
            .context("Unable to deserialize configuration data")?;

        Ok(config)
    }

    fn create_new() -> Self {
        let user = UserConfig::default();
        let install_id = Uuid::new_v4().to_string();
        let org = LcOrg::default();

        info!("install id: {install_id}");

        Self {
            user,
            install_id,
            org,
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_string =
            serde_json::to_string_pretty(self).context("Unable to serialize configuration data")?;

        let config_dir = project_config_dir()?;
        let config_file = config_dir.join(CONFIG_FILE_NAME);

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&config_file)
            .with_context(|| format!("Unable to write {}", config_file.display()))?;

        fd.write_all(config_string.as_bytes()).with_context(|| {
            format!("Failed to write configuration to {}", config_file.display())
        })?;

        Ok(())
    }

    pub fn load() -> Result<Self> {
        let config_dir = project_config_dir()?;
        let config_file = config_dir.join("config.json");

        // Try to load existing config, create new one if file doesn't exist
        // This avoids TOCTOU race between exists() check and read
        match Config::load_existing(&config_file) {
            Ok(config) => Ok(config),
            Err(e) => {
                // Check if the error is due to file not existing
                if config_file.exists() {
                    // File exists but we couldn't load it - propagate the error
                    Err(e)
                } else {
                    // File doesn't exist - create new config
                    Ok(Config::create_new())
                }
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// PUBLIC
////////////////////////////////////////////////////////////////////////////////

pub fn show_configuration() -> Result<()> {
    let config = Config::load()?;

    let title = format!(" {} {} ", PROJECT_NAME, crate::common::PROJECT_VERSION);
    let entries = vec![
        ConfigEntry::bool("Fail Open", config.user.fail_open),
        ConfigEntry::bool("Audit Tool Use", config.user.audit_tool_use),
        ConfigEntry::bool("Audit Prompts", config.user.audit_prompts),
        ConfigEntry::new("Install ID", &config.install_id),
        ConfigEntry::new("Organization", &config.org.name),
        ConfigEntry::new("Organization URL", &config.org.url),
    ];

    ConfigView::new(&title, entries).print();

    Ok(())
}

/// Parses a team URL and extracts the organization ID.
/// URL format: `https://{hooks_domain}/{oid}/{adapter_name}/{secret}`
pub(crate) fn parse_team_url(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url).context("Invalid URL format")?;

    // Validate HTTPS
    if parsed.scheme() != "https" {
        anyhow::bail!("Team URL must use HTTPS");
    }

    // Validate host exists and is a LimaCharlie hook domain
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Team URL must have a valid host"))?;

    if !host.ends_with(".hook.limacharlie.io") {
        anyhow::bail!("Team URL must be a LimaCharlie hook URL (*.hook.limacharlie.io)");
    }

    // Extract path segments: /{oid}/{adapter_name}/{secret}
    let segments: Vec<&str> = parsed
        .path_segments()
        .context("URL has no path segments")?
        .collect();

    if segments.len() < 3 {
        anyhow::bail!(
            "Invalid team URL format. Expected: https://<id>.hook.limacharlie.io/<oid>/<name>/<secret>"
        );
    }

    // First segment is the oid
    let oid = segments
        .first()
        .ok_or_else(|| anyhow::anyhow!("URL has no path segments"))?;
    if oid.is_empty() {
        anyhow::bail!("Organization ID in URL cannot be empty");
    }

    Ok((*oid).to_string())
}

/// Checks if the user is authorized (has valid org configuration).
///
/// Parameters: None
///
/// Returns: true if the user has completed team initialization or joined a team.
#[must_use]
pub fn is_authorized() -> bool {
    Config::load()
        .map(|config| config.org.authorized())
        .unwrap_or(false)
}

/// Enable or disable debug payload logging mode.
/// When enabled, full payloads from AI coding tools are logged to a secure temp directory.
///
/// Parameters:
///   - enable: true to enable debug mode, false to disable
///
/// Returns: Result indicating success or failure
pub fn set_debug_mode(enable: bool) -> Result<()> {
    let mut config = Config::load()?;
    config.user.debug = enable;
    config.save()?;

    if enable {
        println!("Debug mode enabled. Payloads will be logged to:");
        println!("  {}", get_debug_log_path()?.display());
        println!();
        println!("Warning: Debug logs may contain sensitive information.");
        println!("Disable debug mode when done: viberails debug --disable");
    } else {
        println!("Debug mode disabled.");
    }

    Ok(())
}

/// Get the path to the debug log directory.
/// Creates the directory with restrictive permissions if it doesn't exist.
/// Always verifies/fixes permissions on existing directories.
/// Log files inside have unique names with timestamps for security.
///
/// Parameters: None
///
/// Returns: `PathBuf` to the debug log directory
pub fn get_debug_log_path() -> Result<std::path::PathBuf> {
    let data_dir = crate::common::project_data_dir()?;
    let debug_dir = data_dir.join("debug");

    // Create directory with restrictive permissions
    // Use DirBuilder on Unix to set mode atomically, avoiding TOCTOU race
    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        use std::os::unix::fs::PermissionsExt;

        // DirBuilder with mode sets permissions atomically at creation
        // recursive(true) handles parent directories
        let mut builder = DirBuilder::new();
        builder.recursive(true).mode(0o700);

        // create() is idempotent - succeeds if dir exists with any permissions
        // We then ensure permissions are correct (in case dir existed with wrong perms)
        builder.create(&debug_dir).with_context(|| {
            format!("Unable to create debug directory: {}", debug_dir.display())
        })?;

        // Always verify/fix permissions (handles pre-existing directories)
        let perms = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&debug_dir, perms).with_context(|| {
            format!(
                "Unable to set permissions on debug directory: {}",
                debug_dir.display()
            )
        })?;
    }

    #[cfg(not(unix))]
    {
        fs::create_dir_all(&debug_dir)
            .with_context(|| format!("Unable to create debug directory: {}", debug_dir.display()))?;
    }

    // Return directory - individual log files have unique timestamped names
    Ok(debug_dir)
}

/// Internal function to clean .log files from a directory.
/// Used by `clean_debug_logs` and tests.
///
/// Parameters:
///   - dir: Directory path to clean .log files from
///
/// Returns: Tuple of (files removed count, total bytes freed)
pub(crate) fn clean_logs_in_dir(dir: &std::path::Path) -> Result<(usize, u64)> {
    if !dir.exists() {
        return Ok((0, 0));
    }

    let mut removed_count: usize = 0;
    let mut total_bytes: u64 = 0;

    let entries = fs::read_dir(dir)
        .with_context(|| format!("Unable to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| "Unable to read directory entry")?;
        let path = entry.path();

        // Only remove .log files to be safe
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
        {
            // Get file size before removing
            if let Ok(metadata) = fs::metadata(&path) {
                total_bytes = total_bytes.saturating_add(metadata.len());
            }

            // Remove file, ignoring "not found" errors (TOCTOU race)
            match fs::remove_file(&path) {
                Ok(()) => {
                    removed_count = removed_count.saturating_add(1);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // File was already removed, skip it
                }
                Err(e) => {
                    return Err(e)
                        .with_context(|| format!("Unable to remove log file: {}", path.display()));
                }
            }
        }
    }

    Ok((removed_count, total_bytes))
}

/// Clean up debug log files.
/// Removes all .log files from the debug directory.
/// Note: Debug logs accumulate over time (one file per hook invocation when debug mode
/// is enabled). Run this periodically to free disk space.
///
/// Parameters: None
///
/// Returns: Result with count of files removed
pub fn clean_debug_logs() -> Result<usize> {
    let debug_dir = get_debug_log_path()?;
    let (removed_count, total_bytes) = clean_logs_in_dir(&debug_dir)?;

    // Format size for display (precision loss acceptable for human-readable output)
    #[allow(clippy::cast_precision_loss)]
    let size_display = if total_bytes >= 1024 * 1024 {
        format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0))
    } else if total_bytes >= 1024 {
        format!("{:.1} KB", total_bytes as f64 / 1024.0)
    } else {
        format!("{total_bytes} bytes")
    };

    if removed_count > 0 {
        println!("Removed {removed_count} debug log file(s) ({size_display})");
    } else {
        println!("No debug log files to clean.");
    }

    Ok(removed_count)
}

pub fn join_team(args: &JoinTeamArgs) -> Result<()> {
    let mut config = Config::load()?;

    let url = &args.url;
    let oid = parse_team_url(url)?;

    config.org = LcOrg {
        oid,
        name: String::new(), // We don't have the team name when joining
        url: url.clone(),
    };

    config.save()?;

    let program = binary_location()?;

    if let Err(e) = install_binary(&program) {
        eprintln!(
            "Unable to install {PROJECT_NAME} @ {} ({e})",
            program.display()
        );
    }

    println!("Joined team successfully!");
    println!();
    println!("{}", format!("Team URL: {url}").green());
    println!();
    println!("Run to set up hooks for your AI coding tools:\n");
    println!("{}", format!("{} install", program.display()).cyan());
    println!();

    Ok(())
}
