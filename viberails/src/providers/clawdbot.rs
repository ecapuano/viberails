use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use serde_json::{Value, json};

use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// Clawdbot (now `OpenClaw`) uses a hooks system with directory-based handlers.
/// We configure hooks via the JSON config file.
pub const CLAWDBOT_HOOKS: &[&str] = &["hooks"];

/// Discovery implementation for Clawdbot/OpenClaw.
pub struct ClawdbotDiscovery;

impl ClawdbotDiscovery {
    /// Get the Clawdbot config directory and config file name.
    /// Clawdbot has been renamed to `OpenClaw`, check both locations.
    /// Returns (`directory_path`, `config_file_name`)
    fn clawdbot_paths() -> Option<(PathBuf, &'static str)> {
        dirs::home_dir().map(|h| {
            let openclaw = h.join(".openclaw");
            if openclaw.exists() {
                return (openclaw, "openclaw.json");
            }
            let clawdbot = h.join(".clawdbot");
            if clawdbot.exists() {
                // Old clawdbot installation - use old config name
                return (clawdbot, "clawdbot.json");
            }
            // Return openclaw as default for new installations
            (openclaw, "openclaw.json")
        })
    }

    /// Get the Clawdbot config directory path.
    fn clawdbot_dir() -> Option<PathBuf> {
        Self::clawdbot_paths().map(|(dir, _)| dir)
    }

    /// Check if either clawdbot or openclaw directory exists
    fn is_detected() -> bool {
        dirs::home_dir()
            .is_some_and(|h| h.join(".openclaw").exists() || h.join(".clawdbot").exists())
    }
}

impl ProviderDiscovery for ClawdbotDiscovery {
    fn id(&self) -> &'static str {
        "clawdbot"
    }

    fn display_name(&self) -> &'static str {
        "Clawdbot/OpenClaw"
    }

    fn discover(&self) -> DiscoveryResult {
        let detected = Self::is_detected();
        let detected_path = if detected { Self::clawdbot_dir() } else { None };

        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected,
            detected_path,
            detection_hint: Some(
                "Install Clawdbot/OpenClaw: npm install -g clawdbot or visit https://github.com/clawdbot/clawdbot".into(),
            ),
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        CLAWDBOT_HOOKS
    }
}

impl ProviderFactory for ClawdbotDiscovery {
    fn create(&self) -> Result<Box<dyn LLmProviderTrait>> {
        Ok(Box::new(Clawdbot::new()?))
    }
}

pub struct Clawdbot {
    command_line: String,
    config_file: PathBuf,
}

impl Clawdbot {
    pub fn new() -> Result<Self> {
        let exe = std::env::current_exe().context("Unable to determine current executable path")?;
        Self::with_custom_path(exe)
    }

    pub fn with_custom_path<P: AsRef<std::path::Path>>(program: P) -> Result<Self> {
        let (clawdbot_dir, config_name) = ClawdbotDiscovery::clawdbot_paths()
            .ok_or_else(|| anyhow!("Unable to determine Clawdbot config directory"))?;

        let config_file = clawdbot_dir.join(config_name);
        let command_line = format!("{} clawdbot-callback", program.as_ref().display());

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
                    "Unable to create Clawdbot config directory at {}",
                    parent.display()
                )
            })?;
        }

        if !self.config_file.exists() {
            fs::write(&self.config_file, "{}\n").with_context(|| {
                format!(
                    "Unable to create Clawdbot config file at {}",
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

        // Clawdbot/OpenClaw uses:
        // "hooks": { "internal": { "enabled": true, "entries": { "hook-name": { "enabled": true } } } }
        let hooks_obj = root
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks' field in {} to be an object",
                    self.config_file.display()
                )
            })?;

        let internal_obj = hooks_obj
            .entry("internal")
            .or_insert_with(|| json!({"enabled": true, "entries": {}}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks.internal' field in {} to be an object",
                    self.config_file.display()
                )
            })?;

        // Ensure internal hooks are enabled
        internal_obj.insert("enabled".to_string(), json!(true));

        let entries_obj = internal_obj
            .entry("entries")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks.internal.entries' field in {} to be an object",
                    self.config_file.display()
                )
            })?;

        // Check if viberails is already registered
        if let Some(existing) = entries_obj.get("viberails")
            && existing.get("command").and_then(|c| c.as_str()) == Some(&self.command_line)
        {
            warn!(
                "viberails hook already exists in {}",
                self.config_file.display()
            );
            return Ok(());
        }

        // Add our hook entry
        entries_obj.insert(
            "viberails".to_string(),
            json!({
                "enabled": true,
                "command": &self.command_line
            }),
        );

        Ok(())
    }

    pub(crate) fn uninstall_from(&self, _hook_type: &str, json: &mut Value) {
        let entries_obj = json
            .as_object_mut()
            .and_then(|root| root.get_mut("hooks"))
            .and_then(|h| h.as_object_mut())
            .and_then(|h| h.get_mut("internal"))
            .and_then(|i| i.as_object_mut())
            .and_then(|i| i.get_mut("entries"))
            .and_then(|e| e.as_object_mut());

        let Some(entries_obj) = entries_obj else {
            warn!(
                "No hooks.internal.entries found in {}",
                self.config_file.display()
            );
            return;
        };

        if entries_obj.remove("viberails").is_none() {
            warn!("viberails hook not found in {}", self.config_file.display());
        }
    }
}

impl LLmProviderTrait for Clawdbot {
    fn name(&self) -> &'static str {
        "clawdbot"
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
            serde_json::to_string_pretty(&json).context("Failed to serialize Clawdbot config")?;

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
            serde_json::to_string_pretty(&json).context("Failed to serialize Clawdbot config")?;

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

        let entries_obj = json
            .get("hooks")
            .and_then(|h| h.get("internal"))
            .and_then(|i| i.get("entries"))
            .and_then(|e| e.as_object());

        let Some(entries_obj) = entries_obj else {
            return Ok(entries);
        };

        for (hook_name, hook_config) in entries_obj {
            let enabled = hook_config
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);

            let command = hook_config
                .get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("[directory hook]")
                .to_string();

            entries.push(HookEntry {
                hook_type: "internal".to_string(),
                matcher: if enabled {
                    hook_name.clone()
                } else {
                    format!("{hook_name} (disabled)")
                },
                command,
            });
        }

        Ok(entries)
    }
}
