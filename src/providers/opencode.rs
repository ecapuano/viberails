use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use serde_json::{Value, json};

use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// `OpenCode` uses a plugin system for hooks
/// Plugins intercept tool execution and events
pub const OPENCODE_HOOKS: &[&str] = &["plugins"];

/// Discovery implementation for `OpenCode`.
pub struct OpenCodeDiscovery;

impl OpenCodeDiscovery {
    /// Get the `OpenCode` config directory path.
    fn opencode_dir() -> Option<PathBuf> {
        // Check OPENCODE_CONFIG env var first, then standard locations
        std::env::var("OPENCODE_CONFIG")
            .ok()
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .or_else(|| dirs::config_dir().map(|c| c.join("opencode")))
    }
}

impl ProviderDiscovery for OpenCodeDiscovery {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
    }

    fn discover(&self) -> DiscoveryResult {
        let opencode_dir = Self::opencode_dir();
        let detected = opencode_dir
            .as_ref()
            .is_some_and(|p| p.exists() && p.is_dir());
        let detected_path = opencode_dir.filter(|p| p.exists());

        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected,
            detected_path,
            detection_hint: Some("Install OpenCode from https://opencode.ai/docs/cli/".into()),
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        OPENCODE_HOOKS
    }
}

impl ProviderFactory for OpenCodeDiscovery {
    fn create(&self) -> Result<Box<dyn LLmProviderTrait>> {
        Ok(Box::new(OpenCode::new()?))
    }
}

pub struct OpenCode {
    command_line: String,
    config_file: PathBuf,
}

impl OpenCode {
    pub fn new() -> Result<Self> {
        let exe = std::env::current_exe().context("Unable to determine current executable path")?;
        Self::with_custom_path(exe)
    }

    pub fn with_custom_path<P: AsRef<std::path::Path>>(program: P) -> Result<Self> {
        let opencode_dir = OpenCodeDiscovery::opencode_dir()
            .ok_or_else(|| anyhow!("Unable to determine OpenCode config directory"))?;

        let config_file = opencode_dir.join("opencode.json");
        let command_line = format!("{} opencode-callback", program.as_ref().display());

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
                    "Unable to create OpenCode config directory at {}",
                    parent.display()
                )
            })?;
        }

        if !self.config_file.exists() {
            fs::write(&self.config_file, "{}\n").with_context(|| {
                format!(
                    "Unable to create OpenCode config file at {}",
                    self.config_file.display()
                )
            })?;
        }

        Ok(())
    }

    pub(crate) fn install_into(&self, _hook_type: &str, json: &mut Value) -> Result<()> {
        let root = json.as_object_mut().ok_or_else(|| {
            anyhow!(
                "Expected root of {} to be a JSON object",
                self.config_file.display()
            )
        })?;

        // OpenCode uses plugins config like:
        // "plugins": { "plugin-name": { "enabled": true } }
        let plugins_obj = root
            .entry("plugins")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'plugins' field in {} to be an object",
                    self.config_file.display()
                )
            })?;

        // Check if viberails plugin is already registered
        if let Some(existing) = plugins_obj.get("viberails")
            && existing.get("command").and_then(|c| c.as_str()) == Some(&self.command_line)
        {
            warn!(
                "viberails plugin already exists in {}",
                self.config_file.display()
            );
            return Ok(());
        }

        // Add our plugin config
        plugins_obj.insert(
            "viberails".to_string(),
            json!({
                "enabled": true,
                "command": &self.command_line,
                "description": "Viberails security hooks"
            }),
        );

        Ok(())
    }

    pub(crate) fn uninstall_from(&self, _hook_type: &str, json: &mut Value) {
        let plugins_obj = json
            .as_object_mut()
            .and_then(|root| root.get_mut("plugins"))
            .and_then(|p| p.as_object_mut());

        let Some(plugins_obj) = plugins_obj else {
            warn!("No plugins found in {}", self.config_file.display());
            return;
        };

        if plugins_obj.remove("viberails").is_none() {
            warn!(
                "viberails plugin not found in {}",
                self.config_file.display()
            );
        }
    }
}

impl LLmProviderTrait for OpenCode {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn install(&self, hook_type: &str) -> Result<()> {
        info!("Installing {hook_type} in {}", self.config_file.display());

        self.ensure_config_exists()?;

        let data = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Unable to read {}", self.config_file.display()))?;

        let mut json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", self.config_file.display()))?;

        self.install_into(hook_type, &mut json)
            .with_context(|| format!("Unable to update {}", self.config_file.display()))?;

        let json_str =
            serde_json::to_string_pretty(&json).context("Failed to serialize OpenCode config")?;

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.config_file)
            .with_context(|| {
                format!("Unable to open {} for writing", self.config_file.display())
            })?;

        fd.write_all(json_str.as_bytes())
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

        let mut json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", self.config_file.display()))?;

        self.uninstall_from(hook_type, &mut json);

        let json_str =
            serde_json::to_string_pretty(&json).context("Failed to serialize OpenCode config")?;

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.config_file)
            .with_context(|| {
                format!("Unable to open {} for writing", self.config_file.display())
            })?;

        fd.write_all(json_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", self.config_file.display()))?;

        Ok(())
    }

    fn list(&self) -> Result<Vec<HookEntry>> {
        let data = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Unable to read {}", self.config_file.display()))?;

        let json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", self.config_file.display()))?;

        let mut entries = Vec::new();

        let Some(plugins_obj) = json.get("plugins").and_then(|p| p.as_object()) else {
            return Ok(entries);
        };

        for (plugin_name, plugin_config) in plugins_obj {
            if let Some(command) = plugin_config.get("command").and_then(|c| c.as_str()) {
                let enabled = plugin_config
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);

                entries.push(HookEntry {
                    hook_type: "plugin".to_string(),
                    matcher: if enabled {
                        plugin_name.clone()
                    } else {
                        format!("{plugin_name} (disabled)")
                    },
                    command: command.to_string(),
                });
            }
        }

        Ok(entries)
    }
}
