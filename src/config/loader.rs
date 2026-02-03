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

    // Validate host exists
    if parsed.host_str().is_none() {
        anyhow::bail!("Team URL must have a valid host");
    }

    // Extract path segments: /{oid}/{adapter_name}/{secret}
    let segments: Vec<&str> = parsed
        .path_segments()
        .context("URL has no path segments")?
        .collect();

    if segments.len() < 3 {
        anyhow::bail!("Invalid team URL format. Expected: https://hooks.domain/oid/name/secret");
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
/// Returns true if the user has completed team initialization or joined a team.
#[must_use]
pub fn is_authorized() -> bool {
    Config::load()
        .map(|config| config.org.authorized())
        .unwrap_or(false)
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
