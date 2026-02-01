use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use serde_json::{Value, json};

use crate::common::{EXECUTABLE_NAME, PROJECT_NAME};
use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// Supported hooks for Cursor
/// Based on <https://cursor.com/docs/agent/hooks>
pub const CURSOR_HOOKS: &[&str] = &["preToolUse", "beforeSubmitPrompt"];

/// Discovery implementation for Cursor.
pub struct CursorDiscovery;

impl CursorDiscovery {
    /// Get the Cursor config directory path.
    fn cursor_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".cursor"))
    }
}

impl ProviderDiscovery for CursorDiscovery {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn display_name(&self) -> &'static str {
        "Cursor"
    }

    fn discover(&self) -> DiscoveryResult {
        let cursor_dir = Self::cursor_dir();
        let detected = cursor_dir
            .as_ref()
            .is_some_and(|p| p.exists() && p.is_dir());
        let detected_path = cursor_dir.filter(|p| p.exists());

        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected,
            detected_path,
            detection_hint: Some("Install Cursor from https://cursor.com/downloads".into()),
            hooks_installed: false, // Will be set by discover_with_hooks_check
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        CURSOR_HOOKS
    }
}

impl ProviderFactory for CursorDiscovery {
    fn create(&self) -> Result<Box<dyn LLmProviderTrait>> {
        Ok(Box::new(Cursor::new()?))
    }
}

pub struct Cursor {
    command_line: String,
    hooks_file: PathBuf,
}

impl Cursor {
    pub fn new() -> Result<Self> {
        // Always use the installed binary location (~/.local/bin/viberails) rather than
        // current_exe(), so the hook command is consistent regardless of where viberails
        // is run from. This prevents duplicate hooks when running from different locations.
        let exe = Self::binary_location()?;
        Self::with_custom_path(exe)
    }

    /// Get the installed binary location (~/.local/bin/viberails).
    fn binary_location() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| {
            anyhow!("Unable to determine home directory. Ensure HOME environment variable is set")
        })?;

        let local_bin = home.join(".local").join("bin");
        let file_name = if cfg!(target_os = "windows") {
            format!("{PROJECT_NAME}.exe")
        } else {
            PROJECT_NAME.to_string()
        };

        Ok(local_bin.join(file_name))
    }

    pub fn with_custom_path<P: AsRef<std::path::Path>>(program: P) -> Result<Self> {
        let config_dir = dirs::home_dir().ok_or_else(|| {
            anyhow!("Unable to determine home directory. Ensure HOME environment variable is set")
        })?;

        let cursor_dir = config_dir.join(".cursor");
        let hooks_file = cursor_dir.join("hooks.json");

        // Cursor uses a different callback command format
        let command_line = format!("{} cursor-callback", program.as_ref().display());

        Ok(Self {
            command_line,
            hooks_file,
        })
    }

    /// Ensure the hooks file exists, creating it with default structure if needed.
    fn ensure_hooks_exist(&self) -> Result<()> {
        if let Some(parent) = self.hooks_file.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Unable to create Cursor config directory at {}",
                    parent.display()
                )
            })?;
        }

        if !self.hooks_file.exists() {
            let default = json!({
                "version": 1,
                "hooks": {}
            });
            let json_str = serde_json::to_string_pretty(&default)
                .context("Failed to serialize default Cursor hooks")?;
            fs::write(&self.hooks_file, json_str).with_context(|| {
                format!(
                    "Unable to create Cursor hooks file at {}",
                    self.hooks_file.display()
                )
            })?;
        }

        Ok(())
    }

    pub(crate) fn install_into(&self, hook_type: &str, json: &mut Value) -> Result<()> {
        let root = json.as_object_mut().ok_or_else(|| {
            anyhow!(
                "Expected root of {} to be a JSON object",
                self.hooks_file.display()
            )
        })?;

        // Ensure version field exists
        root.entry("version").or_insert(json!(1));

        let hooks_obj = root
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks' field in {} to be an object",
                    self.hooks_file.display()
                )
            })?;

        let hook_type_arr = hooks_obj
            .entry(hook_type)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks.{hook_type}' field in {} to be an array",
                    self.hooks_file.display()
                )
            })?;

        // Cursor uses a flat array structure (no nested matchers)
        // Check if already installed
        let already_installed = hook_type_arr
            .iter()
            .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(&self.command_line));

        if already_installed {
            warn!(
                "{hook_type} already exists in {}",
                self.hooks_file.display()
            );
            return Ok(());
        }

        // Insert our hook at the beginning
        hook_type_arr.insert(
            0,
            json!({
                "type": "command",
                "command": &self.command_line,
                "matcher": "*"
            }),
        );

        Ok(())
    }

    pub(crate) fn uninstall_from(&self, hook_type: &str, json: &mut Value) {
        let hooks_obj = json
            .as_object_mut()
            .and_then(|root| root.get_mut("hooks"))
            .and_then(|h| h.as_object_mut());

        let Some(hooks_obj) = hooks_obj else {
            warn!("No hooks found in {}", self.hooks_file.display());
            return;
        };

        let Some(hook_type_arr) = hooks_obj.get_mut(hook_type).and_then(|v| v.as_array_mut())
        else {
            warn!(
                "No {hook_type} hooks found in {}",
                self.hooks_file.display()
            );
            return;
        };

        // Remove our hook by matching executables ending with EXECUTABLE_NAME
        let original_len = hook_type_arr.len();
        hook_type_arr.retain(|h| {
            let Some(cmd) = h.get("command").and_then(|c| c.as_str()) else {
                return true;
            };
            !cmd.split_whitespace()
                .next()
                .is_some_and(|exe| exe.ends_with(EXECUTABLE_NAME))
        });

        if hook_type_arr.len() == original_len {
            warn!(
                "{hook_type} hook not found in {}",
                self.hooks_file.display()
            );
        }
    }
}

impl LLmProviderTrait for Cursor {
    fn name(&self) -> &'static str {
        "cursor"
    }

    fn install(&self, hook_type: &str) -> Result<()> {
        info!("Installing {hook_type} in {}", self.hooks_file.display());

        self.ensure_hooks_exist()?;

        let data = fs::read_to_string(&self.hooks_file)
            .with_context(|| format!("Unable to read {}", self.hooks_file.display()))?;

        let mut json: Value = serde_json::from_str(&data).with_context(|| {
            format!("Unable to parse JSON data in {}", self.hooks_file.display())
        })?;

        self.install_into(hook_type, &mut json)
            .with_context(|| format!("Unable to update {}", self.hooks_file.display()))?;

        let json_str =
            serde_json::to_string_pretty(&json).context("Failed to serialize Cursor hooks")?;

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.hooks_file)
            .with_context(|| format!("Unable to open {} for writing", self.hooks_file.display()))?;

        fd.write_all(json_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", self.hooks_file.display()))?;

        Ok(())
    }

    fn uninstall(&self, hook_type: &str) -> Result<()> {
        info!(
            "Uninstalling {hook_type} from {}",
            self.hooks_file.display()
        );

        let data = fs::read_to_string(&self.hooks_file)
            .with_context(|| format!("Unable to read {}", self.hooks_file.display()))?;

        let mut json: Value = serde_json::from_str(&data).with_context(|| {
            format!("Unable to parse JSON data in {}", self.hooks_file.display())
        })?;

        self.uninstall_from(hook_type, &mut json);

        let json_str =
            serde_json::to_string_pretty(&json).context("Failed to serialize Cursor hooks")?;

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.hooks_file)
            .with_context(|| format!("Unable to open {} for writing", self.hooks_file.display()))?;

        fd.write_all(json_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", self.hooks_file.display()))?;

        Ok(())
    }

    fn list(&self) -> Result<Vec<HookEntry>> {
        let data = fs::read_to_string(&self.hooks_file)
            .with_context(|| format!("Unable to read {}", self.hooks_file.display()))?;

        let json: Value = serde_json::from_str(&data).with_context(|| {
            format!("Unable to parse JSON data in {}", self.hooks_file.display())
        })?;

        let mut entries = Vec::new();

        let Some(hooks_obj) = json.get("hooks").and_then(|h| h.as_object()) else {
            return Ok(entries);
        };

        for (hook_type, hook_arr) in hooks_obj {
            let Some(hook_arr) = hook_arr.as_array() else {
                continue;
            };

            for hook in hook_arr {
                let matcher = hook
                    .get("matcher")
                    .and_then(|m| m.as_str())
                    .unwrap_or("*")
                    .to_string();

                if let Some(command) = hook.get("command").and_then(|c| c.as_str()) {
                    entries.push(HookEntry {
                        hook_type: hook_type.clone(),
                        matcher,
                        command: command.to_string(),
                    });
                }
            }
        }

        Ok(entries)
    }
}
