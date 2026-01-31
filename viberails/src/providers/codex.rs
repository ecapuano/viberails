use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use toml::{Table, Value};

use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// Codex uses a notification hook system, not traditional pre/post hooks
/// The notify command receives JSON payloads for various events
pub const CODEX_HOOKS: &[&str] = &["notify"];

/// Discovery implementation for `OpenAI` Codex CLI.
pub struct CodexDiscovery;

impl CodexDiscovery {
    /// Get the Codex config directory path.
    fn codex_dir() -> Option<PathBuf> {
        // Check CODEX_HOME env var first, then default to ~/.codex
        std::env::var("CODEX_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
    }
}

impl ProviderDiscovery for CodexDiscovery {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "OpenAI Codex CLI"
    }

    fn discover(&self) -> DiscoveryResult {
        let codex_dir = Self::codex_dir();
        let detected = codex_dir.as_ref().is_some_and(|p| p.exists() && p.is_dir());
        let detected_path = codex_dir.filter(|p| p.exists());

        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected,
            detected_path,
            detection_hint: Some(
                "Install Codex CLI from https://developers.openai.com/codex/quickstart/".into(),
            ),
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        CODEX_HOOKS
    }
}

impl ProviderFactory for CodexDiscovery {
    fn create(&self, program_path: &Path) -> Result<Box<dyn LLmProviderTrait>> {
        Ok(Box::new(Codex::new(program_path)?))
    }
}

pub struct Codex {
    command_line: String,
    config_file: PathBuf,
}

impl Codex {
    pub fn new<P>(self_program: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let codex_dir = CodexDiscovery::codex_dir()
            .ok_or_else(|| anyhow!("Unable to determine Codex config directory"))?;

        let config_file = codex_dir.join("config.toml");
        let command_line = format!("{} codex-callback", self_program.as_ref().display());

        Ok(Self {
            command_line,
            config_file,
        })
    }

    /// Ensure the config file exists, creating it with minimal content if needed.
    fn ensure_config_exists(&self) -> Result<()> {
        if let Some(parent) = self.config_file.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Unable to create Codex config directory at {}",
                    parent.display()
                )
            })?;
        }

        if !self.config_file.exists() {
            fs::write(&self.config_file, "# Codex CLI configuration\n").with_context(|| {
                format!(
                    "Unable to create Codex config file at {}",
                    self.config_file.display()
                )
            })?;
        }

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn install_into(&self, _hook_type: &str, toml: &mut Table) -> Result<()> {
        // Codex uses `notify = ["command", "arg1", "arg2"]` format
        let notify_entry = toml.get("notify");

        if let Some(existing) = notify_entry {
            // Check if it's an array and if our command is already there
            if let Some(arr) = existing.as_array()
                && arr.first().and_then(|v| v.as_str()) == Some(&self.command_line)
            {
                warn!(
                    "notify hook already exists in {}",
                    self.config_file.display()
                );
                return Ok(());
            }
            // If there's an existing notify that's not ours, we can't just overwrite it
            // Log a warning but proceed (user might want to backup their existing config)
            warn!(
                "Existing notify config found in {}. It will be replaced.",
                self.config_file.display()
            );
        }

        // Set notify to our command
        toml.insert(
            "notify".to_string(),
            Value::Array(vec![Value::String(self.command_line.clone())]),
        );

        Ok(())
    }

    pub(crate) fn uninstall_from(&self, _hook_type: &str, toml: &mut Table) {
        let notify_entry = toml.get("notify");

        if let Some(existing) = notify_entry
            && let Some(arr) = existing.as_array()
            && arr.first().and_then(|v| v.as_str()) == Some(&self.command_line)
        {
            toml.remove("notify");
            return;
        }

        warn!("notify hook not found in {}", self.config_file.display());
    }
}

impl LLmProviderTrait for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn install(&self, hook_type: &str) -> Result<()> {
        info!("Installing {hook_type} in {}", self.config_file.display());

        self.ensure_config_exists()?;

        let data = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Unable to read {}", self.config_file.display()))?;

        let mut toml: Table = data
            .parse()
            .with_context(|| format!("Unable to parse TOML in {}", self.config_file.display()))?;

        self.install_into(hook_type, &mut toml)
            .with_context(|| format!("Unable to update {}", self.config_file.display()))?;

        let toml_str = toml::to_string_pretty(&toml).context("Failed to serialize Codex config")?;

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.config_file)
            .with_context(|| {
                format!("Unable to open {} for writing", self.config_file.display())
            })?;

        fd.write_all(toml_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", self.config_file.display()))?;

        Ok(())
    }

    fn uninstall(&self, hook_type: &str) -> Result<()> {
        info!(
            "Uninstalling {hook_type} from {}",
            self.config_file.display()
        );

        let data = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Unable to read {}", self.config_file.display()))?;

        let mut toml: Table = data
            .parse()
            .with_context(|| format!("Unable to parse TOML in {}", self.config_file.display()))?;

        self.uninstall_from(hook_type, &mut toml);

        let toml_str = toml::to_string_pretty(&toml).context("Failed to serialize Codex config")?;

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.config_file)
            .with_context(|| {
                format!("Unable to open {} for writing", self.config_file.display())
            })?;

        fd.write_all(toml_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", self.config_file.display()))?;

        Ok(())
    }

    fn list(&self) -> Result<Vec<HookEntry>> {
        let data = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Unable to read {}", self.config_file.display()))?;

        let toml: Table = data
            .parse()
            .with_context(|| format!("Unable to parse TOML in {}", self.config_file.display()))?;

        let mut entries = Vec::new();

        if let Some(notify) = toml.get("notify")
            && let Some(arr) = notify.as_array()
            && let Some(cmd) = arr.first().and_then(|v| v.as_str())
        {
            entries.push(HookEntry {
                hook_type: "notify".to_string(),
                matcher: "*".to_string(),
                command: cmd.to_string(),
            });
        }

        Ok(entries)
    }
}
