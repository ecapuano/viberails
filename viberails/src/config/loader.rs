use std::{fs, io::Write, path::Path};

use anyhow::{Context, Result};
use bon::Builder;
use log::info;
use serde::{Deserialize, Serialize};
use tabled::{
    Table, Tabled,
    settings::{Margin, Rotate, Style},
};
use uuid::Uuid;

use crate::common::{print_header, project_config_dir};

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(clap::Args)]
pub struct ConfigureArgs {
    /// Accept command on cloud failure
    #[arg(long, default_value_t = true)]
    fail_open: bool,
}

#[derive(clap::Args)]
pub struct JoinTeamArgs {
    /// Team URL to join (obtained from init-team on another machine)
    pub url: String,
}

#[derive(Serialize, Deserialize, Builder)]
pub struct UserConfig {
    pub fail_open: bool,
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
        Self { fail_open: true }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub user: UserConfig,
    pub install_id: String,
    pub org: LcOrg,
}

#[derive(Tabled)]
struct ConfigDisplay<'a> {
    fail_open: bool,
    install_id: &'a str,
    org_name: &'a str,
    org_url: &'a str,
}

impl<'a> From<&'a Config> for ConfigDisplay<'a> {
    fn from(config: &'a Config) -> Self {
        Self {
            fail_open: config.user.fail_open,
            install_id: &config.install_id,
            org_name: &config.org.name,
            org_url: &config.org.url,
        }
    }
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

        if config_file.exists() {
            Config::load_existing(&config_file)
        } else {
            //
            // doesn't exist yet
            //
            Ok(Config::create_new())
        }
    }
}

fn display_configuration(config: &Config) {
    let display = ConfigDisplay::from(config);
    let mut table = Table::new([display]);
    table
        .with(Rotate::Left)
        .with(Style::modern())
        .with(Margin::new(4, 0, 0, 0));

    print_header();
    println!("{table}");
}

////////////////////////////////////////////////////////////////////////////////
// PUBLIC
////////////////////////////////////////////////////////////////////////////////

pub fn uninstall_config() -> Result<()> {
    let config_dir = project_config_dir()?;
    let config_file = config_dir.join("config.json");

    if !config_dir.exists() {
        info!("{} doesn't exist", config_dir.display());
        return Ok(());
    }

    if config_file.exists() {
        info!("removing {}", config_file.display());

        fs::remove_file(&config_file)
            .with_context(|| format!("Unable to delete {}", config_file.display()))?;
    }

    info!("Deleting {}", config_dir.display());

    fs::remove_dir_all(&config_dir)
        .with_context(|| format!("Unable to delete {}", config_dir.display()))?;

    Ok(())
}

pub fn show_configuration() -> Result<()> {
    let config = Config::load()?;

    display_configuration(&config);

    Ok(())
}

pub fn configure(args: &ConfigureArgs) -> Result<()> {
    let user = UserConfig {
        fail_open: args.fail_open,
    };

    let mut config = Config::load()?;

    config.user = user;

    config.save()?;

    display_configuration(&config);

    Ok(())
}

/// Parses a team URL and extracts the organization ID.
/// URL format: https://{hooks_domain}/{oid}/{adapter_name}/{secret}
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
        anyhow::bail!(
            "Invalid team URL format. Expected: https://hooks.domain/oid/name/secret"
        );
    }

    // First segment is the oid
    let oid = segments[0];
    if oid.is_empty() {
        anyhow::bail!("Organization ID in URL cannot be empty");
    }

    Ok(oid.to_string())
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

    println!("Joined team successfully!");
    println!();
    println!("Team URL: {}", url);
    println!();
    println!("Run 'install' to set up hooks for your AI coding tools.");

    Ok(())
}
