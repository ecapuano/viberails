use std::{fs, io::Write, path::Path};

use anyhow::{Context, Result};
use bon::Builder;
use log::info;
use serde::{Deserialize, Serialize};
use tabled::{
    Table, Tabled,
    settings::{Margin, Rotate, Style},
};
use url::Url;
use uuid::Uuid;

use crate::common::{print_header, project_config_dir};
use crate::default::get_embedded_default;

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(clap::Args)]
pub struct ConfigureArgs {
    /// Hook URL
    #[arg(long, default_value_t = default_hook_url())]
    hook_url: Url,

    /// Accept command on cloud failure
    #[arg(long, default_value_t = true)]
    fail_open: bool,
}

fn default_hook_url() -> Url {
    get_embedded_default("default_hook_url")
        .parse()
        .expect("valid hook URL")
}

#[derive(Serialize, Deserialize, Builder, Tabled)]
pub struct UserConfig {
    pub hook_url: String,
    pub fail_open: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct LcOrg {
    pub oid: String,
    pub jwt: String,
}

impl Tabled for LcOrg {
    const LENGTH: usize = 2;

    fn fields(&self) -> Vec<std::borrow::Cow<'_, str>> {
        let truncated_jwt = if self.jwt.len() > 36 {
            format!("{}...", &self.jwt[..36])
        } else {
            self.jwt.clone()
        };
        vec![
            std::borrow::Cow::Borrowed(&self.oid),
            std::borrow::Cow::Owned(truncated_jwt),
        ]
    }

    fn headers() -> Vec<std::borrow::Cow<'static, str>> {
        vec![
            std::borrow::Cow::Borrowed("oid"),
            std::borrow::Cow::Borrowed("jwt"),
        ]
    }
}

impl LcOrg {
    pub fn authorized(&self) -> bool {
        !self.jwt.is_empty() && !self.oid.is_empty()
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            hook_url: get_embedded_default("default_hook_url"),
            fail_open: true,
        }
    }
}

#[derive(Serialize, Deserialize, Tabled)]
pub struct Config {
    #[tabled(inline)]
    pub user: UserConfig,
    pub install_id: String,
    #[tabled(inline)]
    pub org: LcOrg,
}

impl Config {
    pub(crate) fn load_existing(config_file: &Path) -> Result<Self> {
        let config_string = fs::read_to_string(&config_file)
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
    let mut table = Table::new([config]);
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
    let user = UserConfig::builder()
        .hook_url(args.hook_url.to_string())
        .fail_open(args.fail_open)
        .build();

    let mut config = Config::load()?;

    config.user = user;

    config.save()?;

    display_configuration(&config);

    Ok(())
}
